use askama::Template;

#[derive(Template)]
#[template(path = "index.html")]
pub(crate) struct Index {
    releases: Vec<String>,
    stats: Vec<(String, usize)>,
}

impl Index {
    pub fn new(releases: Vec<String>, stats: Vec<(String, usize)>) -> Self {
        Index { releases, stats }
    }
}
