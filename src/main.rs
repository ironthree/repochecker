use std::path::PathBuf;

use log::{debug, error, info};
//use warp::Filter;

use repochecker::config::{get_config, ReleaseType};
use repochecker::pagure::get_admins;
use repochecker::repo::{get_repo_closure, make_cache, BrokenDep};

/*
#[tokio::main]
async fn main() {
    let hello = warp::path!("hello" / String)
        .map(|name| format!("Hello, {}!", name));

    let run = warp::serve(hello)
        .run(([127, 0, 0, 1], 8000));

    println!("Serving at http://localhost:8000 ...");

    run.await;
}
*/

fn get_data_path() -> PathBuf {
    let mut path = PathBuf::new();
    path.push(std::env::current_dir().expect("Unable to determine current directory."));
    path.push("data/");
    path
}

fn get_json_path(release: &str, testing: bool) -> PathBuf {
    let mut path = get_data_path();
    if !testing {
        path.push(format!("{}.json", release));
    } else {
        path.push(format!("{}-testing.json", release));
    }

    path
}

fn write_json_to_file(path: &PathBuf, broken: &[BrokenDep]) -> Result<(), String> {
    let json = match serde_json::to_string_pretty(&broken) {
        Ok(json) => json,
        Err(_) => {
            return Err(String::from(
                "Failed to serialize broken dependencies into JSON.",
            ))
        }
    };

    let data_path = get_data_path();

    if !data_path.exists() {
        std::fs::create_dir_all(data_path).expect("Failed to create data directory.");
    }

    if let Err(_) = std::fs::write(&path, json) {
        error!("Failed to write data to disk: {}", &path.to_string_lossy());
    }

    Ok(())
}

struct MatrixEntry {
    arch: String,
    multi_arch: Vec<String>,
    repos: Vec<String>,
    testing: bool,
}

fn main() -> Result<(), String> {
    env_logger::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let config = get_config()?;

    debug!("{:#?}", config);

    let admins = get_admins(15)?;

    for release in &config.releases {
        let repos = match &release.rtype {
            ReleaseType::Rawhide => vec![(config.repos.rawhide.clone(), false)],
            ReleaseType::PreRelease => vec![(config.repos.stable.clone(), false)],
            ReleaseType::Stable => {
                let mut stable_repos = Vec::new();
                stable_repos.extend(config.repos.stable.clone());
                stable_repos.extend(config.repos.updates.clone());

                let mut testing_repos = Vec::new();
                testing_repos.extend(config.repos.stable.clone());
                testing_repos.extend(config.repos.updates.clone());
                testing_repos.extend(config.repos.testing.clone());

                vec![(stable_repos, false), (testing_repos, true)]
            }
        };

        let mut matrix: Vec<MatrixEntry> = Vec::new();

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
                }
            };

            for repos in &repos {
                matrix.push(MatrixEntry {
                    arch: arch.to_owned(),
                    multi_arch: multi_arch.clone(),
                    repos: repos.0.clone(),
                    testing: repos.1,
                });
            }
        }

        let mut stable_broken: Vec<BrokenDep> = Vec::new();
        let mut testing_broken: Vec<BrokenDep> = Vec::new();

        // TODO: get repository contents to determine Exclude / Exclusive Arch values

        for entry in matrix {
            if !entry.testing {
                info!("Generating data for {} / {}", &release.name, &entry.arch);
            } else {
                info!(
                    "Generating data for {} / {} (testing)",
                    &release.name, &entry.arch
                );
            }

            make_cache(&release.name, &entry.arch, &entry.repos)?;

            let broken = get_repo_closure(
                &release.name,
                &entry.arch,
                &entry.multi_arch,
                &entry.repos,
                &admins,
            )?;

            // TODO: filter out false positives based on Exclude / Exclusive Arch

            debug!("{:#?}", &broken);

            if !entry.testing {
                stable_broken.extend(broken);
            } else {
                testing_broken.extend(broken);
            }
        }

        // TODO: merge and sort data for different arches (get merge logic from fedora-health-check)

        if !stable_broken.is_empty() {
            let json_path = get_json_path(&release.name, false);
            write_json_to_file(&json_path, &stable_broken)?;
        }

        if !testing_broken.is_empty() {
            let json_path = get_json_path(&release.name, true);
            write_json_to_file(&json_path, &testing_broken)?;
        }
    }

    Ok(())
}
