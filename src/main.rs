#![warn(clippy::unwrap_used)]

mod config;
mod data;
mod overrides;
mod pagure;
mod parse;
mod repo;
mod server;
mod templates;
mod utils;

use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use log::{error, info};

use config::get_config;
use overrides::get_overrides;
use pagure::{get_admins, get_maintainers};
use server::{GlobalState, State};

#[tokio::main(worker_threads = 16)]
async fn main() -> Result<(), String> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    let config = get_config()?;
    let overrides = get_overrides()?;

    let admins = tokio::spawn(get_admins(15))
        .await
        .map_err(|error| error.to_string())??;

    let maintainers = tokio::spawn(get_maintainers(15))
        .await
        .map_err(|error| error.to_string())??;

    let state: GlobalState = Arc::new(RwLock::new(State::init(config, overrides, admins, maintainers)));

    tokio::spawn(server::server(state.clone()));

    loop {
        let start = Instant::now();

        let config = {
            let guard = state.read().expect("Found a poisoned lock.");
            guard.config.clone()
        };

        let matrix = config.to_matrix()?;

        // spawn worker threads (.collect() forces the iterator to be evaluated eagerly)
        let handles: Vec<_> = matrix
            .into_iter()
            .map(|entry| tokio::spawn(server::worker(state.clone(), entry)))
            .collect();

        // wait for threads to finish
        for handle in handles {
            handle.await.map_err(|error| error.to_string())?;
        }

        let interval = config.repochecker.interval;
        info!("Finished generating data. Refreshing in {} hours.", interval);

        let stop = Instant::now();
        let busy = stop - start;

        let wait = Duration::from_secs(interval * 60 * 60) - busy;

        tokio::spawn(tokio::time::sleep(wait))
            .await
            .map_err(|error| error.to_string())?;

        if tokio::spawn(server::watcher(state.clone())).await.is_err() {
            error!("Failed to reload configuration from disk.");
        };
    }
}
