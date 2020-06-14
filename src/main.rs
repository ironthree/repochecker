#![warn(clippy::unwrap_used)]

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use log::{error, info};
use tokio::time::delay_for;
use warp::Filter;

use repochecker::config::{get_config, matrix_from_config, Config, MatrixEntry};
use repochecker::overrides::{get_overrides, Overrides};
use repochecker::pagure::get_admins;
use repochecker::repo::{get_repo_closure, BrokenDep};
use repochecker::utils::{get_json_path, read_json_from_file, write_json_to_file};

struct State {
    config: Config,
    overrides: Overrides,
    admins: HashMap<String, String>,
    values: HashMap<String, Vec<BrokenDep>>,
}

impl State {
    fn init(config: Config, overrides: Overrides, admins: HashMap<String, String>) -> State {
        State {
            config,
            overrides,
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
        },
        Err(error) => error!("Failed to read updated configuration: {}", error),
    };

    match get_overrides() {
        Ok(overrides) => {
            let mut guard = state.lock().expect("Found a poisoned mutex.");
            let mut state = &mut *guard;
            state.overrides = overrides;
        },
        Err(error) => error!("Failed to read updated overrides: {}", error),
    };

    match get_admins(15).await {
        Ok(admins) => {
            let mut guard = state.lock().expect("Found a poisoned mutex.");
            let mut state = &mut *guard;
            state.admins = admins;
        },
        Err(error) => error!("Failed to read updated package maintainers: {}", error),
    };
}

async fn worker(state: GlobalState, entry: MatrixEntry) {
    let suffix = if !entry.with_testing { "" } else { "-testing" };
    let pretty = format!("{}{}", &entry.release, suffix);

    let json_path = get_json_path(&entry.release, entry.with_testing);

    // populate data with cached values from file, if available
    let cached = read_json_from_file(&json_path);
    if let Ok(values) = cached {
        info!("Reusing cached data for {} until fresh data is available.", &pretty);

        let mut guard = state.lock().expect("Found a poisoned mutex.");
        let state = &mut *guard;

        state.values.insert(format!("{}{}", &entry.release, suffix), values);
    };

    info!("Generating data for {}", &pretty);

    let mut arches: Vec<String> = Vec::new();
    let mut multi_arch: HashMap<String, Vec<String>> = HashMap::new();

    for arch in &entry.arches {
        arches.push(arch.name.clone());
        multi_arch.insert(arch.name.clone(), arch.multi_arch.clone());
    }

    let overrides = {
        let guard = state.lock().expect("Found a poisoned mutex.");
        let state = &*guard;
        state.overrides.clone()
    };

    let admins = {
        let guard = state.lock().expect("Found a poisoned mutex.");
        let state = &*guard;
        state.admins.clone()
    };

    let broken = match get_repo_closure(
        &entry.release,
        &arches,
        &multi_arch,
        &entry.repos,
        &entry.check,
        &overrides,
        &admins,
    ) {
        Ok(broken) => broken,
        Err(error) => {
            error!("Failed to generate repoclosure: {}", error);
            return;
        },
    };

    if write_json_to_file(&json_path, &broken).is_err() {
        error!("Failed to write results to disk in JSON format.");
    };

    {
        let mut guard = state.lock().expect("Found a poisoned mutex.");
        let state = &mut *guard;

        state.values.insert(
            format!("{}{}", &entry.release, if entry.with_testing { "-testing" } else { "" }),
            broken,
        );
    }

    info!("Generated data for {}.", &pretty);
}

async fn serve(state: GlobalState) {
    let data = warp::path!("data" / String).map(move |release| {
        let guard = state.lock().expect("Found a poisoned mutex.");
        let state = &*guard;

        match state.values.get(&release) {
            Some(values) => warp::http::Response::builder()
                .header("Content-Type", "application/json")
                .body(serde_json::to_string_pretty(values).expect("Failed to serialize into JSON."))
                .expect("Failed to construct data response."),
            None => warp::http::Response::builder()
                .status(404)
                .body(String::from("This page does not exist."))
                .expect("Failed to construct data 404 response."),
        }
    });

    let error = warp::any().map(|| {
        warp::http::Response::builder()
            .status(404)
            .body(String::from("This page does not exist."))
            .expect("Failed to construct generic 404 response.")
    });

    let server = data.or(error);

    warp::serve(server).run(([127, 0, 0, 1], 3030)).await;
}

#[tokio::main]
async fn main() -> Result<(), String> {
    env_logger::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let config = get_config()?;
    let overrides = get_overrides()?;

    let admins = tokio::spawn(get_admins(15))
        .await
        .map_err(|error| error.to_string())??;

    let state: GlobalState = Arc::new(Mutex::new(State::init(config, overrides, admins)));

    tokio::spawn(serve(state.clone()));

    loop {
        let config = {
            let guard = state.lock().expect("Found a poisoned mutex.");
            guard.config.clone()
        };

        let matrix = matrix_from_config(&config)?;

        // spawn worker threads (.collect() forces the iterator to be evaluated eagerly)
        let handles: Vec<_> = matrix
            .into_iter()
            .map(|entry| tokio::spawn(worker(state.clone(), entry)))
            .collect();

        // wait for threads to finish
        for handle in handles {
            handle.await.map_err(|error| error.to_string())?;
        }

        // maybe make this configurable?
        let wait = 6;

        info!("Finished generating data. Refreshing in {} hours.", wait);

        tokio::spawn(delay_for(Duration::from_secs(60 * 60 * wait)))
            .await
            .map_err(|error| error.to_string())?;

        if tokio::spawn(watcher(state.clone())).await.is_err() {
            error!("Failed to reload configuration from disk.");
        };
    }
}
