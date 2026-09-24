use futures_util::Stream;
use futures_util::StreamExt;
use log::trace;
use regex::Regex;
use rpsl::parse_object;
use std::collections::HashMap;
use std::sync::LazyLock;
use std::sync::{Arc, Mutex};

static ASN_REGEX: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^AS[0-9]+$").unwrap());
static ASSET_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^([A-Z]+::)?(AS[0-9]+[:]+)?AS-[A-Z0-9-]+$").unwrap());

pub struct DataStore {
    pub datasources: std::sync::Mutex<HashMap<String, DataSource>>,
    as_sets: std::sync::Mutex<HashMap<(String, String), AsSet>>,
    as_routes: std::sync::Mutex<HashMap<(String, String), AsRoutes>>,
}

#[derive(Clone, Default)]
pub struct DataSource {
    pub current_serial: u64,
    pub priority: i64,
}

#[derive(Debug, Clone)]
pub struct AsSet {
    pub asns: Vec<String>,
    pub as_sets: Vec<String>,
}

impl AsSet {
    fn new() -> Self {
        Self {
            asns: Vec::new(),
            as_sets: Vec::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct AsRoutes {
    pub(crate) prefixes: Vec<String>,
}

impl Default for DataStore {
    fn default() -> Self {
        Self::new()
    }
}

impl DataStore {
    pub fn new() -> Self {
        Self {
            datasources: Mutex::new(HashMap::new()),
            as_sets: Mutex::new(HashMap::new()),
            as_routes: Mutex::new(HashMap::new()),
        }
    }

    pub fn new_data_source(self: &Arc<Self>, name: String, serial: u64, priority: i64) {
        self.datasources.lock().unwrap().insert(
            name,
            DataSource {
                current_serial: serial,
                priority,
            },
        );
    }

    fn get_sorted_data_sources(&self, exclude: &[String]) -> Vec<String> {
        // TODO: Data source sorting
        let mut d = self
            .datasources
            .lock()
            .unwrap()
            .clone()
            .into_iter()
            .filter(|(s, _)| !exclude.contains(s)) // drop excluded
            .collect::<Vec<(String, DataSource)>>();
        d.sort_by_key(|ds| std::cmp::Reverse(ds.1.priority)); // sort by prio
        d.into_iter().map(|(s, _)| s).collect() // return keys
    }

    pub async fn import_objects(
        self: &Arc<Self>,
        data_source: &str,
        mut objects: impl Stream<Item = std::io::Result<String>> + Unpin,
    ) -> Result<(), String> {
        while let Some(object) = objects
            .next()
            .await
            .transpose()
            .map_err(|e| e.to_string())?
        {
            let arc_cloned = self.clone();
            let source_cloned = data_source.to_owned();
            arc_cloned.import_object(source_cloned, object)?;
        }
        Ok(())
    }

    fn clean_string(text: &str) -> String {
        let mut cleaned_text = text;

        // remove everything after #
        if cleaned_text.contains("#") {
            (cleaned_text, _) = cleaned_text.split_once("#").unwrap();
        }

        // Remove whitespace on both ends and uppercase
        cleaned_text.trim().to_uppercase()
    }

    fn parse_members(members: Vec<&str>) -> (Vec<String>, Vec<String>) {
        // TODO: Normalize AS-Set Data
        // - casing (Done)
        // - IRR:: prefixes
        // - on/two colons
        // - comma separated values (Done)
        // - comments after entries (see: AS57555:AS-MEMBERS) (Done)

        let mut asns = Vec::new();
        let mut assets = Vec::new();

        for m in members {
            if m.contains(",") {
                // Comma separated values
                let (collected_asns, collected_assets) =
                    Self::parse_members(m.split(",").collect());
                asns.extend(collected_asns);
                assets.extend(collected_assets);
                continue;
            }

            let clean_string = Self::clean_string(m);

            if ASN_REGEX.is_match(&clean_string) {
                // Regular ASN
                asns.push(clean_string);
                continue;
            }
            if ASSET_REGEX.is_match(&clean_string) {
                // Regular AS-Sets
                assets.push(clean_string);
                continue;
            }
        }
        asns.sort();
        asns.dedup();
        assets.sort();
        assets.dedup();

        (asns, assets)
    }

    pub fn import_object(&self, data_source: String, object_buf: String) -> Result<(), String> {
        let parsed = parse_object(&object_buf);
        if let Err(_err) = parsed {
            return Err(format!("Error parsing obj: {:?}, {:?}", object_buf, _err));
        }
        let result = parsed.unwrap();
        let obj_type = result[0].name.to_string();
        let obj_name_content = result[0].value.with_content();
        if obj_name_content.is_empty() {
            trace!("Skipped object type {} and no name", obj_type);
            return Err("".to_string());
        }
        let obj_name = Self::clean_string(obj_name_content[0]);

        match obj_type.as_str() {
            "as-set" => {
                //trace!("Installed #{} {}: {}", obj_num, obj_type, obj_name);
                let (asns, assets) = Self::parse_members(result.get("members"));
                self.as_sets.lock().unwrap().insert(
                    (data_source.clone(), obj_name),
                    AsSet {
                        asns,
                        as_sets: assets,
                    },
                );
            }
            "route" | "route6" => {
                let origins = result.get("origin");
                for i in origins {
                    self.as_routes
                        .lock()
                        .unwrap()
                        .entry((data_source.clone(), i.to_uppercase().to_string()))
                        .and_modify(|asn| asn.prefixes.push(obj_name.to_string()))
                        .or_insert(AsRoutes {
                            prefixes: vec![obj_name.to_string()],
                        });
                    trace!("Installed {}: {} in {}", obj_type, obj_name, i);
                }
            }
            _ => {
                trace!("Skipped object {} and name {}", obj_type, obj_name);
            }
        }
        Ok(())
    }

    pub fn query_asn(&self, data_sources: Vec<String>, asn: String) -> Option<AsRoutes> {
        for data_source in data_sources {
            if let Some(res) = self
                .as_routes
                .lock()
                .unwrap()
                .get(&(data_source, asn.clone()))
            {
                return Some(res.clone());
            }
        }
        None
    }

    pub fn query_as_set(
        &self,
        data_sources: Vec<String>,
        as_set: String,
        ignore_datasource: &[String],
    ) -> Option<AsSet> {
        let datasources: Vec<String> = if data_sources.clone().is_empty() {
            self.get_sorted_data_sources(ignore_datasource)
        } else {
            data_sources
                .into_iter()
                .filter(|s| !&ignore_datasource.contains(s)) // drop excluded
                .collect()
        };
        for data_source in datasources {
            if let Some(res) = self
                .as_sets
                .lock()
                .unwrap()
                .get(&(data_source, as_set.to_uppercase()))
            {
                return Some(res.clone());
            }
        }
        None
    }

    pub fn query_as_set_recursive(
        &self,
        as_set: String,
        depth: u32,
        ignore_as_sets: Vec<String>,
        ignore_datasource: &Vec<String>,
    ) -> Option<Vec<String>> {
        self._query_as_set_recursive(
            as_set,
            depth,
            &mut ignore_as_sets.clone(),
            ignore_datasource,
        )
    }

    pub fn _query_as_set_recursive(
        &self,
        as_set: String,
        depth: u32,
        ignore_as_sets: &mut Vec<String>,
        ignore_datasource: &Vec<String>,
    ) -> Option<Vec<String>> {
        if ignore_as_sets.contains(&as_set) {
            return Some(vec![]);
        }
        ignore_as_sets.push(as_set.clone());

        // TODO: yeah, uh
        let dummy = AsSet::new();
        let data_sources = self.get_sorted_data_sources(ignore_datasource);

        let res = self
            .query_as_set(data_sources, as_set, &Vec::new())
            .unwrap_or(dummy);
        let mut as_list = res.asns.clone();

        if depth > 0 {
            for a in res.as_sets.clone() {
                as_list.append(
                    &mut self
                        ._query_as_set_recursive(
                            a,
                            depth - 1,
                            &mut ignore_as_sets.clone(),
                            ignore_datasource,
                        )
                        .unwrap_or(vec![]),
                );
            }
        }

        Some(as_list)
    }

    pub fn query_as_set_prefixes_recursive(
        &self,
        as_set: String,
        depth: u32,
        ignore_as_sets: Vec<String>,
        ignore_datasource: &Vec<String>,
    ) -> Option<Vec<String>> {
        // TODO: add ignore_as_sets
        let as_list = self
            .query_as_set_recursive(as_set, depth, ignore_as_sets.clone(), ignore_datasource)
            .unwrap();
        let mut prefixes: Vec<String> = vec![];
        let data_sources = self.get_sorted_data_sources(ignore_datasource);

        for asn in as_list {
            prefixes.append(
                &mut self
                    .query_asn(data_sources.clone(), asn)
                    .unwrap_or(AsRoutes { prefixes: vec![] })
                    .prefixes
                    .clone(),
            );
        }

        Some(prefixes)
    }
}
