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
use cli::{Cli, Cmd};
use config::Config;
use paths::Paths;
use registry::Registry;
use state::State;

fn main() {
    let cli = Cli::parse();
    if let Err(e) = run(cli) {
        eprintln!("error: {e:#}");
        std::process::exit(1);
    }
}

/// Open the registry, cloning it first if it isn't present yet.
fn registry(paths: &Paths) -> Result<Registry> {
    if paths.registry().exists() {
        return Registry::open(paths);
    }
    let url = Config::load(paths)?.registry_url()?;
    println!("fetching registry {url}");
    registry::update(paths, &url)
}

fn run(cli: Cli) -> Result<()> {
    let paths = Paths::new()?;
    match cli.cmd {
        Cmd::Update => {
            let url = Config::load(&paths)?.registry_url()?;
            let reg = registry::update(&paths, &url)?;
            println!("registry updated ({} packages)", reg.all()?.len());
        }
        Cmd::Search { query } => {
            let reg = registry(&paths)?;
            let q = query.unwrap_or_default().to_lowercase();
            let state = State::load(&paths)?;
            let mut found = 0;
            for m in reg.all()? {
                if q.is_empty()
                    || m.name.contains(&q)
                    || m.description.to_lowercase().contains(&q)
                {
                    let latest = m.latest().map(|(v, _)| v).unwrap_or("-");
                    let mark = if state.packages.contains_key(&m.name) { " [installed]" } else { "" };
                    println!("{:<20} {:<12} {}{mark}", m.name, latest, m.description);
                    found += 1;
                }
            }
            if found == 0 {
                println!("no packages found");
            }
        }
        Cmd::Info { name } => {
            let reg = registry(&paths)?;
            let m = reg.load(&name)?;
            let state = State::load(&paths)?;
            let target = platform::current_target().unwrap_or_default();
            println!("name:        {}", m.name);
            println!("description: {}", m.description);
            println!("repo:        {}", m.repo);
            if let Some(l) = &m.license {
                println!("license:     {l}");
            }
            println!("bin:         {}", m.bin.join(", "));
            match state.packages.get(&m.name) {
                Some(i) => println!("installed:   {}", i.version),
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
            let reg = registry(&paths)?;
            let mut failed = 0;
            for p in &packages {
                if let Err(e) = install::install(&paths, &reg, p, force) {
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
                install::uninstall(&paths, p)?;
            }
        }
        Cmd::List => {
            let state = State::load(&paths)?;
            if state.packages.is_empty() {
                println!("no packages installed");
            }
            for (n, i) in &state.packages {
                println!("{n:<20} {:<12} {}", i.version, i.bins.join(", "));
            }
        }
        Cmd::Upgrade { name } => {
            let url = Config::load(&paths)?.registry_url()?;
            let reg = registry::update(&paths, &url)?;
            install::upgrade(&paths, &reg, name.as_deref())?;
        }
        Cmd::Doctor => doctor(&paths)?,
        Cmd::Registry { url } => {
            let mut cfg = Config::load(&paths)?;
            match url {
                Some(u) => {
                    cfg.registry = Some(u.clone());
                    cfg.save(&paths)?;
                    println!("registry set to {u}; run `hako update`");
                }
                None => match cfg.registry_url() {
                    Ok(u) => println!("{u}"),
                    Err(e) => println!("{e}"),
                },
            }
        }
    }
    Ok(())
}

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
    match Config::load(paths).and_then(|c| c.registry_url()) {
        Ok(u) => check(true, format!("registry configured ({u})")),
        Err(e) => check(false, format!("{e:#}")),
    }
    match Registry::open(paths) {
        Ok(_) => check(true, "registry cloned and schema supported".into()),
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
