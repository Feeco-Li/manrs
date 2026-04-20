# Contributing to manrs

manrs is a community-maintained fork of the original [rusty-man](https://git.sr.ht/~ireas/rusty-man) project by Robin Krahl, which is no longer actively maintained. Development continues on GitHub at **https://github.com/Feeco-Li/manrs**.

## How to contribute

### Reporting issues

Open an issue on the [GitHub issue tracker](https://github.com/Feeco-Li/manrs/issues). Please include:
- Your operating system and Rust toolchain version (`rustc --version`)
- The command you ran and the output you saw
- What you expected to happen

### Writing code

Browse [open issues](https://github.com/Feeco-Li/manrs/issues) to find something to work on. For larger changes, open an issue first to discuss the approach before investing time in implementation.

### Writing documentation

Help is welcome for:
- Proofreading and improving the README
- Writing a man page
- Improving inline code documentation

### Testing

Bug reports from non-Linux systems are especially valuable — the project has primarily been tested on Linux.

## Submitting changes

1. Fork the repository and create a branch from `master`.
2. Make your changes. Run the test suite and lints before submitting:
   ```bash
   cargo doc
   cargo test --bins        # unit tests (requires docs generated first)
   cargo test               # integration tests (snapshot-based)
   cargo fmt
   cargo clippy
   ```
3. If your changes affect rendered output, update insta snapshots:
   ```bash
   cargo insta review
   # or: INSTA_UPDATE=always cargo test
   ```
4. Open a pull request against `master` at https://github.com/Feeco-Li/manrs/pulls with a clear description of what changed and why.

## Code guidelines

- Format with `cargo fmt` — no exceptions.
- Fix all `cargo clippy` warnings before submitting.
- Keep changes focused; separate refactors from feature work when practical.
- The search index parser intentionally only supports rustdoc formats up to Rust 1.56 — see `CLAUDE.md` for known limitations.
