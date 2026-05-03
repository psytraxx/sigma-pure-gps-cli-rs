# Changelog

## Unreleased

### Added
- `get-totals` subcommand — reads cumulative totals from EEPROM offset 304 (total distance, training time, calories, climb, reset date); distance raw = mm → /1e6 = km; climb raw = mm/100 → /10000 = m; time raw × 1000 = ms
- `get-settings` subcommand — reads device settings from EEPROM offset 272 and prints all fields (timezone, language, units, contrast, NFC, auto-pause, auto-lap distance, name, altitude/sea-level references); timezone displayed as named GMT offset using the GPS10 lookup table from `CommonTimeZoneDataProvider.as`
- `download-tracks` subcommand — reads all recorded tracks from device flash and saves them as GPX 1.1 files (one per track)
- `src/decoder.rs` — decodes 65-byte log headers and 25/32-byte log entries; ported from `Gps10Decoder.as`
- `src/gpx.rs` — GPX 1.1 writer with elevation, timestamps, speed and temperature extensions

### Changed
- Renamed `show-unit-info` subcommand to `info`
- Renamed `download` subcommand to `download-agps`
- `src/protocol/` is now a module directory (`mod.rs` + `commands.rs`) instead of a single `protocol.rs`
- No default subcommand — a subcommand is now required (previously `update` ran by default)

### Fixed
- `download-tracks` log headers read from wrong flash address — base address is `0x1FDFFF`, not `0x1FFFFF`
- `download-tracks` flash-read command must send `len-1` as the length field; outer response frame validated with checksum seed 0
- `download-tracks` GPS coordinates decoded incorrectly — direction bits for both axes share byte 13 (`bit4` = lat, `bit5` = lon); longitude minutes high-nibble must come from byte 17, not the flags byte
- `update` and `info` commands hung requiring USB reconnect — `CMD_CHECK_CONNECTED` (`0xF4`) must not be sent manually; removed entirely
- `update` timed out on AGPS upload — device requires `CMD_LOAD_UNIT_INFO` → `CMD_GET_COMPLETE_EEPROM` handshake before accepting AGPS data
- `info` serial number and firmware version displayed garbled — serial is a 6-byte little-endian integer; firmware byte is hex-formatted then parsed as decimal (e.g. `0x42` → `4.2`)

### Documentation
- [`docs/protocol.md`](docs/protocol.md) — USB protocol reference reverse-engineered from `DataCenter_Desktop.swf`
- [`CLAUDE.md`](CLAUDE.md) — updated with new modules, commands, and post-change checklist

## Restructure

### Changed
- Each subcommand is now its own file under `src/commands/`
- `src/util.rs` added for shared `resolve_port` and `build_http_client` helpers
- `src/main.rs` reduced to arg parsing and dispatch only
