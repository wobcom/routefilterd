use crate::api;
use log::LevelFilter;
use log::{Level, Metadata, Record};
use log::{info, warn};
use poof::*;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs::File;
use std::fs::read_to_string;
use std::sync::Arc;
use tokio::task;

struct SimpleLogger;

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

static LOGGER: SimpleLogger = SimpleLogger;

#[derive(Deserialize)]
struct Config {
    api: ConfigAPI,
    data_sources: HashMap<String, ConfigDataSources>,
    log_level: String,
}

#[derive(Deserialize)]
struct ConfigAPI {
    listen_address: String,
}

#[derive(Deserialize)]
struct ConfigDataSources {
    import_sources: Vec<String>,
    import_serial: Option<String>,
    nrtm_host: Option<String>,
    #[serde(default)]
    serial: u64,
    #[serde(default)]
    priority: i64,
}

fn parse_config(filename: String) -> Config {
    let contents = read_to_string(filename).expect("Could not open config file");
    toml::from_str(&contents).expect("Could not parse config file")
}

#[tokio::main(worker_threads = 4)]
async fn main() {
    info!("Hello, poof!");
    info!("initializing data..");

    let config = parse_config(String::from("config.toml"));

    let _ = log::set_logger(&LOGGER).map(|()| {
        log::set_max_level(match config.log_level.as_str() {
            "trace" => LevelFilter::Trace,
            "debug" => LevelFilter::Debug,
            "error" => LevelFilter::Error,
            "warn" => LevelFilter::Warn,
            _ => LevelFilter::Info,
        })
    });

    let store = Arc::new(store::PoofStore::new());

    for (name, options) in config.data_sources {
        let store_cloned = store.clone();
        task::spawn(async move {
            let _ = store_cloned.new_data_source(name.clone(), options.serial, options.priority);
            for file in options.import_sources {
                if file.clone().starts_with("ftp://") {
                    warn!("FTP data source not implemented yet. Skipping")
                }
                if file.clone().starts_with("http://") || file.clone().starts_with("https://") {
                    warn!("http(s) data source not implemented yet. Skipping")
                }
                info!("Importing {}", file.clone());
                let socket = File::open(file.clone()).expect("Couldn't open file");
                let _ = store_cloned.import_objects(&name, store_importer::RpslParser::new(socket));
                info!("Done importing {}", file.clone());
            }
        });
    }

    info!("Ready to serve your requests!");

    let _ = task::spawn(api::listen(config.api.listen_address, store.clone())).await;
}
