use crate::common_loader::LoadFromURLError::{
    FileError, HTTPError, RequestError, UnsupportedSchemaError,
};
use flate2::bufread::GzDecoder;
use reqwest::{Error, StatusCode, Url};
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

pub struct CommonLoader {
    http_client: reqwest::blocking::Client,
}

impl CommonLoader {
    pub fn new(http_client: reqwest::blocking::Client) -> Self {
        Self { http_client }
    }
}

#[derive(Debug)]
pub enum LoadFromFileError {
    OpenError(std::io::Error),
    UnsupportedExtensionError,
    NonUtf8ExtensionError,
}

pub trait LoadFromFile<T> {
    fn load(path: impl AsRef<Path>) -> Result<T, LoadFromFileError>; // static as it does not need the http client
}

impl LoadFromFile<Box<dyn BufRead>> for CommonLoader {
    fn load(path: impl AsRef<Path>) -> Result<Box<dyn BufRead>, LoadFromFileError> {
        fn bufrd_fromraw(fd: File) -> Box<dyn BufRead> {
            Box::new(BufReader::new(fd))
        }
        fn bufrd_fromgz(fd: File) -> Box<dyn BufRead> {
            Box::new(BufReader::new(GzDecoder::new(BufReader::new(fd))))
        }

        let fd = File::open(&path).map_err(LoadFromFileError::OpenError)?;

        match path.as_ref().extension() {
            None => Ok(bufrd_fromraw(fd)), // no ext file, attempt direct read
            Some(ext) => match ext.to_str() {
                Some("gz") => Ok(bufrd_fromgz(fd)),
                Some("db") => Ok(bufrd_fromraw(fd)),
                Some(_) => Err(LoadFromFileError::UnsupportedExtensionError),
                None => Err(LoadFromFileError::NonUtf8ExtensionError),
            },
        }
    }
}

#[derive(Debug)]
pub enum LoadFromURLError {
    UnsupportedSchemaError(String),
    RequestError(Error),
    HTTPError(StatusCode),
    FileError(LoadFromFileError),
}

pub trait LoadFromURL<T> {
    fn load_from_url(&self, url: &Url) -> Result<T, LoadFromURLError>; // method as its stateful
}

impl LoadFromURL<Box<dyn BufRead>> for CommonLoader {
    fn load_from_url(&self, url: &Url) -> Result<Box<dyn BufRead>, LoadFromURLError> {
        // for 1st implem just ditch cache, and make request sync.
        // can be reimplemented better afterwards with http-cache middleware crate
        match url.scheme() {
            "http" | "https" => {
                let response = self
                    .http_client
                    .get(url.as_str())
                    .send()
                    .map_err(RequestError)?;
                let status = response.status();

                if status.is_success() {
                    Ok(Box::new(BufReader::new(response)))
                } else {
                    Err(HTTPError(status))
                }
            }
            "file" => Self::load(url.path()).map_err(FileError),
            scheme @ _ => Err(UnsupportedSchemaError(String::from(scheme))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::Compression;
    use flate2::bufread::GzEncoder;
    use std::io::{Read, Write};
    use tempfile::{Builder, NamedTempFile};
    use tokio::task;
    use wiremock::matchers::method;
    use wiremock::{Mock, MockServer, ResponseTemplate};

    const TEST_DATA: &str = "TESTDATA\nTESTADA\nTESADA";

    fn assert_lines_eq(res: Box<dyn BufRead>) {
        let mut split_loader = res.split(b'\n');
        let mut split_data = TEST_DATA.split("\n");

        while let Some(line) = split_data.next() {
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
            Err(LoadFromFileError::UnsupportedExtensionError) => (),
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
            .expect(format!("failed parsing MockServer uri {}", &mock_server.uri()).as_str());

        let _ = task::spawn_blocking(move || {
            let reqwest_client = reqwest::blocking::Client::new();
            let loader = CommonLoader::new(reqwest_client);

            let res = loader
                .load_from_url(&url)
                .expect("failed loading response from MockServer");

            assert_lines_eq(res);
        }).await;
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
            .expect(format!("failed parsing MockServer uri {}", &mock_server.uri()).as_str());

        task::spawn_blocking(move || {
            let reqwest_client = reqwest::blocking::Client::new();
            let loader = CommonLoader::new(reqwest_client);

            let _ = loader.load_from_url(&url).unwrap();
        }).await.unwrap();
    }
}
