use anyhow::{bail, Result};

/// Target key used in manifests, e.g. `x86_64-linux`, `aarch64-macos`.
pub fn current_target() -> Result<String> {
    target_for(std::env::consts::ARCH, std::env::consts::OS)
}

pub fn target_for(arch: &str, os: &str) -> Result<String> {
    let arch = match arch {
        "x86_64" | "aarch64" => arch,
        other => bail!("unsupported architecture: {other}"),
    };
    let os = match os {
        "linux" | "macos" => os,
        other => bail!("unsupported operating system: {other}"),
    };
    Ok(format!("{arch}-{os}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn targets() {
        assert_eq!(target_for("x86_64", "linux").unwrap(), "x86_64-linux");
        assert_eq!(target_for("aarch64", "macos").unwrap(), "aarch64-macos");
        assert!(target_for("x86", "linux").is_err());
        assert!(target_for("x86_64", "windows").is_err());
    }
}
