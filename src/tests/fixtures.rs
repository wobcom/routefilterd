use crate::store::DataStore;
use reqwest::Client;
use reqwest_middleware::{ClientBuilder, ClientWithMiddleware};
use std::sync::Arc;

pub fn get_new_store() -> Arc<DataStore> {
    Arc::new(DataStore::new())
}

pub fn get_new_client_with_middleware() -> ClientWithMiddleware {
    ClientBuilder::new(Client::new()).build()
}
