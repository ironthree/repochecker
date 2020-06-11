use std::fs::read_to_string;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Config {
    pub repos: RepoConfig,
    #[serde(rename = "arch")]
    pub arches: Vec<ArchConfig>,
    #[serde(rename = "release")]
    pub releases: Vec<ReleaseConfig>,
}

#[derive(Debug, Deserialize)]
pub struct RepoConfig {
    pub stable: Vec<String>,
    pub updates: Vec<String>,
    pub testing: Vec<String>,
    pub rawhide: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct ArchConfig {
    pub name: String,
    pub multiarch: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct ReleaseConfig {
    pub name: String,
    #[serde(rename = "type")]
    pub rtype: ReleaseType,
    pub arches: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub enum ReleaseType {
    #[serde(rename = "rawhide")]
    Rawhide,
    #[serde(rename = "prerelease")]
    PreRelease,
    #[serde(rename = "stable")]
    Stable,
}

pub fn get_config() -> Result<Config, String> {
    let path = "repochecker.toml";
    let contents = match read_to_string(path) {
        Ok(string) => string,
        Err(error) => return Err(error.to_string()),
    };

    let config: Config = match toml::from_str(&contents) {
        Ok(config) => config,
        Err(error) => return Err(error.to_string()),
    };

    Ok(config)
}
