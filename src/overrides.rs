use std::collections::HashMap;
use std::fs::read_to_string;

use serde::Deserialize;

type Overrides = HashMap<String, ReleaseOverrides>;
type ReleaseOverrides = HashMap<String, PackageOverrides>;
type PackageOverrides = HashMap<String, OverrideEntry>;

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum OverrideEntry {
    All(String),
    Packages(Vec<String>),
}

fn get_overrides() -> Result<Overrides, String> {
    let path = "overrides.json";
    let contents = match read_to_string(path) {
        Ok(string) => string,
        Err(error) => return Err(error.to_string()),
    };

    let overrides: Overrides = match serde_json::from_str(&contents) {
        Ok(overrides) => overrides,
        Err(error) => return Err(error.to_string()),
    };

    Ok(overrides)
}
