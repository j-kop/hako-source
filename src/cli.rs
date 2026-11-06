use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(name = "hako", version, about = "箱 (hako) — a git-based package manager")]
pub struct Cli {
    #[command(subcommand)]
    pub cmd: Cmd,
}

#[derive(Subcommand, Debug)]
pub enum Cmd {
    /// Clone or pull the package registry
    Update,
    /// Search packages by name or description (lists all without a query)
    Search { query: Option<String> },
    /// Show details about a package
    Info { name: String },
    /// Install packages (`name` or `name@version`)
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
    /// Show or set the registry git URL
    Registry { url: Option<String> },
}
