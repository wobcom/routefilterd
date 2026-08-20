use crate::common_loader::{CommonLoader, LoadFromFile};
use crate::store::DataStore;
use crate::store_importer::parse_rpsl;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;
use tokio::io::AsyncBufRead;

async fn get_async_buf_read_for_file(path: &Path) -> Pin<Box<dyn AsyncBufRead + Send>> {
    CommonLoader::load_from_file(path).await.unwrap()
}

async fn import_buf_in_store(
    source_name: &str,
    store: Arc<DataStore>,
    buf: Pin<Box<dyn AsyncBufRead + Send>>,
) {
    store
        .import_objects(source_name, Box::pin(parse_rpsl(buf)))
        .await
        .unwrap();
}

pub async fn import_file_in_store(
    store: &Arc<DataStore>,
    relpath: &str,
    source_name: &str,
    ds_priority: i64,
) {
    let wd = Path::new(file!()).parent().unwrap();
    let path = wd.join(relpath);
    let buf = get_async_buf_read_for_file(&path).await;

    store.new_data_source(String::from(source_name), 0, ds_priority); // TODO: should be refactored?
    import_buf_in_store(source_name, store.clone(), buf).await;
}

pub fn sorted_vec<T: Ord>(mut vec: Vec<T>) -> Vec<T> {
    vec.sort();
    vec
}
