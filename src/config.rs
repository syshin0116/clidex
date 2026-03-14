use std::path::PathBuf;

pub fn clidex_dir() -> PathBuf {
    dirs::home_dir()
        .expect("Could not find home directory")
        .join(".clidex")
}

pub fn index_path() -> PathBuf {
    clidex_dir().join("index.yaml")
}

pub const INDEX_URL: &str =
    "https://github.com/syshin0116/clidex/releases/download/index/index.yaml";

pub const DEFAULT_MAX_RESULTS: usize = 10;
