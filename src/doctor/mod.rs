//! Read-only preflight shared by doctor and setup. Networking requires opt-in.
mod local;
mod network;

use anyhow::Result;
use serde::Serialize;
use std::path::{Path, PathBuf};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Ok,
    Warning,
    Error,
    Skipped,
}

#[derive(Debug, Serialize)]
pub struct Check {
    pub id: &'static str,
    pub status: Status,
    pub detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub remedy: Option<String>,
}
impl Check {
    fn new(
        id: &'static str,
        status: Status,
        detail: impl Into<String>,
        remedy: Option<&str>,
    ) -> Self {
        Self {
            id,
            status,
            detail: detail.into(),
            remedy: remedy.map(str::to_owned),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct Report {
    pub schema_version: u32,
    pub root: PathBuf,
    pub selected_version: String,
    pub installed_version: Option<String>,
    pub needs_install: bool,
    pub patchelf_path: Option<PathBuf>,
    pub checks: Vec<Check>,
}
impl Report {
    pub fn is_ready(&self) -> bool {
        self.checks.iter().all(|c| c.status != Status::Error)
    }
    pub fn print(&self) {
        println!("Claude Termux checks");
        for check in &self.checks {
            let label = match check.status {
                Status::Ok => "OK",
                Status::Warning => "WARN",
                Status::Error => "FAIL",
                Status::Skipped => "SKIP",
            };
            println!("[{label}] {}: {}", check.id, check.detail);
            if let Some(remedy) = &check.remedy {
                println!("       {remedy}");
            }
        }
        if self.is_ready() {
            println!("Required checks passed.");
        } else {
            println!("Fix the failed checks before setup. No automatic repairs were made.");
        }
    }
}

pub struct Options {
    pub network: bool,
    pub requested_version: Option<String>,
    pub patchelf: PathBuf,
}
impl Default for Options {
    fn default() -> Self {
        Self {
            network: false,
            requested_version: None,
            patchelf: PathBuf::from("patchelf"),
        }
    }
}

pub async fn examine(root: &Path, options: &Options) -> Result<Report> {
    let mut report = local::examine(root, options)?;
    if options.network {
        report
            .checks
            .extend(network::examine(&report.selected_version).await);
    } else {
        report.checks.push(Check::new("network",Status::Skipped,"Not requested; no network checks were made.",Some("Use doctor --network to test verified HTTPS connectivity without account credentials.")));
    }
    Ok(report)
}
