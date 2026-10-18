//! Native Termux compatibility, with explicit artifact trust and no global setup.
pub mod artifacts;
pub mod cli;
pub mod doctor;
pub mod installation;
pub mod launcher;
pub mod proxy;
pub mod setup;

/// Reviewed fallback for first-time setup; existing healthy installs are retained.
pub const DEFAULT_CLAUDE_VERSION: &str = "2.1.302";
