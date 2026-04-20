# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

```bash
# Build
cargo build

# Run
cargo run -- <keyword>         # e.g. cargo run -- anyhow::Error

# Install locally
cargo install --path . --locked

# Test (unit tests require docs to be generated first)
cargo doc
cargo test --bins

# Integration tests (snapshot-based via insta)
cargo test                          # uses pre-generated HTML in tests/html/
RUSTY_MAN_GENERATE=1 cargo test     # also generates docs from current toolchain

# Update insta snapshots after intentional output changes
cargo insta review                  # or: INSTA_UPDATE=always cargo test

# Format / Lint
cargo fmt
cargo clippy
```

## Architecture

manrs is a CLI viewer for rustdoc-generated HTML documentation. The lookup pipeline is:

1. **`args.rs`** — CLI arguments via `clap` derive (`Parser` trait). `ViewerName` is a newtype wrapper used for serde/clap compatibility; actual `Box<dyn Viewer>` is resolved via `args.get_viewer()` in `main.rs`.
2. **`source.rs`** — `Source` trait + `DirSource` for local doc directories; `Sources` aggregates multiple sources and searches them in reverse-priority order (last-added = highest priority, so `--source` paths win over defaults).
3. **`parser/html/`** — scrapes rustdoc HTML into `doc::Doc` structs using `scraper` (CSS selectors) + `ego-tree` for DOM traversal. `util.rs` provides `NodeRefExt` trait over scraper's `ElementRef` and `ego_tree::NodeRef<Node>`.
4. **`index.rs`** — parses `search-index.js` for partial matching; `index/v1_44.rs` and `index/v1_52.rs` handle two index format versions (changed in Rust 1.44 and 1.52). Post-1.56 rustdoc index formats are not yet supported.
5. **`doc.rs`** — core data types: `Name`/`Fqn`, `ItemType`, `Doc`, `Text`, `Code`, `Example`.
6. **`viewer/`** — `Viewer` trait with three implementations:
   - `viewer/text/` — `plain` (no formatting) and `rich` (ANSI + syntax highlighting via syntect); auto-selected based on TTY detection.
   - `viewer/tui/` — interactive terminal UI using `cursive 0.21`.

## Dependencies (key ones)

| Crate | Purpose |
|---|---|
| `clap 4` | CLI argument parsing (derive API) |
| `scraper` + `ego-tree` | HTML parsing and DOM traversal (replaced kuchiki) |
| `html2text 0.12` | HTML → plain/rich text rendering |
| `syntect 4` | Syntax highlighting for code blocks |
| `cursive 0.21` | TUI framework |
| `text-style 0.3` | Bridge between syntect/termion/cursive styling |
| `merge` + `serde` | Config file merging with CLI args |

## Testing

Integration tests in `tests/output.rs` use `insta` snapshots in `tests/snapshots/`. HTML fixtures in `tests/html/` cover rustdoc output for Rust 1.40–1.56. `with_rustdoc()` in `src/test_utils.rs` iterates over them with semver filtering.

`RUSTY_MAN_GENERATE=1` additionally generates docs via `cargo doc` for the running toolchain and tests against that output.

## Known limitations

- The search index parser only supports rustdoc formats up to Rust 1.56. Post-1.56 docs fall back to direct HTML lookup (no search index).
- `text-style 0.3` requires `cursive 0.16` as a transitive dep; use `cargo install --locked` to avoid resolution conflicts until text-style is upgraded.
- External link opening in the TUI was removed when `cursive-markup` was dropped.
