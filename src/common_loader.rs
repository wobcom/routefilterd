use crate::common_loader::LoadFromURLError::{
    FileLoad, HTTPRequest, HTTPStatus, UnsupportedSchema,
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
    Open(std::io::Error),
    UnsupportedExtension,
    NonUtf8Extension,
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

        let fd = File::open(&path).map_err(LoadFromFileError::Open)?;

        match path.as_ref().extension() {
            None => Ok(bufrd_fromraw(fd)), // no ext file, attempt direct read
            Some(ext) => match ext.to_str() {
                Some("gz") => Ok(bufrd_fromgz(fd)),
                Some("db") => Ok(bufrd_fromraw(fd)),
                Some(_) => Err(LoadFromFileError::UnsupportedExtension),
                None => Err(LoadFromFileError::NonUtf8Extension),
            },
        }
    }
}

#[derive(Debug)]
pub enum LoadFromURLError {
    UnsupportedSchema(String),
    HTTPRequest(Error),
    HTTPStatus(StatusCode),
    FileLoad(LoadFromFileError),
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
                    .map_err(HTTPRequest)?;
                let status = response.status();

                if status.is_success() {
                    Ok(Box::new(BufReader::new(response)))
                } else {
                    Err(HTTPStatus(status))
                }
            }
            "file" => Self::load(url.path()).map_err(FileLoad),
            scheme => Err(UnsupportedSchema(String::from(scheme))),
        }
    }
}
