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

pub struct Registry {
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

/// Clone the registry, or fast-forward it if already present.
pub fn update(paths: &Paths, url: &str) -> Result<Registry> {
    let dir = paths.registry();
    if dir.join(".git").exists() {
        let current = git(&["remote", "get-url", "origin"], Some(&dir)).unwrap_or_default();
        if current == url {
            git(&["pull", "--ff-only", "--quiet"], Some(&dir))?;
            return Registry::open(paths);
        }
        eprintln!("registry URL changed, re-cloning");
        std::fs::remove_dir_all(&dir)?;
    } else if dir.exists() {
        std::fs::remove_dir_all(&dir)?;
    }
    std::fs::create_dir_all(&paths.root)?;
    let dir_s = dir.to_string_lossy();
    git(&["clone", "--quiet", "--depth", "1", url, &dir_s], None)?;
    Registry::open(paths)
}

impl Registry {
    pub fn open(paths: &Paths) -> Result<Self> {
        let dir = paths.registry();
        if !dir.exists() {
            bail!("registry not found; run `hako update`");
        }
        let meta_path = dir.join("hako-registry.toml");
        let meta: RegistryMeta = toml::from_str(
            &std::fs::read_to_string(&meta_path)
                .context("registry is missing hako-registry.toml — is this a hako registry?")?,
        )
        .context("invalid hako-registry.toml")?;
        if meta.schema != SCHEMA {
            bail!(
                "registry uses schema {} but this hako supports schema {SCHEMA}; upgrade hako",
                meta.schema
            );
        }
        Ok(Self { dir })
    }

    pub fn load(&self, name: &str) -> Result<Manifest> {
        if !valid_name(name) {
            bail!("invalid package name `{name}`");
        }
        let path = self.dir.join("packages").join(format!("{name}.toml"));
        let src = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                bail!("package `{name}` not found (try `hako search`)")
            }
            Err(e) => return Err(e).with_context(|| format!("reading {}", path.display())),
        };
        Manifest::parse(&src, name).with_context(|| format!("in manifest {}", path.display()))
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
