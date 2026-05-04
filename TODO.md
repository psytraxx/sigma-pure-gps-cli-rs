# TODO

Feature gap analysis vs. the original Adobe AIR DataCenter application.

| # | Feature | AIR app | CLI |
|---|---------|:-------:|:---:|
| 1 | Query device info (serial, firmware) | ✅ | ✅ `info` |
| 2 | Upload AGPS data to device | ✅ | ✅ `update` |
| 3 | Download AGPS data to file (no device) | ✅ | ✅ `download-agps` |
| 4 | Download activity log headers | ✅ | ✅ `download-tracks` |
| 5 | Download activity log data (GPS points) | ✅ | ✅ `download-tracks` / `download-tracks-raw` |
| 6 | Save tracks as GPX files | ✅ | ✅ `download-tracks` (DEM-corrected) / `download-tracks-raw` (barometric) |
| 7 | Check AGPS data sync date on device | ✅ | ✅ `agps-date` |
| 8 | Read device settings (timezone, language, units, contrast, …) | ✅ | ✅ `get-settings` |
| 9 | Write device settings (timezone, language, speed/temp/altitude units, date format, contrast, system tone, NFC, auto-pause, auto-lap distance, user name) | ✅ | ❌ |
| 10 | Set altitude reference (actual altitude / sea level pressure) | ✅ | ❌ |
| 11 | Set home altitude 1 & 2 | ✅ | ✅ `set-home-altitude` |
| 12 | Read cumulative totals (total distance, training time, calories, climb) | ✅ | ✅ `get-totals` |
| 13 | Write cumulative totals back to device | ✅ | ❌ |
| 14 | Configure sleep screen / watch face (16×59 px bitmap, clock & name position) | ✅ | ❌ |
| 15 | Write point navigation / waypoints (named GPS waypoints) | ✅ | ❌ |
| 16 | Delete all activity memory on device | ✅ | ✅ `delete-tracks` |
| 17 | List serial ports | — | ✅ `list-ports` |

## Not yet implemented (priority order)

- [x] **`get-settings`** — read EEPROM offset 272 (32 bytes) and print all device settings
- [ ] **`set-settings`** — write settings back (timezone, language, units, contrast, auto-pause, auto-lap, user name, …); see `Gps10Decoder.as` `decodeSettings` / `encodeSettings`
- [x] **`get-totals`** — read cumulative totals from EEPROM offset 304 (20 bytes)
- [ ] **`set-totals`** — write cumulative totals back to device
- [x] **`delete-tracks`** — erase all activity log data from device flash (UPDATE_FLAG_TRIP_DATA_RESET = 4)
- [x] **`agps-date`** — read AGPS last-sync date from device; see `AgpsLoader.as` `decodeAgpsOfflineDataUploadDate`
- [ ] **`set-waypoints`** — upload up to N named waypoints (EEPROM offset 336, 27 bytes each); see `Gps10Decoder.as` `encodePointNavigation`
- [ ] **`set-sleep-screen`** — upload custom watch face bitmap (EEPROM offset 96, 172 bytes); see `Gps10Decoder.as` `encodeSleepScreen`
- [x] **`set-home-altitude`** — set home altitude 1 and/or 2; encoding: raw = altitude_m × 10 + 10000 (16-bit LE) at settings offset +7/+9; UPDATE_FLAG_SETTINGS=16
- [ ] **`set-altitude`** — set actual altitude or sea level pressure reference on device

## AS3 reference locations

| Topic | File | Function |
|-------|------|----------|
| Settings decode/encode | `decoder/Gps10Decoder.as` | `decodeSettings`, `encodeSettings` |
| Totals decode/encode | `decoder/Gps10Decoder.as` | `decodeTotalValues`, `encodeTotalValues` |
| Sleep screen encode | `decoder/Gps10Decoder.as` | `encodeSleepScreen` |
| Waypoints encode | `decoder/Gps10Decoder.as` | `encodePointNavigation` |
| AGPS date decode | `core/agps/AgpsLoader.as` | `decodeAgpsOfflineDataUploadDate` |
| Delete memory | `handler/Gps10Handler.as` | UPDATE_FLAG_TRIP_DATA_RESET (4) |
| EEPROM write command | `handler/Gps10Handler.as` | CMD_SEND_EEPROM = `52 12 0 0 0 0 4 0 0 0 0 98` |
