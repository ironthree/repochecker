use std::time::Duration;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct Compose {
    new_compose: String,
    old_compose: String,
    added_packages: Vec<AddedPackage>,
    upgraded_packages: Vec<UpgradedPackage>,
    summary: ComposeSummary,
    // incomplete
}

#[derive(Debug, Deserialize)]
struct AddedPackage {
    name: String,
    nvr: String,
    rpms: Vec<String>,
    size: u64,
    summary: String,
}

#[derive(Debug, Deserialize)]
struct UpgradedPackage {
    name: String,
    nvr: String,
    old_nvr: String,
    added_rpms: Vec<String>,
    changelog: Vec<String>,
    common_rpms: Vec<String>,
    dropped_rpms: Vec<String>,
    old_rpms: Vec<String>,
    rpms: Vec<String>,
    size: u64,
    size_change: i64,
    summary: String,
}

#[derive(Debug, Deserialize)]
struct ComposeSummary {
    added_images: u64,
    added_packages: u64,
    added_packages_size: u64,
    downgraded_packages: u64,
    downgraded_packages_size: u64,
    downgraded_packages_size_change: i64,
    dropped_images: u64,
    dropped_packages: u64,
    dropped_packages_size: u64,
    upgraded_packages: u64,
    upgraded_packages_size: u64,
    upgraded_packages_size_change: i64,
}

fn get_rawhide_compose() -> Result<Compose, String> {
    let id_url =
        "https://kojipkgs.fedoraproject.org/compose/rawhide/latest-Fedora-Rawhide/COMPOSE_ID";

    let client: reqwest::blocking::Client = match reqwest::blocking::ClientBuilder::new()
        .timeout(Duration::from_secs(10))
        .build()
    {
        Ok(client) => client,
        Err(error) => return Err(error.to_string()),
    };

    let response: reqwest::blocking::Response = match client.get(id_url).send() {
        Ok(response) => response,
        Err(error) => return Err(error.to_string()),
    };

    let id = match response.text() {
        Ok(id) => id,
        Err(error) => return Err(error.to_string()),
    };

    let compose_url = format!(
        "https://kojipkgs.fedoraproject.org/compose/rawhide/{}/logs/changelog-{}.json",
        &id, &id,
    );

    let response = match client.get(&compose_url).send() {
        Ok(response) => response,
        Err(error) => return Err(error.to_string()),
    };

    let compose: Compose = match response.json() {
        Ok(compose) => compose,
        Err(error) => return Err(error.to_string()),
    };

    Ok(compose)
}
