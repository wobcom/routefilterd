use crate::common_loader::{CommonLoader, LoadFromURL};
use crate::store::DataStore;
use futures_util::Stream;
use log::{info, trace};
use reqwest::Url;
use std::sync::Arc;
use tokio::io::AsyncBufRead;
use tokio::io::AsyncBufReadExt;

pub fn parse_rpsl(
    source: impl AsyncBufRead + Send + Unpin,
) -> impl Stream<Item = std::io::Result<String>> {
    let mut reader = source.lines();
    let mut object_buf = String::with_capacity(8192);
    let mut line_num = 0;
    let mut obj_num = 0;

    async_stream::try_stream! {
        loop {
            let line = match reader.next_line().await {
                Err(e) if e.kind() == std::io::ErrorKind::InvalidData => {
                    trace!("ignoring line: {}", e);
                    continue;
                },
                Err(e) => Err(e)?,
                Ok(Some(l)) => l,
                Ok(None) => break,
            };
            line_num += 1;

            if line.starts_with("#") {
                // Ignore comments and empty lines
                continue;
            }

            if !line.is_empty() {
                object_buf.push_str(&line);
                object_buf.push('\n');
            }
            if line.is_empty() && !object_buf.is_empty() {
                obj_num += 1;
                object_buf.push('\n');
                yield std::mem::replace(&mut object_buf, String::with_capacity(8192));
            }
        }

        if !object_buf.is_empty() {
            // Additional new line is part of the format, so we have to readd it for the last element, otherwise the last element would not parse.
            object_buf.push('\n');

            // Yield last object
            yield std::mem::replace(&mut object_buf, String::with_capacity(8192));
        }

        info!(
            "Successfully parsed {} lines into {} objects.",
            line_num, obj_num
        );
    }
}

pub async fn import_source(store: &Arc<DataStore>, name: &str, url: String, _cache_dir: String) {
    let loader = CommonLoader::new(reqwest::Client::new());

    info!("Importing {}", url);
    store
        .import_objects(
            name,
            Box::pin(parse_rpsl(
                loader
                    .load_from_url(&Url::parse(&url).unwrap())
                    .await
                    .unwrap(),
            )),
        )
        .await
        .unwrap();
    info!("Done importing {}", url);
}
