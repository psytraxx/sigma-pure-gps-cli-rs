# Changelog

## Unreleased

### Added
- `download-tracks` subcommand — reads all recorded tracks from device flash and saves them as GPX 1.1 files (one per track)
- `src/decoder.rs` — decodes 65-byte log headers and 25/32-byte log entries; ported from `Gps10Decoder.as`
- `src/gpx.rs` — GPX 1.1 writer with elevation, timestamps, speed and temperature extensions
- `src/protocol/commands.rs` — `CMD_GET_LOG_HEADER_COUNT`, `LOG_HEADER_END`, `build_flash_read_cmd`
- `src/protocol/mod.rs` — `get_log_header_count`, `get_log_headers`, `get_log_data`

### Changed
- `src/protocol/` is now a module directory (`mod.rs` + `commands.rs`) instead of a single `protocol.rs`
- No default subcommand — a subcommand is now required (previously `update` ran by default)
- CLI description updated to reflect the broader feature set

### Documentation
- [`docs/protocol.md`](docs/protocol.md) — comprehensive USB protocol reference reverse-engineered from `DataCenter_Desktop.swf`
- [`CLAUDE.md`](CLAUDE.md) — updated with new modules, commands, and post-change checklist

## Restructure

### Changed
- Each subcommand is now its own file under `src/commands/` — adding a new command means adding one file and two lines in `main.rs`
- `src/util.rs` added for shared `resolve_port` and `build_http_client` helpers
- `src/main.rs` reduced to arg parsing and dispatch only
