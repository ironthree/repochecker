use std::collections::HashMap;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct PocPage {
    rpms: HashMap<String, Users>,
    // incomplete
}

#[derive(Debug, Deserialize)]
struct Users {
    admin: String,
    fedora: String,
    epel: String,
}

pub async fn get_admins(timeout: u64) -> Result<HashMap<String, String>, String> {
    let url = "https://src.fedoraproject.org/extras/pagure_poc.json";

    let client: reqwest::Client = match reqwest::ClientBuilder::new()
        .timeout(std::time::Duration::from_secs(timeout))
        .build()
    {
        Ok(client) => client,
        Err(error) => return Err(error.to_string()),
    };

    let response = match client.get(url).send().await {
        Ok(response) => response,
        Err(error) => return Err(error.to_string()),
    };

    let pocs: PocPage =
        match serde_json::from_str(&response.text().await.map_err(|error| error.to_string())?) {
            Ok(pocs) => pocs,
            Err(error) => return Err(error.to_string()),
        };

    Ok(pocs
        .rpms
        .into_iter()
        .map(|(source, users)| (source, users.admin))
        .collect())
}
