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

use chrono::Utc;
use log::{error, info};

use config::get_config;
use overrides::Overrides;
use pagure::{get_admins, get_maintainers};
use server::{GlobalState, State};

#[tokio::main(worker_threads = 16)]
async fn main() -> Result<(), String> {
    env_logger::builder()
        .filter_level(log::LevelFilter::Info)
        .parse_env("REPOCHECKER_LOG")
        .init();

    let config = get_config()?;
    let overrides = Overrides::load_from_disk()?;

    // fetch main admins and lists of maintainers concurrently
    let (admins, maintainers) = tokio::join!(tokio::spawn(get_admins(15)), tokio::spawn(get_maintainers(15)),);
    let admins = admins.map_err(|error| error.to_string())??;
    let maintainers = maintainers.map_err(|error| error.to_string())??;

    // initialize global state
    let state: GlobalState = Arc::new(RwLock::new(State::init(config, overrides, admins, maintainers)));

    // spawn server thread
    tokio::spawn(server::server(state.clone()));

    loop {
        let start = Instant::now();

        let config = {
            let guard = state.read().expect("Found a poisoned lock.");
            guard.config.clone()
        };

        let matrix = config.to_matrix()?;

        // spawn worker threads
        let handles: Vec<_> = matrix
            .into_iter()
            .map(|entry| tokio::spawn(server::worker(state.clone(), entry)))
            .collect();

        // wait for worker threads
        for handle in handles {
            handle.await.map_err(|error| error.to_string())?;
        }

        let interval = config.repochecker.interval;

        let stop = Instant::now();
        let busy = stop - start;

        let wait = Duration::from_secs_f64(interval * 60.0 * 60.0).saturating_sub(busy);

        if !wait.is_zero() {
            info!(
                "Finished generating data. Refreshing in {:.1} hours.",
                wait.as_secs_f64() / 3600.0
            );
            state.write().expect("Found a poisoned lock.").date_refreshed = Some(Utc::now());
            tokio::time::sleep(wait).await;
        }

        if tokio::spawn(server::watcher(state.clone())).await.is_err() {
            error!("Failed to reload configuration from disk.");
        };
    }
}
