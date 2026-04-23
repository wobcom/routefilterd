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
    use std::io::Write;
    use tempfile::NamedTempFile;

    #[test]
    fn test_load_from_file() {
        let test_data = "TESTDATA\nTESTADA\nTESADA";
        let mut temp = NamedTempFile::new().unwrap();

        temp.write_all(test_data.as_bytes()).unwrap();

        let res = CommonLoader::load(temp.as_ref()).unwrap();
        let mut split_loader = res.split(b'\n');
        let mut split_data = test_data.split("\n");

        while let Some(line) = split_data.next() {
            assert_eq!(split_loader.next().unwrap().unwrap(), line.as_bytes());
        }
    }
}
