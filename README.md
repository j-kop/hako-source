# hako (箱)

A git-based package manager. hako clones a **registry** git repo of package manifests, then downloads prebuilt binaries from each package's **own repo** (GitHub Release assets), verifies their sha256, and links them into `~/.hako/bin`.

## Repositories

| Repo | Contents |
|---|---|
| [`hako-source`](https://github.com/j-kop/hako-source) (this repo) | The `hako` CLI (Rust) |
| [`hako-packages`](https://github.com/j-kop/hako-packages) | The registry: `hako-registry.toml` + `packages/<name>.toml` |
| per-package repos | Package source + release assets |

## Install

```sh
cargo install --git https://github.com/j-kop/hako-source
```

## Usage

```sh
hako update                 # clone / pull the registry
hako search [query]
hako info <pkg>
hako install <pkg>[@version] [--force]
hako list
hako upgrade [pkg]
hako uninstall <pkg>
hako doctor
```

Add `~/.hako/bin` to your `PATH`:

```sh
export PATH="$HOME/.hako/bin:$PATH"
```

## Environment

- `HAKO_HOME`: root directory (default `~/.hako`)
- `HAKO_REGISTRY`: overrides the configured registry URL

## Layout

```
~/.hako/
  config.toml        registry URL
  registry/          git clone of the registry
  pkgs/<name>/<ver>/ extracted packages
  bin/               symlinks to package executables
  installed.toml     installed package state
  cache/             downloaded assets
```

## Development

```sh
cargo build
cargo test        # includes an end-to-end test against a local git registry
```

The built-in default registry is `DEFAULT_REGISTRY` in `src/config.rs`. To use a different one, run `hako registry <git-url>` or set `HAKO_REGISTRY`.
