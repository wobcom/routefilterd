use async_compression::tokio::bufread::GzipDecoder;
use futures_util::StreamExt;
use futures_util::TryStreamExt;
use reqwest::{Error, StatusCode, Url};
use std::path::Path;
use std::pin::Pin;
use tokio::fs::File;
use tokio::io::AsyncBufRead;
use tokio::io::BufReader;
use tokio_util::io::StreamReader;

pub struct CommonLoader {
    http_client: reqwest::Client,
}

impl CommonLoader {
    pub fn new(http_client: reqwest::Client) -> Self {
        Self { http_client }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum LoadFromFileError {
    #[error("{0}")]
    Open(std::io::Error),
    #[error("Unsupported extension")]
    UnsupportedExtension,
    #[error("Non-UTF8 extension")]
    NonUtf8Extension,
}

pub trait LoadFromFile<T> {
    async fn load(path: impl AsRef<Path>) -> Result<T, LoadFromFileError>; // static as it does not need the http client
}

impl LoadFromFile<Pin<Box<dyn AsyncBufRead + Send>>> for CommonLoader {
    async fn load(
        path: impl AsRef<Path>,
    ) -> Result<Pin<Box<dyn AsyncBufRead + Send>>, LoadFromFileError> {
        let file = File::open(&path).await.map_err(LoadFromFileError::Open)?;
        let file = BufReader::new(file);

        match path.as_ref().extension() {
            None => Ok(Box::pin(file)), // no ext file, attempt direct read
            Some(ext) => match ext.to_str() {
                Some("gz") => Ok(Box::pin(BufReader::new(GzipDecoder::new(file)))),
                Some("db") => Ok(Box::pin(file)),
                Some(_) => Err(LoadFromFileError::UnsupportedExtension),
                None => Err(LoadFromFileError::NonUtf8Extension),
            },
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum LoadFromURLError {
    #[error("{0}")]
    UnsupportedSchema(String),
    #[error("{0}")]
    HTTPRequest(Error),
    #[error("{0}")]
    HTTPStatus(StatusCode),
    #[error("{0}")]
    FileLoad(LoadFromFileError),
}

pub trait LoadFromURL<T> {
    async fn load_from_url(&self, url: &Url) -> Result<T, LoadFromURLError>; // method as its stateful
}

impl LoadFromURL<Pin<Box<dyn AsyncBufRead + Send>>> for CommonLoader {
    async fn load_from_url(
        &self,
        url: &Url,
    ) -> Result<Pin<Box<dyn AsyncBufRead + Send>>, LoadFromURLError> {
        // for 1st implem just ditch cache, and make request sync.
        // can be reimplemented better afterwards with http-cache middleware crate
        match url.scheme() {
            "http" | "https" => {
                let response = self
                    .http_client
                    .get(url.as_str())
                    .send()
                    .await
                    .map_err(LoadFromURLError::HTTPRequest)?;
                let status = response.status();

                if status.is_success() {
                    let mut stream = Box::pin(
                        response
                            .bytes_stream()
                            .map_err(std::io::Error::other)
                            .peekable(),
                    );
                    if let Some(Ok(first_bytes)) = stream.as_mut().peek().await
                        && first_bytes.starts_with(b"\x1f\x8b")
                    {
                        let reader = StreamReader::new(stream);
                        let decompressed = GzipDecoder::new(reader);
                        return Ok(Box::pin(BufReader::new(decompressed)));
                    }
                    Ok(Box::pin(StreamReader::new(stream)))
                } else {
                    Err(LoadFromURLError::HTTPStatus(status))
                }
            }
            "file" => Self::load(url.path()).await.map_err(FileLoad),
            scheme => Err(LoadFromURLError::UnsupportedSchema(String::from(scheme))),
        }
    }
}
