use std::collections::{HashMap, HashSet};
use std::fs::read_to_string;
use std::path::{Path, PathBuf};

use log::{debug, error, info};

use serde::Deserialize;

// TODO: add statistics for which overrides are actually used

const OVERRIDES_FILENAME: &str = "overrides.json";

pub type Overrides = HashMap<String, ReleaseOverrides>;
pub type ReleaseOverrides = HashMap<String, PackageOverrides>;
pub type PackageOverrides = HashMap<String, OverrideEntry>;

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum OverrideEntry {
    All(String),
    Packages(Vec<String>),
}

fn get_overrides_path() -> Result<Box<Path>, String> {
    let local = {
        let mut path = std::env::current_dir().map_err(|error| error.to_string())?;
        path.push(OVERRIDES_FILENAME);
        path
    };

    if local.exists() {
        return Ok(local.into_boxed_path());
    };

    let site = {
        let mut path = PathBuf::new();
        path.push("/etc/repochecker/");
        path.push(OVERRIDES_FILENAME);
        path
    };

    if site.exists() {
        return Ok(site.into_boxed_path());
    }

    let default = {
        let mut path = PathBuf::new();
        path.push("/usr/share/repochecker/");
        path.push(OVERRIDES_FILENAME);
        path
    };

    if default.exists() {
        return Ok(default.into_boxed_path());
    }

    Err(String::from("No overrides file was found."))
}

pub fn get_overrides() -> Result<Overrides, String> {
    let path = get_overrides_path()?;

    info!("Using overrides file: {}", path.to_string_lossy());

    let contents = match read_to_string(path) {
        Ok(string) => string,
        Err(error) => return Err(error.to_string()),
    };

    let overrides: Overrides = match serde_json::from_str(&contents) {
        Ok(overrides) => overrides,
        Err(error) => return Err(error.to_string()),
    };

    Ok(overrides)
}

pub fn is_overridden(overrides: &Overrides, release: &str, arch: &str, package: &str, broken: &str) -> bool {
    let all_release = match overrides.get("all") {
        Some(overrides) => overrides,
        None => {
            error!("Overrides configuration invalid or incomplete for release 'all'.");
            return false;
        },
    };

    let all_release_all_arch = match all_release.get("all") {
        Some(overrides) => overrides,
        None => {
            error!("Overrides configuration invalid or incomplete for 'all/all'.");
            return false;
        },
    };

    let all_release_per_arch = match all_release.get(arch) {
        Some(overrides) => overrides,
        None => {
            error!("Overrides configuration invalid or incomplete for 'all/{}'.", arch);
            return false;
        },
    };

    let per_release = match overrides.get(release) {
        Some(overrides) => overrides,
        None => {
            error!(
                "Overrides configuration is invalid or incomplete for release '{}'.",
                release
            );
            return false;
        },
    };

    let per_release_all_arch = match per_release.get("all") {
        Some(overrides) => overrides,
        None => {
            error!("Overrides configuration invalid or incomplete for '{}/all'.", release);
            return false;
        },
    };

    let per_release_per_arch = match per_release.get(arch) {
        Some(overrides) => overrides,
        None => {
            error!(
                "Overrides configuration invalid or incomplete for '{}/{}'.",
                release, arch
            );
            return false;
        },
    };

    #[derive(Debug)]
    enum Override<'a> {
        All,
        Packages(HashSet<&'a str>),
    }

    let mut overrides: HashMap<&str, Override> = HashMap::new();

    // this is not useless
    #[allow(clippy::useless_vec)]
    for x in vec![
        all_release_all_arch,
        all_release_per_arch,
        per_release_all_arch,
        per_release_per_arch,
    ] {
        for (key, value) in x {
            match value {
                OverrideEntry::All(_) => {
                    overrides.insert(key, Override::All);
                },
                OverrideEntry::Packages(ps) => {
                    overrides
                        .entry(key)
                        .and_modify(|list| match list {
                            Override::All => {},
                            Override::Packages(list) => {
                                for p in ps {
                                    list.insert(p);
                                }
                            },
                        })
                        .or_insert_with(|| {
                            let mut list: HashSet<&str> = HashSet::new();
                            for p in ps {
                                list.insert(p);
                            }

                            Override::Packages(list)
                        });
                },
            };
        }
    }

    let matched = match overrides.get(broken) {
        Some(value) => match value {
            Override::All => true,
            Override::Packages(packages) => packages.contains(package),
        },
        None => false,
    };

    if matched {
        debug!(
            "Matched override for {} / {} / {} / {}.",
            release, arch, broken, package
        );
    }

    matched
}
