use crate::store::DataStore;
use flate2::bufread::GzDecoder;
use futures_util::StreamExt;
use log::{info, trace, warn};
use reqwest::Url;
use std::fs::File;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::io::BufRead;
use std::io::BufReader;
use std::io::Lines;
use std::io::Write;
use std::sync::Arc;

pub struct RpslParser {
    reader: Lines<Box<dyn BufRead>>,
    line_num: u64,
    obj_num: u64,
}

impl RpslParser {
    pub fn new_from_file(path: String) -> Self {
        let socket = BufReader::new(File::open(&path).expect("Couldn't open file"));
        if path.ends_with(".gz") {
            return Self::new(Box::new(BufReader::new(GzDecoder::new(socket))));
        }

        return Self::new(Box::new(socket));
    }

    pub fn new(buf: Box<dyn BufRead>) -> Self {
        Self {
            reader: buf.lines(),
            line_num: 0,
            obj_num: 0,
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

pub async fn import_source(store: &Arc<DataStore>, name: &String, file: String, cache_dir: String) {
    match Url::parse(&file).unwrap().scheme() {
        "ftp" => warn!("FTP data source not implemented yet. Skipping"),
        "http" | "https" => {
            // TODO: Implement stream-parsing without cache
            let filename = hash_filename(&file, &name, 1234);
            println!("{}", filename.clone());
            let path = cache_dir + "/" + &filename;
            info!("Downloading {} to cache: {}", &file, &path);
            let path = cache_http(file.clone(), path.clone()).await;
            info!("Importing downloaded file {} from {}", &file, &path);
            let _ = store.import_objects(&name, RpslParser::new_from_file(path.clone()));
            info!("Done importing {} from {}", file.clone(), &path);
        }
        _ => {
            info!("Importing local file {}", &file);
            let _ = store.import_objects(&name, RpslParser::new_from_file(file.clone()));
            info!("Done importing {}", file.clone());
        }
    }
}
