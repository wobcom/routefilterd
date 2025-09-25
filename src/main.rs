use std::fs::File;
use std::io::BufReader;
use std::io::BufRead;
use rpsl::parse_object;
use std::collections::HashMap;
use regex::Regex;
use std::sync::LazyLock;
use std::time::Instant;
use warp::Filter;
use tokio::task;
use http::StatusCode;

#[derive(Clone)]
struct PoofData {
    datasources: HashMap<String, DataSource>,
    as_sets: HashMap<(String, String), AsSet>,
    as_routes: HashMap<(String, String), AsRoutes>,
}

#[derive(Clone)]
struct DataSource {
    serial: u64,
    priority: i64,
}

#[derive(Debug)]
#[derive(Clone)]
struct AsSet {
    asns: Vec<String>,
    as_sets: Vec<String>,
}

impl AsSet {
    fn new() -> Self {
        Self {
            asns: Vec::new(),
            as_sets: Vec::new(),
        }
    }
}

#[derive(Debug)]
#[derive(Clone)]
struct AsRoutes {
    prefixes: Vec<String>,
}

static ASN_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^AS[0-9]+$").unwrap());
static ASSET_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(.*[:]+)?AS-|as-[a-zA-Z][^ ]*$").unwrap());

impl PoofData {
    fn new() -> Self {
        Self {
            datasources: HashMap::new(),
            as_sets: HashMap::new(),
            as_routes: HashMap::new(),
        }
    }

    fn newDataSource(&mut self, name: String, serial: u64, priority: i64) {
        self.datasources.insert(
            name,
            DataSource {
                serial: serial,
                priority: priority,
            },
        );
    }

    fn getSortedDataSources(&self, exclude: Vec<String>) -> Vec<String> {
        // TODO: Data source sorting
        self.datasources.iter().filter(|(s, _)| !exclude.contains(s)).map(|(s, _)| s.clone()).collect()
    }

    fn import_from_file(&mut self, data_source: String, filename: String) -> Result<(), String> {
        let file = File::open(filename.clone()).expect("Couldn't open file");
        let reader = BufReader::new(file);

        let old_time = Instant::now();

        let mut object_buf = String::new();
        let mut line_num = 0;
        let mut obj_num = 0;

        for line in reader.lines() {
            if obj_num > 100000 { // DEBUG
                //break;
            }
            let l = line.unwrap_or_else(|err| {
                //println!("Error encountered in line {}: {}", line_num, err);
                "".to_string()
            });
            line_num = line_num + 1;
            if l.starts_with("#") || (l.eq("") && object_buf.eq("")) {
                continue;
            }
            object_buf.push_str(&(l.clone() + "\n"));
            if l.eq("") {
                let parsed = parse_object(&object_buf);
                if let Err(err) = parsed {
                    //println!("Error parsing obj {} in line {}", obj_num, line_num);
                    object_buf.clear();
                    continue;
                }
                let result = parsed.unwrap();
                let obj_type = result[0].name.to_string();
                let obj_name_content = result[0].value.with_content();
                if obj_name_content.len() == 0 {
                    //println!("Skipped object {} of type {} and no name", obj_num, obj_type);
                    object_buf.clear();
                    continue;
                }
                let obj_name = obj_name_content[0].to_uppercase().to_string();

                match obj_type.as_str() {
                    "as-set" => {
                        //println!("Installed #{} {}: {}", obj_num, obj_type, obj_name);
                        let members = result.get("members");
                        let asns: Vec<String> = members.clone().into_iter().filter(|i| Self::_is_asn(i)).map(|s| s.to_uppercase().to_string()).collect();
                        let assets: Vec<String> = members.clone().into_iter().filter(|i| Self::_is_as_set(i)).map(|s| s.to_uppercase().to_string()).collect();
                        self.as_sets.insert((data_source.clone(), obj_name), AsSet { asns: asns, as_sets: assets });
                        // TODO: Normalize AS-Set Data further (casing, IRR:: prefixes, on/two colons, etc.)
                    }
                    "route" | "route6" => {
                        let origins = result.get("origin");
                        for i in origins {
                            self.as_routes.entry((data_source.clone(), i.to_uppercase().to_string())).and_modify(|asn| asn.prefixes.push(obj_name.to_string())).or_insert(AsRoutes { prefixes: vec![obj_name.to_string()] });
                            //println!("Installed #{} {}: {} in {}", obj_num, obj_type, obj_name, i);
                        }
                    }
                    _ => {
                        //println!("Skipped object {} of type {} and name {}", obj_num, obj_type, obj_name);
                    }
                }
                //println!("{:#?}", parsed);

                obj_num = obj_num + 1;
                object_buf.clear();
            }
        }

        println!("Successfully parsed {} lines into {} objects.", line_num, obj_num);
        println!("Processed {} in {}ms", filename, old_time.elapsed().as_millis());
        Ok(())
    }

    pub fn _is_asn(v: &str) -> bool {
        ASN_REGEX.is_match(v)
    }

    pub fn _is_as_set(v: &str) -> bool {
        // TODO: Compile regex once
        ASSET_REGEX.is_match(v)
    }

    pub fn query_asn(&self, data_sources: Vec<String>, asn: String) -> Option<&AsRoutes> {
        for data_source in data_sources {
            if let Some(res) = self.as_routes.get(&(data_source, asn.clone())) {
                return Some(res);
            }
        }
        return None;
    }
    
    pub fn query_as_set(&self, data_sources: Vec<String>, as_set: String) -> Option<&AsSet> {
        for data_source in data_sources {
            if let Some(res) = self.as_sets.get(&(data_source, as_set.to_uppercase())) {
                return Some(res);
            }
        }
        return None;
    }

