use crate::api;
use log::LevelFilter;
use log::{info, warn};
use routefilterd::*;
use std::fs::File;
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

    let _ = task::spawn(api::listen(config.api, store.clone())).await;
}
