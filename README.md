# sigma-pure-gps-cli

A command-line tool for managing the **Sigma Sport Pure GPS** (GPS10) GPS bicycle computer via USB. Update AGPS satellite prediction data and download recorded tracks as GPX files.

## Features

- Upload u-blox AssistNow AGPS data for faster GPS fixes
- Download recorded tracks from device flash as GPX 1.1 files
- Query device info (serial number, firmware version)
- Auto-detect the device by USB VID — no manual port selection needed

## Requirements

- Rust toolchain (stable)
- Sigma Sport Pure GPS (GPS10) connected via USB
- **Linux:** `cdc_acm` kernel module (usually loaded automatically); add yourself to the `dialout` group: `sudo usermod -aG dialout $USER`
- **Windows:** Device appears as `COMx`, no extra drivers needed
- **macOS:** Device appears as `/dev/tty.usbmodem*`

## Installation

```bash
git clone https://github.com/your-username/sigma-pure-gps-cli-rs
cd sigma-pure-gps-cli-rs
cargo build --release
# binary at target/release/sigma-pure-gps-cli
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
  download            Download AGPS data to a local file
  download-tracks     Download recorded tracks from device as GPX files
  show-unit-info      Query device serial number and firmware version
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
sigma-pure-gps-cli download-tracks ./tracks
```

Each track is saved as `track_NNN.gpx` with elevation, speed, and temperature extensions.

### Download AGPS data to file

```bash
sigma-pure-gps-cli download agps.bin
```

### Query device info

```bash
sigma-pure-gps-cli show-unit-info
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

## Protocol

The USB serial protocol was reverse-engineered from the original `DataCenter_Desktop.swf` Flash application. See [docs/protocol.md](docs/protocol.md) for the full reference.

## License

MIT
