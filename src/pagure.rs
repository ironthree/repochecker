use std::collections::HashMap;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
struct ProjectPage {
    projects: Vec<PagureProject>,
    pagination: PagurePagination,
}

#[derive(Debug, Deserialize)]
struct PagurePagination {
    next: Option<String>,
    // incomplete
}

#[derive(Debug, Deserialize)]
struct PagureProject {
    name: String,
    access_users: PagureUsers,
    // incomplete
}

#[derive(Debug, Deserialize)]
struct PagureUsers {
    owner: Vec<String>,
    // incomplete
}

fn get_admins(api_url: &str, timeout: u64) -> Result<HashMap<String, String>, String> {
    let mut url = format!(
        "{}/projects?fork=false&per_page=100&namespace=rpms&page=1",
        api_url
    );

    let client: reqwest::blocking::Client = match reqwest::blocking::ClientBuilder::new()
        .timeout(std::time::Duration::from_secs(timeout))
        .build()
    {
        Ok(client) => client,
        Err(error) => return Err(error.to_string()),
    };

    let mut projects: Vec<PagureProject> = Vec::new();

    loop {
        let query = || -> Result<ProjectPage, String> {
            let response = match client.get(&url).send() {
                Ok(response) => response,
                Err(error) => return Err(error.to_string()),
            };

            let page: ProjectPage = match response.json() {
                Ok(result) => result,
                Err(error) => return Err(error.to_string()),
            };

            Ok(page)
        };

        let page = match retry::retry(retry::delay::Fibonacci::from_millis(1000), query) {
            Ok(result) => result,
            Err(error) => {
                return Err(match error {
                    retry::Error::Operation { error, .. } => error,
                    retry::Error::Internal(x) => x,
                })
            }
        };

        projects.extend(page.projects);

        match &page.pagination.next {
            Some(string) => {
                url = string.to_owned();
            }
            None => break,
        }
    }

    let admins: HashMap<String, String> = projects
        .into_iter()
        .map(|p| (p.name, p.access_users.owner[0].to_owned()))
        .collect();

    Ok(admins)
}
