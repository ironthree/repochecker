use std::fs::read_to_string;
use std::path::{Path, PathBuf};

use log::info;

use serde::{Deserialize, Serialize};

const CONFIG_FILENAME: &str = "repochecker.toml";

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Config {
    pub repochecker: RepoCheckerConfig,
    pub repos: RepoConfig,
    #[serde(rename = "arch")]
    pub arches: Vec<ArchConfig>,
    #[serde(rename = "release")]
    pub releases: Vec<ReleaseConfig>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RepoCheckerConfig {
    pub interval: f64,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RepoConfig {
    pub stable: Vec<String>,
    pub updates: Vec<String>,
    pub testing: Vec<String>,
    pub rawhide: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ArchConfig {
    pub name: String,
    pub multiarch: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ReleaseConfig {
    pub name: String,
    #[serde(rename = "type")]
    pub rtype: ReleaseType,
    pub arches: Vec<String>,
    pub archived: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub enum ReleaseType {
    #[serde(rename = "rawhide")]
    Rawhide,
    #[serde(rename = "prerelease")]
    PreRelease,
    #[serde(rename = "stable")]
    Stable,
}

fn get_config_path() -> Result<Box<Path>, String> {
    let local = {
        let mut path = std::env::current_dir().map_err(|error| error.to_string())?;
        path.push(CONFIG_FILENAME);
        path
    };

    if local.exists() {
        return Ok(local.into_boxed_path());
    };

    let site = {
        let mut path = PathBuf::new();
        path.push("/etc/repochecker/");
        path.push(CONFIG_FILENAME);
        path
    };

    if site.exists() {
        return Ok(site.into_boxed_path());
    }

    let default = {
        let mut path = PathBuf::new();
        path.push("/usr/share/repochecker/");
        path.push(CONFIG_FILENAME);
        path
    };

    if default.exists() {
        return Ok(default.into_boxed_path());
    }

    Err(String::from("No configuration file was found."))
}

pub fn get_config() -> Result<Config, String> {
    let path = get_config_path()?;

    info!("Using configuration file: {}", path.to_string_lossy());

    let contents = match read_to_string(&path) {
        Ok(string) => string,
        Err(error) => return Err(error.to_string()),
    };

    let config: Config = match basic_toml::from_str(&contents) {
        Ok(config) => config,
        Err(error) => return Err(error.to_string()),
    };

    Ok(config)
}

#[derive(Debug)]
pub struct MatrixEntry {
    pub release: String,
    pub arches: Vec<Arch>,
    pub repos: Vec<String>,
    pub check: Vec<String>,
    pub with_testing: bool,
    pub archived: bool,
}

#[derive(Clone, Debug)]
pub struct Arch {
    pub name: String,
    pub multi_arch: Vec<String>,
}

impl Config {
    pub fn to_matrix(&self) -> Result<Vec<MatrixEntry>, String> {
        let mut matrix: Vec<MatrixEntry> = Vec::new();

        #[derive(Debug)]
        struct Repos {
            repos: Vec<String>,
            check: Vec<String>,
            with_testing: bool,
        }

        for release in &self.releases {
            let repos = match &release.rtype {
                ReleaseType::Rawhide => vec![Repos {
                    repos: self.repos.rawhide.clone(),
                    check: self.repos.rawhide.clone(),
                    with_testing: false,
                }],
                ReleaseType::PreRelease => vec![Repos {
                    repos: self.repos.stable.clone(),
                    check: self.repos.stable.clone(),
                    with_testing: false,
                }],
                ReleaseType::Stable => {
                    let mut stable_repos = Vec::new();
                    stable_repos.extend(self.repos.stable.clone());
                    stable_repos.extend(self.repos.updates.clone());

                    let mut testing_repos = Vec::new();
                    testing_repos.extend(self.repos.stable.clone());
                    testing_repos.extend(self.repos.updates.clone());
                    testing_repos.extend(self.repos.testing.clone());

                    vec![
                        Repos {
                            repos: stable_repos.clone(),
                            check: stable_repos.clone(),
                            with_testing: false,
                        },
                        Repos {
                            repos: testing_repos,
                            check: self.repos.testing.clone(),
                            with_testing: true,
                        },
                    ]
                },
            };

            let mut arches: Vec<Arch> = Vec::new();

            for arch in &release.arches {
                let mut multi_arch: Option<Vec<String>> = None;

                for arch_config in &self.arches {
                    if &arch_config.name == arch {
                        multi_arch = Some(arch_config.multiarch.clone());
                    }
                }

                let multi_arch = match multi_arch {
                    Some(values) => values,
                    None => {
                        return Err(format!(
                            "Could not find multiarch configuration for {}/{}.",
                            &release.name, &arch
                        ))
                    },
                };

                arches.push(Arch {
                    name: arch.clone(),
                    multi_arch,
                });
            }

            for repo in repos {
                matrix.push(MatrixEntry {
                    release: release.name.to_string(),
                    arches: arches.clone(),
                    repos: repo.repos,
                    check: repo.check,
                    with_testing: repo.with_testing,
                    archived: release.archived,
                });
            }
        }

        Ok(matrix)
    }
}
