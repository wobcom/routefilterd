use crate::store::DataStore;
use libunftp::options::{ActivePassiveMode, Shutdown};
use libunftp::{Server, ServerBuilder, options};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::Receiver;
use unftp_core::auth::UserDetail;
use unftp_sbe_fs::Filesystem;
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

pub fn get_new_store() -> Arc<DataStore> {
    Arc::new(DataStore::new())
}

pub fn get_new_stoppable_ftp_server_with_fs_path(
    path: PathBuf,
    mut channel_recv: Receiver<()>,
) -> Server<Filesystem, impl UserDetail> {
    ServerBuilder::new(Box::new(move || Filesystem::new(path.clone()).unwrap()))
        .greeting("Test FTP Server")
        .passive_ports(50000..=65535)
        .passive_host(options::PassiveHost::FromConnection)
        .active_passive_mode(ActivePassiveMode::ActiveAndPassive)
        .shutdown_indicator(async move {
            channel_recv.recv().await.unwrap(); // block on shutdown message recv
            Shutdown::new().grace_period(Duration::from_millis(10))
        })
        .build()
        .unwrap()
}

pub async fn get_http_server_with(response_template: ResponseTemplate) -> MockServer {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .respond_with(response_template)
        .mount(&mock_server)
        .await;

    mock_server
}
