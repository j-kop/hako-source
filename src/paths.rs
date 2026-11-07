//! On-disk layout of `$HAKO_HOME` (default `~/.hako`).

use anyhow::{Context, Result};
use std::path::PathBuf;

/// All on-disk locations used by hako, rooted at `$HAKO_HOME` (default `~/.hako`).
#[derive(Debug, Clone)]
pub struct Paths {
    pub root: PathBuf,
}

impl Paths {
    /// Uses `$HAKO_HOME` as the root, falling back to `~/.hako`.
    pub fn new() -> Result<Self> {
        let root = match std::env::var_os("HAKO_HOME") {
            Some(p) if !p.is_empty() => PathBuf::from(p),
            _ => dirs::home_dir()
                .context("could not determine home directory; set HAKO_HOME")?
                .join(".hako"),
        };
        Ok(Self { root })
    }

    /// Directory holding all registry clones.
    pub fn registries(&self) -> PathBuf {
        self.root.join("registries")
    }
    /// Clone directory of the registry `name`.
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
    /// Extracted packages, as `pkgs/<name>/<version>/`.
    pub fn pkgs(&self) -> PathBuf {
        self.root.join("pkgs")
    }
    /// Symlinks to package executables; this is the directory users put on `$PATH`.
    pub fn bin(&self) -> PathBuf {
        self.root.join("bin")
    }
    /// Downloads in progress; each file is deleted once its install finishes.
    pub fn cache(&self) -> PathBuf {
        self.root.join("cache")
    }
    /// Staging area for extracting archives.
    pub fn tmp(&self) -> PathBuf {
        self.root.join("tmp")
    }
    /// Path of `config.toml`.
    pub fn config_file(&self) -> PathBuf {
        self.root.join("config.toml")
    }
    /// Path of `installed.toml`.
    pub fn state_file(&self) -> PathBuf {
        self.root.join("installed.toml")
    }
    /// Lock file that keeps two hako runs from changing `~/.hako` at the same time.
    pub fn lock_file(&self) -> PathBuf {
        self.root.join("lock")
    }

    /// Takes the exclusive lock on `~/.hako`, first waiting for any other running hako.
    /// The OS releases it when the returned file is dropped or hako exits, even after a crash.
    pub fn lock(&self) -> Result<std::fs::File> {
        std::fs::create_dir_all(&self.root)
            .with_context(|| format!("creating {}", self.root.display()))?;
        let path = self.lock_file();
        let file = std::fs::OpenOptions::new()
            .create(true)
            .truncate(false)
            .write(true)
            .open(&path)
            .with_context(|| format!("opening {}", path.display()))?;
        match file.try_lock() {
            Ok(()) => {}
            Err(std::fs::TryLockError::WouldBlock) => {
                eprintln!("waiting for another hako process to finish...");
                file.lock().with_context(|| format!("locking {}", path.display()))?;
            }
            Err(std::fs::TryLockError::Error(e)) => {
                return Err(e).with_context(|| format!("locking {}", path.display()));
            }
        }
        Ok(file)
    }

    /// Creates the working directories if they don't exist.
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
