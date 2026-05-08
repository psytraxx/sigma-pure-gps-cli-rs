# sigma-pure-gps-cli

A command-line tool for managing the **Sigma Sport Pure GPS** (GPS10) GPS bicycle computer via USB. Update AGPS satellite prediction data and download recorded tracks as GPX files.

> **Why this exists:** The official [Sigma DATA CENTER](https://sigma.bike/product/data-center/) desktop application — the only supported way to manage this device — was **Windows-only** and had its **support discontinued on 1 December 2024**. It never ran on Linux or macOS. This tool fills that gap with a cross-platform CLI that covers the essential workflows.

## Features

- Upload u-blox AssistNow AGPS data for faster GPS fixes
- Download recorded tracks from device flash as GPX 1.1 files (with or without elevation correction)
- Query device info (serial number, firmware version)
- Read device settings (timezone, language, units, contrast, …)
- Read cumulative totals (distance, time, calories, climb)
- Show the AGPS data date currently stored on the device
- Set home altitude 1 and 2 on the device
- Delete all activity data from the device
- Auto-detect the device by USB VID — no manual port selection needed

## Requirements

- Rust toolchain (stable)
- Sigma Sport Pure GPS (GPS10) connected via USB
- A u-blox AssistNow token in `UBLOX_AGPS_TOKEN` (see [Configuration](#configuration))
- **Linux:** `cdc_acm` kernel module (usually loaded automatically); add yourself to the `dialout` group: `sudo usermod -aG dialout $USER`
- **Windows:** Device appears as `COMx`, no extra drivers needed
- **macOS:** Device appears as `/dev/tty.usbmodem*`

## Installation

```bash
git clone https://github.com/psytraxx/sigma-pure-gps-cli-rs
cd sigma-pure-gps-cli-rs
cargo build --release
# binary at target/release/sigma-pure-gps-cli
```

## Configuration

AGPS downloads require a **u-blox AssistNow token**. [Request a free evaluation token](https://www.u-blox.com/en/assistnow-service-evaluation-token-request), then create a `.env` file in the project root:

```bash
cp .env.example .env
# edit .env and set UBLOX_AGPS_TOKEN=your_token_here
```

Or export the variable directly:

```bash
export UBLOX_AGPS_TOKEN=your_token_here
```

## Usage

```
sigma-pure-gps-cli [OPTIONS] <COMMAND>

Options:
  -p, --port <PORT>   Serial port (auto-detected if omitted)
  -v, --verbose       Enable debug logging
  -h, --help          Print help

Commands:
  update              Download AGPS data and upload to device
  download-agps       Download AGPS data to a local file (no device needed)
  download-tracks     Download recorded tracks with elevation correction via Sigma service
  download-tracks-raw Download recorded tracks with raw barometric elevation (no correction)
  info                Query device serial number and firmware version
  get-settings        Read device settings (timezone, language, units, contrast, …)
  get-totals          Read cumulative totals (distance, time, calories, climb)
  get-sleep-screen    Read the sleep screen / watch face bitmap from the device and save as PNG
  set-sleep-screen    Upload a PNG bitmap as the device sleep screen / watch face
  agps-date           Show the AGPS data date currently stored on the device
  set-home-altitude   Set home altitude 1 and/or 2 on the device (in metres)
  delete-tracks       Permanently erase all activity data from the device
  get-waypoint        Read the point navigation waypoint stored on the device
  set-waypoint        Write a named GPS waypoint (point navigation) to the device
  list-ports          List available serial ports with USB VID/PID info
```

### Update AGPS data

Fetches current satellite prediction data from u-blox AssistNow and writes it to the device. Run this before a workout for a faster GPS fix.

```bash
sigma-pure-gps-cli update
```

### Download recorded tracks

Reads all tracks stored in device flash memory and writes them as individual GPX files.

```bash
# with elevation correction (recommended)
sigma-pure-gps-cli download-tracks ./tracks

# with raw barometric elevation
sigma-pure-gps-cli download-tracks-raw ./tracks
```

Each track is saved as `track_NNN_YYYYMMDD_HHMMSS.gpx` with a `<desc>` summary (distance, duration, avg/max speed, calories) and per-point elevation, speed, and temperature extensions.

### Download AGPS data to file

```bash
sigma-pure-gps-cli download-agps agps.bin
```

### Query device info

```bash
sigma-pure-gps-cli info
sigma-pure-gps-cli get-settings
sigma-pure-gps-cli get-totals
sigma-pure-gps-cli agps-date
```

### Download sleep screen / watch face

Reads the watch face bitmap from the device and saves it as a 16×59 PNG. The PNG includes `clock_x`, `clock_y`, and `name_pos` metadata so it can be edited and uploaded back later.

```bash
sigma-pure-gps-cli get-sleep-screen
sigma-pure-gps-cli get-sleep-screen my_face.png
```

### Upload sleep screen / watch face

Uploads a PNG (16×59 px, 1-bit grayscale) to the device. Use `get-sleep-screen` or `scripts/generate_bitmaps.sh` to create a valid PNG. The PNG must have `clock_x`, `clock_y`, and `name_pos` `tEXt` metadata chunks.

```bash
sigma-pure-gps-cli set-sleep-screen bitmaps/bike_and_hills.png
sigma-pure-gps-cli set-sleep-screen my_face.png
```

### Waypoint / Point navigation

Read the waypoint currently stored on the device:

```bash
sigma-pure-gps-cli get-waypoint
```

Write a named GPS coordinate to the device's single point navigation slot. The device will display a compass arrow and distance to this location during a workout.

```bash
sigma-pure-gps-cli set-waypoint --name "Summit" --lat 47.3769 --lon 8.5417
sigma-pure-gps-cli set-waypoint --name "Summit" --label "Zurich" --lat 47.3769 --lon 8.5417
```

- `--name` — first line shown on the device (max 9 characters)
- `--label` — second line (optional, max 9 characters)
- `--lat` — latitude in decimal degrees (negative = South)
- `--lon` — longitude in decimal degrees (negative = West)

### Set home altitude

Writes one or both home altitude slots to the device (in metres). At least one flag is required.

```bash
sigma-pure-gps-cli set-home-altitude --alt1 442
sigma-pure-gps-cli set-home-altitude --alt1 442 --alt2 442
```

### Delete all activity data

Permanently erases all recorded tracks from device flash. Prompts for confirmation.

```bash
sigma-pure-gps-cli delete-tracks
```

### List serial ports

```bash
sigma-pure-gps-cli list-ports
```

### Verbose logging

Pass `-v` (or `--verbose`) before any subcommand to enable debug output:

```bash
sigma-pure-gps-cli -v update
```

## Device internals

### Suspected hardware

| Component | Best guess | Evidence |
|-----------|-----------|----------|
| MCU | STM32F1xx / STM32F2xx | DFU mode command in firmware update flow; USB CDC-ACM; era (2013) |
| GPS | u-blox MAX-7C or similar | AssistNow API token in source; 32 760 byte AGPS payload matches u-blox sizing exactly |
| Barometer | Bosch BMP180 or MEAS MS5611 | Sea-level pressure field encoded as `(hPa − 900) × 10`; 1 m altitude resolution in logs |
| SPI NOR flash | 2 MB (e.g. Winbond W25Q16 / Macronix MX25L1606) | Top log address `0x1FE000`; AGPS region starts at `0x1000` |
| EEPROM | 1 KB (internal STM32 or small I²C) | Exactly 1024-byte config layout |
| NFC | Dual-interface memory (ST M24LR or NXP NT3H) | NFC path exposes identical read/write commands to both EEPROM and flash |

### Memory map

```
0x000000   (    0)  EEPROM — 1 KB configuration block
0x001000   ( 4096)  AGPS data — up to 32 760 bytes (u-blox AssistNow offline)
                    ... gap ...
0x1FE000   (≈2 MB)  Log header index — 65 bytes per activity, grows downward
                    Track data — lower flash addresses, referenced by start/stop pairs in log headers
```

### EEPROM layout (1024 bytes)

| Offset | Size | Content |
|--------|------|---------|
| 0 | 6 | Serial number (48-bit LE integer) |
| 64 | 6 | Unit type byte + firmware version |
| 80 | 4 | Update flags (tell firmware which block changed) |
| 96 | 172 | Sleep screen / watch face bitmap + metadata |
| 272 | 32 | Settings (timezone, language, units, altitudes, name…) |
| 304 | 20 | Cumulative totals (distance, time, calories, climb) |
| 336 | 27 | Point navigation waypoint |

### Firmware update

The MCU is put into **DFU (Device Firmware Upgrade) mode** over the same USB CDC-ACM connection before flashing. Firmware files use the `.GHX` extension (Sigma-proprietary container). The update sequence is: start-update → enter DFU → erase → stream 77-byte blocks → finalize.

### AGPS

The device uses u-blox **AssistNow Offline** (`period=2;resolution=1` — 2-week orbital predictions at 1-hour resolution). The CLI downloads this from `offline-live1.services.u-blox.com` using your token and writes it to flash at address `0x1000`.

### NFC

The original Sigma Data Center app also supported NFC sync (via a docking station with an NFC reader). The NFC path exposes the same EEPROM and flash regions through a dual-interface memory chip, using a custom block-addressed protocol with 18 ms inter-block delays (`FIFO_BIT = 1`, `READ_DELAY = 18 ms`). This CLI does not implement NFC — USB only.

## Development

### Running tests

```bash
cargo test
```

### Generating an HTML coverage report

Requires [`cargo-llvm-cov`](https://github.com/taiki-e/cargo-llvm-cov):

```bash
cargo install cargo-llvm-cov
cargo llvm-cov --html --open
# report opens at target/llvm-cov/html/index.html
```

## License

MIT
