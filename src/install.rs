use crate::extract::{self, Kind};
use crate::manifest::Manifest;
use crate::paths::Paths;
use crate::platform;
use crate::registry::{split_qualified, Registries};
use crate::state::{Installed, State};
use crate::{download, verify};
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};

pub fn split_spec(spec: &str) -> (&str, Option<&str>) {
    match spec.split_once('@') {
        Some((n, v)) => (n, Some(v)),
        None => (spec, None),
    }
}

fn link_name(bin: &str) -> String {
    Path::new(bin)
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| bin.to_string())
}

pub fn install(paths: &Paths, regs: &Registries, spec: &str, force: bool) -> Result<()> {
    let (qualified, req) = split_spec(spec);
    let (_, bare) = split_qualified(qualified);
    let mut state = State::load(paths)?;
    let previous = state.packages.get(bare).cloned();
    let (reg, m) = regs.find(qualified, previous.as_ref().map(|p| p.registry.as_str()))?;
    let name = m.name.as_str();
    if let Some(p) = &previous {
        if p.registry != reg.name {
            bail!(
                "{name} is already installed from registry `{}`; uninstall it first to switch to `{}`",
                p.registry,
                reg.name
            );
        }
    }
    let (version, ver) = m.resolve(req)?;
    let target_key = platform::current_target()?;
    let target = ver.targets.get(&target_key).with_context(|| {
        let avail: Vec<_> = ver.targets.keys().cloned().collect();
        format!(
            "{name} {version} has no build for {target_key} (available: {})",
            avail.join(", ")
        )
    })?;

    paths.ensure()?;
    if let Some(p) = &previous {
        if p.version == version && !force {
            println!("{name} {version} is already installed (use --force to reinstall)");
            return Ok(());
        }
    }

    // Refuse to clobber executables owned by other packages or not managed by hako.
    let links: Vec<String> = m.bin.iter().map(|b| link_name(b)).collect();
    for l in &links {
        if let Some(owner) = state.owner_of_bin(l) {
            if owner != name {
                let from = &state.packages[owner].registry;
                bail!("`{l}` is already provided by package `{owner}` (from registry `{from}`)");
            }
        } else if paths.bin().join(l).symlink_metadata().is_ok() {
            bail!("{} already exists and is not managed by hako", paths.bin().join(l).display());
        }
    }

    // Download (reusing a verified cached copy if present).
    let file_name = target
        .url
        .rsplit('/')
        .next()
        .filter(|s| !s.is_empty())
        .unwrap_or("download");
    let cached = paths
        .cache()
        .join(format!("{name}-{version}-{target_key}-{file_name}"));
    if !(cached.exists() && verify::verify(&cached, &target.sha256).is_ok()) {
        println!("downloading {name} {version} ({target_key})");
        download::download(&target.url, &cached)?;
        if let Err(e) = verify::verify(&cached, &target.sha256) {
            let _ = std::fs::remove_file(&cached);
            return Err(e).context(format!("refusing to install {name} {version}"));
        }
    }

    // Extract into a staging directory.
    let staging = paths.tmp().join(format!("{name}-{version}-{}", std::process::id()));
    if staging.exists() {
        std::fs::remove_dir_all(&staging)?;
    }
    let result = stage_and_activate(paths, &m, version, &target.url, &cached, &staging);
    let _ = std::fs::remove_dir_all(&staging);
    let final_dir = result?;

    // Swap symlinks.
    if let Some(p) = &previous {
        remove_links(paths, name, &p.bins);
    }
    for (bin, link) in m.bin.iter().zip(&links) {
        let dst = paths.bin().join(link);
        let _ = std::fs::remove_file(&dst);
        symlink(&final_dir.join(bin), &dst)
            .with_context(|| format!("linking {}", dst.display()))?;
    }

    state.packages.insert(
        name.to_string(),
        Installed {
            registry: reg.name.clone(),
            version: version.to_string(),
            target: target_key,
            bins: links.clone(),
        },
    );
    state.save(paths)?;

    if let Some(p) = &previous {
        if p.version != version {
            let _ = std::fs::remove_dir_all(paths.pkgs().join(name).join(&p.version));
        }
    }

    match &previous {
        Some(p) if p.version != version => {
            println!("upgraded {name} {} -> {version}", p.version)
        }
        _ => println!(
            "installed {name} {version} from {} ({})",
            reg.name,
            links.join(", ")
        ),
    }
    path_hint(paths);
    Ok(())
}

