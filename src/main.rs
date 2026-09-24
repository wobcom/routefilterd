use crate::api;
use log::LevelFilter;
use log::info;
use routefilterd::config::parse_config;
use routefilterd::nrtm_importer::{NRTMImporter, NRTMRefreshMode};
use routefilterd::*;
use std::sync::Arc;
use std::time::Duration;
use tokio::task;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

const NRTM_IMPORT_INTERVAL_MIN: u64 = 10;

#[tokio::main(worker_threads = 12)]
async fn main() {
    let cancel_token = CancellationToken::new();

    info!("Starting routefilterd");
    info!("Preparing data..");

    let config = parse_config(String::from("config.toml"));

    let _ = log::set_logger(&SimpleLogger).map(|()| {
        log::set_max_level(match config.log_level.as_str().trim() {
            "trace" => LevelFilter::Trace,
            "debug" => LevelFilter::Debug,
            "error" => LevelFilter::Error,
            "warn" => LevelFilter::Warn,
            "info" => LevelFilter::Info,
            _ => LevelFilter::Debug,
        })
    });

    let store = Arc::new(store::DataStore::new());

    // do the initial import for all sources
    let mut initial_imports = JoinSet::new();
    for (name, options) in config.data_sources.clone() {
        let store_cloned = store.clone();
        let cache_dir = config.cache_dir.clone();

        store_importer::new_datasource(
            &store_cloned,
            name.clone(),
            options.import_serial,
            options.priority,
        )
        .await;

        initial_imports.spawn(async move {
            // TODO: Move all of this out of main.rs

            for file in options.import_sources {
                store_importer::import_source(&store_cloned, &name, file, cache_dir.clone()).await;
            }
        });
    }
    // await all initial imports before proceeding
    let _ = initial_imports.join_all().await;

    // start NRTM imports
    let mut nrtm_imports = JoinSet::new();
    for (name, options) in config.data_sources.clone() {
        let store_cloned = store.clone();
        let inner_cancel_token = cancel_token.clone();

        if let (Some(nrtm_host), Some(nrtm_port)) = (options.nrtm_host, options.nrtm_port) {
            nrtm_imports.spawn(async move {
                info!("starting NRTM importer for source {}", name);

                let importer = NRTMImporter::new(
                    (nrtm_host, nrtm_port),
                    name.clone(),
                    store_cloned,
                    match options.nrtm_streaming_supported {
                        true => NRTMRefreshMode::SingleLongLastingConnection,
                        false => NRTMRefreshMode::ScheduledMultipleConnection,
                    },
                    Duration::from_mins(NRTM_IMPORT_INTERVAL_MIN),
                    inner_cancel_token,
                );

                importer.start().await.unwrap();
            });
        }
    }

    info!("Ready to serve your requests!");

    let _ = task::spawn(api::listen(config.api, store.clone())).await;
}
