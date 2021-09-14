use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Config {
    pub client_id: String,
    pub secret: String,
    pub state: String,
}

impl Config {
    pub fn from_file(path: &str) -> Self {
        let config_data = std::fs::read(path).unwrap();
        serde_json::from_slice(config_data.as_slice()).unwrap()
    }
}
