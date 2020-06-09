use std::fs::read_to_string;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Config {
    fedora: FedoraConfig,
    repos: RepoConfig,
    #[serde(rename = "arch")]
    arches: Vec<ArchConfig>,
    #[serde(rename = "release")]
    releases: Vec<ReleaseConfig>,
}

#[derive(Debug, Deserialize)]
struct FedoraConfig {
    #[serde(rename = "api-url")]
    api_url: String,
    timeout: u64,
}

#[derive(Debug, Deserialize)]
struct RepoConfig {
    stable: Vec<String>,
    updates: Vec<String>,
    testing: Vec<String>,
    rawhide: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ArchConfig {
    name: String,
    multiarch: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct ReleaseConfig {
    name: String,
    #[serde(rename = "type")]
    rtype: ReleaseType,
    arches: Vec<String>,
}

#[derive(Debug, Deserialize)]
enum ReleaseType {
    #[serde(rename = "rawhide")]
    Rawhide,
    #[serde(rename = "prerelease")]
    PreRelease,
    #[serde(rename = "stable")]
    Stable,
    #[serde(rename = "oldstable")]
    OldStable,
}

fn get_config() -> Result<Config, String> {
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
