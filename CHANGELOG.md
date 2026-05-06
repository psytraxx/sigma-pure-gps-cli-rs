# Changelog

## Unreleased

### Added
- `get-sleep-screen` subcommand — reads the 172-byte sleep screen block from EEPROM offset 96, decodes the 16×59 pixel watch face bitmap and metadata (clock position, name position), and saves it as a 1-bit grayscale PNG with `clock_x`, `clock_y`, and `name_pos` `tEXt` metadata chunks; the PNG can be edited in any image editor and later uploaded back to the device
- `decode_sleep_screen` in `src/decoder.rs` — decodes the 172-byte EEPROM block into a `SleepScreen` struct; verifies the CRC (seed=1, over bytes 0–169, stored at byte 171)
- `sleep_screen_to_png` / `sleep_screen_from_png` in `src/decoder.rs` — PNG round-trip encode/decode; bit order is reversed per byte (device: LSB-first, PNG: MSB-first)
- `png` crate dependency (`0.17`) for 1-bit grayscale PNG with `tEXt` chunk support
- `GpxMeta` struct in `src/gpx.rs` — decouples track metadata from `LogHeader`; enables the GPX writer to carry summary stats independent of the data source

### Changed
- GPX output now includes a `<desc>` element on each track with summary stats: distance, duration, average speed, max speed, and calories
- GPX writer switched from manual string building to `quick-xml` for correct XML escaping of all text content
- `write_gpx` and `track_filename` now accept `&GpxMeta` instead of `&LogHeader`
- GPX creator tag corrected from `sigma-pure-gps-updater` to `sigma-pure-gps-cli`

## [0.2.0]

### Added
- `delete-tracks` subcommand — permanently erases all activity log data from the device; prompts for confirmation before proceeding; writes `UPDATE_FLAG_TRIP_DATA_RESET` (flag=4) update flags `[0, 6, 1, 8]` to EEPROM offset 80 and uploads the full 1024-byte EEPROM image via `CMD_SEND_EEPROM`
- `CMD_SEND_EEPROM` protocol constant (`0x52 0x0C ...`) and `write_eeprom` / `delete_tracks_memory` functions in `src/protocol/`
- `set-home-altitude` subcommand — sets home altitude 1 (`--alt1`) and/or home altitude 2 (`--alt2`) in metres on the device; patches the settings block (EEPROM offset 272, bytes +7/+9) with `raw = altitude_m × 10 + 10000` and writes the full EEPROM with `UPDATE_FLAG_SETTINGS=16`

## [0.1.0]

### Added
- `download-tracks-raw` subcommand — downloads recorded tracks with raw barometric elevation (no correction); shares device I/O with `download-tracks` via `download_from_device()`
- `download-tracks` now corrects elevation via Sigma's elevation service (`elevation.sigma-dc-control.com`) — single POST with all coordinates as a GeoJSON `LineString`, response provides elevation in mm
- `agps-date` subcommand — reads the date of AGPS data stored on the device from flash address 0x1000; command sends `len-1=14`, response is 21 bytes; date decoded from payload bytes 10–12 (year+2000, month, day; ported from `AgpsLoader.decodeAgpsOfflineDataUploadDate`)
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
