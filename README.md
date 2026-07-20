# routefilterd

routerfilterd is a tool to generate filters for use in BGP routing.
It consumes RPSL feeds from IRR databases and allows to query the data via HTTP.

⚠️ The project is still under heavy development. Production use is not recommended. ⚠️

The tool is written in Rust and performance optimized. There is no database - all state is held in Memory. Data population is fast enough to perform a full ingest on startup. On my laptop it takes less than 30 seconds to ingest over 6 Gb of IRR data. The typical request to recurse a medium-sized AS-SET takes less than 5 ms to return.

### Advantages
- Small project scope
- Extremely fast
- Low system requirements
- No persistent state

### Example config
config.toml
```toml
log_level = "info"
cache_dir = "cache"

[api]
listen_address = "[::]:1337"
default_recursion_depth = 30

[data_sources.RIPE]
import_sources = ["https://ftp.ripe.net/ripe/dbase/ripe.db.gz"]]
priority = 500

[data_sources.ARIN]
import_sources = ["data/arin.db"]
priority = 400
```

## Authors
- Fiona Weber
- Lou Lecrivain

## License
See LICENSE

## Contact
You can contact the team behind this project by emailing: wan@wobcom.de
