use askama::Template;

#[derive(Template)]
#[template(path = "index.html")]
pub(crate) struct Index {
    releases: Vec<String>,
}

impl Index {
    pub fn new(releases: Vec<String>) -> Self {
        Index { releases }
    }
}
