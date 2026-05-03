# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
cargo build                          # debug build
cargo build --release                # release build
cargo run -- --help                  # show CLI help
cargo run -- list-ports              # list serial ports
cargo run -- show-unit-info          # query connected device
cargo run -- update                  # download AGPS + upload
cargo run -- download agps.bin       # download AGPS only, save to file
cargo run -- download-tracks ./out   # download recorded tracks as GPX
cargo run -- -v update               # verbose (debug logging)
cargo clippy                         # lint
cargo test                           # run tests
```

## Architecture

Single-binary CLI tool (`src/main.rs`) with these modules:

**`src/downloader.rs`** — Downloads AGPS satellite prediction data from u-blox AssistNow servers. Two fallback URLs are tried in sequence. Payload capped at 32 760 bytes.

**`src/device.rs`** — Enumerates serial ports via `serialport` and selects the first one with USB VID `0x1D9D` (Sigma Sport). On Windows this is a COMx port; on Linux `/dev/ttyACM0`.

**`src/protocol/`** — Implements the SIGMA USB serial protocol. `mod.rs` contains all port I/O functions; `commands.rs` holds command byte constants and the flash-read command builder. All I/O is synchronous blocking; callers run this on `tokio::task::spawn_blocking`. See [`docs/protocol.md`](docs/protocol.md) for the full command reference.

**`src/decoder.rs`** — Decodes binary log data read from device flash into `LogHeader` and `TrackPoint` structs. Ported from `source/decompiled/scripts/decoder/Gps10Decoder.as`.

**`src/gpx.rs`** — Writes GPX 1.1 files from decoded track data.

**`src/main.rs`** — `clap`-based CLI; async via `tokio`. Subcommands: `update`, `download`, `list-ports`, `show-unit-info`, `download-tracks`.

## After every code change

```bash
cargo clippy
cargo fmt
```

Then update [`docs/protocol.md`](docs/protocol.md) if any protocol details changed, and add an entry to [`CHANGELOG.md`](CHANGELOG.md).
