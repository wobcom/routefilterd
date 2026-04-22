use crate::common_loader::{CommonLoader, LoadFromURL, LoadFromURLError};
use crate::store::DataStore;
use log::{info, trace};
use reqwest::{StatusCode, Url};
use std::io::BufRead;
use std::io::Lines;
use std::sync::Arc;

pub struct RpslParser {
    reader: Lines<Box<dyn BufRead>>,
    line_num: u64,
    obj_num: u64,
}
impl RpslParser {
    pub fn new(buf: Box<dyn BufRead>) -> Self {
        Self {
            reader: buf.lines(),
            line_num: 0,
            obj_num: 0,
        }
    }
    pub fn new_from_url(
        loader: Box<dyn LoadFromURL<Box<dyn BufRead>>>,
        url: &Url,
    ) -> Result<Self, LoadFromURLError> {
        let b = loader.load_from_url(url)?;
        Ok(Self::new(b))
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
    let loader = CommonLoader::new(reqwest::blocking::Client::new());

    info!("Importing {}", &file);
    let _ = store.import_objects(
        &name,
        RpslParser::new_from_url(Box::new(loader), &Url::parse(&file).unwrap()).unwrap(),
    );
    info!("Done importing {}", &file);
}
