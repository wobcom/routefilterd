use log::LevelFilter;
use log::{Level, Metadata, Record};
use log::{info, warn};
use poof::*;
use serde::Deserialize;
use std::collections::HashMap;
use std::fs::read_to_string;
use std::net::SocketAddr;
use tokio::task;
use warp::Filter;

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

#[tokio::main]
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

    let data = task::spawn_blocking(move || {
        let mut poof = store::PoofData::new();
        for (name, options) in config.data_sources {
            poof.new_data_source(name.clone(), options.serial, options.priority);
            for file in options.import_sources {
                if file.clone().starts_with("ftp://") {
                    warn!("FTP data source not implemented yet. Skipping")
                }
                if file.clone().starts_with("http://") || file.clone().starts_with("https://") {
                    warn!("http(s) data source not implemented yet. Skipping")
                }
                let _ = poof.import_from_file(name.clone(), file.clone());
            }
        }

        info!("Ready to serve your requests!");
        poof
    })
    .await
    .unwrap();

    let data_moved = warp::any().map(move || data.clone());

    let routes = warp::get()
        .and(warp::path!("api" / "v1" / "asnsFromAsSet" / String))
        .and(data_moved.clone())
        .and_then(api::get_asn_from_as_set);
    let routes2 = warp::get()
        .and(warp::path!("api" / "v1" / "routesFromAsSet" / String))
        .and(data_moved.clone())
        .and_then(api::get_route_from_as_set);

    let listen_address: SocketAddr = config
        .api
        .listen_address
        .parse()
        .expect("Unable to parse socket address");

    warp::serve(routes.or(routes2)).run(listen_address).await;
}
