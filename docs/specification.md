# Specification

The purpose of routefilterd is to combine multiple datasources to generate filters for global routing use cases.

## Data sources:
- RPSL
  - Database dumps
  - NRTM feed
- RPKI (TODO spec)

## Data ingest

### RPSL

The RPSL datasources can be the RIR databases and other IRR providers like RADB.
The datasources are provided upon start of the daemon in a config file.
Data serial and a priority are defined along the datastore.
A datasource can contain multiple source dumps, which are provided from one of these sources:

- Local file
- HTTP
- HTTPS
- FTP

Provided files can be either plain-text or gzipped plain-text.
Remote paths are fetched to a cache directory for the following reasons:

- Prevent processing backpressure to the data source
- Allow reuse of cached files between restarts if serial hasn't changed

#### Route(6) Objects

Parsed fields:
- route/route6
  - the prefix (example: 1.1.1.0/24)
- origin
  - ASN that's allowed to announce the prefix (example: AS65000)

#### AS-Set

Parsed fields:
- `as-set`
  - Name of the as-set (Examples: AS-EXAMPLE, AS65000:AS-EXAMPLE)
- `members`
  - Either ASNs (AS65000) or AS-Sets (AS-EXAMPLE, AS65000:AS-EXAMPLE, RIPE::AS-EXAMPLE, etc.)
  - Multiple `members` fields allowed per AS-Set
  - Multiple entries per field allowed (comma or space separated)

## Data normalization

- Comments are removed from the end of lines
- Empty fields are ignored
- Trailing and following spaces are ignored
- IP Prefixes are normalized
- AS-Sets are uppercased
- ASNs are normalized to AS[0-9]+
- Non-Public prefixes and ASNs are removed (TODO: Add reference to lists)

## Data processing

Data is ingested into the in-memory datastore that stores the following data per data source:

- Datasource Meta
  - name
  - serial
  - priority
- HashMap of Routes
  - List of ASNs
- HashMap of AS-Sets
  - List of ASNs
  - List of AS-Sets

## Data retrieval

[TODO: Embed flowchart diagrams]

## Performance
