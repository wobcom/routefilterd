use crate::api;
use log::LevelFilter;
use log::{info, warn};
use log::{Level, Metadata, Record};
use routefilterd::*;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs::read_to_string;
use std::fs::File;
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
    #[serde(default)]
    default_recursion_depth: u32,
    // TODO: Implement default recursion depth
}

#[derive(Deserialize)]
struct ConfigDataSources {
    import_sources: Vec<String>,
    // TODO: Implement downloads
    import_serial: Option<String>,
    nrtm_host: Option<String>,
    // TODO: Implement NRTM
    #[serde(default)]
    serial: u64,
    // TODO: Implement Serial
    #[serde(default)]
    priority: i64,
    // TODO: Finish implementing priorities
}

fn parse_config(filename: String) -> Config {
    let contents = read_to_string(filename).expect("Could not open config file");
    toml::from_str(&contents).expect("Could not parse config file")
}

#[tokio::main(worker_threads = 8)]
async fn main() {
    info!("Starting routefilterd");
    info!("Preparing data..");

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

    let store = Arc::new(store::DataStore::new());

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
