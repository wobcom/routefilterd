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

### To do
- [x] Import RPSL dumps
- [x] HTTP API
- [x] Recursive AS-Set resolution
- [x] Recursive Route resolution
- [ ] Download RPSL Dumps
- [ ] NRTM ingest
- [ ] Tests
- [ ] Document and compare AS-Set Resolution to other tools
