use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use log::{error, info};
use tokio::time::delay_for;
use warp::Filter;

use repochecker::config::{get_config, Config, ReleaseType};
use repochecker::pagure::get_admins;
use repochecker::repo::{get_repo_closure, BrokenDep};

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

#[derive(Debug)]
struct MatrixEntry {
    release: String,
    arches: Vec<Arch>,
    repos: Vec<String>,
    with_testing: bool,
}

#[derive(Clone, Debug)]
struct Arch {
    name: String,
    multi_arch: Vec<String>,
}

#[derive(Debug)]
struct Repos {
    repos: Vec<String>,
    with_testing: bool,
}

fn matrix_from_config(config: &Config) -> Result<Vec<MatrixEntry>, String> {
    let mut matrix: Vec<MatrixEntry> = Vec::new();

    for release in &config.releases {
        let repos = match &release.rtype {
            ReleaseType::Rawhide => vec![Repos {
                repos: config.repos.rawhide.clone(),
                with_testing: false,
            }],
            ReleaseType::PreRelease => vec![Repos {
                repos: config.repos.stable.clone(),
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
                        repos: stable_repos,
                        with_testing: false,
                    },
                    Repos {
                        repos: testing_repos,
                        with_testing: true,
                    },
                ]
            }
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
                }
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
                with_testing: repo.with_testing,
            });
        }
    }

    Ok(matrix)
}

struct State {
    config: Config,
    admins: HashMap<String, String>,
    values: HashMap<String, Vec<BrokenDep>>,
}

impl State {
    fn init(config: Config, admins: HashMap<String, String>) -> State {
        State {
            config,
            admins,
            values: HashMap::new(),
        }
    }
}

type GlobalState = Arc<Mutex<State>>;

async fn watcher(state: GlobalState) {
    match get_config() {
        Ok(config) => {
            let mut guard = state.lock().expect("Found a poisoned mutex.");
            let mut state = &mut *guard;
            state.config = config;
        }
        Err(error) => error!("Failed to read updated configuration: {}", error),
    };

    match get_admins(15).await {
        Ok(admins) => {
            let mut guard = state.lock().expect("Found a poisoned mutex.");
            let mut state = &mut *guard;
            state.admins = admins;
        }
        Err(error) => error!("Failed to read updated package maintainers: {}", error),
    };
}

async fn worker(state: GlobalState, entry: MatrixEntry) {
    if !entry.with_testing {
        info!("Generating data for {}", &entry.release);
    } else {
        info!("Generating data for {} (testing)", &entry.release);
    };

    let mut arches: Vec<String> = Vec::new();
    let mut multi_arch: HashMap<String, Vec<String>> = HashMap::new();

    for arch in &entry.arches {
        arches.push(arch.name.clone());
        multi_arch.insert(arch.name.clone(), arch.multi_arch.clone());
    }

    let admins = {
        let guard = state.lock().expect("Found a poisoned mutex.");
        let state = &*guard;
        state.admins.clone()
    };

    let broken = match get_repo_closure(&entry.release, &arches, &multi_arch, &entry.repos, &admins)
    {
        Ok(broken) => broken,
        Err(error) => {
            error!("Failed to generate repoclosure: {}", error);
            return;
        }
    };

    let json_path = get_json_path(&entry.release, entry.with_testing);

    if write_json_to_file(&json_path, &broken).is_err() {
        error!("Failed to write results to disk in JSON format.");
    };

    {
        let mut guard = state.lock().expect("Found a poisoned mutex.");
        let state = &mut *guard;

        state.values.insert(
            format!(
                "{}{}",
                &entry.release,
                if entry.with_testing { "-testing" } else { "" }
            ),
            broken,
        );
    }
}

async fn serve(state: GlobalState) {
    let data = warp::path!("data" / String).map(move |release| {
        let guard = state.lock().expect("Found a poisoned mutex.");
        let state = &*guard;

        match state.values.get(&release) {
            Some(values) => warp::http::Response::builder()
                .header("Content-Type", "application/json")
                .body(serde_json::to_string_pretty(values).unwrap())
                .unwrap(),
            None => warp::http::Response::builder()
                .status(404)
                .body(String::from("This page does not exist."))
                .unwrap(),
        }
    });

    info!("Serving at http://localhost:3030 ...");

    warp::serve(data).run(([127, 0, 0, 1], 3030)).await;
}

#[tokio::main]
async fn main() -> Result<(), String> {
    env_logger::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let config = get_config()?;
    let admins = tokio::spawn(get_admins(15))
        .await
        .map_err(|error| error.to_string())??;

    let state: GlobalState = Arc::new(Mutex::new(State::init(config, admins)));

    tokio::spawn(serve(state.clone()));

    loop {
        tokio::spawn(watcher(state.clone()));

        let config = {
            let guard = state.lock().expect("Found a poisoned mutex.");
            (&*guard).config.clone()
        };

        let matrix = matrix_from_config(&config)?;

        let handles = matrix
            .into_iter()
            .map(|entry| tokio::spawn(worker(state.clone(), entry)));

        for handle in handles {
            handle.await.map_err(|error| error.to_string())?;
        }

        info!("Finished generating data. Refreshing in 12 hours.");

        tokio::spawn(delay_for(Duration::from_secs(60 * 60 * 12)))
            .await
            .map_err(|error| error.to_string())?;
    }
}
