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
cargo install --git ssh://git@github.com/j-kop/hako-source
```

The repos are private, so hako fetches the registry over SSH (`git@github.com:j-kop/hako-packages.git`). You need an SSH key added to your GitHub account.

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
