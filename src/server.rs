use std::collections::HashMap;
use std::sync::{Arc, RwLock};

use askama::Template;
use chrono::Utc;
use log::{error, info};
use warp::Filter;

use crate::config::{get_config, Config, MatrixEntry};
use crate::data::BrokenItem;
use crate::overrides::{get_overrides, Overrides};
use crate::pagure::{get_admins, get_maintainers};
use crate::repo::get_repo_closure;
use crate::templates::Index;
use crate::utils::{get_json_path, read_json_from_file, write_json_to_file};

pub(crate) struct State {
    pub(crate) config: Config,
    pub(crate) overrides: Overrides,
    pub(crate) admins: HashMap<String, String>,
    pub(crate) maintainers: HashMap<String, Vec<String>>,
    pub(crate) values: HashMap<String, Arc<Vec<BrokenItem>>>,
}

impl State {
    pub(crate) fn init(
        config: Config,
        overrides: Overrides,
        admins: HashMap<String, String>,
        maintainers: HashMap<String, Vec<String>>,
    ) -> State {
        State {
            config,
            overrides,
            admins,
            maintainers,
            values: HashMap::new(),
        }
    }
}

pub(crate) type GlobalState = Arc<RwLock<State>>;

pub(crate) async fn watcher(state: GlobalState) {
    match get_config() {
        Ok(config) => {
            let mut guard = state.write().expect("Found a poisoned lock.");
            let mut state = &mut *guard;
            state.config = config;
        },
        Err(error) => error!("Failed to read updated configuration: {}", error),
    };

    match get_overrides() {
        Ok(overrides) => {
            let mut guard = state.write().expect("Found a poisoned lock.");
            let mut state = &mut *guard;
            state.overrides = overrides;
        },
        Err(error) => error!("Failed to read updated overrides: {}", error),
    };

    match get_admins(15).await {
        Ok(admins) => {
            let mut guard = state.write().expect("Found a poisoned lock.");
            let mut state = &mut *guard;
            state.admins = admins;
        },
        Err(error) => error!("Failed to read updated package maintainers: {}", error),
    }

    match get_maintainers(15).await {
        Ok(maintainers) => {
            let mut guard = state.write().expect("Found a poisoned lock.");
            let mut state = &mut *guard;
            state.maintainers = maintainers;
        },
        Err(error) => error!("Failed to read updated package maintainers: {}", error),
    };
}

pub(crate) async fn worker(state: GlobalState, entry: MatrixEntry) {
    let suffix = if !entry.with_testing { "" } else { "-testing" };
    let pretty = format!("{}{}", &entry.release, suffix);

    let json_path = get_json_path(&entry.release, entry.with_testing);

    let previous = {
        let guard = state.read().expect("Found a poisoned lock.");
        let state = &*guard;

        state.values.contains_key(&pretty)
    };

    if !previous {
        // populate data with cached values from file, if available
        let cached = read_json_from_file(&json_path);
        if let Ok(values) = cached {
            if !entry.archived {
                info!("Reusing cached data for {} until fresh data is available.", &pretty);
            } else {
                info!("Reusing archival data for {}.", &pretty);
            }

            let mut guard = state.write().expect("Found a poisoned lock.");
            let state = &mut *guard;

            state.values.insert(pretty.clone(), Arc::new(values));

            if entry.archived {
                return;
            }
        };
    }

    info!("Generating data for {}", &pretty);

    let mut arches: Vec<String> = Vec::new();
    let mut multi_arch: HashMap<String, Vec<String>> = HashMap::new();

    for arch in &entry.arches {
        arches.push(arch.name.clone());
        multi_arch.insert(arch.name.clone(), arch.multi_arch.clone());
    }

    let overrides = {
        let guard = state.read().expect("Found a poisoned lock.");
        let state = &*guard;
        state.overrides.clone()
    };

    let admins = {
        let guard = state.read().expect("Found a poisoned lock.");
        let state = &*guard;

        state.admins.clone()
    };

    let maintainers = {
        let guard = state.read().expect("Found a poisoned lock.");
        let state = &*guard;
        state.maintainers.clone()
    };

    let broken = match get_repo_closure(
        &entry.release,
        &arches,
        &multi_arch,
        &entry.repos,
        &entry.check,
        &overrides,
        &admins,
        &maintainers,
    )
    .await
    {
        Ok(broken) => broken,
        Err(error) => {
            error!("Failed to generate repoclosure: {}", error);
            return;
        },
    };

    {
        let mut guard = state.write().expect("Found a poisoned lock.");
        let state = &mut *guard;

        let old_broken = state.values.remove(&pretty);
        let mut new_broken = broken;

        // check if packages were already broken and set "since" datetime accordingly
        if let Some(old_broken) = old_broken {
            fn matches(old: &BrokenItem, new: &BrokenItem) -> bool {
                old.package == new.package && old.repo == new.repo && old.repo_arch == new.repo_arch
            }

            for new in new_broken.iter_mut() {
                for old in old_broken.iter() {
                    if matches(old, new) {
                        // use old "since" time in case of a match
                        new.since = old.since;
                        // there can only be one match per package+repo+repo_arch combination
                        break;
                    }
                }

                // if no old "since" time was found or the entry is new, set "since" to "now"
                if new.since.is_none() {
                    new.since = Some(Utc::now());
                }
            }
        }

        if write_json_to_file(&json_path, &new_broken).is_err() {
            error!("Failed to write results to disk in JSON format.");
        };

        state.values.insert(pretty.clone(), Arc::new(new_broken));
    }

    info!("Generated data for {}.", &pretty);
}

pub(crate) async fn server(state: GlobalState) {
    let index_state = state.clone();

    let index = warp::path::end().and(warp::get()).map(move || {
        let (mut releases, mut stats): (Vec<String>, Vec<(String, usize)>) = {
            let guard = index_state.read().expect("Found a poisoned lock.");
            let state = &*guard;

            (
                state.values.keys().cloned().collect(),
                state
                    .values
                    .iter()
                    .map(|(release, broken_items)| (release.to_owned(), broken_items.len()))
                    .collect(),
            )
        };

        releases.sort();
        releases.reverse();

        stats.sort();
        stats.reverse();

        let index = Index::new(releases, stats);

        match index.render() {
            Ok(body) => warp::http::Response::builder()
                .header("Content-Type", "text/html")
                .body(body)
                .expect("Failed to construct response for index page."),
            Err(error) => warp::http::Response::builder()
                .status(500)
                .body(format!("Internal template rendering error: {}", error))
                .expect("Failed to construct error 500 response."),
        }
    });

    let release_state = state.clone();

    let data = warp::path!("data" / String).map(move |release| {
        let values = {
            let guard = release_state.read().expect("Found a poisoned lock.");
            let state = &*guard;
            state.values.get(&release).cloned()
        };

        match values {
            Some(values) => warp::http::Response::builder()
                .header("Content-Type", "application/json")
                .body(serde_json::to_string_pretty(&*values).expect("Failed to serialize into JSON."))
                .expect("Failed to construct data response."),
            None => warp::http::Response::builder()
                .status(404)
                .body(String::from("This release does not exist."))
                .expect("Failed to construct data 404 response."),
        }
    });

    let error = warp::any().map(|| {
        warp::http::Response::builder()
            .status(404)
            .body(String::from(
                "This page does not exist. Navigate to /data/{release} instead.",
            ))
            .expect("Failed to construct generic 404 response.")
    });

    let server = index.or(data).or(error);

    warp::serve(server).run(([127, 0, 0, 1], 3030)).await;
}
