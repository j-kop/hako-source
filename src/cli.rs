//! Command-line interface, defined with clap.

use clap::{Parser, Subcommand};

/// Parsed command line.
#[derive(Parser, Debug)]
#[command(name = "hako", version, about = "箱 (hako) — a git-based package manager")]
pub struct Cli {
    #[command(subcommand)]
    pub cmd: Cmd,
}

/// Top-level commands.
#[derive(Subcommand, Debug)]
pub enum Cmd {
    /// Clone or pull the package registry
    Update,
    /// Search packages by name or description (lists all without a query)
    Search { query: Option<String> },
    /// Show details about a package (`name` or `registry/name`)
    Info { name: String },
    /// Install packages (`name`, `name@version`, `registry/name[@version]`)
    Install {
        #[arg(required = true)]
        packages: Vec<String>,
        /// Reinstall even if the same version is already installed
        #[arg(long)]
        force: bool,
    },
    /// Uninstall packages
    #[command(alias = "remove")]
    Uninstall {
        #[arg(required = true)]
        packages: Vec<String>,
    },
    /// List installed packages
    #[command(alias = "ls")]
    List,
    /// Update the registry and upgrade installed packages
    Upgrade { name: Option<String> },
    /// Check the hako environment for problems
    Doctor,
    /// Manage registries (lists them when no subcommand is given)
    Registry {
        #[command(subcommand)]
        action: Option<RegistryCmd>,
    },
}

/// Subcommands of `hako registry`.
#[derive(Subcommand, Debug)]
pub enum RegistryCmd {
    /// List configured registries
    #[command(alias = "ls")]
    List,
    /// Add a registry, or change the URL of an existing one
    Add { name: String, url: String },
    /// Remove a registry (installed packages stay installed)
    #[command(alias = "rm")]
    Remove { name: String },
}
