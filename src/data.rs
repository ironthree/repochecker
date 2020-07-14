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

#[derive(Debug, Deserialize, Serialize)]
pub struct BrokenDep {
    pub package: String,
    pub epoch: String,
    pub version: String,
    pub release: String,
    pub arch: String,
    pub repo: String,
    pub repo_arch: String,
    pub source: String,
    pub broken: Vec<String>,
    pub admin: String,
}
