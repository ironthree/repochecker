use chrono::{DateTime, Utc};
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
pub struct BrokenItem {
    pub source: String,
    pub package: String,
    pub epoch: String,
    pub version: String,
    pub release: String,
    pub arch: String,
    pub admin: String,
    #[serde(default = "Vec::new")]
    pub maintainers: Vec<String>,
    pub repo: String,
    pub repo_arch: String,
    pub broken: Vec<String>,
    pub since: Option<DateTime<Utc>>,
}
