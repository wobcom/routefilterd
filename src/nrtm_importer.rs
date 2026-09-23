use crate::store::{DataSource, DataStore};
use futures_util::TryStreamExt;
use nrtm_parser::streaming::NRTMStreamError;
use nrtm_parser::{NRTMV3Parser, OpType, StreamingNRTMParser, Verb};
use re_delimiter_codec::REDelimiterCodecError;
use std::collections::HashMap;
use std::sync::{Arc, MutexGuard};
use std::time::Duration;
use tokio::io::AsyncWriteExt;
use tokio::net::{TcpStream, ToSocketAddrs};
use tokio::{io, time};
use tokio_util::sync::CancellationToken;

type Serial = u64;

const MAX_CHUNK_LEN: usize = 1024 * 1024; // 1M

pub enum NRTMRefreshMode {
    SingleLongLastingConnection, // maintain continuously open connection
    ScheduledMultipleConnection, // schedule refreshes
}

pub struct NRTMImporter<T: ToSocketAddrs + Clone> {
    nrtm_source_sockaddr: T,
    data_source_name: String,
    store_handle: Arc<DataStore>,
    mode: NRTMRefreshMode,
    refresh_interval: Duration,
    cancel_token: CancellationToken,
}

#[derive(thiserror::Error, Debug)]
pub enum NRTMImporterError {
    #[error("cannot acquire data source lock")]
    DataSourceLock, // not including MutexGuard in error, cos, logically, it is not Send trait.
    #[error("no matching data source for this name {0}")]
    NoMatchingDataSourceFor(String),
    #[error("{0}")]
    IO(io::Error),
    #[error("Store API answered with an error {0}")]
    StoreError(String),
}

impl<T: ToSocketAddrs + Clone> NRTMImporter<T> {
    pub fn new(
        address: T,
        ds_name: String,
        data_store: Arc<DataStore>,
        mode: NRTMRefreshMode,
        refresh_interval: Duration,
        cancel_token: CancellationToken,
    ) -> Self {
        Self {
            nrtm_source_sockaddr: address,
            data_source_name: ds_name,
            store_handle: data_store,
            mode,
            refresh_interval,
            cancel_token,
        }
    }

    pub async fn start(self) -> Result<(), NRTMImporterError> {
        // call with
        match self.mode {
            NRTMRefreshMode::SingleLongLastingConnection => self.single_connection_import().await,
            NRTMRefreshMode::ScheduledMultipleConnection => self.continuous_import_loop().await,
        }
    }

    fn format_request(&self, serial: Serial) -> String {
        let k = match self.mode {
            NRTMRefreshMode::SingleLongLastingConnection => "k",
            NRTMRefreshMode::ScheduledMultipleConnection => "",
        };

        format!("-{k}g {}:3:{serial}-LAST\n", self.data_source_name)
    }

    fn get_data_sources_lock(
        &self,
    ) -> Result<MutexGuard<'_, HashMap<String, DataSource>>, NRTMImporterError> {
        self.store_handle
            .datasources
            .lock()
            .map_err(|_e| NRTMImporterError::DataSourceLock)
    }

    async fn single_connection_import(&self) -> Result<(), NRTMImporterError> {
        let current_serial = {
            let data_source_map = self.get_data_sources_lock()?;
            let my_ds = data_source_map.get(&self.data_source_name).ok_or(
                NRTMImporterError::NoMatchingDataSourceFor(self.data_source_name.clone()),
            )?;
            my_ds.current_serial
        };

        let mut tcp_stream = TcpStream::connect(self.nrtm_source_sockaddr.clone())
            .await
            .map_err(NRTMImporterError::IO)?;

        // send out request string
        // load for atomic u64 is SeqCst ordering as we need serial to be monotonically increasing
        tcp_stream
            .write_all(self.format_request(current_serial).as_bytes())
            .await
            .map_err(NRTMImporterError::IO)?;

        let mut v3_parser = NRTMV3Parser::new(MAX_CHUNK_LEN);
        let mut message_reader = v3_parser.stream_from(tcp_stream);

        loop {
            let optional_result = message_reader.try_next().await;

            match optional_result {
                Ok(None) => return Ok(()), // end of stream, if no error encountered then all good! :)
                Ok(Some(nrtm_message)) => match nrtm_message.update {
                    OpType::V2(_) => {} // no v2 support
                    OpType::V3(verb, serial) => match verb {
                        Verb::ADD => {
                            // import object
                            let import_ok = self
                                .store_handle
                                .import_object(self.data_source_name.clone(), nrtm_message.rpsl);

                            if let Err(e) = import_ok {
                                log::warn!(
                                    "skipping update {} as store returned an error on import: {}",
                                    serial,
                                    e
                                )
                            } else {
                                // increase serial
                                // acquire lock
                                let mut data_source_map = self.get_data_sources_lock()?;
                                data_source_map
                                    .entry(self.data_source_name.clone())
                                    .and_modify(
                                        // not atomic but source has NRTM authority on serial atomicity,
                                        // so should be fine, plus we only have one NRTM import per source
                                        |ds| ds.current_serial = serial,
                                    )
                                    .or_insert(DataSource::default()); // should not happen as its checked above
                                // drop lock
                                drop(data_source_map);
                            }
                        }
                        Verb::DEL => {
                            // store deletion not yet implemented
                            // TODO implement deletion in store
                            log::warn!(
                                "NRTM import DS {} asked for deletion, this is not implemented yet.",
                                self.data_source_name,
                            );
                        }
                    },
                },
                Err(NRTMStreamError::Parser(parse_e)) => {
                    log::error!(
                        "encountered recoverable parser error, continuing {:?}",
                        parse_e
                    );
                }
                Err(NRTMStreamError::REDelimiterCodec(
                    REDelimiterCodecError::MaxChunkLengthExceeded,
                )) => {
                    log::error!(
                        "max chunk length exceeded, one or more update(s) discarded. continuing."
                    );
                }
                Err(e) => {
                    panic!(
                        "irrecoverable error encountered during stream processing {:?}",
                        e
                    );
                }
            }
        }
    }

    async fn continuous_import_loop(&self) -> Result<(), NRTMImporterError> {
        let mut interval = time::interval(self.refresh_interval);

        loop {
            tokio::select! {
                 // exit, connections will close by being dropped
                _shutdown = self.cancel_token.cancelled() => break,
                else => {
                     // schedule
                    interval.tick().await;
                    // bubble up errors if any, will exit loop
                    self.single_connection_import().await?;
                },
            }
        }
        Ok(()) // import called off
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::fixtures::{get_new_store, get_single_connection_tcp_server_with};
    use std::fs::read_to_string;

    #[tokio::test]
    async fn test_single_import() {
        let ds_name = String::from("RIPE");
        let store = get_new_store();
        let cancel_token = CancellationToken::new();
        store.new_data_source(ds_name.to_string(), 65887900, 100);
        let answer = read_to_string("./src/tests/data/ripe_nrtmv3_session.txt").unwrap();
        let (server, listen_socket) =
            get_single_connection_tcp_server_with(answer.into_bytes()).await;

        let importer = NRTMImporter::new(
            listen_socket,
            ds_name.clone(),
            store.clone(),
            NRTMRefreshMode::SingleLongLastingConnection,
            Default::default(),
            cancel_token,
        );

        importer
            .start()
            .await
            .expect("should have parsed stream until end");

        // join server thread
        server.await.expect("server should have closed");

        let _ = store
            .query_as_set(vec![ds_name], String::from("AS62425:AS-KZYDC"), &[])
            .expect("as-set should be present after nrtm import");
    }
}
