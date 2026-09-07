use crate::config;
use crate::model::Index;
use std::fs;
use std::path::Path;

pub fn load_index() -> Result<Index, String> {
    let path = config::index_path();
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
        serde_yaml::from_str(content).map_err(|error| format!("Invalid index: {error}"))?;
    if index.version != 1 || index.tools.is_empty() {
        return Err("Unsupported index version or empty tool index".into());
    }
    Ok(index)
}

pub async fn update_index() -> Result<usize, String> {
    let path = config::index_path();
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
    eprintln!("Downloading index from {}...", config::INDEX_URL);
    let body = client
        .get(config::INDEX_URL)
        .send()
        .await
        .and_then(reqwest::Response::error_for_status)
        .map_err(|error| format!("Download failed: {error}"))?
        .text()
        .await
        .map_err(|error| error.to_string())?;
    let index = parse_index(&body)?;
    write_atomic(&path, body.as_bytes())?;

    #[cfg(feature = "semantic")]
    {
        let result = async {
            let bytes = client
                .get(config::EMBEDDINGS_URL)
                .send()
                .await?
                .error_for_status()?
                .bytes()
                .await?;
            crate::semantic::read_tool_embeddings(
                bytes.as_ref(),
                bytes.len() as u64,
                &index.tools,
            )?;
            write_atomic(&config::embeddings_path(), &bytes)?;
            Ok::<_, Box<dyn std::error::Error>>(())
        }
        .await;
        if let Err(error) = result {
            eprintln!("Semantic embeddings unavailable: {error}. Using lexical search.");
        }
    }
    Ok(index.tools.len())
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
        serde_yaml::to_string(index).map_err(|e| format!("Failed to serialize index: {e}"))?;
    write_atomic(path, yaml.as_bytes())?;
    Ok(())
}
