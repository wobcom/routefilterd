use super::super::common_loader::{CommonLoader, LoadFromFile, LoadFromFileError, LoadFromURL};
use crate::tests::fixtures::get_new_stoppable_ftp_server_with_fs_path;
use flate2::Compression;
use flate2::bufread::GzEncoder;
use openport::pick_random_unused_port;
use reqwest::Url;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::pin::Pin;
use std::time::Duration;
use tempfile::{Builder, NamedTempFile};
use tokio::io::AsyncBufRead;
use tokio::io::AsyncBufReadExt;
use tokio::sync::mpsc::channel;
use tokio::time::sleep;
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

const TEST_DATA: &str = "TESTDATA\nTESTADA\nTESADA";
const WRONG_TEST_DATA: &str = "TEATATA\nTA\nTDA";
const GZ_TEST_DATA: fn() -> Vec<u8> = || {
    let mut compressed = Vec::new();
    GzEncoder::new(TEST_DATA.as_bytes(), Compression::fast())
        .read_to_end(&mut compressed)
        .unwrap();
    compressed
};
async fn assert_lines_eq(res: Pin<Box<dyn AsyncBufRead + Send>>) {
    let mut split_loader = res.lines();
    let split_data = TEST_DATA.split("\n");

    for line in split_data {
        assert_eq!(split_loader.next_line().await.unwrap().unwrap(), line);
    }
}

#[tokio::test]
async fn test_load_from_raw_file() {
    let mut temp = NamedTempFile::new().unwrap();

    temp.write_all(TEST_DATA.as_bytes()).unwrap();

    let res = CommonLoader::load_from_file(temp.as_ref()).await.unwrap();
    assert_lines_eq(res).await;
}

#[tokio::test]
async fn test_load_from_db_file() {
    let mut temp = Builder::new()
        .suffix(".db")
        .tempfile()
        .expect("cannot create test temporary file");

    temp.write_all(TEST_DATA.as_bytes()).unwrap();

    let res = CommonLoader::load_from_file(temp.as_ref()).await.unwrap();
    assert_lines_eq(res).await;
}

#[tokio::test]
async fn test_load_from_gz_file() {
    let mut temp = Builder::new()
        .suffix(".gz")
        .tempfile()
        .expect("cannot create test temporary file");
    let mut compressed = Vec::new();
    let mut gz_encoder = GzEncoder::new(TEST_DATA.as_bytes(), Compression::fast());

    gz_encoder.read_to_end(&mut compressed).unwrap();
    temp.write_all(&compressed[..]).unwrap();

    let res = CommonLoader::load_from_file(temp.as_ref()).await.unwrap();
    assert_lines_eq(res).await;
}

#[tokio::test]
async fn test_load_from_unknown_suffix() {
    let temp = Builder::new()
        .suffix(".bad")
        .tempfile()
        .expect("cannot create test temporary file");

    let res = CommonLoader::load_from_file(temp.as_ref()).await;

    match res {
        Err(LoadFromFileError::UnsupportedExtension) => (),
        _ => panic!("did not error out on unknown suffix"),
    };
}

#[tokio::test]
#[should_panic]
async fn test_unknown_scheme_fail() {
    let reqwest_client = reqwest::Client::new();
    let mut loader = CommonLoader::new(reqwest_client);
    // per https://www.iana.org/assignments/uri-schemes/uri-schemes.xhtml
    let url = Url::parse("gopher://example.com/trusted.db.gz").unwrap();

    let _ = loader.load_from_url(&url).await.unwrap();
}

#[tokio::test]
async fn test_load_from_file_url() {
    let reqwest_client = reqwest::Client::new();
    let mut loader = CommonLoader::new(reqwest_client);
    let mut temp = Builder::new()
        .suffix(".db")
        .tempfile()
        .expect("cannot create test temporary file");
    let url = Url::from_file_path(&temp).unwrap();

    temp.write_all(TEST_DATA.as_bytes()).unwrap();

    let res = loader.load_from_url(&url).await.unwrap();
    assert_lines_eq(res).await;
}

async fn assert_load_from_http_url_with(response_template: ResponseTemplate, file_path: &str) {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .respond_with(response_template)
        .mount(&mock_server)
        .await;

    let mut url = Url::parse(&mock_server.uri())
        .unwrap_or_else(|_| panic!("failed parsing MockServer uri {}", mock_server.uri()));
    url.set_path(file_path);

    let reqwest_client = reqwest::Client::new();
    let mut loader = CommonLoader::new(reqwest_client);

    let res = loader
        .load_from_url(&url)
        .await
        .expect("failed loading response from MockServer");

    assert_lines_eq(res).await;
}

#[tokio::test]
async fn test_load_from_http_url() {
    assert_load_from_http_url_with(
        ResponseTemplate::new(200).set_body_string(TEST_DATA),
        "/test.db",
    )
    .await;
}

#[tokio::test]
async fn test_load_gz_from_http_url() {
    assert_load_from_http_url_with(
        ResponseTemplate::new(200).set_body_bytes(GZ_TEST_DATA()),
        "/test.db.gz",
    )
    .await;
}

#[tokio::test]
#[should_panic]
async fn test_load_from_http_url_wrong_data() {
    assert_load_from_http_url_with(
        ResponseTemplate::new(200).set_body_string(WRONG_TEST_DATA),
        "/test.db",
    )
    .await;
}

#[tokio::test]
#[should_panic]
async fn test_load_from_http_403() {
    assert_load_from_http_url_with(ResponseTemplate::new(403), "/test.db").await;
}

async fn assert_load_from_ftp_url_with(content: Vec<u8>) {
    let free_tcp_port = pick_random_unused_port().unwrap();
    let mock_server_bind = format!("127.0.0.1:{}", free_tcp_port);
    let mock_server_uri = format!("ftp://{}/", mock_server_bind);

    let mut temp = NamedTempFile::new().unwrap();
    temp.write_all(&content).unwrap();
    let file_name: &str = temp.path().file_name().unwrap().to_str().unwrap();

    let (send, recv) = channel(1);
    let ftp_server = get_new_stoppable_ftp_server_with_fs_path(
        PathBuf::from(temp.path().parent().unwrap()),
        recv,
    );

    let mut url = Url::parse(&mock_server_uri)
        .unwrap_or_else(|_| panic!("failed parsing mock ftp server uri {}", mock_server_uri));
    url.set_path(file_name);

    let reqwest_client = reqwest::Client::new();
    let mut loader = CommonLoader::new(reqwest_client);

    let handle = tokio::task::spawn(ftp_server.listen(mock_server_bind));

    // stupid hack to await for FTP server thread to be ready and start serving requests
    sleep(Duration::from_millis(10)).await;

    let res = loader
        .load_from_url(&url)
        .await
        .expect("failed loading response from mock ftp server");

    assert_lines_eq(res).await;

    send.send(()).await.unwrap(); // send ftp server thread shutdown message
    let _ = handle.await.unwrap(); // join ftp server thread
}

#[tokio::test]
async fn test_load_from_ftp_url() {
    assert_load_from_ftp_url_with(Vec::from(TEST_DATA.as_bytes())).await;
}

#[tokio::test]
async fn test_load_gz_from_ftp_url() {
    assert_load_from_ftp_url_with(GZ_TEST_DATA()).await;
}
