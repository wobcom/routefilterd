use crate::api;
use log::LevelFilter;
use log::{info, warn};
use reqwest::Client;
use reqwest::Url;
use routefilterd::*;
use std::fs::File;
use std::io::Write;
use std::sync::Arc;
use tokio::task;

#[tokio::main(worker_threads = 12)]
async fn main() {
    info!("Starting routefilterd");
    info!("Preparing data..");

    let config = parse_config(String::from("config.toml"));

    let _ = log::set_logger(&SimpleLogger).map(|()| {
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
        let cache_dir = config.cache_dir.clone();
        task::spawn(async move {
            // TODO: Move all of this out of main.rs
            let _ = store_cloned.new_data_source(name.clone(), options.serial, options.priority);
            for file in options.import_sources {
                store_importer::import_source(&store_cloned, &name, file, cache_dir.clone()).await;
            }
        });
    }

    info!("Ready to serve your requests!");

    let _ = task::spawn(api::listen(config.api, store.clone())).await;
}
