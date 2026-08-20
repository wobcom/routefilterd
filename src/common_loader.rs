use crate::common_loader::LoadFromURLError::FTPError;
use async_compression::tokio::bufread::GzipDecoder;
use file_type::FileType;
use futures_util::StreamExt;
use futures_util::TryStreamExt;
use reqwest::{Error, StatusCode, Url};
use std::collections::HashMap;
use std::path::Path;
use std::pin::Pin;
use suppaftp::FtpError;
use suppaftp::tokio::AsyncFtpStream;
use suppaftp::types::FileType as FtpFileType;
use tokio::fs::File;
use tokio::io::BufReader;
use tokio::io::{AsyncBufRead, AsyncRead};
use tokio_util::io::StreamReader;

pub struct CommonLoader {
    http_client: reqwest::Client,
    //                    host   port
    ftp_clients: HashMap<(String, u16), AsyncFtpStream>,
}

mod supported_mime_types {
    pub const APPLICATION_GZIP: &str = "application/gzip";
}

const MIME_BUFFER_SIZE: usize = 8192;
const FTP_CMD_DEFAULT_PORT: u16 = 21;

impl CommonLoader {
    pub fn new(http_client: reqwest::Client) -> Self {
        Self {
            http_client,
            ftp_clients: HashMap::new(),
        }
    }

    fn get_ftp_client_mut_ref(
        &mut self,
        addr: &(String, u16),
    ) -> Result<&mut AsyncFtpStream, LoadFromURLError> {
        self.ftp_clients
            .get_mut(addr)
            .ok_or_else(|| LoadFromURLError::NoAddrMapping(addr.0.clone(), addr.1))
    }

    fn matching_decompressor_or_direct_stream(
        file_types: Option<&[&str]>,
        stream: Box<dyn AsyncRead + Send + Unpin>,
        path: String,
    ) -> Box<dyn AsyncBufRead + Send + Unpin> {
        let path_parts: Vec<&str> = path.split('.').collect();

        match path_parts[..] {
            [] => Box::new(BufReader::new(stream)), // root, attempt direct read
            [.., "gz"] => Box::new(BufReader::new(GzipDecoder::new(BufReader::new(stream)))), // gz
            [.., "db"] => Box::new(BufReader::new(stream)), // db file, direct read OK
            [.., &_] => {
                // unhandled extension, attempt mime type conversion
                if let Some(file_types) = file_types {
                    for file_type in file_types {
                        #[allow(clippy::single_match)] // will be extended later
                        match *file_type {
                            supported_mime_types::APPLICATION_GZIP => {
                                return Box::new(BufReader::new(GzipDecoder::new(BufReader::new(
                                    stream,
                                ))));
                            }
                            _ => {}
                        };
                    }
                }
                // cannot determine mime type, attempt direct read
                Box::new(BufReader::new(stream))
            }
        }
    }
}

#[derive(thiserror::Error, Debug)]
pub enum LoadFromFileError {
    #[error("{0}")]
    Open(std::io::Error),
    #[error("Non-UTF8 path")]
    NonUtf8Path,
}

pub trait LoadFromFile<T> {
    async fn load_from_file(path: impl AsRef<Path>) -> Result<T, LoadFromFileError>; // static as it does not need the http client
}

impl LoadFromFile<Pin<Box<dyn AsyncBufRead + Send>>> for CommonLoader {
    async fn load_from_file(
        path: impl AsRef<Path>,
    ) -> Result<Pin<Box<dyn AsyncBufRead + Send>>, LoadFromFileError> {
        let file = File::open(&path).await.map_err(LoadFromFileError::Open)?;
        let file = BufReader::new(file);

        match path.as_ref().to_str() {
            Some(path_str) => Ok(Box::pin(Self::matching_decompressor_or_direct_stream(
                None,
                Box::new(file),
                String::from(path_str),
            ))),
            None => Err(LoadFromFileError::NonUtf8Path),
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
    #[error("Unmappable address {0}:{1}")]
    NoAddrMapping(String, u16),
}

pub trait LoadFromURL<T> {
    async fn load_from_url(&mut self, url: &Url) -> Result<T, LoadFromURLError>; // method as its stateful
}

impl LoadFromURL<Pin<Box<dyn AsyncBufRead + Send>>> for CommonLoader {
    async fn load_from_url(
        &mut self,
        url: &Url,
    ) -> Result<Pin<Box<dyn AsyncBufRead + Send>>, LoadFromURLError> {
        // for 1st implem just ditch cache, and make request sync.
        // can be reimplemented better afterwards with http-cache middleware crate
        match url.scheme() {
            "ftp" => match url.host_str() {
                None => Err(LoadFromURLError::NoHost),
                Some(host_str) => {
                    let addr = (
                        String::from(host_str),
                        *url.port_or_known_default()
                            .get_or_insert(FTP_CMD_DEFAULT_PORT),
                    );
                    let ftp_client = AsyncFtpStream::connect(&addr).await.map_err(FTPError)?;
                    self.ftp_clients.insert(addr.clone(), ftp_client);

                    self.get_ftp_client_mut_ref(&addr)?
                        .login("anonymous", "")
                        .await
                        .map_err(FTPError)?;

                    self.get_ftp_client_mut_ref(&addr)?
                        .transfer_type(FtpFileType::Binary)
                        .await
                        .map_err(FTPError)?;

                    let data_stream = self
                        .get_ftp_client_mut_ref(&addr)?
                        .retr_as_stream(url.path())
                        .await
                        .map_err(FTPError)?;

                    let mut mime_buffer: [u8; _] = [0x00; MIME_BUFFER_SIZE];
                    let raw_tcp_stream = data_stream.into_tcp_stream();

                    let first_bytes = raw_tcp_stream.peek(&mut mime_buffer).await;
                    let file_types = {
                        if let Ok(first_bytes) = first_bytes
                            && first_bytes <= MIME_BUFFER_SIZE
                            && first_bytes > 0
                        {
                            Some(FileType::from_bytes(mime_buffer).media_types())
                        } else {
                            None
                        }
                    };
                    Ok(Box::pin(Self::matching_decompressor_or_direct_stream(
                        file_types,
                        Box::new(raw_tcp_stream),
                        String::from(url.path()),
                    )))
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

                    let first_bytes = stream.as_mut().peek().await;
                    let file_types = if let Some(Ok(first_bytes)) = first_bytes {
                        Some(FileType::from_bytes(first_bytes).media_types())
                    } else {
                        None
                    };
                    Ok(Box::pin(Self::matching_decompressor_or_direct_stream(
                        file_types,
                        Box::new(StreamReader::new(stream)),
                        String::from(url.path()),
                    )))
                } else {
                    Err(LoadFromURLError::HTTPStatus(status))
                }
            }
            "file" => Self::load_from_file(
                url.to_file_path()
                    .map_err(|_| LoadFromURLError::InvalidFileUrl)?,
            )
            .await
            .map_err(LoadFromURLError::FileLoad),
            scheme => Err(LoadFromURLError::UnsupportedSchema(String::from(scheme))),
        }
    }
}
