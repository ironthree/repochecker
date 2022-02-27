use std::collections::HashMap;
use std::fs::read_to_string;
use std::path::{Path, PathBuf};

use log::{debug, error, info};

use serde::Deserialize;

const OVERRIDES_FILENAME: &str = "overrides.json";

pub type OverrideValues = HashMap<String, ReleaseOverrides>;
pub type ReleaseOverrides = HashMap<String, PackageOverrides>;
pub type PackageOverrides = HashMap<String, OverrideEntry>;
pub type OverrideStats = HashMap<String, u32>;

#[derive(Clone, Debug, Deserialize)]
#[serde(untagged)]
pub enum OverrideEntry {
    All(String),
    Packages(Vec<String>),
}

#[derive(Clone, Debug)]
pub struct Overrides {
    pub data: OverrideValues,
    pub stats: OverrideStats,
}

impl Overrides {
    pub fn load_from_disk() -> Result<Self, String> {
        let path = get_overrides_path()?;

        info!("Using overrides file: {}", path.to_string_lossy());

        let contents = match read_to_string(path) {
            Ok(string) => string,
            Err(error) => return Err(error.to_string()),
        };

        let overrides: OverrideValues = match serde_json::from_str(&contents) {
            Ok(overrides) => overrides,
            Err(error) => return Err(error.to_string()),
        };

        // initialize usage count for every override path with 0
        let mut stats: OverrideStats = HashMap::new();
        for (release, ros) in &overrides {
            for (arch, aos) in ros {
                for (broken, bos) in aos {
                    match bos {
                        OverrideEntry::All(_) => {
                            stats.insert(opath_to_str(release, arch, broken, "all"), 0);
                        },
                        OverrideEntry::Packages(entries) => {
                            for entry in entries {
                                stats.insert(opath_to_str(release, arch, broken, entry), 0);
                            }
                        },
                    }
                }
            }
        }

        Ok(Overrides { data: overrides, stats })
    }

    pub fn lookup(&mut self, release: &str, arch: &str, package: &str, broken: &str) -> bool {
        // extract and validate release- and / or arch-specific and unspecific overrides

        let all_release = match self.data.get("all") {
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

        let per_release = match self.data.get(release) {
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

        // check arguments against overrides (most specific overrides first)

        // check release- and arch-specific overrides
        if let Some(entry) = per_release_per_arch.get(broken) {
            let matched = match entry {
                OverrideEntry::All(_) => true,
                OverrideEntry::Packages(packages) => packages.contains(&package.to_owned()),
            };

            if matched {
                let path = opath_to_str(release, arch, broken, package);
                self.stats
                    .entry(path.to_owned())
                    .and_modify(|count| *count += 1)
                    .or_insert_with(|| {
                        error!("Failed to match override path in stats: {}", path);
                        1
                    });

                debug!(
                    "Matched override for {} / {} / {} / {}.",
                    release, arch, broken, package
                );
                return true;
            }
        }

        // check release-specific overrides
        if let Some(entry) = per_release_all_arch.get(broken) {
            let matched = match entry {
                OverrideEntry::All(_) => true,
                OverrideEntry::Packages(packages) => packages.contains(&package.to_owned()),
            };

            if matched {
                let path = opath_to_str(release, "all", broken, package);
                self.stats
                    .entry(path.to_owned())
                    .and_modify(|count| *count += 1)
                    .or_insert_with(|| {
                        error!("Failed to match override path in stats: {}", path);
                        1
                    });

                debug!(
                    "Matched override for {} / {} / {} / {}.",
                    release, "all", broken, package
                );
                return true;
            }
        }

        // check arch-specific overrides
        if let Some(entry) = all_release_per_arch.get(broken) {
            let matched = match entry {
                OverrideEntry::All(_) => true,
                OverrideEntry::Packages(packages) => packages.contains(&package.to_owned()),
            };

            if matched {
                let path = opath_to_str("all", arch, broken, package);
                self.stats
                    .entry(path.to_owned())
                    .and_modify(|count| *count += 1)
                    .or_insert_with(|| {
                        error!("Failed to match override path in stats: {}", path);
                        1
                    });

                debug!("Matched override for {} / {} / {} / {}.", "all", arch, broken, package);
                return true;
            }
        }

        // check generic overrides
        if let Some(entry) = all_release_all_arch.get(broken) {
            let matched = match entry {
                OverrideEntry::All(_) => true,
                OverrideEntry::Packages(packages) => packages.contains(&package.to_owned()),
            };

            if matched {
                let path = opath_to_str("all", "all", broken, package);
                self.stats
                    .entry(path.to_owned())
                    .and_modify(|count| *count += 1)
                    .or_insert_with(|| {
                        error!("Failed to match override path in stats: {}", path);
                        1
                    });

                debug!("Matched override for {} / {} / {} / {}.", "all", "all", broken, package);
                return true;
            }
        }

        false
    }
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

fn opath_to_str(release: &str, arch: &str, broken: &str, package: &str) -> String {
    format!("{}/{}/{}/{}", release, arch, broken, package)
}
