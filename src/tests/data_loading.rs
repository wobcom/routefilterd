use super::super::common_loader::{CommonLoader, LoadFromFile, LoadFromFileError, LoadFromURL};
use flate2::Compression;
use flate2::bufread::GzEncoder;
use reqwest::Url;
use std::io::{BufRead, Read, Write};
use tempfile::{Builder, NamedTempFile};
use tokio::task;
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

const TEST_DATA: &str = "TESTDATA\nTESTADA\nTESADA";

fn assert_lines_eq(res: Box<dyn BufRead>) {
    let mut split_loader = res.split(b'\n');
    let split_data = TEST_DATA.split("\n");

    for line in split_data {
        assert_eq!(split_loader.next().unwrap().unwrap(), line.as_bytes());
    }
}

#[test]
fn test_load_from_raw_file() {
    let mut temp = NamedTempFile::new().unwrap();

    temp.write_all(TEST_DATA.as_bytes()).unwrap();

    let res = CommonLoader::load(temp.as_ref()).unwrap();
    assert_lines_eq(res);
}

#[test]
fn test_load_from_db_file() {
    let mut temp = Builder::new()
        .suffix(".db")
        .tempfile()
        .expect("cannot create test temporary file");

    temp.write_all(TEST_DATA.as_bytes()).unwrap();

    let res = CommonLoader::load(temp.as_ref()).unwrap();
    assert_lines_eq(res);
}

#[test]
fn test_load_from_gz_file() {
    let mut temp = Builder::new()
        .suffix(".gz")
        .tempfile()
        .expect("cannot create test temporary file");
    let mut compressed = Vec::new();
    let mut gz_encoder = GzEncoder::new(TEST_DATA.as_bytes(), Compression::fast());

    gz_encoder.read_to_end(&mut compressed).unwrap();
    temp.write_all(&compressed[..]).unwrap();

    let res = CommonLoader::load(temp.as_ref()).unwrap();
    assert_lines_eq(res);
}

#[test]
fn test_load_from_unknown_suffix() {
    let temp = Builder::new()
        .suffix(".bad")
        .tempfile()
        .expect("cannot create test temporary file");

    let res = CommonLoader::load(temp.as_ref());

    match res {
        Err(LoadFromFileError::UnsupportedExtension) => (),
        _ => panic!("did not error out on unknown suffix"),
    };
}

#[test]
#[should_panic]
fn test_unknown_scheme_fail() {
    let reqwest_client = reqwest::blocking::Client::new();
    let loader = CommonLoader::new(reqwest_client);
    // per https://www.iana.org/assignments/uri-schemes/uri-schemes.xhtml
    let url = Url::parse("gopher://example.com/trusted.db.gz").unwrap();

    let _ = loader.load_from_url(&url).unwrap();
}

#[test]
fn test_load_from_file_url() {
    let reqwest_client = reqwest::blocking::Client::new();
    let loader = CommonLoader::new(reqwest_client);
    let mut temp = Builder::new()
        .suffix(".db")
        .tempfile()
        .expect("cannot create test temporary file");
    let url = Url::from_file_path(&temp).unwrap();

    temp.write_all(TEST_DATA.as_bytes()).unwrap();

    let res = loader.load_from_url(&url).unwrap();
    assert_lines_eq(res);
}

#[tokio::test]
async fn test_load_from_http_url() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_string(TEST_DATA))
        .mount(&mock_server)
        .await;

    let url = Url::parse(&mock_server.uri())
        .unwrap_or_else(|_| panic!("failed parsing MockServer uri {}", &mock_server.uri()));

    let _ = task::spawn_blocking(move || {
        let reqwest_client = reqwest::blocking::Client::new();
        let loader = CommonLoader::new(reqwest_client);

        let res = loader
            .load_from_url(&url)
            .expect("failed loading response from MockServer");

        assert_lines_eq(res);
    })
    .await;
}

#[tokio::test]
#[should_panic]
async fn test_load_from_http_403() {
    let mock_server = MockServer::start().await;

    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(403).set_body_string(TEST_DATA))
        .mount(&mock_server)
        .await;

    let url = Url::parse(&mock_server.uri())
        .unwrap_or_else(|_| panic!("failed parsing MockServer uri {}", &mock_server.uri()));

    task::spawn_blocking(move || {
        let reqwest_client = reqwest::blocking::Client::new();
        let loader = CommonLoader::new(reqwest_client);

        let _ = loader.load_from_url(&url).unwrap();
    })
    .await
    .unwrap();
}
