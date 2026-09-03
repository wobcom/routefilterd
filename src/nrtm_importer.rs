use crate::store::{DataSource, DataStore};
use futures_util::StreamExt;
use nrtm_parser::{NRTMV3Parser, OpType, StreamingNRTMParser, Verb};
use std::collections::HashMap;
use std::num::ParseIntError;
use std::sync::{Arc, MutexGuard, PoisonError};
use tokio::io;
use tokio::io::AsyncWriteExt;
use tokio::net::TcpStream;

type Serial = u64;

enum NRTMRefreshMode {
    Continuous, // maintain continuously open connection
    Scheduled,  // schedule refreshes
}

struct NRTMImporter {
    nrtm_source_sockaddr: (String, u16),
    data_source_name: String,
    store_handle: Arc<DataStore>,
    mode: NRTMRefreshMode,
    // TODO add shutdown message channel to gracefully close TCP connection
}

#[derive(Debug)]
enum NRTMImporterError<'a> {
    DataSourceLock(PoisonError<MutexGuard<'a, HashMap<String, DataSource>>>),
    NoMatchingDataSourceFor(String),
    WrongAddressFormat(String),
    UnParseablePortNum(ParseIntError),
    IO(io::Error),
    StoreError(String),
}

impl NRTMImporter {
    /// `address` should be either:
    /// - direct ip address with port. example: `127.0.0.1:4444`
    /// - dns name with port. example: `localhost:4444`
    pub fn new<'a>(
        address: String,
        ds_name: String,
        data_store: Arc<DataStore>,
        mode: NRTMRefreshMode,
    ) -> Result<Self, NRTMImporterError<'a>> {
        let split_index = address
            .find(":")
            .ok_or(NRTMImporterError::WrongAddressFormat(address.clone()))?;
        let (address_str, socket_str) = address.split_at(split_index);

        Ok(Self {
            nrtm_source_sockaddr: (
                String::from(address_str),
                socket_str
                    .parse()
                    .map_err(NRTMImporterError::UnParseablePortNum)?,
            ),
            data_source_name: ds_name,
            store_handle: data_store,
            mode,
        })
    }

    fn format_request(&self, serial: Serial) -> String {
        let k = match self.mode {
            NRTMRefreshMode::Continuous => "k",
            NRTMRefreshMode::Scheduled => "",
        };

        format!("-{k}g {}:3:{serial}-LAST", self.data_source_name)
    }

    fn get_data_sources_lock(
        &self,
    ) -> Result<MutexGuard<'_, HashMap<String, DataSource>>, NRTMImporterError<'_>> {
        self.store_handle
            .datasources
            .lock()
            .map_err(NRTMImporterError::DataSourceLock)
    }

    async fn sequential_import(&self) -> Result<(), NRTMImporterError<'_>> {
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

        let mut message_reader = NRTMV3Parser::reader_from(tcp_stream);

        while let Some(Ok(nrtm_message)) = message_reader.next().await {
            match nrtm_message.update {
                OpType::V2(_) => {} // no v2 support
                OpType::V3(verb, serial) => match verb {
                    Verb::ADD => {
                        // import object
                        self.store_handle
                            .import_object(self.data_source_name.clone(), nrtm_message.rpsl)
                            .map_err(NRTMImporterError::StoreError)?;

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
                    Verb::DEL => {
                        // store deletion not yet implemented
                        todo!();
                    }
                },
            }
        }

        Ok(())
    }

    fn continuous_import_loop(&self) {
        loop {
            todo!()
        }
    }
}
