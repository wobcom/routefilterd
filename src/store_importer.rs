use log::{info, trace};
use std::fs::File;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Lines;

pub struct RpslParser {
    reader: Lines<BufReader<File>>,
    line_num: u64,
    obj_num: u64,
}

impl RpslParser {
    pub fn new(file: File) -> Self {
        Self {
            reader: BufReader::new(file).lines(),
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
