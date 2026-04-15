use crate::store::DataStore;
use crate::store_importer::LoadFromURLError::{
    FileError, HTTPError, RequestError, UnsupportedSchemaError,
};
use flate2::bufread::GzDecoder;
use futures_util::stream::StreamExt;
use log::{info, trace, warn};
use reqwest::{Error, StatusCode, Url};
use std::fs::File;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::Lines;
use std::io::Write;
use std::io::{BufRead, BufReader};
use std::path::Path;
use std::sync::Arc;

pub struct RpslParser {
    reader: Lines<Box<dyn BufRead>>,
    line_num: u64,
    obj_num: u64,
}

#[derive(Debug)]
enum LoadFromFileError {
    OpenError(std::io::Error),
    UnsupportedExtensionError,
    NonUtf8ExtensionError,
}
trait LoadFromFile<T> {
    fn load(path: impl AsRef<Path>) -> Result<T, LoadFromFileError>;
}

#[derive(Debug)]
enum LoadFromURLError {
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
    pub fn new(buf: Box<dyn BufRead>) -> Self {
        Self {
            reader: buf.lines(),
            line_num: 0,
            obj_num: 0,
        }
    }
}

impl LoadFromFile<RpslParser> for RpslParser {
    fn load(path: impl AsRef<Path>) -> Result<RpslParser, LoadFromFileError> {
        fn bufrd_fromraw(fd: File) -> Box<dyn BufRead> {
            Box::new(BufReader::new(fd))
        }
        fn bufrd_fromgz(fd: File) -> Box<dyn BufRead> {
            Box::new(BufReader::new(GzDecoder::new(BufReader::new(fd))))
        }

        let fd = File::open(&path).map_err(LoadFromFileError::OpenError)?;

        match path.as_ref().extension() {
            None => Ok(Self::new(bufrd_fromraw(fd))), // no ext file, attempt direct read
            Some(ext) => match ext.to_str() {
                Some("gz") => Ok(Self::new(bufrd_fromgz(fd))),
                Some(_) => Err(LoadFromFileError::UnsupportedExtensionError),
                None => Err(LoadFromFileError::NonUtf8ExtensionError),
            },
        }
    }
}

impl LoadFromURL<RpslParser> for RpslParser {
    fn load_from_url(
        http_client: &reqwest::blocking::Client, // dep injection for easy testing
        url: &Url,
    ) -> Result<RpslParser, LoadFromURLError> {
        // for 1st implem just ditch cache, and make request sync.
        // can be reimplemented better afterwards with http-cache middleware crate
        match url.scheme() {
            "http" | "https" => {
                let response = http_client.get(url.as_str()).send().map_err(RequestError)?;
                let status = response.status();

                status
                    .is_success()
                    .then(move || Self::new(Box::new(BufReader::new(response))))
                    .ok_or(HTTPError(status))
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

async fn cache_http(url: String, path: String) -> String {
    let client = reqwest::Client::new();

    let res = client
        .get(&url)
        .send()
        .await
        .or(Err(format!("Failed to GET from '{}'", &url)))
        .unwrap();

    let total_size = res
        .content_length()
        .ok_or(format!("Failed to get content length from '{}'", &url))
        .unwrap();

    println!("downloading file of {} bytes", total_size);

    let mut file = File::create(&path)
        .or(Err(format!("Failed to create file '{}'", &path)))
        .unwrap();
    let mut stream = res.bytes_stream();

    while let Some(item) = stream.next().await {
        let chunk = item
            .or(Err(format!("Error while downloading file")))
            .unwrap();
        file.write_all(&chunk)
            .or(Err(format!("Error while writing to file")))
            .unwrap();
    }

    println!("success");
    return path;
}

fn hash_filename(path: &str, name: &str, serial: u64) -> String {
    let extension = &path.split(".").last().unwrap();

    let mut h = DefaultHasher::new();
    path.hash(&mut h);
    name.hash(&mut h);
    serial.hash(&mut h);

    format!("{:X}.{}", h.finish(), extension)
}

pub fn import_source(store: &Arc<DataStore>, name: &String, file: String, cache_dir: String) {
    info!("Importing {}", &file);
    let _ = store.import_objects(
        &name,
        RpslParser::load_from_url(
            &reqwest::blocking::Client::new(),
            &Url::parse(&file).unwrap(),
        )
        .unwrap(),
    );
    info!("Done importing {}", &file);
}
