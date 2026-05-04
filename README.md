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
  agps-date           Show the AGPS data date currently stored on the device
  set-home-altitude   Set home altitude 1 and/or 2 on the device (in metres)
  delete-tracks       Permanently erase all activity data from the device
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

Each track is saved as `track_NNN.gpx` with elevation, speed, and temperature extensions.

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

## License

MIT
