use super::super::config::parse_config;
use const_format::formatcp;
use std::io::Write;
use tempfile::NamedTempFile;

const API_LISTEN_ADDRESS: &str = "[::]:1337";
const DEFAULT_RECURSION_DEPTH: &str = "0";
const LOG_LEVEL: &str = "debug";
const CACHE_DIR: &str = "cache";
const RIPE_PRIORITY: &str = "500";
const RIPE_DB_URI: &str = "https://ftp.ripe.net/ripe/dbase/ripe.db.gz";
const RIPE_SERIAL_URI: &str = "https://ftp.ripe.net/ripe/dbase/RIPE.CURRENTSERIAL";
const RIPE_NRTM_HOST: &str = "whois.ripe.net:4444";
const TEST_CORRECT_TOML: &str = formatcp!(
    r#"
log_level = "{log_level}"
cache_dir = "{cache_dir}"

[api]
listen_address = "{listen_address}"
default_recursion_depth = {default_recursion_depth}

[data_sources.RIPE]
import_sources = ["{ripe_db}"]
import_serial = "{ripe_serial}"
nrtm_host = "{ripe_nrtm_host}"
priority = {ripe_priority}
"#,
    listen_address = API_LISTEN_ADDRESS,
    default_recursion_depth = DEFAULT_RECURSION_DEPTH,
    log_level = LOG_LEVEL,
    cache_dir = CACHE_DIR,
    ripe_priority = RIPE_PRIORITY,
    ripe_db = RIPE_DB_URI,
    ripe_nrtm_host = RIPE_NRTM_HOST,
    ripe_serial = RIPE_SERIAL_URI,
);

const TEST_INCORRECT_TOML: &str = r#"
log_level = 0
cache_dir = 3

[api]
listen_address = "504.502.403.404"
default_recursion_depth = "vier"

[data_sources.INVALID]
import_sources = "none"
priority = -2
"#;

#[test]
fn test_load_correct_toml() {
    let mut temp = NamedTempFile::new().unwrap();
    temp.write_all(TEST_CORRECT_TOML.as_bytes()).unwrap();

    let config = parse_config(String::from(temp.path().to_str().unwrap()));

    assert_eq!(config.api.listen_address, API_LISTEN_ADDRESS);
    assert_eq!(
        config.api.default_recursion_depth.to_string(),
        DEFAULT_RECURSION_DEPTH
    );
    assert_eq!(config.log_level, LOG_LEVEL);
    assert_eq!(config.cache_dir, CACHE_DIR);

    let mut datasources_iter = config.data_sources.iter();
    let (datasource_ripe_name, datasource_ripe_data) = datasources_iter.next().unwrap();
    assert_eq!(datasource_ripe_name, "RIPE");
    assert_eq!(datasource_ripe_data.import_sources, vec![RIPE_DB_URI]);
    assert_eq!(datasource_ripe_data.priority.to_string(), RIPE_PRIORITY);
}

#[test]
#[should_panic(expected = "invalid type")]
fn test_load_incorrect_toml() {
    let mut temp = NamedTempFile::new().unwrap();
    temp.write_all(TEST_INCORRECT_TOML.as_bytes()).unwrap();

    let _config = parse_config(String::from(temp.path().to_str().unwrap()));
}
