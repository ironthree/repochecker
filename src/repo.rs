use std::process::Command;

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

fn get_rawhide_contents(arch: &str) -> Result<Vec<Package>, String> {
    let mut path = std::path::PathBuf::new();
    path.push(std::env::current_dir().map_err(|error| error.to_string())?);
    path.push(format!("cache/rawhide/{}", arch));

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
        .arg("rawhide")
        .arg("--repo")
        .arg("rawhide")
        .arg("--repo")
        .arg("rawhide-source")
        .arg("--forcearch")
        .arg(arch)
        .arg("repoquery")
        .arg("--queryformat")
        .arg("%{name} %{source_name} %{epoch} %{version} %{release} %{arch}");

    let output = match dnf.output() {
        Ok(output) => output,
        Err(error) => return Err(error.to_string()),
    };

    if !output.status.success() {
        println!("{}", String::from_utf8(output.stdout).unwrap());
        println!("{}", String::from_utf8(output.stderr).unwrap());
        return Err(String::from("repoquery returned error code."));
    };

    let string = match String::from_utf8(output.stdout) {
        Ok(string) => string,
        Err(error) => return Err(error.to_string()),
    }
    .trim()
    .to_string();

    let lines = string.split('\n');

    let mut packages: Vec<Package> = Vec::new();
    for line in lines {
        let mut entries: Vec<&str> = line.split(' ').collect();

        if entries.len() != 6 {
            return Err(format!("Failed to parse line: {}", line));
        };

        let arch = entries.pop().unwrap().to_string();
        let release = entries.pop().unwrap().to_string();
        let version = entries.pop().unwrap().to_string();
        let epoch: i32 = entries.pop().unwrap().parse().unwrap();
        let source_name = entries.pop().unwrap().to_string();
        let name = entries.pop().unwrap().to_string();

        packages.push(Package {
            name,
            source_name,
            epoch,
            version,
            release,
            arch,
        });
    }

    Ok(packages)
}

fn get_rawhide_repoclosure(arch: &str) -> Result<Vec<BrokenDep>, String> {
    let mut path = std::path::PathBuf::new();
    path.push(std::env::current_dir().map_err(|error| error.to_string())?);
    path.push(format!("cache/rawhide/{}", arch));

    if !path.exists() || !path.is_dir() {
        return Err(String::from("Cache does not exist."));
    };

    let mut dnf = Command::new("dnf");

    dnf.arg("--quiet")
        .arg("--installroot")
        .arg(&path)
        .arg("--releasever")
        .arg("rawhide")
        .arg("--repo")
        .arg("rawhide")
        .arg("--repo")
        .arg("rawhide-source")
        .arg("--forcearch")
        .arg(arch)
        .arg("repoclosure")
        .arg("--newest");

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

    fn state_to_dep(state: State) -> BrokenDep {
        BrokenDep {
            package: state.nevra.0.to_string(),
            epoch: state.nevra.1.to_string(),
            version: state.nevra.2.to_string(),
            release: state.nevra.3.to_string(),
            arch: state.nevra.4.to_string(),
            repo: state.repo.to_string(),
            broken: state.broken.iter().map(|s| s.to_string()).collect(),
        }
    }

    let mut state: Option<State> = None;

    for line in lines {
        if line.starts_with("package: ") {
            if let Some(status) = state {
                broken_deps.push(state_to_dep(status));
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

    if let Some(status) = state {
        broken_deps.push(state_to_dep(status));
    }

    Ok(broken_deps)
}

#[derive(Debug, Deserialize, Serialize)]
struct BrokenDep {
    package: String,
    epoch: String,
    version: String,
    release: String,
    arch: String,
    repo: String,
    // source: String, TODO
    broken: Vec<String>,
}
