use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Command;

use log::debug;
use log::error;
use serde::{Deserialize, Serialize};

use crate::parse::parse_nevra;

#[derive(Debug)]
struct Package {
    name: String,
    source_name: String,
    epoch: i32,
    version: String,
    release: String,
    arch: String,
}

fn get_cache_path(release: &str, arch: &str) -> Result<PathBuf, String> {
    let mut path = PathBuf::new();
    path.push(std::env::current_dir().map_err(|error| error.to_string())?);
    path.push(format!("cache/{}/{}", release, arch));
    Ok(path)
}

fn make_cache(release: &str, arch: &str, repos: &[String]) -> Result<(), String> {
    let path = get_cache_path(release, arch)?;

    let mut dnf = Command::new("dnf");

    dnf.arg("--quiet")
        .arg("--installroot")
        .arg(&path)
        .arg("--releasever")
        .arg(release);

    for repo in repos {
        dnf.arg("--repo");
        dnf.arg(repo);
    }

    dnf.arg("--forcearch").arg(arch);
    dnf.arg("makecache").arg("--refresh");

    let output = dnf.output().map_err(|error| error.to_string())?;

    if !output.status.success() {
        debug!("dnf makecache exited with an error code:",);
        debug!("{}", String::from_utf8(output.stdout).unwrap());
        debug!("{}", String::from_utf8(output.stderr).unwrap());
        return Err(String::from("dnf makecache exited with an error code."));
    };

    Ok(())
}

fn get_repo_contents(release: &str, arch: &str, repos: &[String]) -> Result<Vec<Package>, String> {
    let path = get_cache_path(release, arch)?;

    if !path.exists() {
        if let Err(error) = std::fs::create_dir_all(&path) {
            return Err(error.to_string());
        }
    };

    if !path.is_dir() {
        return Err(String::from("Cache directory path is not a directory."));
    }

    let mut dnf = Command::new("dnf");

    dnf.arg("--quiet")
        .arg("--installroot")
        .arg(&path)
        .arg("--releasever")
        .arg(release);

    for repo in repos {
        dnf.arg("--repo");
        dnf.arg(repo);
    }

    dnf.arg("--forcearch").arg(arch);

    dnf.arg("repoquery")
        .arg("--queryformat")
        .arg("%{name} %{source_name} %{epoch} %{version} %{release} %{arch}");

    let output = dnf.output().map_err(|error| error.to_string())?;

    if !output.status.success() {
        debug!("dnf makecache exited with an error code:",);
        debug!("{}", String::from_utf8(output.stdout).unwrap());
        debug!("{}", String::from_utf8(output.stderr).unwrap());
        return Err(String::from("dnf repoquery exited with an error code."));
    };

    let string = String::from_utf8(output.stdout)
        .map_err(|error| error.to_string())?
        .trim()
        .to_string();

    let lines = string.split('\n');

    let mut packages: Vec<Package> = Vec::new();
    for line in lines {
        let mut split = line.split(' ');

        // match only exactly 6 components
        match (
            split.next(),
            split.next(),
            split.next(),
            split.next(),
            split.next(),
            split.next(),
            split.next(),
        ) {
            (
                Some(name),
                Some(source),
                Some(epoch),
                Some(version),
                Some(release),
                Some(arch),
                None,
            ) => packages.push(Package {
                name: name.to_string(),
                source_name: source.to_string(),
                epoch: epoch.parse().unwrap(),
                version: version.to_string(),
                release: release.to_string(),
                arch: arch.to_string(),
            }),
            _ => return Err(format!("Failed to parse line: {}", line)),
        };
    }

    Ok(packages)
}

fn get_source_map(contents: &[Package]) -> HashMap<&str, &str> {
    let mut map: HashMap<&str, &str> = HashMap::new();

    for package in contents {
        if package.arch == "src" {
            continue;
        }

        map.insert(&package.name, &package.source_name);
    }

    map
}

#[derive(Debug, Deserialize, Serialize)]
pub struct BrokenDep {
    pub package: String,
    pub epoch: String,
    pub version: String,
    pub release: String,
    pub arch: String,
    pub repo: String,
    pub source: String,
    pub broken: Vec<String>,
    pub admin: String,
}

