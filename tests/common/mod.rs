pub fn load_real_index() -> Option<Vec<clidex::model::Tool>> {
    let path = clidex::config::index_path();
    if !path.exists() && std::env::var_os("CLIDEX_INDEX_PATH").is_none() {
        return None;
    }
    let index = clidex::index::load_index().expect("real index must exist and contain valid YAML");
    assert!(
        index.tools.len() >= 100,
        "real index unexpectedly contains fewer than 100 tools"
    );
    Some(index.tools)
}
