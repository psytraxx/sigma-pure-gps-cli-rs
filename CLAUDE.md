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

Single-binary CLI tool. `main.rs` contains only arg parsing and dispatch. Each subcommand is a module under `src/commands/`. To add a new subcommand: create `src/commands/my_cmd.rs` with a `pub async fn run(...)`, register it in `src/commands/mod.rs`, add the variant to `Command` in `main.rs`, and wire it in `match cli.command`.

**`src/commands/`** — One file per subcommand: `update`, `download`, `download_tracks`, `show_unit_info`, `list_ports`.

**`src/util.rs`** — Shared helpers: `resolve_port` (auto-detect or use CLI arg) and `build_http_client`.

**`src/downloader.rs`** — Downloads AGPS satellite prediction data from u-blox AssistNow servers. Two fallback URLs tried in sequence. Payload capped at 32 760 bytes.

**`src/device.rs`** — Enumerates serial ports via `serialport` and selects the first with USB VID `0x1D9D` (Sigma Sport).

**`src/protocol/`** — SIGMA USB serial protocol. `mod.rs` has all port I/O functions; `commands.rs` holds byte constants and the flash-read command builder. All I/O is synchronous blocking (callers use `tokio::task::spawn_blocking`). See [`docs/protocol.md`](docs/protocol.md) for the full reference.

**`src/decoder.rs`** — Decodes binary log data into `LogHeader` and `TrackPoint` structs. Ported from `Gps10Decoder.as`.

**`src/gpx.rs`** — Writes GPX 1.1 files from decoded track data.

## Implementing new features

Before implementing any new feature or protocol detail, always check the decompiled ActionScript source files in `source/decompiled/scripts/` first. They are the authoritative reference for decoding logic, byte layouts, and command sequences.

Key files:
- `handler/dockingstation/Gps10DSHandler.as` — device detection, unit info command
- `handler/Gps10Handler.as` — full USB state machine, all command constants
- `decoder/Gps10Decoder.as` — all binary decoding logic
- `core/agps/AgpsLoader.as` — AGPS download and validity date decoding
- `utils/ChecksumUtil.as` — checksum algorithm

## After every code change

```bash
cargo clippy
cargo fmt
```

Then update [`docs/protocol.md`](docs/protocol.md) if any protocol details changed, and add an entry to [`CHANGELOG.md`](CHANGELOG.md).