fn get_repo_closure_arched(
    release: &str,
    arch: &str,
    multi_arch: &[String],
    repos: &[String],
    admins: &HashMap<String, String>,
) -> Result<Vec<BrokenDep>, String> {
    let path = get_cache_path(release, arch)?;

    if !path.exists() || !path.is_dir() {
        return Err(String::from("Cache does not exist."));
    };

    let contents = get_repo_contents(release, arch, repos)?;
    let source_map = get_source_map(&contents);

    let mut dnf = Command::new("dnf");

    dnf.arg("--quiet");
    dnf.arg("--installroot").arg(&path);
    dnf.arg("--releasever").arg(release);
    dnf.arg("--forcearch").arg(arch);

    for repo in repos {
        dnf.arg("--repo");
        dnf.arg(repo);
    }

    dnf.arg("repoclosure").arg("--newest");

    for multi in multi_arch {
        dnf.arg("--arch");
        dnf.arg(multi);
    }

    debug!("running dnf command: {:#?}", &dnf);

    let output = dnf.output().map_err(|error| error.to_string())?;

    let string = String::from_utf8(output.stdout)
        .map_err(|error| error.to_string())?
        .trim()
        .to_string();

    let lines = string.split('\n');

    let mut broken_deps: Vec<BrokenDep> = Vec::new();

    struct State<'a> {
        nevra: (&'a str, &'a str, &'a str, &'a str, &'a str),
        repo: &'a str,
        broken: Vec<&'a str>,
    };

    let state_to_dep = |state: State| -> Result<BrokenDep, String> {
        let package = state.nevra.0;
        let arch = state.nevra.4;

        let source = if arch == "src" {
            package
        } else {
            match source_map.get(package) {
                Some(source) => source,
                None => return Err(format!("Unable to find source package for {}", &package)),
            }
        };

        let admin = match admins.get(&source.to_string()) {
            Some(admin) => admin.to_string(),
            None => {
                error!("Unable to determine maintainer for {}", &source);
                String::from("(N/A)")
            }
        };

        Ok(BrokenDep {
            package: state.nevra.0.to_string(),
            epoch: state.nevra.1.to_string(),
            version: state.nevra.2.to_string(),
            release: state.nevra.3.to_string(),
            arch: state.nevra.4.to_string(),
            repo: state.repo.to_string(),
            source: source.to_string(),
            broken: state.broken.iter().map(|s| s.to_string()).collect(),
            admin,
        })
    };

    let mut state: Option<State> = None;

    for line in lines {
        if line.starts_with("package: ") {
            if let Some(status) = state {
                broken_deps.push(state_to_dep(status)?);
            }

            let mut split = line.split(' ');
            match (split.next(), split.next(), split.next(), split.next()) {
                (Some(_), Some(nevra), Some(_), Some(repo)) => {
                    state = Some(State {
                        nevra: parse_nevra(nevra)?,
                        repo,
                        broken: Vec::new(),
                    });
                }
                _ => {
                    return Err(format!(
                        "Failed to parse line from repoclosure output: {}",
                        line
                    ))
                }
            }
        } else if line.starts_with("  unresolved deps:") {
            continue;
        } else if line.starts_with("    ") {
            match &mut state {
                Some(state) => state.broken.push(line.trim()),
                None => return Err(String::from("Unrecognised output from repoclosure.")),
            };
        } else {
            continue;
        }
    }

    // this should always be true
    if let Some(status) = state {
        broken_deps.push(state_to_dep(status)?);
    }

    Ok(broken_deps)
}

pub fn get_repo_closure(
    release: &str,
    arches: &[String],
    multi_arch: &HashMap<String, Vec<String>>,
    repos: &[String],
    admins: &HashMap<String, String>,
) -> Result<Vec<BrokenDep>, String> {
    let mut all_broken: Vec<BrokenDep> = Vec::new();

    for arch in arches {
        make_cache(release, arch, repos)?;

        let multi = multi_arch.get(arch).unwrap();

        let broken = get_repo_closure_arched(release, arch, multi, repos, admins)?;

        all_broken.extend(broken);
    }

    // sort by (source, package, arch)
    all_broken
        .sort_by(|a, b| (&a.source, &a.package, &a.arch).cmp(&(&b.source, &b.package, &b.arch)));

    Ok(all_broken)
}
