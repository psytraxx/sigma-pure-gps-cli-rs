---
name: release
description: Prepare and publish a release of sigma-pure-gps-cli. Use when the user asks to cut, prepare, or publish a release, bump the version, or tag a new version (e.g. "release 0.7.0", "cut a patch release"). Sets the version in Cargo.toml and CHANGELOG.md, commits, pushes, and creates the GitHub release that triggers the build workflow.
---

# Release

Prepare a release by entering the new version number in **both** `CHANGELOG.md` and
`Cargo.toml`. These two must always match, and the git tag must be that same version
prefixed with `v` — the GitHub Actions release workflow validates all three and fails
the release if any disagree.

## 1. Pre-flight

```bash
git status --porcelain          # must be clean
git rev-parse --abbrev-ref HEAD # must be main
git pull --ff-only origin main
cargo clippy && cargo fmt --check && cargo test
```

Do not release from a feature branch or a dirty tree. If `cargo fmt --check` fails,
run `cargo fmt`, commit that separately, then restart.

## 2. Pick the version

Read the `## Unreleased` section of `CHANGELOG.md` and choose per [semver](https://semver.org/):

- **patch** (0.6.1) — bug fixes only
- **minor** (0.7.0) — new backwards-compatible features (new subcommand, new protocol support)
- **major** (1.0.0) — breaking changes (removed/renamed subcommand, changed output format)

State the version and the reason to the user before editing anything. If `Unreleased`
is empty or missing, there is nothing to release — stop and say so.

## 3. Enter the version in Cargo.toml

Set `version = "X.Y.Z"` under `[package]` — **no `v` prefix**.

```bash
cargo build   # refreshes Cargo.lock with the new version; both files get committed
```

## 4. Enter the version in CHANGELOG.md

Rename the top `## Unreleased` heading to `## [X.Y.Z]` and add a fresh empty
`## Unreleased` above it:

```markdown
## Unreleased

(nothing yet)

## [X.Y.Z]

### Added
- ...
```

Keep the existing `### Added` / `### Changed` / `### Fixed` / `### Security`
subsections and their wording as-is — only the heading changes.

## 5. Verify the two match

```bash
grep -m1 '^version' Cargo.toml
grep -m1 '^## \[' CHANGELOG.md
```

Both must show the same X.Y.Z. This is the check the CI workflow repeats.

## 6. Commit and push

```bash
git add Cargo.toml Cargo.lock CHANGELOG.md
git commit -m "Release vX.Y.Z"
git push origin main
```

Commit message ends with the attribution line this session was given, if any.

## 7. Create the GitHub release

Confirm with the user before this step — it is public and triggers the build.

```bash
gh release create vX.Y.Z --title "vX.Y.Z" --notes "$(sed -n '/^## \[X.Y.Z\]/,/^## /p' CHANGELOG.md | sed '$d')"
```

Tag format must be `vX.Y.Z`. If `gh` is unavailable, direct the user to
https://github.com/psytraxx/sigma-pure-gps-cli-rs/releases → *Draft a new release*,
tag `vX.Y.Z`, body = that CHANGELOG section.

## 8. Confirm the build

```bash
gh run list --workflow release --limit 1
```

The workflow validates tag format, the Cargo.toml/tag match, and the CHANGELOG
section, then builds `x86_64-unknown-linux-gnu` and `aarch64-unknown-linux-gnu`
binaries and uploads them as release assets. Report to the user whether it passed,
with the failing step's output if it did not.
