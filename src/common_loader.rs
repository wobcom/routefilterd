use crate::common_loader::LoadFromURLError::FTPError;
use async_compression::tokio::bufread::GzipDecoder;
use file_type::FileType;
use futures_util::StreamExt;
use futures_util::TryStreamExt;
use reqwest::{Error, StatusCode, Url};
use std::path::Path;
use std::pin::Pin;
use suppaftp::FtpError;
use suppaftp::tokio::AsyncFtpStream;
use tokio::fs::File;
use tokio::io::AsyncBufRead;
use tokio::io::BufReader;
use tokio_util::io::StreamReader;

pub struct CommonLoader {
    http_client: reqwest::Client,
}

mod supported_mime_types {
    pub const APPLICATION_GZIP: &str = "application/gzip";
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
    #[error("Invalid file url")]
    InvalidFileUrl,
    #[error("{0}")]
    FTPError(FtpError),
    #[error("No Host in URL")]
    NoHost,
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
            "ftp" => match url.host_str() {
                None => Err(LoadFromURLError::NoHost),
                Some(host_str) => {
                    let mut ftp_client = AsyncFtpStream::connect(format!("{}:21", host_str))
                        .await
                        .map_err(FTPError)?;
                    ftp_client.login("anonymous", "").await.map_err(FTPError)?;

                    let data_stream = ftp_client
                        .retr_as_stream(url.path())
                        .await
                        .map_err(FTPError)?;

                    let mut mime_buffer: [u8; 4] = [0x00, 0x00, 0x00, 0x00];

                    let raw_tcpstream = data_stream.into_tcp_stream();

                    if let Ok(first_bytes) = raw_tcpstream.peek(&mut mime_buffer).await
                        && first_bytes == 4
                        && FileType::from_bytes(mime_buffer)
                            .media_types()
                            .contains(&supported_mime_types::APPLICATION_GZIP)
                    {
                        let reader = BufReader::new(raw_tcpstream);
                        let decompressed = GzipDecoder::new(reader);
                        Ok(Box::pin(BufReader::new(decompressed)))
                    } else {
                        Ok(Box::pin(BufReader::new(raw_tcpstream)))
                    }
                }
            },
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
                        && FileType::from_bytes(first_bytes)
                            .media_types()
                            .contains(&supported_mime_types::APPLICATION_GZIP)
                    {
                        let reader = StreamReader::new(stream);
                        let decompressed = GzipDecoder::new(reader);
                        Ok(Box::pin(BufReader::new(decompressed)))
                    } else {
                        Ok(Box::pin(StreamReader::new(stream)))
                    }
                } else {
                    Err(LoadFromURLError::HTTPStatus(status))
                }
            }
            "file" => Self::load(
                url.to_file_path()
                    .map_err(|_| LoadFromURLError::InvalidFileUrl)?,
            )
            .await
            .map_err(LoadFromURLError::FileLoad),
            scheme => Err(LoadFromURLError::UnsupportedSchema(String::from(scheme))),
        }
    }
}
