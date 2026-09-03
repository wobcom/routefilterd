use crate::common_loader::{CommonLoader, LoadFromURL, LoadFromURLError};
use reqwest::Url;
use std::num::ParseIntError;
use std::str::FromStr;
use tokio::io;
use tokio::io::AsyncReadExt;

pub(crate) struct SerialLoader {
    inner_loader: CommonLoader,
}

#[derive(thiserror::Error, Debug)]
pub enum SerialLoaderError {
    #[error("Cannot load from this url, encountered: {0}")]
    LoadFromURL(LoadFromURLError),
    #[error("IO Error during serial fetching, encountered: {0}")]
    IO(io::Error),
    #[error("Invalid serial error. {0}")]
    ParseFromStr(ParseIntError),
}

impl SerialLoader {
    pub(crate) fn new(common_loader: CommonLoader) -> Self {
        Self {
            inner_loader: common_loader,
        }
    }

    pub(crate) async fn load_serial_from<T: FromStr<Err = ParseIntError>>(
        &mut self,
        url: &Url,
    ) -> Result<T, SerialLoaderError> {
        let mut async_res = self
            .inner_loader
            .load_from_url(url)
            .await
            .map_err(SerialLoaderError::LoadFromURL)?;
        let mut serial = String::new();

        async_res
            .read_to_string(&mut serial)
            .await
            .map_err(SerialLoaderError::IO)?;
        let serial_slice = serial.trim();

        serial_slice
            .parse()
            .map_err(SerialLoaderError::ParseFromStr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::fixtures::get_http_server_with;
    use wiremock::ResponseTemplate;

    #[tokio::test]
    pub async fn test_load_serial_from_http() {
        let common_loader = CommonLoader::new(reqwest::Client::new());
        let mut serial_loader = SerialLoader::new(common_loader);
        let mock_server = get_http_server_with(
            ResponseTemplate::new(200).set_body_string("\n  9999999999999999999999999999\n"),
        )
        .await;

        let url = Url::parse(&mock_server.uri()).unwrap();
        let serial = serial_loader
            .load_serial_from::<u128>(&url)
            .await
            .expect("failed to load serial from mock loader");

        assert_eq!(serial, 9999999999999999999999999999);
    }
}
