//! Record of installed packages (`installed.toml`).

use crate::paths::{write_atomic, Paths};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Installed packages, stored in `installed.toml`.
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct State {
    #[serde(default)]
    pub packages: BTreeMap<String, Installed>,
}

/// One installed package.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Installed {
    /// Registry the package was installed from (older state files predate this).
    #[serde(default = "default_registry")]
    pub registry: String,
    pub version: String,
    pub target: String,
    /// Link names created in `~/.hako/bin`.
    pub bins: Vec<String>,
}

/// Registry assumed for packages in state files written before multiple registries existed.
fn default_registry() -> String {
    crate::config::DEFAULT_NAME.to_string()
}

impl State {
    /// Loads `installed.toml`, or an empty state if nothing is installed yet.
    pub fn load(paths: &Paths) -> Result<Self> {
        let f = paths.state_file();
        if !f.exists() {
            return Ok(Self::default());
        }
        let s = std::fs::read_to_string(&f)?;
        toml::from_str(&s).with_context(|| format!("parsing {}", f.display()))
    }

    /// Writes the state to `installed.toml`.
    pub fn save(&self, paths: &Paths) -> Result<()> {
        write_atomic(&paths.state_file(), &toml::to_string_pretty(self)?)
    }

    /// The installed package that provides the executable `bin`, if any.
    pub fn owner_of_bin(&self, bin: &str) -> Option<&str> {
        self.packages
            .iter()
            .find(|(_, i)| i.bins.iter().any(|b| b == bin))
            .map(|(n, _)| n.as_str())
    }
}
