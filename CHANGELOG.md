# Changelog

## Unreleased

### Added
- `download-tracks` subcommand — reads all recorded tracks from device flash and saves them as GPX 1.1 files (one per track)
- `src/decoder.rs` — decodes 65-byte log headers and 25/32-byte log entries; ported from `Gps10Decoder.as`
- `src/gpx.rs` — GPX 1.1 writer with elevation, timestamps, speed and temperature extensions
- `src/protocol/commands.rs` — `CMD_GET_LOG_HEADER_COUNT`, `LOG_HEADER_END`, `build_flash_read_cmd`
- `src/protocol/mod.rs` — `get_log_header_count`, `get_log_headers`, `get_log_data`

### Changed
- Renamed `show-unit-info` subcommand to `info`
- Renamed `download` subcommand to `download-agps`

### Fixed
- `update` and `show-unit-info` commands hung/timed out requiring USB reconnect — root cause was `CMD_CHECK_CONNECTED` (`0xF4`) being sent manually; in the original Flash app this command is handled asynchronously by the USB driver layer and must not be sent as part of a command sequence; removed `check_device_connected` entirely
- `update` command timed out on AGPS upload — device requires `CMD_LOAD_UNIT_INFO` → `CMD_GET_COMPLETE_EEPROM` before accepting `CMD_SEND_AGPS`, matching the original app's handshake; added both steps to the upload flow
- `show-unit-info` serial number and firmware version displayed garbled bytes — corrected decoding to match `Gps10Decoder.decodeInitialInformation`: serial is a 6-byte little-endian integer; firmware byte is formatted as hex then parsed as decimal (e.g. `0x42` → `"42"` → `4.2`)

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
