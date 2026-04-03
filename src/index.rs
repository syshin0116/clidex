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
    let index: Index =
        serde_yaml::from_str(&content).map_err(|e| format!("Failed to parse index: {e}"))?;
    Ok(index)
}

pub async fn update_index() -> Result<usize, String> {
    let dir = config::clidex_dir();
    if !dir.exists() {
        fs::create_dir_all(&dir).map_err(|e| format!("Failed to create {}: {e}", dir.display()))?;
    }

    eprintln!("Downloading index from {}...", config::INDEX_URL);

    let resp = reqwest::get(config::INDEX_URL)
        .await
        .map_err(|e| format!("Download failed: {e}"))?;

    if !resp.status().is_success() {
        return Err(format!("Download failed: HTTP {}", resp.status()));
    }

    let body = resp
        .text()
        .await
        .map_err(|e| format!("Failed to read response: {e}"))?;

    // Validate before saving
    let index: Index =
        serde_yaml::from_str(&body).map_err(|e| format!("Invalid index data: {e}"))?;

    let path = config::index_path();

    // Atomic write: write to temp file then rename to avoid corruption on interruption
    let tmp_path = path.with_extension("yaml.tmp");
    fs::write(&tmp_path, &body)
        .map_err(|e| format!("Failed to write {}: {e}", tmp_path.display()))?;
    fs::rename(&tmp_path, &path).map_err(|e| format!("Failed to rename temp file: {e}"))?;

    Ok(index.tools.len())
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
    fs::write(path, yaml).map_err(|e| format!("Failed to write {}: {e}", path.display()))?;
    Ok(())
}
