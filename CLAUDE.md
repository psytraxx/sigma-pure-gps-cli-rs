# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
cargo build                               # debug build
cargo build --release                     # release build
cargo run -- --help                       # show CLI help
cargo run -- list-ports                   # list serial ports
cargo run -- info                         # query connected device
cargo run -- update                       # download AGPS + upload
cargo run -- download-agps agps.bin       # download AGPS only, save to file
cargo run -- download-tracks ./out        # download tracks with elevation correction
cargo run -- download-tracks-raw ./out    # download tracks with raw barometric elevation
cargo run -- get-settings                 # read device settings
cargo run -- get-totals                   # read cumulative totals
cargo run -- agps-date                    # show AGPS data date on device
cargo run -- -v update                    # verbose (debug logging)
cargo clippy                              # lint
cargo test                                # run tests
```

## Architecture

Single-binary CLI tool. `main.rs` contains only arg parsing and dispatch. Each subcommand is a module under `src/commands/`. To add a new subcommand: create `src/commands/my_cmd.rs` with a `pub async fn run(...)`, register it in `src/commands/mod.rs`, add the variant to `Command` in `main.rs`, and wire it in `match cli.command`.

**`src/commands/`** — One file per subcommand: `update`, `download_agps`, `download_tracks`, `info`, `list_ports`.

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

**Required — do not skip any of these:**

```bash
cargo clippy
cargo fmt
```

- Add an entry to [`CHANGELOG.md`](CHANGELOG.md) under `## Unreleased` — always, for every change
- Update [`docs/protocol.md`](docs/protocol.md) if any protocol details changed

## Releasing

The repository uses semantic versioning (v0.1.0, v0.2.0, v1.0.0, etc.) and a GitHub Actions workflow to build and publish binaries.

### Release checklist

1. **Decide on version bump** using [semver](https://semver.org/):
   - Patch (v0.1.1): bug fixes only
   - Minor (v0.2.0): new backwards-compatible features
   - Major (v1.0.0): breaking changes

2. **Update `Cargo.toml`** — change `version = "..."` to the new version (without the `v` prefix)

3. **Organize `CHANGELOG.md`**:
   - Rename the top-level section from `## Unreleased` to `## [X.Y.Z]` (using the same version as Cargo.toml, without `v`)
   - Add a new `## Unreleased` section below it for future changes
   - Example:
     ```markdown
     ## [0.2.0]

     ### Added
     - New feature X
     - New feature Y

     ### Fixed
     - Bug fix Z

     ## Unreleased

     (nothing yet)
     ```

4. **Commit and push** these changes to `main`:
   ```bash
   git add Cargo.toml CHANGELOG.md
   git commit -m "Release v0.2.0"
   git push origin main
   ```

5. **Create the GitHub Release**:
   - Go to https://github.com/psytraxx/sigma-pure-gps-cli-rs/releases
   - Click **Draft a new release**
   - Tag version: `v0.2.0` (must match the tag format `vX.Y.Z`)
   - Release title: `v0.2.0` or `Release 0.2.0`
   - Release description: Copy the relevant section from `CHANGELOG.md` (everything between the `## [X.Y.Z]` header and the next section)
   - Click **Publish release**

The GitHub Actions workflow will automatically:
- Validate that the tag matches semantic versioning (`vX.Y.Z`)
- Validate that `Cargo.toml` version matches the tag version
- Validate that `CHANGELOG.md` has a section for the version
- Build binaries for: Linux x86_64, Linux arm64
- Upload all binaries as release assets

### Build targets

- `x86_64-unknown-linux-gnu` → `sigma-pure-gps-cli-X.Y.Z-linux-x86_64`
- `aarch64-unknown-linux-gnu` → `sigma-pure-gps-cli-X.Y.Z-linux-arm64`
