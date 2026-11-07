# Contributing to hako

Thanks for your interest! Bug reports, ideas and pull requests are all welcome.

## Before you start

- **Bugs and ideas:** open an [issue](https://github.com/j-kop/hako-source/issues/new/choose) first, especially for larger changes, so we can agree on the approach.
- **Security problems:** report them privately, as described in [SECURITY.md](SECURITY.md). Please don't open a public issue.
- **New packages** belong in the registry, [hako-packages](https://github.com/j-kop/hako-packages), not here.

## Development

You need [Rust](https://rustup.rs), a C compiler (on Debian/Ubuntu: `sudo apt install build-essential`), and `git`.

```sh
cargo build
cargo test                                  # unit tests + end-to-end tests with local git registries
cargo clippy --all-targets -- -D warnings   # must be clean; CI runs exactly this
cargo doc --no-deps --document-private-items --open
```

## Pull requests

- Open pull requests against `main`. Direct pushes to `main` are blocked.
- CI runs clippy and the tests on Linux and macOS, and must pass before merging.
- Add or update tests for behaviour changes. `tests/e2e.rs` has helpers for building throwaway registries and packages.
- Give new functions, types and modules a short doc comment (`///` or `//!`) of at most two sentences.
- Keep each pull request focused on one change.

## Releases

The maintainer handles releases (see [Releasing](README.md#releasing)). Contributors don't need to bump the version.

## License

By contributing, you agree that your contributions are licensed under the [MIT License](LICENSE).
