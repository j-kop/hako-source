# hako (箱)

A git-based package manager. hako clones a **registry** git repo of package manifests, then downloads prebuilt binaries from each package's **own repo** (GitHub Release assets), verifies their sha256, and links them into `~/.hako/bin`.

## Repositories

| Repo | Contents |
|---|---|
| [`hako-source`](https://github.com/j-kop/hako-source) (this repo) | The `hako` CLI (Rust) |
| [`hako-packages`](https://github.com/j-kop/hako-packages) | The registry: `hako-registry.toml` + `packages/<name>.toml` |
| per-package repos | Package source + release assets |

## Install

On Linux or macOS (needs `git`):

```sh
curl -fsSL https://raw.githubusercontent.com/j-kop/hako-source/main/install.sh | sh
```

This downloads the latest release, then has hako install itself into `~/.hako`. From then on, `hako upgrade` keeps hako up to date along with your other packages.

Or build from source: `cargo install --git https://github.com/j-kop/hako-source`. A copy installed this way isn't managed by hako. To switch to the managed copy, run `hako install hako`, then `cargo uninstall hako`.

## Usage

```sh
hako update                 # clone / pull all registries
hako search [query]
hako info <pkg>
hako install <pkg>[@version] [--force]
hako list
hako upgrade [pkg]
hako uninstall <pkg>
hako doctor
```

## Multiple registries

hako can use several registries at once, e.g. one per team:

```sh
hako registry add team-a git@github.com:team-a/hako-packages.git
hako registry add team-b git@github.com:team-b/hako-packages.git
hako registry                  # list registries
hako registry remove team-b    # its installed packages stay, but won't be upgraded
```

- `hako install tool` finds `tool` in whichever registry has it.
- If two registries have a package with the same name, choose one: `hako install team-a/tool`.
- hako remembers which registry each package came from, and `hako upgrade` upgrades it from there.
- Two packages can't install the same executable name, even if they come from different registries.

The built-in registry is called `default`. To point it elsewhere, run `hako registry add default <url>`.

Add `~/.hako/bin` to your `PATH`:

```sh
export PATH="$HOME/.hako/bin:$PATH"
```

## Environment

- `HAKO_HOME`: root directory (default `~/.hako`)
- `HAKO_REGISTRY`: overrides the URL of the `default` registry

## Layout

```
~/.hako/
  config.toml          registry names and URLs
  registries/<name>/   git clone of each registry
  pkgs/<name>/<ver>/ extracted packages
  bin/               symlinks to package executables
  installed.toml     installed package state
  cache/             downloaded assets
```

## Development

```sh
cargo build
cargo test        # includes end-to-end tests against local git registries
```

The built-in default registry is `DEFAULT_REGISTRY` in `src/config.rs`. To use a different one, run `hako registry add default <git-url>` or set `HAKO_REGISTRY`.

## Releasing

1. Bump `version` in `Cargo.toml`, commit, and push.
2. Tag and push: `git tag v0.3.0 && git push origin v0.3.0`.
3. The `release` workflow builds hako for `x86_64`/`aarch64` × Linux (static musl)/macOS and publishes a GitHub Release. The release includes the `.tar.gz` files, `SHA256SUMS`, and `hako.toml`.
4. Copy the new `[versions."…"]` sections from the release's `hako.toml` into `hako-packages/packages/hako.toml` and push. Users then get the new version with `hako upgrade`.
