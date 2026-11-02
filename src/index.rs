use crate::config;
use crate::model::Index;
use std::fs;
use std::path::Path;

pub fn load_index() -> Result<Index, String> {
    let path = config::index_path()?;
    if !path.exists() {
        return Err(format!(
            "Index not found at {}. Run `clidex update` to download it.",
            path.display()
        ));
    }
    let content = fs::read_to_string(&path).map_err(|e| format!("Failed to read index: {e}"))?;
    parse_index(&content)
}

fn parse_index(content: &str) -> Result<Index, String> {
    let index: Index =
        serde_norway::from_str(content).map_err(|error| format!("Invalid index: {error}"))?;
    if index.version != 1 || index.tools.is_empty() {
        return Err("Unsupported index version or empty tool index".into());
    }
    Ok(index)
}

/// Days after which `load_index` warns that the local index is out of date.
pub const STALE_AFTER_DAYS: i64 = 14;

/// Days elapsed since an RFC 3339 UTC timestamp such as `2026-09-07T00:00:00Z`.
pub fn age_in_days(generated: &str, now_unix_secs: i64) -> Option<i64> {
    let date = generated.get(..10)?;
    let mut parts = date.split('-').map(|part| part.parse::<i64>().ok());
    let (year, month, day) = (parts.next()??, parts.next()??, parts.next()??);
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    // Howard Hinnant's days_from_civil.
    let year = if month <= 2 { year - 1 } else { year };
    let era = year.div_euclid(400);
    let year_of_era = year - era * 400;
    let day_of_year = (153 * ((month + 9) % 12) + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    let days_since_epoch = era * 146_097 + day_of_era - 719_468;
    Some(now_unix_secs.div_euclid(86_400) - days_since_epoch)
}

fn now_unix_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs() as i64)
        .unwrap_or(0)
}

/// Warn on stderr when the index is older than [`STALE_AFTER_DAYS`].
pub fn warn_if_stale(index: &Index) {
    if std::env::var_os("CLIDEX_NO_STALE_WARNING").is_some() {
        return;
    }
    if let Some(days) = age_in_days(&index.generated, now_unix_secs()) {
        if days > STALE_AFTER_DAYS {
            eprintln!("Warning: index is {days} days old. Run `clidex update` to refresh it.");
        }
    }
}

pub enum UpdateOutcome {
    Updated(usize),
    AlreadyCurrent(usize),
}

fn etag_path(index_path: &Path) -> std::path::PathBuf {
    index_path.with_extension("etag")
}

fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

/// Fetch the published checksum (`<hex>  index.yaml`) and compare it with `body`.
async fn verify_checksum(client: &reqwest::Client, body: &[u8]) -> Result<(), String> {
    let published = client
        .get(config::INDEX_SHA256_URL)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|error| format!("Checksum download failed: {error}"))?
        .text()
        .await
        .map_err(|error| error.to_string())?;
    let expected = published.split_whitespace().next().unwrap_or_default();
    if expected.eq_ignore_ascii_case(&sha256_hex(body)) {
        Ok(())
    } else {
        Err("Index checksum mismatch".into())
    }
}

pub async fn update_index() -> Result<UpdateOutcome, String> {
    let path = config::index_path()?;
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|error| error.to_string())?;
    }
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(60))
        .build()
        .map_err(|error| error.to_string())?;
    let local = load_index().ok();
    let etag = local
        .as_ref()
        .and_then(|_| fs::read_to_string(etag_path(&path)).ok());

    eprintln!("Downloading index from {}...", config::INDEX_URL);
    // The index and its checksum are separate release assets uploaded one after
    // the other, so a download can land between the two uploads. Retry once.
    let mut attempt = 0;
    let (index, body, new_etag) = loop {
        attempt += 1;
        let mut request = client.get(config::INDEX_URL);
        if let Some(etag) = etag.as_deref() {
            request = request.header(reqwest::header::IF_NONE_MATCH, etag.trim());
        }
        let response = request
            .send()
            .await
            .and_then(reqwest::Response::error_for_status)
            .map_err(|error| format!("Download failed: {error}"))?;
        if response.status() == reqwest::StatusCode::NOT_MODIFIED {
            if let Some(local) = local {
                #[cfg(feature = "semantic")]
                if !config::embeddings_path()?.exists() {
                    update_embeddings(&client, &local).await;
                }
                return Ok(UpdateOutcome::AlreadyCurrent(local.tools.len()));
            }
        }
        let new_etag = response
            .headers()
            .get(reqwest::header::ETAG)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let body = response.text().await.map_err(|error| error.to_string())?;
        match verify_checksum(&client, body.as_bytes()).await {
            Ok(()) => break (parse_index(&body)?, body, new_etag),
            Err(_) if attempt < 2 => tokio::time::sleep(std::time::Duration::from_secs(3)).await,
            Err(error) => return Err(error),
        }
    };
    write_atomic(&path, body.as_bytes())?;
    match new_etag {
        Some(etag) => write_atomic(&etag_path(&path), etag.as_bytes())?,
        None => {
            let _ = fs::remove_file(etag_path(&path));
        }
    }

    #[cfg(feature = "semantic")]
    update_embeddings(&client, &index).await;
    Ok(UpdateOutcome::Updated(index.tools.len()))
}

