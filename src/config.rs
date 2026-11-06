use crate::paths::{write_atomic, Paths};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};

/// Official registry, used unless overridden by `hako registry <url>` or `$HAKO_REGISTRY`.
pub const DEFAULT_REGISTRY: Option<&str> = Some("git@github.com:j-kop/hako-packages.git");

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Config {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub registry: Option<String>,
}

impl Config {
    pub fn load(paths: &Paths) -> Result<Self> {
        let f = paths.config_file();
        if !f.exists() {
            return Ok(Self::default());
        }
        let s = std::fs::read_to_string(&f)?;
        toml::from_str(&s).with_context(|| format!("parsing {}", f.display()))
    }

    pub fn save(&self, paths: &Paths) -> Result<()> {
        write_atomic(&paths.config_file(), &toml::to_string_pretty(self)?)
    }

    /// Resolution order: `$HAKO_REGISTRY` > config.toml > built-in default.
    pub fn registry_url(&self) -> Result<String> {
        if let Ok(u) = std::env::var("HAKO_REGISTRY") {
            if !u.is_empty() {
                return Ok(u);
            }
        }
        if let Some(u) = &self.registry {
            return Ok(u.clone());
        }
        match DEFAULT_REGISTRY {
            Some(u) => Ok(u.to_string()),
            None => bail!("no registry configured; run `hako registry <git-url>`"),
        }
    }
}
