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
struct DataSources {
    name: String,
    as_sets: HashMap<String, AsSet>,
    routes: HashMap<String, ASN>,
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
struct ASN {
    prefixes: Vec<String>,
}

static ASN_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^AS[0-9]+$").unwrap());
static ASSET_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^(.*[:]+)?AS-|as-[a-zA-Z][^ ]*$").unwrap());

impl DataSources {
    
    fn new(name: String) -> Self {
        Self {
            name: name,
            as_sets: HashMap::new(),
            routes: HashMap::new(),
        }
    }

    fn import_from_file(&mut self, filename: String) -> Result<(), String> {
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
                        self.as_sets.insert(obj_name, AsSet { asns: asns, as_sets: assets });
                    }
                    "route" | "route6" => {
                        let origins = result.get("origin");
                        for i in origins {
                            self.routes.entry(i.to_uppercase().to_string()).and_modify(|asn| asn.prefixes.push(obj_name.to_string())).or_insert(ASN { prefixes: vec![obj_name.to_string()] });
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
        println!("Processed {} in {}ms", filename.clone(), old_time.elapsed().as_millis());
        Ok(())
    }


    fn _is_asn(v: &str) -> bool {
        ASN_REGEX.is_match(v)
    }

    fn _is_as_set(v: &str) -> bool {
        // TODO: Compile regex once
        ASSET_REGEX.is_match(v)
    }


    fn query_asn(&self, asn: String) -> Option<&ASN> {
        self.routes.get(&asn)
    }
    
    fn query_as_set(&self, as_set: String) -> Option<&AsSet> {
        self.as_sets.get(&as_set.to_uppercase())
    }

    fn query_as_set_recursive(&self, as_set: String, depth: u32) -> Option<Vec<String>> {
        self._query_as_set_recursive(as_set, depth, &mut vec![String::from("AS-LOREMIPSUM")])
    }

    fn _query_as_set_recursive(&self, as_set: String, depth: u32, ignore_as_sets: &mut Vec<String>) -> Option<Vec<String>> {
        if depth == 0 || ignore_as_sets.contains(&as_set) {
            return None;
        }
        ignore_as_sets.push(as_set.clone());

        // TODO: yeah, uh
        let dummy = AsSet::new();

        let res = self.query_as_set(as_set).unwrap_or(&dummy);
        let mut as_list = res.asns.clone();
        
        for a in res.as_sets.clone() {
            as_list.append(&mut self._query_as_set_recursive(a, depth - 1, ignore_as_sets).unwrap_or(vec![]));
        }

        Some(as_list)
    }

    fn query_as_set_prefixes_recursive(&self, as_set: String, depth: u32) -> Option<Vec<String>> {
        let as_list = self.query_as_set_recursive(as_set, depth).unwrap();
        let mut prefixes: Vec<String> = vec![];

        for asn in as_list {
            prefixes.append(&mut self.query_asn(asn).unwrap_or(&ASN { prefixes: vec![] }).prefixes.clone());
        }

        Some(prefixes)
    }

}

fn initData() -> DataSources {
    println!("Hello, poof!");
    println!("initializing data..");

    let mut ripe = DataSources::new(String::from("RIPE"));
    let _ = ripe.import_from_file(String::from("data/ripe.db"));
    
    println!("Ready to serve your requests!");
    
    return ripe;
}

async fn get_route_from_as_set(name: String, data: DataSources) -> Result<impl warp::Reply, warp::Rejection> {
    let old_time = Instant::now();
    if let Some(value) = data.query_as_set_prefixes_recursive(name.to_string(), 10) {
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

async fn get_asn_from_as_set(name: String, data: DataSources) -> Result<impl warp::Reply, warp::Rejection> {
    let old_time = Instant::now();
    if let Some(value) = data.query_as_set_recursive(name.to_string(), 10) {
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

    let mut data = task::spawn_blocking(|| {initData()}).await.unwrap();

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
