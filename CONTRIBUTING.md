# Contributing to hako

Thanks for your interest! Bug reports, ideas and pull requests are all welcome.

## Before you start

- **Bugs and ideas:** open an [issue](https://github.com/j-kop/hako-source/issues/new/choose) first, especially for larger changes, so we can agree on the approach.
- **Packages:** [hako-packages](https://github.com/j-kop/hako-packages) is only a demo registry and doesn't take package submissions. To distribute packages, [set up your own registry](https://github.com/j-kop/hako-packages#how-to-set-up-your-own-registry).

## Development

You need [Rust](https://rustup.rs), a C compiler (on Debian/Ubuntu: `sudo apt install build-essential`), and `git`.

```sh
cargo build
cargo test                                  # unit tests + end-to-end tests with local git registries
cargo clippy --all-targets -- -D warnings   # must be clean; CI runs exactly this
cargo doc --no-deps --document-private-items --open
```

## Pull requests

Contributions come in through **forks**: you can't create branches in this repository, and nobody can push to `main` directly.

1. [Fork the repository](https://github.com/j-kop/hako-source/fork) and clone your fork.
2. Create a branch **in your fork**, commit your change, and push it there.
3. Open a pull request from your fork's branch against `main` of `j-kop/hako-source`. Leave **Allow edits by maintainers** ticked.

What reviewers expect:
- CI runs clippy and the tests on Linux and macOS, and must pass before merging.
- Add or update tests for behaviour changes. `tests/e2e.rs` has helpers for building throwaway registries and packages.
- Give new functions, types and modules a short doc comment (`///` or `//!`) of at most two sentences.
- Keep each pull request focused on one change.

## Releases

The maintainer handles releases (see [Releasing](README.md#releasing)). Contributors don't need to bump the version.

## License

By contributing, you agree that your contributions are licensed under the [MIT License](LICENSE).
