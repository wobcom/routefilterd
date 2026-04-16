use crate::store::DataStore;
use crate::store_importer::LoadFromURLError::{
    FileError, HTTPError, RequestError, UnsupportedSchemaError,
};
use flate2::bufread::GzDecoder;
use log::{info, trace};
use reqwest::{Error, StatusCode, Url};
use std::fs::File;
use std::io::Lines;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::sync::Arc;

pub struct RpslParser {
    reader: Lines<Box<dyn BufRead>>,
    line_num: u64,
    obj_num: u64,
}

#[derive(Debug)]
pub enum LoadFromFileError {
    OpenError(std::io::Error),
    UnsupportedExtensionError,
    NonUtf8ExtensionError,
}
trait LoadFromFile<T> {
    fn load(path: impl AsRef<Path>) -> Result<T, LoadFromFileError>;
}

#[derive(Debug)]
pub enum LoadFromURLError {
    UnsupportedSchemaError(String),
    RequestError(Error),
    HTTPError(StatusCode),
    FileError(LoadFromFileError),
}
trait LoadFromURL<T> {
    fn load_from_url(
        http_client: &reqwest::blocking::Client,
        url: &Url,
    ) -> Result<T, LoadFromURLError>;
}

impl RpslParser {
    fn new(buf: Box<dyn BufRead>) -> Self {
        Self {
            reader: buf.lines(),
            line_num: 0,
            obj_num: 0,
        }
    }
    pub fn new_from_url(
        client: &reqwest::blocking::Client,
        url: &Url,
    ) -> Result<Self, LoadFromURLError> {
        let b = Self::load_from_url(client, url)?;
        Ok(Self::new(b))
    }
}

impl LoadFromFile<Box<dyn BufRead>> for RpslParser {
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

impl LoadFromURL<Box<dyn BufRead>> for RpslParser {
    fn load_from_url(
        http_client: &reqwest::blocking::Client, // dep injection for easy testing
        url: &Url,
    ) -> Result<Box<dyn BufRead>, LoadFromURLError> {
        // for 1st implem just ditch cache, and make request sync.
        // can be reimplemented better afterwards with http-cache middleware crate
        match url.scheme() {
            "http" | "https" => {
                let response = http_client.get(url.as_str()).send().map_err(RequestError)?;
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

impl Iterator for RpslParser {
    type Item = String;

    fn next(&mut self) -> Option<Self::Item> {
        let mut object_buf = String::with_capacity(8192);

        while let Some(line) = self.reader.next() {
            let l = line.unwrap_or_else(|err| {
                trace!("Error encountered reading line {}: {}", self.line_num, err);
                "".to_string()
            });
            self.line_num = self.line_num + 1;

            if l.starts_with("#") {
                // Ignore comments and empty lines
                continue;
            }
            object_buf.push_str(&l);
            object_buf.push_str("\n");

            if l.eq("") && !object_buf.eq("") {
                self.obj_num = self.obj_num + 1;
                return Some(object_buf);
            }
        }

        if !object_buf.eq("") {
            // Yield last object
            return Some(object_buf);
        }

        info!(
            "Successfully parsed {} lines into {} objects.",
            self.line_num, self.obj_num
        );
        None
    }
}

pub fn import_source(store: &Arc<DataStore>, name: &String, file: String, _cache_dir: String) {
    info!("Importing {}", &file);
    let _ = store.import_objects(
        &name,
        RpslParser::new_from_url(
            &reqwest::blocking::Client::new(),
            &Url::parse(&file).unwrap(),
        )
        .unwrap(),
    );
    info!("Done importing {}", &file);
}
