use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, RwLock};

use askama::Template;
use chrono::Utc;
use log::{error, info};
use serde::Serialize;

use axum::extract::Path;
use axum::http::header::CONTENT_TYPE;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::get;
use axum::{Router, Server};

use crate::config::{get_config, Config, MatrixEntry};
use crate::data::BrokenItem;
use crate::overrides::Overrides;
use crate::pagure::{get_admins, get_maintainers};
use crate::repo::get_repo_closure;
use crate::templates::Index;
use crate::utils::{get_json_path, read_json_from_file, write_json_to_file};

pub(crate) struct State {
    pub(crate) config: Config,
    pub(crate) overrides: Arc<RwLock<Overrides>>,
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
            overrides: Arc::new(RwLock::new(overrides)),
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

    match Overrides::load_from_disk() {
        Ok(overrides) => {
            let mut guard = state.write().expect("Found a poisoned lock.");
            let mut state = &mut *guard;
            state.overrides = Arc::new(RwLock::new(overrides));
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
        overrides,
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
    let router = Router::new();

    let index_state = state.clone();
    let router = router.route(
        "/",
        get(move || async move {
            let (mut releases, mut stats): (Vec<String>, Vec<(String, usize)>) = {
                let guard = index_state.read().expect("Found a poisoned lock.");
                let state = &*guard;

                let releases = state.values.keys().cloned().collect();
                let stats = state
                    .values
                    .iter()
                    .map(|(release, broken_items)| (release.to_owned(), broken_items.len()))
                    .collect();
                (releases, stats)
            };

            releases.sort();
            releases.reverse();

            stats.sort();
            stats.reverse();

            let index = Index::new(releases, stats);
            match index.render() {
                Ok(body) => {
                    let mut headers = HeaderMap::new();
                    headers.insert(
                        CONTENT_TYPE,
                        "text/html".parse().expect("Failed to parse hardcoded header value."),
                    );
                    (StatusCode::OK, headers, body)
                },
                Err(error) => (StatusCode::INTERNAL_SERVER_ERROR, HeaderMap::new(), error.to_string()),
            }
        }),
    );

    let release_state = state.clone();
    let router = router.route(
        "/data/:release",
        get(move |release: Path<String>| async move {
            let values = {
                let guard = release_state.read().expect("Found a poisoned lock.");
                let state = &*guard;
                state.values.get(&release.0).cloned()
            };

            match values {
                Some(values) => {
                    let mut headers = HeaderMap::new();
                    headers.insert(
                        CONTENT_TYPE,
                        "application/json"
                            .parse()
                            .expect("Failed to parse hardcoded header value."),
                    );
                    let body = serde_json::to_string_pretty(&*values).expect("Failed to serialize into JSON.");
                    (StatusCode::OK, headers, body)
                },
                None => {
                    let body = String::from("This release does not exist.");
                    (StatusCode::NOT_FOUND, HeaderMap::new(), body)
                },
            }
        }),
    );

    let config_state = state.clone();
    let router = router.route(
        "/config",
        get(move || async move {
            let body = {
                let state = config_state.read().expect("Found a poisoned lock.");
                basic_toml::to_string(&state.config).expect("Failed to serialize into TOML.")
            };

            let mut headers = HeaderMap::new();
            headers.insert(
                CONTENT_TYPE,
                "text/plain".parse().expect("Failed to parse hardcoded header value."),
            );
            (StatusCode::OK, headers, body)
        }),
    );

    let overrides_state = state.clone();
    let router = router.route(
        "/overrides",
        get(move || async move {
            let body = {
                let state = overrides_state.read().expect("Found a poisoned lock.");
                let overrides = state.overrides.read().expect("Found a poisoned lock.");
                serde_json::to_string_pretty(&overrides.data).expect("Failed to serialize into JSON.")
            };

            let mut headers = HeaderMap::new();
            headers.insert(
                CONTENT_TYPE,
                "application/json"
                    .parse()
                    .expect("Failed to parse hardcoded header value."),
            );

            (StatusCode::OK, headers, body)
        }),
    );

    let stats_state = state.clone();
    let router = router.route(
        "/stats",
        get(move || async move {
            let values = {
                let state = stats_state.read().expect("Found a poisoned lock.");
                state.overrides.clone()
            };

            let body = {
                let overrides = values.read().expect("Found a poisoned lock.");
                let stats = &overrides.stats;

                #[derive(Serialize)]
                struct StatsEntry<'a> {
                    path: &'a str,
                    count: u32,
                }

                let mut output: Vec<StatsEntry> = stats
                    .iter()
                    .map(|(path, count)| StatsEntry { path, count: *count })
                    .collect();

                output.sort_by_key(|b| b.count);
                output.reverse();

                serde_json::to_string_pretty(&output).expect("Failed to serialize into JSON.")
            };

            let mut headers = HeaderMap::new();
            headers.insert(
                CONTENT_TYPE,
                "application/json"
                    .parse()
                    .expect("Failed to parse hardcoded header value."),
            );

            (StatusCode::OK, headers, body)
        }),
    );

    // add custom 404 handler
    let router = router.fallback(get(move || async move {
        (
            StatusCode::NOT_FOUND,
            HeaderMap::new(),
            String::from("This page does not exist."),
        )
    }));

    let address: SocketAddr = "127.0.0.1:3030".parse().expect("Failed to parse server address.");
    info!("Listening on http://{} ...", &address);

    Server::bind(&address)
        .serve(router.into_make_service())
        .await
        .expect("Server failure.");
}
