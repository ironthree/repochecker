use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::process::Command;

use log::{debug, error};

use crate::data::{BrokenItem, Package};
use crate::overrides::{is_overridden, Overrides};
use crate::parse::{parse_repoclosure, parse_repoquery};

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
        debug!("dnf makecache for {} / {} exited with an error code:", release, arch);

        debug!(
            "{}",
            match String::from_utf8(output.stdout) {
                Ok(string) => string,
                Err(error) => format!("Failed to decode dnf output: {}", error.to_string()),
            }
        );

        debug!(
            "{}",
            match String::from_utf8(output.stderr) {
                Ok(string) => string,
                Err(error) => format!("Failed to decode dnf output: {}", error.to_string()),
            }
        );

        return Err(format!(
            "dnf makecache for {} / {} exited with an error code.",
            release, arch
        ));
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
        debug!(
            "{}",
            match String::from_utf8(output.stdout) {
                Ok(string) => string,
                Err(error) => format!("Failed to decode dnf output: {}", error.to_string()),
            }
        );
        debug!(
            "{}",
            match String::from_utf8(output.stderr) {
                Ok(string) => string,
                Err(error) => format!("Failed to decode dnf output: {}", error.to_string()),
            }
        );
        return Err(String::from("dnf repoquery exited with an error code."));
    };

    let string = String::from_utf8(output.stdout)
        .map_err(|error| error.to_string())?
        .trim()
        .to_string();

    parse_repoquery(&string)
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

#[allow(clippy::many_single_char_names)]
fn get_repo_closure_arched_repo(
    release: &str,
    arch: &str,
    multi_arch: &[String],
    repos: &[String],
    check: &str,
    admins: &HashMap<String, String>,
) -> Result<Vec<BrokenItem>, String> {
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

    dnf.arg("--check");
    dnf.arg(check);

    let output = dnf.output().map_err(|error| error.to_string())?;

    let string = String::from_utf8(output.stdout)
        .map_err(|error| error.to_string())?
        .trim()
        .to_string();

    let closure = parse_repoclosure(&string)?;

    let mut broken_deps: Vec<BrokenItem> = Vec::new();
    for item in closure {
        let source = if item.arch == "src" {
            item.package.as_str()
        } else {
            match source_map.get(item.package.as_str()) {
                Some(source) => source,
                None => return Err(format!("Unable to find source package for {}", &item.package)),
            }
        };

        let admin = match admins.get(&source.to_string()) {
            Some(admin) => admin.to_string(),
            None => {
                error!("Unable to determine maintainer for {}", &source);
                String::from("(N/A)")
            },
        };

        let broken_dep = BrokenItem {
            source: source.to_string(),
            package: item.package,
            epoch: item.epoch,
            version: item.version,
            release: item.release,
            arch: item.arch,
            admin,
            repo: item.repo,
            repo_arch: arch.to_string(),
            broken: item.broken,
            since: None,
        };

        broken_deps.push(broken_dep);
    }

    Ok(broken_deps)
}

fn get_repo_closure_arched(
    release: &str,
    arch: &str,
    multi_arch: &[String],
    repos: &[String],
    check: &[String],
    admins: &HashMap<String, String>,
) -> Result<Vec<BrokenItem>, String> {
    let mut all_broken: Vec<BrokenItem> = Vec::new();

    for checked in check {
        let broken = get_repo_closure_arched_repo(release, arch, multi_arch, repos, checked, admins)?;
        all_broken.extend(broken);
    }

    Ok(all_broken)
}

pub fn get_repo_closure(
    release: &str,
    arches: &[String],
    multi_arch: &HashMap<String, Vec<String>>,
    repos: &[String],
    check: &[String],
    overrides: &Overrides,
    admins: &HashMap<String, String>,
) -> Result<Vec<BrokenItem>, String> {
    // check which source packages do not produce any binary packages on a given architecture
    // (emulates detection of ExcludeArch / ExclusiveArch, which cannot be queried directly)
    let mut all_packages: HashSet<String> = HashSet::new();
    let mut arch_map: HashMap<&str, Vec<String>> = HashMap::new();

    for arch in arches {
        let packages = get_repo_contents(release, arch, repos)?;
        let mut built: Vec<String> = Vec::new();

        for package in packages {
            if package.arch == "src" {
                continue;
            }

            built.push(package.source_name.clone());
            all_packages.insert(package.source_name.clone());
        }

        arch_map.insert(arch, built);
    }

    let mut excluded: HashMap<&str, HashSet<&str>> = HashMap::new();
    for arch in arches {
        let arch_packages = arch_map.get(arch.as_str()).expect("Something went terribly wrong.");
        let mut arch_excluded: HashSet<&str> = HashSet::new();

        for package in &all_packages {
            if !arch_packages.contains(package) {
                debug!(
                    "Skipping {} on {} / {} due to detected ExclusiveArch / ExcludeArch.",
                    package, release, arch
                );
                arch_excluded.insert(package);
            }
        }

        excluded.insert(arch, arch_excluded);
    }

    let mut all_broken: Vec<BrokenItem> = Vec::new();
    for arch in arches {
        make_cache(release, arch, repos)?;

        let multi = multi_arch.get(arch).unwrap();
        let arch_excluded = excluded.get(arch.as_str()).expect("Something went terribly wrong.");

        let mut broken = get_repo_closure_arched(release, arch, multi, repos, check, admins)?;

        // skip source packages that do not produce any binaries on this architecture,
        // because this means that the current architecture is probably excluded
        broken.retain(|item| !(item.arch == "src" && arch_excluded.contains(&item.source.as_str())));

        all_broken.extend(broken);
    }

    all_broken.iter_mut().for_each(|item| {
        let arch = item.repo_arch.clone();
        let package = item.package.clone();

        item.broken
            .retain(|broken| !is_overridden(overrides, release, &arch, &package, broken))
    });

    all_broken.retain(|item| !item.broken.is_empty());

    // sort by (source, package, arch)
    all_broken.sort_by(|a, b| (&a.source, &a.package, &a.arch).cmp(&(&b.source, &b.package, &b.arch)));

    Ok(all_broken)
}
