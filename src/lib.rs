pub mod api;
pub mod store;
pub mod store_importer;
use log::{Level, Metadata, Record};
use serde::Deserialize;
use std::collections::HashMap;
use std::fs::read_to_string;

pub struct SimpleLogger;

impl log::Log for SimpleLogger {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= Level::Info
    }

    fn log(&self, record: &Record) {
        if self.enabled(record.metadata()) {
            println!("{} - {}", record.level(), record.args());
        }
    }

    fn flush(&self) {}
}

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
    // TODO: Implement downloads
    import_serial: Option<String>,
    nrtm_host: Option<String>,
    // TODO: Implement NRTM
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
