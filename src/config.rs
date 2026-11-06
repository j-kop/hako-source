use crate::manifest::valid_name;
use crate::paths::{write_atomic, Paths};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Name of the registry that `DEFAULT_REGISTRY` and `$HAKO_REGISTRY` refer to.
pub const DEFAULT_NAME: &str = "default";

/// Built-in registry, used until the user configures registries themselves.
pub const DEFAULT_REGISTRY: Option<&str> = Some("https://github.com/j-kop/hako-packages.git");

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Config {
    /// Named registries (name -> git URL). `None` means "never configured":
    /// the built-in default is used.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    registries: Option<BTreeMap<String, String>>,
    /// Legacy single-registry setting from hako <= 0.1; migrated on load.
    #[serde(default, skip_serializing)]
    registry: Option<String>,
}

fn builtin() -> BTreeMap<String, String> {
    DEFAULT_REGISTRY
        .map(|u| BTreeMap::from([(DEFAULT_NAME.to_string(), u.to_string())]))
        .unwrap_or_default()
}

impl Config {
    pub fn load(paths: &Paths) -> Result<Self> {
        let f = paths.config_file();
        if !f.exists() {
            return Ok(Self::default());
        }
        let s = std::fs::read_to_string(&f)?;
        let mut cfg: Config =
            toml::from_str(&s).with_context(|| format!("parsing {}", f.display()))?;
        if let Some(url) = cfg.registry.take() {
            cfg.registries
                .get_or_insert_with(BTreeMap::new)
                .entry(DEFAULT_NAME.to_string())
                .or_insert(url);
        }
        Ok(cfg)
    }

    pub fn save(&self, paths: &Paths) -> Result<()> {
        write_atomic(&paths.config_file(), &toml::to_string_pretty(self)?)
    }

    /// Effective registries: configured (or built-in) ones, with `$HAKO_REGISTRY`
    /// overriding the URL of the `default` registry.
    pub fn registries(&self) -> Result<BTreeMap<String, String>> {
        let mut map = self.registries.clone().unwrap_or_else(builtin);
        if let Ok(u) = std::env::var("HAKO_REGISTRY") {
            if !u.is_empty() {
                map.insert(DEFAULT_NAME.to_string(), u);
            }
        }
        if map.is_empty() {
            bail!("no registries configured; run `hako registry add <name> <git-url>`");
        }
        Ok(map)
    }

    fn stored_mut(&mut self) -> &mut BTreeMap<String, String> {
        self.registries.get_or_insert_with(builtin)
    }

    /// Add or replace a registry. Returns the previous URL, if any.
    pub fn add(&mut self, name: &str, url: &str) -> Result<Option<String>> {
        if !valid_name(name) {
            bail!("invalid registry name `{name}` (use lowercase letters, digits, - and _)");
        }
        Ok(self.stored_mut().insert(name.to_string(), url.to_string()))
    }

    /// Remove a registry. Returns whether it existed.
    pub fn remove(&mut self, name: &str) -> bool {
        self.stored_mut().remove(name).is_some()
    }
}
