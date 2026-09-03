pub mod api;
mod common_loader;
pub mod config;
mod nrtm_importer;
mod serial_loader;
pub mod store;
pub mod store_importer;
#[cfg(test)]
mod tests;

use log::{Level, Metadata, Record};
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
