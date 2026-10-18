use clap::{Parser, Subcommand};
use std::path::PathBuf;

#[derive(Debug, Parser)]
#[command(version, about)]
pub struct Cli {
    /// Installation directory. Defaults to ~/.local/share/claude-termux.
    #[arg(long, global = true)]
    pub root: Option<PathBuf>,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Check this device without changing files. Network checks are opt-in.
    Doctor {
        #[arg(long)]
        network: bool,
        /// Print a stable JSON report; exit 1 when a required check fails.
        #[arg(long)]
        json: bool,
        #[arg(long, default_value = "patchelf")]
        patchelf: PathBuf,
    },
    /// Check this device and install a verified Claude release when needed.
    Setup {
        /// Retain a healthy installed version by default; otherwise use the reviewed default.
        #[arg(long)]
        version: Option<String>,
        /// Show checks and the installation plan without downloads or file changes.
        #[arg(long)]
        dry_run: bool,
        #[arg(long, default_value = "patchelf")]
        patchelf: PathBuf,
    },
    /// Fetch, authenticate, inspect and stage a specific official release.
    Install {
        #[arg(long)]
        version: String,
        /// Trusted Termux patchelf executable (never downloaded implicitly).
        #[arg(long, default_value = "patchelf")]
        patchelf: PathBuf,
    },
    /// Verify installed hashes and display the recorded dependency report.
    Inspect,
    /// Run a foreground session with our authenticated DNS proxy.
    Run {
        /// Additional exact HTTPS hostname to admit through the proxy.
        #[arg(long = "allow-host")]
        allow_hosts: Vec<String>,
        #[arg(last = true)]
        args: Vec<String>,
    },
    /// Activate the previous verified release.
    Rollback,
    /// Remove this tool's managed installation, preserving Claude credentials.
    Uninstall,
}
