use crate::store::DataStore;
use std::sync::Arc;

pub fn get_new_store() -> Arc<DataStore> {
    Arc::new(DataStore::new())
}
