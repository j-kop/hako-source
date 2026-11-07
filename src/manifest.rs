//! Package manifests (`packages/<name>.toml`): parsing, validation and version selection.

use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Component, Path};

/// A package manifest: `packages/<name>.toml` in the registry repo.
/// Unknown fields are ignored, so new optional fields don't break older hako versions.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub name: String,
    pub description: String,
    /// The package's own repository (where releases are published).
    pub repo: String,
    #[serde(default)]
    pub license: Option<String>,
    /// Paths (relative to the extracted package root) of executables to put on PATH.
    pub bin: Vec<String>,
    pub versions: BTreeMap<String, Version>,
}

/// One version of a package: its release file for each platform.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Version {
    pub targets: BTreeMap<String, Target>,
}

/// A release file to download and its expected checksum.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Target {
    pub url: String,
    pub sha256: String,
}

/// Whether `name` is a valid package or registry name: lowercase letters, digits, `-` and `_`.
pub fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-' || c == '_')
        && !name.starts_with('-')
}

/// A relative path with only normal components (no `..`, no root).
pub fn is_safe_relative(p: &Path) -> bool {
    !p.as_os_str().is_empty() && p.components().all(|c| matches!(c, Component::Normal(_)))
}

impl Manifest {
    /// Parses and validates a manifest read from `packages/<expected_name>.toml`.
    pub fn parse(src: &str, expected_name: &str) -> Result<Self> {
        let m: Manifest = toml::from_str(src).context("invalid manifest TOML")?;
        m.validate(expected_name)?;
        Ok(m)
    }

    /// Checks the name, bin paths, versions, checksums and URL schemes.
    fn validate(&self, expected_name: &str) -> Result<()> {
        if self.name != expected_name {
            bail!("manifest name `{}` does not match file name `{expected_name}`", self.name);
        }
        if !valid_name(&self.name) {
            bail!("invalid package name `{}`", self.name);
        }
        if self.bin.is_empty() {
            bail!("`bin` must list at least one executable");
        }
        for b in &self.bin {
            if !is_safe_relative(Path::new(b)) {
                bail!("invalid bin path `{b}`");
            }
        }
        if self.versions.is_empty() {
            bail!("no versions listed");
        }
        for (v, ver) in &self.versions {
            semver::Version::parse(v).with_context(|| format!("version `{v}` is not semver"))?;
            for (key, t) in &ver.targets {
                if t.sha256.len() != 64 || !t.sha256.chars().all(|c| c.is_ascii_hexdigit()) {
                    bail!("{v}/{key}: sha256 must be 64 hex characters");
                }
                if !(t.url.starts_with("https://")
                    || t.url.starts_with("http://")
                    || t.url.starts_with("file://"))
                {
                    bail!("{v}/{key}: unsupported url scheme");
                }
            }
        }
        Ok(())
    }

    /// Highest stable version (or highest pre-release if there are no stable ones).
    pub fn latest(&self) -> Option<(&str, &Version)> {
        let mut parsed: Vec<_> = self
            .versions
            .iter()
            .filter_map(|(k, v)| semver::Version::parse(k).ok().map(|sv| (sv, k.as_str(), v)))
            .collect();
        parsed.sort_by(|a, b| a.0.cmp(&b.0));
        parsed
            .iter()
            .rev()
            .find(|(sv, _, _)| sv.pre.is_empty())
            .or_else(|| parsed.last())
            .map(|(_, k, v)| (*k, *v))
    }

    /// Picks the requested version (a leading `v` is allowed), or the latest one if none is given.
    pub fn resolve(&self, requested: Option<&str>) -> Result<(&str, &Version)> {
        match requested {
            None => self.latest().context("no versions available"),
            Some(r) => {
                let r = r.strip_prefix('v').unwrap_or(r);
                self.versions
                    .get_key_value(r)
                    .map(|(k, v)| (k.as_str(), v))
                    .with_context(|| {
                        let avail: Vec<_> = self.versions.keys().cloned().collect();
                        format!("{} has no version {r} (available: {})", self.name, avail.join(", "))
                    })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Placeholder checksum for test manifests.
    const SHA: &str = "0000000000000000000000000000000000000000000000000000000000000000";

    /// A valid manifest with stable and pre-release versions.
    fn sample() -> String {
        format!(
            r#"
name = "hello"
description = "hi"
repo = "https://github.com/x/hello"
bin = ["hello"]

[versions."1.0.0".targets.x86_64-linux]
url = "https://example.com/a.tar.gz"
sha256 = "{SHA}"

[versions."1.10.0".targets.x86_64-linux]
url = "https://example.com/b.tar.gz"
sha256 = "{SHA}"

[versions."2.0.0-beta.1".targets.x86_64-linux]
url = "https://example.com/c.tar.gz"
sha256 = "{SHA}"
"#
        )
    }

    #[test]
    fn parse_and_resolve() {
        let m = Manifest::parse(&sample(), "hello").unwrap();
        assert_eq!(m.latest().unwrap().0, "1.10.0");
        assert_eq!(m.resolve(Some("v1.0.0")).unwrap().0, "1.0.0");
        assert_eq!(m.resolve(Some("2.0.0-beta.1")).unwrap().0, "2.0.0-beta.1");
        assert!(m.resolve(Some("9.9.9")).is_err());
    }

    #[test]
    fn rejects_bad_manifests() {
        assert!(Manifest::parse(&sample(), "other").is_err());
        assert!(Manifest::parse(&sample().replace(SHA, "abc"), "hello").is_err());
        assert!(Manifest::parse(&sample().replace("[\"hello\"]", "[\"../x\"]"), "hello").is_err());
        assert!(Manifest::parse(&sample().replace("1.0.0\"", "one\""), "hello").is_err());
    }

    #[test]
    fn ignores_unknown_fields() {
        let extra = sample()
            .replace("bin = [", "homepage = \"https://example.com\"\nbin = [")
            .replace("url = \"https://example.com/a.tar.gz\"", "url = \"https://example.com/a.tar.gz\"\nsize = 123");
        assert!(extra.contains("homepage") && extra.contains("size = 123"));
        let m = Manifest::parse(&extra, "hello").unwrap();
        assert_eq!(m.latest().unwrap().0, "1.10.0");
    }

    #[test]
    fn names() {
        assert!(valid_name("ripgrep"));
        assert!(valid_name("my-tool_2"));
        assert!(!valid_name("../etc"));
        assert!(!valid_name("Upper"));
        assert!(!valid_name(""));
    }
}
