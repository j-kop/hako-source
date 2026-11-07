//! hako (箱): a git-based package manager.
//! Registries are git repos of package manifests; packages are prebuilt binaries installed into `~/.hako`.

mod cli;
mod config;
mod download;
mod extract;
mod install;
mod manifest;
mod paths;
mod platform;
mod registry;
mod state;
mod verify;

use anyhow::{bail, Result};
use clap::Parser;
use cli::{Cli, Cmd, RegistryCmd};
use config::Config;
use paths::Paths;
use registry::{Registries, Registry};
use state::State;

/// Entry point: runs the command and exits with status 1 on error.
fn main() {
    let cli = Cli::parse();
    if let Err(e) = run(cli) {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}

/// Open all configured registries, cloning any that aren't present yet.
/// A registry that fails is reported and skipped.
fn open_registries(paths: &Paths) -> Result<Registries> {
    let mut list = Vec::new();
    for (name, url) in Config::load(paths)?.registries()? {
        let result = if paths.registry(&name).exists() {
            Registry::open(paths, &name)
        } else {
            println!("fetching registry {name} ({url})");
            registry::update(paths, &name, &url)
        };
        match result {
            Ok(r) => list.push(r),
            Err(e) => eprintln!("warning: {e:#}"),
        }
    }
    if list.is_empty() {
        bail!("no usable registries");
    }
    Ok(Registries { list })
}

/// Pull every configured registry. Returns the registries that updated and the failure count.
fn update_all(paths: &Paths) -> Result<(Registries, usize)> {
    let mut list = Vec::new();
    let mut failed = 0;
    for (name, url) in Config::load(paths)?.registries()? {
        match registry::update(paths, &name, &url) {
            Ok(r) => {
                println!("updated {name} ({} packages)", r.all()?.len());
                list.push(r);
            }
            Err(e) => {
                eprintln!("error: {e:#}");
                failed += 1;
            }
        }
    }
    Ok((Registries { list }, failed))
}

/// Runs one CLI command.
fn run(cli: Cli) -> Result<()> {
    let paths = Paths::new()?;
    // Held until hako exits, so concurrent runs wait instead of overwriting each other's changes.
    let _lock = paths.lock()?;
    paths.migrate()?;
    match cli.cmd {
        Cmd::Update => {
            let (_, failed) = update_all(&paths)?;
            if failed > 0 {
                bail!("{failed} registry(ies) failed to update");
            }
        }
        Cmd::Search { query } => {
            let regs = open_registries(&paths)?;
            let q = query.unwrap_or_default().to_lowercase();
            let state = State::load(&paths)?;
            let mut found = 0;
            for reg in &regs.list {
                for m in reg.all()? {
                    if !(q.is_empty()
                        || m.name.contains(&q)
                        || m.description.to_lowercase().contains(&q))
                    {
                        continue;
                    }
                    let latest = m.latest().map(|(v, _)| v).unwrap_or("-");
                    let mark = match state.packages.get(&m.name) {
                        Some(i) if i.registry == reg.name => " [installed]",
                        _ => "",
                    };
                    println!(
                        "{:<20} {:<10} {:<12} {}{mark}",
                        m.name, reg.name, latest, m.description
                    );
                    found += 1;
                }
            }
            if found == 0 {
                println!("no packages found");
            }
        }
        Cmd::Info { name } => {
            let regs = open_registries(&paths)?;
            let state = State::load(&paths)?;
            let (_, bare) = registry::split_qualified(&name);
            let prefer = state.packages.get(bare).map(|i| i.registry.as_str());
            let (reg, m) = regs.find(&name, prefer)?;
            let target = platform::current_target().unwrap_or_default();
            println!("name:        {}", m.name);
            println!("registry:    {}", reg.name);
            println!("description: {}", m.description);
            println!("repo:        {}", m.repo);
            if let Some(l) = &m.license {
                println!("license:     {l}");
            }
            println!("bin:         {}", m.bin.join(", "));
            match state.packages.get(&m.name) {
                Some(i) if i.registry == reg.name => println!("installed:   {}", i.version),
                Some(i) => println!("installed:   {} (from registry {})", i.version, i.registry),
                None => println!("installed:   no"),
            }
            println!("versions:");
            let mut versions: Vec<_> = m.versions.iter().collect();
            versions.sort_by(|a, b| {
                semver::Version::parse(b.0)
                    .ok()
                    .cmp(&semver::Version::parse(a.0).ok())
            });
            for (v, ver) in versions {
                let targets: Vec<_> = ver
                    .targets
                    .keys()
                    .map(|t| if *t == target { format!("{t}*") } else { t.clone() })
                    .collect();
                println!("  {v:<12} {}", targets.join(" "));
            }
        }
        Cmd::Install { packages, force } => {
            let regs = open_registries(&paths)?;
            let mut failed = 0;
            for p in &packages {
                if let Err(e) = install::install(&paths, &regs, p, force) {
                    eprintln!("error: {p}: {e:#}");
                    failed += 1;
                }
            }
            if failed > 0 {
                bail!("{failed} package(s) failed to install");
            }
        }
        Cmd::Uninstall { packages } => {
            for p in &packages {
                let (_, bare) = registry::split_qualified(p);
                install::uninstall(&paths, bare)?;
            }
        }
        Cmd::List => {
            let state = State::load(&paths)?;
            if state.packages.is_empty() {
                println!("no packages installed");
            }
            for (n, i) in &state.packages {
                println!(
                    "{n:<20} {:<10} {:<12} {}",
                    i.registry,
                    i.version,
                    i.bins.join(", ")
                );
            }
        }
        Cmd::Upgrade { name } => {
            let (regs, failed) = update_all(&paths)?;
            if failed > 0 {
                eprintln!("warning: some registries failed to update; upgrading from the rest");
            }
            let only = name.as_deref().map(|n| registry::split_qualified(n).1);
            install::upgrade(&paths, &regs, only)?;
        }
        Cmd::Doctor => doctor(&paths)?,
        Cmd::Registry { action } => registry_cmd(&paths, action.unwrap_or(RegistryCmd::List))?,
    }
    Ok(())
}

/// Handles `hako registry list|add|remove`.
fn registry_cmd(paths: &Paths, action: RegistryCmd) -> Result<()> {
    let mut cfg = Config::load(paths)?;
    match action {
        RegistryCmd::List => {
            for (name, url) in cfg.registries()? {
                let status = match Registry::open(paths, &name).and_then(|r| r.all()) {
                    Ok(pkgs) => format!("{} packages", pkgs.len()),
                    Err(_) if !paths.registry(&name).exists() => "not cloned yet".into(),
                    Err(e) => format!("error: {e:#}"),
                };
                println!("{name:<12} {url}  ({status})");
            }
        }
        RegistryCmd::Add { name, url } => {
            let old = cfg.add(&name, &url)?;
            // Clone before saving, so a bad URL doesn't end up in the config.
            let reg = registry::update(paths, &name, &url)?;
            cfg.save(paths)?;
            let n = reg.all()?.len();
            match old {
                Some(o) if o != url => println!("updated registry {name}: {o} -> {url} ({n} packages)"),
                Some(_) => println!("registry {name} is already configured ({n} packages)"),
                None => println!("added registry {name} ({n} packages)"),
            }
        }
        RegistryCmd::Remove { name } => {
            if !cfg.remove(&name) {
                bail!("no registry named `{name}`");
            }
            cfg.save(paths)?;
            let dir = paths.registry(&name);
            if dir.exists() {
                std::fs::remove_dir_all(&dir)?;
            }
            println!("removed registry {name}");
            let orphans: Vec<_> = State::load(paths)?
                .packages
                .into_iter()
                .filter(|(_, i)| i.registry == name)
                .map(|(n, _)| n)
                .collect();
            if !orphans.is_empty() {
                println!(
                    "note: still installed from {name} (won't be upgraded): {}",
                    orphans.join(", ")
                );
            }
        }
    }
    Ok(())
}

/// Checks the environment and prints one ok/FAIL line per check.
fn doctor(paths: &Paths) -> Result<()> {
    let mut ok = true;
    let mut check = |good: bool, msg: String| {
        println!("{} {msg}", if good { "ok  " } else { "FAIL" });
        ok &= good;
    };
    let git = std::process::Command::new("git").arg("--version").output();
    check(
        matches!(&git, Ok(o) if o.status.success()),
        "git is installed".into(),
    );
    match platform::current_target() {
        Ok(t) => check(true, format!("platform supported ({t})")),
        Err(e) => check(false, format!("{e}")),
    }
    match Config::load(paths).and_then(|c| c.registries()) {
        Ok(regs) => {
            for (name, url) in regs {
                match Registry::open(paths, &name) {
                    Ok(_) => check(true, format!("registry {name} ({url})")),
                    Err(e) => check(false, format!("{e:#}")),
                }
            }
        }
        Err(e) => check(false, format!("{e:#}")),
    }
    check(
        install::bin_on_path(paths),
        format!("{} is on PATH", paths.bin().display()),
    );
    if !ok {
        bail!("some checks failed");
    }
    Ok(())
}
