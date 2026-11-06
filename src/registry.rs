use crate::manifest::{valid_name, Manifest};
use crate::paths::Paths;
use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Registry schema major version this client understands.
pub const SCHEMA: u32 = 1;

#[derive(Deserialize)]
struct RegistryMeta {
    schema: u32,
}

/// One cloned registry (e.g. `default`, `registry-a`).
pub struct Registry {
    pub name: String,
    dir: PathBuf,
}

fn git(args: &[&str], cwd: Option<&Path>) -> Result<String> {
    let mut cmd = Command::new("git");
    if let Some(d) = cwd {
        cmd.arg("-C").arg(d);
    }
    let out = cmd
        .args(args)
        .output()
        .context("failed to run `git` — is it installed?")?;
    if !out.status.success() {
        bail!(
            "git {} failed: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Clone the registry `name`, or fast-forward it if already present.
pub fn update(paths: &Paths, name: &str, url: &str) -> Result<Registry> {
    let dir = paths.registry(name);
    if dir.join(".git").exists() {
        let current = git(&["remote", "get-url", "origin"], Some(&dir)).unwrap_or_default();
        if current == url {
            git(&["pull", "--ff-only", "--quiet"], Some(&dir))?;
            return Registry::open(paths, name);
        }
        eprintln!("registry `{name}` URL changed, re-cloning");
        std::fs::remove_dir_all(&dir)?;
    } else if dir.exists() {
        std::fs::remove_dir_all(&dir)?;
    }
    std::fs::create_dir_all(paths.registries())?;
    let dir_s = dir.to_string_lossy();
    git(&["clone", "--quiet", "--depth", "1", url, &dir_s], None)?;
    Registry::open(paths, name)
}

impl Registry {
    pub fn open(paths: &Paths, name: &str) -> Result<Self> {
        let dir = paths.registry(name);
        if !dir.exists() {
            bail!("registry `{name}` is not cloned; run `hako update`");
        }
        let meta: RegistryMeta = toml::from_str(
            &std::fs::read_to_string(dir.join("hako-registry.toml")).with_context(|| {
                format!("registry `{name}` is missing hako-registry.toml — is it a hako registry?")
            })?,
        )
        .with_context(|| format!("registry `{name}`: invalid hako-registry.toml"))?;
        if meta.schema != SCHEMA {
            bail!(
                "registry `{name}` uses schema {} but this hako supports schema {SCHEMA}; upgrade hako",
                meta.schema
            );
        }
        Ok(Self {
            name: name.to_string(),
            dir,
        })
    }

    fn manifest_path(&self, pkg: &str) -> PathBuf {
        self.dir.join("packages").join(format!("{pkg}.toml"))
    }

    pub fn has(&self, pkg: &str) -> bool {
        valid_name(pkg) && self.manifest_path(pkg).is_file()
    }

    pub fn load(&self, pkg: &str) -> Result<Manifest> {
        if !valid_name(pkg) {
            bail!("invalid package name `{pkg}`");
        }
        let path = self.manifest_path(pkg);
        let src = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                bail!("package `{pkg}` not found in registry `{}`", self.name)
            }
            Err(e) => return Err(e).with_context(|| format!("reading {}", path.display())),
        };
        Manifest::parse(&src, pkg).with_context(|| format!("in manifest {}", path.display()))
    }

    /// All valid manifests, sorted by name. Invalid ones are reported and skipped.
    pub fn all(&self) -> Result<Vec<Manifest>> {
        let mut out = Vec::new();
        let dir = self.dir.join("packages");
        if !dir.exists() {
            return Ok(out);
        }
        for entry in std::fs::read_dir(&dir)? {
            let path = entry?.path();
            if path.extension().and_then(|e| e.to_str()) != Some("toml") {
                continue;
            }
            let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
                continue;
            };
            match self.load(stem) {
                Ok(m) => out.push(m),
                Err(e) => eprintln!("warning: skipping {}: {e:#}", path.display()),
            }
        }
        out.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(out)
    }
}

/// Split `registry-a/tool` into (`Some("registry-a")`, `tool`); `tool` into (`None`, `tool`).
pub fn split_qualified(name: &str) -> (Option<&str>, &str) {
    match name.split_once('/') {
        Some((r, p)) => (Some(r), p),
        None => (None, name),
    }
}

/// All configured registries.
pub struct Registries {
    pub list: Vec<Registry>,
}

impl Registries {
    pub fn get(&self, name: &str) -> Option<&Registry> {
        self.list.iter().find(|r| r.name == name)
    }

    /// Find a package by `name` or `registry/name`. If an unqualified name exists in
    /// several registries, `prefer` (e.g. where it is already installed from) breaks the tie.
    pub fn find(&self, name: &str, prefer: Option<&str>) -> Result<(&Registry, Manifest)> {
        let (reg, pkg) = split_qualified(name);
        if let Some(r) = reg {
            let registry = self.get(r).with_context(|| {
                format!("unknown registry `{r}` (see `hako registry list`)")
            })?;
            return Ok((registry, registry.load(pkg)?));
        }
        let hits: Vec<&Registry> = self.list.iter().filter(|r| r.has(pkg)).collect();
        let registry = match hits.as_slice() {
            [] => bail!("package `{pkg}` not found (try `hako search`)"),
            [only] => *only,
            many => match many.iter().find(|r| Some(r.name.as_str()) == prefer) {
                Some(r) => *r,
                None => {
                    let options: Vec<_> =
                        many.iter().map(|r| format!("{}/{pkg}", r.name)).collect();
                    bail!(
                        "package `{pkg}` exists in several registries; choose one: {}",
                        options.join(", ")
                    )
                }
            },
        };
        Ok((registry, registry.load(pkg)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qualified_names() {
        assert_eq!(split_qualified("registry-a/tool"), (Some("registry-a"), "tool"));
        assert_eq!(split_qualified("tool"), (None, "tool"));
    }
}
