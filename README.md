⚠️ I vibe-coded this whole thing...

# hako 📦 (箱)

A git-based package manager. hako clones a **registry** git repo of package manifests, then downloads prebuilt binaries from each package's **own repo** (GitHub Release assets), verifies their sha256, and links them into `~/.hako/bin`.

## Repositories

| Repo | Contents |
|---|---|
| [`hako-source`](https://github.com/j-kop/hako-source) (this repo) | The `hako` CLI (Rust) |
| [`hako-packages`](https://github.com/j-kop/hako-packages) | The registry: `hako-registry.toml` + `packages/<name>.toml` |
| per-package repos | Package source + release assets |

## Installing hako for the first time

hako can update itself, but only once a first copy is installed. The first install always works the same way: a downloaded or self-built hako runs `hako install hako`. That copy is then installed from the registry into `~/.hako`, like any other package, and later `hako upgrade` runs keep it up to date.

**Requirements:** Linux or macOS (x86_64 or ARM64), plus `git`. hako uses `git` to clone registries. No sudo is needed: everything goes into `~/.hako`.

### Option 1: install script (recommended)

```sh
curl -fsSL https://raw.githubusercontent.com/j-kop/hako-source/main/install.sh | sh
```

[`install.sh`](install.sh) does the following:
1. Detects your platform, e.g. `x86_64-linux` or `aarch64-macos`.
2. Downloads the matching `hako-<platform>.tar.gz` from the [latest release](https://github.com/j-kop/hako-source/releases/latest) into a temporary folder.
3. Runs that temporary hako as `hako install hako`. hako then clones the registry, downloads hako again from there with its checksum verified, and installs it into `~/.hako`.
4. Deletes the temporary folder.

### Option 2: manually, without piping a script into `sh`

Pick the file for your platform from the [releases page](https://github.com/j-kop/hako-source/releases/latest): `hako-x86_64-linux.tar.gz`, `hako-aarch64-linux.tar.gz`, `hako-x86_64-macos.tar.gz`, or `hako-aarch64-macos.tar.gz`. Then:

```sh
cd "$(mktemp -d)"
curl -fsSLO https://github.com/j-kop/hako-source/releases/latest/download/hako-x86_64-linux.tar.gz
tar xzf hako-x86_64-linux.tar.gz
./hako-*/hako install hako
```

The extracted copy is only needed for this one command, so you can delete the folder afterwards.

### Option 3: build from source

This needs [Rust](https://rustup.rs) and a C compiler (on Debian/Ubuntu: `sudo apt install build-essential`).

```sh
cargo install --git https://github.com/j-kop/hako-source
hako install hako      # install the managed copy into ~/.hako
cargo uninstall hako   # remove the cargo-built copy, so the managed one is used
```

### Then: add `~/.hako/bin` to your PATH

The installer doesn't edit your shell config, so add this line yourself. Use `~/.bashrc` for bash (the Linux default) or `~/.zshrc` for zsh (the macOS default):

```sh
export PATH="$HOME/.hako/bin:$PATH"
```

Open a new terminal, or run `source ~/.bashrc`, and check the result:

```sh
hako --version         # e.g. hako 0.2.0
command -v hako        # should print /home/<you>/.hako/bin/hako
hako doctor            # checks git, platform, registries, and PATH
```

### From now on: hako updates itself

```sh
hako upgrade           # updates the registry, then upgrades hako and every other package
```

To remove hako completely, delete `~/.hako` and the `PATH` line from your shell config.

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

hako can use several registries at once:

```sh
hako registry add registry-a https://github.com/you/registry-a.git
hako registry add registry-b https://github.com/you/registry-b.git
hako registry                      # list registries
hako registry remove registry-b    # its installed packages stay, but won't be upgraded
```

- `hako install tool` finds `tool` in whichever registry has it.
- If two registries have a package with the same name, choose one: `hako install registry-a/tool`.
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
cargo doc --no-deps --document-private-items --open   # code docs (hako is a binary, so its items are private)
```

The built-in default registry is `DEFAULT_REGISTRY` in `src/config.rs`. To use a different one, run `hako registry add default <git-url>` or set `HAKO_REGISTRY`.

## Releasing

Releases are started by hand, and only the repository owner can start them.

1. On GitHub, go to **Actions → release → Run workflow**, and keep **Use workflow from: main**.
2. Fill in:
   - **commit:** `latest` for the newest commit on `main`, or the id of any commit in `main`'s history;
   - **bump:** `patch`, `minor` or `major`. The new version is counted from the latest release tag, e.g. `v0.2.0` + `minor` gives `v0.3.0`.
3. The [`release`](.github/workflows/release.yml) workflow then:
   1. runs clippy and the tests on Linux and macOS, for that commit;
   2. creates a release commit (the chosen commit plus the version bump in `Cargo.toml` and `Cargo.lock`) and tags it `vX.Y.Z`;
   3. builds hako for `x86_64`/`aarch64` × Linux (static musl)/macOS from the tag;
   4. publishes a GitHub Release with the `.tar.gz` files, `SHA256SUMS` and `hako.toml`.

   If anything fails or is cancelled after the tag was created, the workflow deletes the tag and any partial release, so you can just run it again.
4. Copy the new `[versions."…"]` sections from the release's `hako.toml` (also shown in the run's summary) into `hako-packages/packages/hako.toml` and push. Users then get the new version with `hako upgrade`.

The release commit exists only on its tag, and `main` is never changed by a release. As a result, `version` in `Cargo.toml` on `main` stays at whatever it was last set to. The released binaries always report the correct version.

## TODO / possible upgrades

### Automatic registry PR for new hako releases
Replace manual step 4 of [Releasing](#releasing) with a job in the release workflow:

1. Download the new release's `hako.toml`.
2. Clone `hako-packages` and add the new `[versions."X.Y.Z"]` sections to `packages/hako.toml`, keeping older versions.
3. Test the change: the freshly built hako installs itself from that branch, checksum included, so a broken manifest never gets a PR.
4. Push a `hako-X.Y.Z` branch and open a PR "hako X.Y.Z" on `hako-packages`, to be reviewed and merged by hand.

Notes:
- **Safe to re-run:** do nothing if the version is already in the registry or a PR for it already exists.
- **Manual trigger:** add a "Run workflow" button that takes a tag, for re-running it for any existing release.
- **Permissions:** the workflow's built-in `GITHUB_TOKEN` can't write to another repository. It needs a fine-grained personal access token with access to `hako-packages` only, with **Contents** and **Pull requests** set to read and write. Store it in `hako-source` as the Actions secret `HAKO_PACKAGES_TOKEN`. A GitHub App would avoid an expiring token, at the cost of more setup.
- **Testing:** cut a small real release (e.g. `v0.2.1`) and check that the PR appears on `hako-packages`.

## License

[MIT](LICENSE)
