use serde::{Deserialize, Serialize};

#[derive(Debug)]
pub struct Package {
    pub name: String,
    pub source_name: String,
    pub epoch: i32,
    pub version: String,
    pub release: String,
    pub arch: String,
}

#[derive(Debug, Deserialize, PartialEq, Serialize)]
pub struct BrokenDep {
    pub package: String,
    pub epoch: String,
    pub version: String,
    pub release: String,
    pub arch: String,
    pub repo: String,
    pub broken: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repo_arch: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub admin: Option<String>,
}
