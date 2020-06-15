use std::fs::read_to_string;

use serde::Deserialize;

#[derive(Clone, Debug, Deserialize)]
pub struct Config {
    pub repochecker: RepoCheckerConfig,
    pub repos: RepoConfig,
    #[serde(rename = "arch")]
    pub arches: Vec<ArchConfig>,
    #[serde(rename = "release")]
    pub releases: Vec<ReleaseConfig>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RepoCheckerConfig {
    pub interval: u64,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RepoConfig {
    pub stable: Vec<String>,
    pub updates: Vec<String>,
    pub testing: Vec<String>,
    pub rawhide: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ArchConfig {
    pub name: String,
    pub multiarch: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct ReleaseConfig {
    pub name: String,
    #[serde(rename = "type")]
    pub rtype: ReleaseType,
    pub arches: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
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

#[derive(Debug)]
pub struct MatrixEntry {
    pub release: String,
    pub arches: Vec<Arch>,
    pub repos: Vec<String>,
    pub check: Vec<String>,
    pub with_testing: bool,
}

#[derive(Clone, Debug)]
pub struct Arch {
    pub name: String,
    pub multi_arch: Vec<String>,
}

pub fn matrix_from_config(config: &Config) -> Result<Vec<MatrixEntry>, String> {
    let mut matrix: Vec<MatrixEntry> = Vec::new();

    #[derive(Debug)]
    struct Repos {
        repos: Vec<String>,
        check: Vec<String>,
        with_testing: bool,
    }

    for release in &config.releases {
        let repos = match &release.rtype {
            ReleaseType::Rawhide => vec![Repos {
                repos: config.repos.rawhide.clone(),
                check: config.repos.rawhide.clone(),
                with_testing: false,
            }],
            ReleaseType::PreRelease => vec![Repos {
                repos: config.repos.stable.clone(),
                check: config.repos.stable.clone(),
                with_testing: false,
            }],
            ReleaseType::Stable => {
                let mut stable_repos = Vec::new();
                stable_repos.extend(config.repos.stable.clone());
                stable_repos.extend(config.repos.updates.clone());

                let mut testing_repos = Vec::new();
                testing_repos.extend(config.repos.stable.clone());
                testing_repos.extend(config.repos.updates.clone());
                testing_repos.extend(config.repos.testing.clone());

                vec![
                    Repos {
                        repos: stable_repos.clone(),
                        check: stable_repos.clone(),
                        with_testing: false,
                    },
                    Repos {
                        repos: testing_repos,
                        check: config.repos.testing.clone(),
                        with_testing: true,
                    },
                ]
            },
        };

        let mut arches: Vec<Arch> = Vec::new();

        for arch in &release.arches {
            let mut multi_arch: Option<Vec<String>> = None;

            for arch_config in &config.arches {
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
            });
        }
    }

    Ok(matrix)
}
