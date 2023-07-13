use askama::Template;

#[derive(Template)]
#[template(path = "index.html")]
pub(crate) struct Index {
    releases: Vec<String>,
    stats: Vec<(String, usize)>,
    date_refreshed: String,
}

impl Index {
    pub fn new(releases: Vec<String>, stats: Vec<(String, usize)>, date_refreshed: String) -> Self {
        Index {
            releases,
            stats,
            date_refreshed,
        }
    }
}
