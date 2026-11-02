use std::path::PathBuf;

pub fn clidex_dir() -> Result<PathBuf, String> {
    dirs::home_dir()
        .map(|home| home.join(".clidex"))
        .ok_or_else(|| "Could not find home directory; set CLIDEX_INDEX_PATH".into())
}

pub fn index_path() -> Result<PathBuf, String> {
    match std::env::var_os("CLIDEX_INDEX_PATH") {
        Some(path) => Ok(PathBuf::from(path)),
        None => Ok(clidex_dir()?.join("index.yaml")),
    }
}

pub const INDEX_URL: &str =
    "https://github.com/syshin0116/clidex/releases/download/index/index.yaml";

pub const INDEX_SHA256_URL: &str =
    "https://github.com/syshin0116/clidex/releases/download/index/index.yaml.sha256";

pub const DEFAULT_MAX_RESULTS: usize = 10;

pub fn embeddings_path() -> Result<PathBuf, String> {
    Ok(index_path()?.with_extension("embeddings.bin"))
}

pub const EMBEDDINGS_URL: &str =
    "https://github.com/syshin0116/clidex/releases/download/index/index.embeddings.bin";