/// Extract into `staging`, locate the package root, and move it into `pkgs/<name>/<version>`.
fn stage_and_activate(
    paths: &Paths,
    m: &Manifest,
    version: &str,
    url: &str,
    archive: &Path,
    staging: &Path,
) -> Result<PathBuf> {
    let kind = Kind::from_url(url);
    if kind == Kind::Raw && m.bin.len() != 1 {
        bail!("a raw (non-archive) asset must declare exactly one bin");
    }
    extract::extract(archive, kind, staging, Path::new(&m.bin[0]))?;

    let root = find_root(staging, &m.bin).with_context(|| {
        format!("archive does not contain the declared executables: {}", m.bin.join(", "))
    })?;
    for b in &m.bin {
        make_executable(&root.join(b))?;
    }

    let final_dir = paths.pkgs().join(&m.name).join(version);
    if final_dir.exists() {
        std::fs::remove_dir_all(&final_dir)?;
    }
    std::fs::create_dir_all(paths.pkgs().join(&m.name))?;
    std::fs::rename(&root, &final_dir)
        .with_context(|| format!("moving package into {}", final_dir.display()))?;
    Ok(final_dir)
}

/// The package root is `staging` itself, or its single top-level directory
/// (the common `tool-1.0/` wrapper), whichever contains all declared bins.
fn find_root(staging: &Path, bins: &[String]) -> Option<PathBuf> {
    let has_all = |dir: &Path| bins.iter().all(|b| dir.join(b).is_file());
    if has_all(staging) {
        return Some(staging.to_path_buf());
    }
    let entries: Vec<_> = std::fs::read_dir(staging).ok()?.flatten().collect();
    if let [only] = entries.as_slice() {
        let p = only.path();
        if p.is_dir() && has_all(&p) {
            return Some(p);
        }
    }
    None
}

#[cfg(unix)]
fn make_executable(p: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perm = std::fs::metadata(p)?.permissions();
    perm.set_mode(perm.mode() | 0o755);
    std::fs::set_permissions(p, perm)?;
    Ok(())
}

#[cfg(unix)]
fn symlink(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(src, dst)
}

/// Remove links in `bin/` but only if they point into this package's directory.
fn remove_links(paths: &Paths, name: &str, bins: &[String]) {
    let pkg_dir = paths.pkgs().join(name);
    for b in bins {
        let link = paths.bin().join(b);
        if let Ok(target) = std::fs::read_link(&link) {
            if target.starts_with(&pkg_dir) {
                let _ = std::fs::remove_file(&link);
            }
        }
    }
}

pub fn uninstall(paths: &Paths, name: &str) -> Result<()> {
    let mut state = State::load(paths)?;
    let Some(inst) = state.packages.remove(name) else {
        bail!("{name} is not installed");
    };
    remove_links(paths, name, &inst.bins);
    let dir = paths.pkgs().join(name);
    if dir.exists() {
        std::fs::remove_dir_all(&dir).with_context(|| format!("removing {}", dir.display()))?;
    }
    state.save(paths)?;
    println!("uninstalled {name} {}", inst.version);
    Ok(())
}

pub fn upgrade(paths: &Paths, regs: &Registries, only: Option<&str>) -> Result<()> {
    let state = State::load(paths)?;
    let names: Vec<String> = match only {
        Some(n) => {
            if !state.packages.contains_key(n) {
                bail!("{n} is not installed");
            }
            vec![n.to_string()]
        }
        None => state.packages.keys().cloned().collect(),
    };
    let mut upgraded = 0;
    let mut failed = 0;
    for n in names {
        let installed = &state.packages[&n];
        let Some(reg) = regs.get(&installed.registry) else {
            eprintln!(
                "warning: {n}: registry `{}` is no longer configured; skipping",
                installed.registry
            );
            continue;
        };
        let m = match reg.load(&n) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("warning: {n}: {e:#}");
                continue;
            }
        };
        let Some((latest, _)) = m.latest() else { continue };
        let newer = match (
            semver::Version::parse(latest),
            semver::Version::parse(&installed.version),
        ) {
            (Ok(a), Ok(b)) => a > b,
            _ => false,
        };
        if newer {
            match install(paths, regs, &format!("{}/{n}@{latest}", reg.name), false) {
                Ok(()) => upgraded += 1,
                Err(e) => {
                    eprintln!("error: upgrading {n}: {e:#}");
                    failed += 1;
                }
            }
        }
    }
    if upgraded == 0 && failed == 0 {
        println!("everything is up to date");
    }
    if failed > 0 {
        bail!("{failed} package(s) failed to upgrade");
    }
    Ok(())
}

pub fn bin_on_path(paths: &Paths) -> bool {
    let bin = paths.bin();
    let canon = std::fs::canonicalize(&bin).unwrap_or(bin);
    std::env::var_os("PATH")
        .map(|p| {
            std::env::split_paths(&p)
                .any(|d| std::fs::canonicalize(&d).unwrap_or(d) == canon)
        })
        .unwrap_or(false)
}

fn path_hint(paths: &Paths) {
    if !bin_on_path(paths) {
        eprintln!(
            "note: {} is not on your PATH. Add this to your shell profile:\n  export PATH=\"{}:$PATH\"",
            paths.bin().display(),
            paths.bin().display()
        );
    }
}