#[cfg(feature = "semantic")]
async fn update_embeddings(client: &reqwest::Client, index: &Index) {
    let result = async {
        let bytes = client
            .get(config::EMBEDDINGS_URL)
            .send()
            .await?
            .error_for_status()?
            .bytes()
            .await?;
        crate::semantic::read_tool_embeddings(bytes.as_ref(), bytes.len() as u64, &index.tools)?;
        write_atomic(&config::embeddings_path()?, &bytes)?;
        Ok::<_, Box<dyn std::error::Error>>(())
    }
    .await;
    if let Err(error) = result {
        eprintln!("Semantic embeddings unavailable: {error}. Using lexical search.");
    }
}

fn write_atomic(path: &Path, bytes: &[u8]) -> Result<(), String> {
    use std::io::Write;
    let nonce = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_nanos();
    let tmp = path.with_extension(format!("{}.{}.tmp", std::process::id(), nonce));
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&tmp)
        .map_err(|error| error.to_string())?;
    let result = (|| {
        file.write_all(bytes)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&tmp, path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(tmp);
    }
    result.map_err(|error| format!("Failed to save {}: {error}", path.display()))
}

pub fn index_stats(index: &Index) -> IndexStats {
    let total = index.tools.len();
    let with_install = index.tools.iter().filter(|t| !t.install.is_empty()).count();
    let with_stars = index.tools.iter().filter(|t| t.stars.is_some()).count();
    let with_docs = index
        .tools
        .iter()
        .filter(|t| t.links.docs.is_some())
        .count();
    let with_llms_txt = index
        .tools
        .iter()
        .filter(|t| t.links.llms_txt.is_some())
        .count();

    let mut categories = std::collections::BTreeMap::new();
    for tool in &index.tools {
        *categories.entry(tool.category.clone()).or_insert(0usize) += 1;
    }

    IndexStats {
        version: index.version,
        generated: index.generated.clone(),
        total,
        categories: categories.len(),
        with_install,
        with_stars,
        with_docs,
        with_llms_txt,
    }
}

#[derive(serde::Serialize)]
pub struct IndexStats {
    pub version: u32,
    pub generated: String,
    pub total: usize,
    pub categories: usize,
    pub with_install: usize,
    pub with_stars: usize,
    pub with_docs: usize,
    pub with_llms_txt: usize,
}

pub fn save_index(index: &Index, path: &Path) -> Result<(), String> {
    let yaml =
        serde_norway::to_string(index).map_err(|e| format!("Failed to serialize index: {e}"))?;
    write_atomic(path, yaml.as_bytes())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn age_in_days_counts_calendar_days_in_utc() {
        let day = 86_400;
        // 2026-09-07T00:00:00Z
        let epoch_day = 20_703;
        assert_eq!(
            age_in_days("2026-09-07T00:00:00Z", epoch_day * day),
            Some(0)
        );
        assert_eq!(
            age_in_days("2026-09-07T23:59:59Z", epoch_day * day + 5),
            Some(0)
        );
        assert_eq!(
            age_in_days("2026-08-24T12:00:00Z", epoch_day * day),
            Some(14)
        );
        assert_eq!(age_in_days("2024-02-29T00:00:00Z", 19_782 * day), Some(0));
        assert_eq!(age_in_days("1970-01-01T00:00:00Z", 0), Some(0));
        assert_eq!(age_in_days("test", 0), None);
        assert_eq!(age_in_days("2026-13-01T00:00:00Z", 0), None);
    }

    #[test]
    fn sha256_hex_matches_known_digest() {
        assert_eq!(
            sha256_hex(b"abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }
}