    pub fn query_as_set_recursive(&self, as_set: String, depth: u32) -> Option<Vec<String>> {
        self._query_as_set_recursive(as_set, depth, &mut vec![String::from("AS-LOREMIPSUM")])
    }

    pub fn _query_as_set_recursive(&self, as_set: String, depth: u32, ignore_as_sets: &mut Vec<String>) -> Option<Vec<String>> {
        if depth == 0 || ignore_as_sets.contains(&as_set) {
            return None;
        }
        ignore_as_sets.push(as_set.clone());

        // TODO: yeah, uh
        let dummy = AsSet::new();
        let data_sources = self.getSortedDataSources(vec![]);

        let res = self.query_as_set(data_sources, as_set).unwrap_or(&dummy);
        let mut as_list = res.asns.clone();
        
        for a in res.as_sets.clone() {
            as_list.append(&mut self._query_as_set_recursive(a, depth - 1, ignore_as_sets).unwrap_or(vec![]));
        }

        Some(as_list)
    }

    pub fn query_as_set_prefixes_recursive(&self, as_set: String, depth: u32) -> Option<Vec<String>> {
        let as_list = self.query_as_set_recursive(as_set, depth).unwrap();
        let mut prefixes: Vec<String> = vec![];
        let data_sources = self.getSortedDataSources(vec![]);

        for asn in as_list {
            prefixes.append(
                &mut self.query_asn(data_sources.clone(), asn)
                .unwrap_or(&AsRoutes { prefixes: vec![] })
                .prefixes
                .clone());
        }

        Some(prefixes)
    }

}

async fn get_route_from_as_set(name: String, data: PoofData) -> Result<impl warp::Reply, warp::Rejection> {
    let old_time = Instant::now();
    if let Some(value) = data.query_as_set_prefixes_recursive(name.to_string(), 25) {
        Ok(warp::reply::with_status(
            format!("# Recursed route resolution for '{}' in {}μs, {} items\n{:#?}", name, old_time.elapsed().as_micros(), value.len(), value),
            StatusCode::OK,
        ))
    } else {
        Ok(warp::reply::with_status(
            format!("Value for '{}' not found in cache.", name),
            StatusCode::NOT_FOUND,
        ))
    }
}

async fn get_asn_from_as_set(name: String, data: PoofData) -> Result<impl warp::Reply, warp::Rejection> {
    let old_time = Instant::now();
    if let Some(value) = data.query_as_set_recursive(name.to_string(), 25) {
        Ok(warp::reply::with_status(
            format!("# Recursed AS-Set resolution for '{}' in {}μs, {} items\n{:#?}", name, old_time.elapsed().as_micros(), value.len(), value),
            StatusCode::OK,
        ))
    } else {
        Ok(warp::reply::with_status(
            format!("Value for '{}' not found in cache.", name),
            StatusCode::NOT_FOUND,
        ))
    }

}

#[tokio::main]
async fn main() {
    println!("Hello, poof!");
    println!("initializing data..");


    let mut data = task::spawn_blocking(move || {
        let mut poof = PoofData::new();
        poof.newDataSource(String::from("RIPE"), 0, 100);
        let _ = poof.import_from_file(String::from("RIPE"), String::from("data/ripe.db"));
        poof.newDataSource(String::from("ARIN"), 0, 100);
        let _ = poof.import_from_file(String::from("ARIN"), String::from("data/arin.db"));
        poof.newDataSource(String::from("RADB"), 0, 100);
        let _ = poof.import_from_file(String::from("RADB"), String::from("data/radb.db"));
        poof.newDataSource(String::from("LACNIC"), 0, 100);
        let _ = poof.import_from_file(String::from("LACNIC"), String::from("data/lacnic.db"));
        poof.newDataSource(String::from("NTT"), 0, 100);
        let _ = poof.import_from_file(String::from("NTT"), String::from("data/nttcom.db"));
        poof.newDataSource(String::from("AFRINIC"), 0, 100);
        let _ = poof.import_from_file(String::from("AFRINIC"), String::from("data/afrinic.db"));
        poof.newDataSource(String::from("LEVEL3"), 0, 100);
        let _ = poof.import_from_file(String::from("LEVEL3"), String::from("data/level3.db"));
        poof.newDataSource(String::from("ALTDB"), 0, 100);
        let _ = poof.import_from_file(String::from("ALTDB"), String::from("data/altdb.db"));
        poof.newDataSource(String::from("IDNIC"), 0, 100);
        let _ = poof.import_from_file(String::from("IDNIC"), String::from("data/idnic.db"));
        poof.newDataSource(String::from("APNIC"), 0, 100);
        let _ = poof.import_from_file(String::from("APNIC"), String::from("data/apnic.db.route"));
        let _ = poof.import_from_file(String::from("APNIC"), String::from("data/apnic.db.route6"));
        let _ = poof.import_from_file(String::from("APNIC"), String::from("data/apnic.db.as-set"));

        println!("Ready to serve your requests!");
        poof
    }).await.unwrap();

    let data_moved = warp::any().map(move || data.clone());

    let routes = warp::get()
        .and(warp::path!("api" / "v1" / "asnsFromAsSet" / String))
        .and(data_moved.clone())
        .and_then(get_asn_from_as_set);
    let routes2 = warp::get()
        .and(warp::path!("api" / "v1" / "routesFromAsSet" / String))
        .and(data_moved.clone())
        .and_then(get_route_from_as_set);

    warp::serve(routes.or(routes2))
        .run(([127, 0, 0, 1], 3030))
        .await;
}
