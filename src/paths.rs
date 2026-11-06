use anyhow::{Context, Result};
use std::path::PathBuf;

/// All on-disk locations used by hako, rooted at `$HAKO_HOME` (default `~/.hako`).
#[derive(Debug, Clone)]
pub struct Paths {
    pub root: PathBuf,
}

impl Paths {
    pub fn new() -> Result<Self> {
        let root = match std::env::var_os("HAKO_HOME") {
            Some(p) if !p.is_empty() => PathBuf::from(p),
            _ => dirs::home_dir()
                .context("could not determine home directory; set HAKO_HOME")?
                .join(".hako"),
        };
        Ok(Self { root })
    }

    pub fn registries(&self) -> PathBuf {
        self.root.join("registries")
    }
    pub fn registry(&self, name: &str) -> PathBuf {
        self.registries().join(name)
    }

    /// hako <= 0.1 kept a single clone in `registry/`; move it to `registries/default/`.
    pub fn migrate(&self) -> Result<()> {
        let old = self.root.join("registry");
        let new = self.registry(crate::config::DEFAULT_NAME);
        if old.join(".git").exists() && !new.exists() {
            std::fs::create_dir_all(self.registries())?;
            std::fs::rename(&old, &new).context("migrating registry clone")?;
        }
        Ok(())
    }
    pub fn pkgs(&self) -> PathBuf {
        self.root.join("pkgs")
    }
    pub fn bin(&self) -> PathBuf {
        self.root.join("bin")
    }
    pub fn cache(&self) -> PathBuf {
        self.root.join("cache")
    }
    pub fn tmp(&self) -> PathBuf {
        self.root.join("tmp")
    }
    pub fn config_file(&self) -> PathBuf {
        self.root.join("config.toml")
    }
    pub fn state_file(&self) -> PathBuf {
        self.root.join("installed.toml")
    }

    pub fn ensure(&self) -> Result<()> {
        for d in [self.pkgs(), self.bin(), self.cache(), self.tmp()] {
            std::fs::create_dir_all(&d)
                .with_context(|| format!("creating {}", d.display()))?;
        }
        Ok(())
    }
}

/// Write `contents` to `path` atomically (temp file + rename).
pub fn write_atomic(path: &std::path::Path, contents: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let tmp = path.with_extension("tmp");
    std::fs::write(&tmp, contents).with_context(|| format!("writing {}", tmp.display()))?;
    std::fs::rename(&tmp, path).with_context(|| format!("writing {}", path.display()))?;
    Ok(())
}
