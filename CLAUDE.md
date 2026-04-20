# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
# Build
cargo build

# Run
cargo run -- <keyword>         # e.g. cargo run -- kuchiki::NodeRef

# Test (run unit tests; requires docs to be generated first)
cargo doc
cargo test --bins

# Integration tests (snapshot-based via insta)
cargo test                     # uses pre-generated HTML in tests/html/
RUSTY_MAN_GENERATE=1 cargo test  # also generates docs from current toolchain

# Update insta snapshots after intentional output changes
cargo insta review             # or: INSTA_UPDATE=always cargo test

# Format
cargo fmt

# Lint
cargo clippy
```

## Architecture

rusty-man is a CLI viewer for rustdoc-generated HTML documentation. The lookup pipeline is:

1. **`args.rs`** — parses CLI arguments (keyword, `--source`, `--viewer`, `--examples`, etc.)
2. **`source.rs`** — `Source` trait + `DirSource` for local doc directories; `Sources` aggregates multiple sources and searches them in reverse-priority order
3. **`parser/html/`** — scrapes rustdoc HTML files into `doc::Doc` structs; handles different rustdoc versions
4. **`index.rs`** — parses `search-index.js` for fuzzy/partial matching; `index/v1_44.rs` and `index/v1_52.rs` handle two index format versions (the format changed in Rust 1.44 and again in 1.52)
5. **`doc.rs`** — core data types: `Name`/`Fqn`, `ItemType`, `Doc`, `Text`, `Code`, `Example`
6. **`viewer/`** — `Viewer` trait with three implementations:
   - `viewer/text/` — `plain` (no formatting) and `rich` (ANSI formatting + syntax highlighting via syntect); auto-selected based on whether stdout is a TTY
   - `viewer/tui/` — interactive terminal UI using cursive + cursive-markup

**Key design note:** `Sources` searches last-added first (sources vec is reversed after construction), so `--source` paths take priority over defaults.

## Testing

Integration tests live in `tests/output.rs` and use `insta` snapshots in `tests/snapshots/`. Test HTML fixtures in `tests/html/` are checked in for Rust versions 1.40–1.56; `with_rustdoc()` in `src/test_utils.rs` iterates over them with semver version filtering.

Setting `RUSTY_MAN_GENERATE=1` causes tests to also run `cargo doc` for the current toolchain and test against that output.

## Compatibility

The codebase targets MSRV 1.40. Some clippy lints are suppressed in `main.rs` due to this (e.g., `clippy::manual_strip` because `slice::strip_suffix` was only stabilized in 1.51).
