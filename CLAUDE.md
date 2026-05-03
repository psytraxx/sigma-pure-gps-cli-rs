# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
cargo build                    # debug build
cargo build --release          # release build
cargo run -- --help            # show CLI help
cargo run -- list-ports        # list serial ports
cargo run -- show-unit-info    # query connected device
cargo run -- update            # download AGPS + upload (default)
cargo run -- download agps.bin # download only, save to file
cargo run -- -v update         # verbose (debug logging)
cargo clippy                   # lint
cargo test                     # run tests
```

## Architecture

Single-binary CLI tool (`src/main.rs`) with three modules:

**`src/downloader.rs`** — Downloads AGPS satellite prediction data from u-blox AssistNow servers. Two fallback URLs are tried in sequence. Payload capped at 32 760 bytes.

**`src/device.rs`** — Enumerates serial ports via `serialport` and selects the first one with USB VID `0x1D9D` (Sigma Sport). On Windows this is a COMx port; on Linux `/dev/ttyACM0`.

**`src/protocol.rs`** — Implements the SIGMA USB serial protocol. All I/O is synchronous blocking; callers run this on `tokio::task::spawn_blocking`. See [`docs/protocol.md`](docs/protocol.md) for the full command reference, byte sequences, AGPS upload handshake, and NFC notes.

**`src/main.rs`** — `clap`-based CLI; async via `tokio`. Subcommands: `update` (default), `download`, `list-ports`, `show-unit-info`.

## After every code change

```bash
cargo clippy
cargo fmt
```
