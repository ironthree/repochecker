use std::path::PathBuf;

use crate::data::BrokenItem;

fn get_data_path() -> PathBuf {
    let mut path = PathBuf::new();
    path.push(std::env::current_dir().expect("Unable to determine current directory."));
    path.push("data/");
    path
}

pub fn get_json_path(release: &str, testing: bool) -> PathBuf {
    let mut path = get_data_path();
    if !testing {
        path.push(format!("{}.json", release));
    } else {
        path.push(format!("{}-testing.json", release));
    }

    path
}

pub fn write_json_to_file(path: &PathBuf, broken: &[BrokenItem]) -> Result<(), String> {
    let json = match serde_json::to_string_pretty(&broken) {
        Ok(json) => json,
        Err(_) => return Err(String::from("Failed to serialize broken dependencies into JSON.")),
    };

    let data_path = get_data_path();

    if !data_path.exists() {
        std::fs::create_dir_all(data_path).expect("Failed to create data directory.");
    }

    if std::fs::write(&path, json).is_err() {
        return Err(format!("Failed to write data to disk: {}", &path.to_string_lossy()));
    }

    Ok(())
}

pub fn read_json_from_file(path: &PathBuf) -> Result<Vec<BrokenItem>, String> {
    if !path.exists() {
        return Err(String::from("Data has not been generated yet."));
    }

    let string = match std::fs::read_to_string(&path) {
        Ok(string) => string,
        Err(_) => return Err(String::from("Failed to read cached JSON data.")),
    };

    let values: Vec<BrokenItem> = match serde_json::from_str(&string) {
        Ok(values) => values,
        Err(_) => return Err(String::from("Failed to deserialize cached JSON data.")),
    };

    Ok(values)
}
