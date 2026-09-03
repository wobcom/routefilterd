use serde::Deserialize;
use std::collections::HashMap;
use std::fs::read_to_string;

#[derive(Deserialize, Clone)]
pub struct Config {
    pub api: ConfigAPI,
    pub data_sources: HashMap<String, ConfigDataSources>,
    pub log_level: String,
    pub cache_dir: String,
}

#[derive(Deserialize, Clone)]
pub struct ConfigAPI {
    pub listen_address: String,
    #[serde(default = "default_recursion_depth")]
    pub default_recursion_depth: u32,
}

fn default_recursion_depth() -> u32 {
    64
}

#[derive(Deserialize, Clone)]
pub struct ConfigDataSources {
    pub import_sources: Vec<String>,
    pub import_serial: Option<String>,
    pub nrtm_host: Option<String>,
    #[serde(default)]
    pub nrtm_streaming_supported: bool,
    #[serde(default)]
    pub serial: u64,
    // TODO: Implement Serial
    #[serde(default)]
    pub priority: i64,
}

pub fn parse_config(filename: String) -> Config {
    let contents = read_to_string(filename).expect("Could not open config file");
    toml::from_str(&contents).expect("Could not parse config file")
}
