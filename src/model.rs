use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Tool {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub binary: Option<String>,
    #[serde(alias = "description")]
    pub desc: String,
    pub category: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub install: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stars: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub brew_installs_30d: Option<u64>,
    #[serde(default)]
    pub links: Links,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_updated: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Links {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repo: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub homepage: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub docs: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub llms_txt: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Index {
    pub version: u32,
    pub generated: String,
    pub tools: Vec<Tool>,
}
