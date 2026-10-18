//! Beginner setup composes read-only preflight and the existing verified installer.
//! It does not add a second downloader, repair path, or credential manager.
use crate::{
    doctor::{self, Options, Report},
    installation,
};
use anyhow::{Result, ensure};
use std::{
    future::Future,
    path::{Path, PathBuf},
};

#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    DryRun,
    Reused,
    Installed,
}

pub async fn run(
    root: &Path,
    version: Option<String>,
    patchelf: PathBuf,
    dry_run: bool,
) -> Result<Outcome> {
    let report = doctor::examine(
        root,
        &Options {
            requested_version: version,
            patchelf,
            network: false,
        },
    )
    .await?;
    report.print();
    println!("\nSetup plan (runs only after required checks pass):");
    println!("  Claude version: {}", report.selected_version);
    println!("  Managed directory: {}", report.root.display());
    if report.needs_install {
        println!("  Download and verify the official Claude package and pinned Alpine loader.");
        println!("  Activate only after integrity, dependency and launch checks pass.");
    } else {
        println!("  Reuse the healthy active release; no downloads or installation changes.");
    }
    println!(
        "  Existing command shortcuts, npm installations and login credentials are preserved."
    );
    let outcome = apply(&report, dry_run, || async {
        let patcher = report
            .patchelf_path
            .as_deref()
            .ok_or_else(|| anyhow::anyhow!("trusted patchelf executable was not found"))?;
        installation::install(&report.root, &report.selected_version, patcher).await
    })
    .await?;
    match outcome {
        Outcome::DryRun => {
            println!("\nDry run complete. No files were changed and no downloads were made.")
        }
        Outcome::Reused | Outcome::Installed => {
            let executable = std::env::current_exe()?;
            println!("\nReady. Start Claude and follow its sign-in prompts:");
            println!(
                "  {} --root {} run",
                shell_quote(&executable.to_string_lossy()),
                shell_quote(&report.root.to_string_lossy())
            );
        }
    }
    Ok(outcome)
}

async fn apply<F, Fut>(report: &Report, dry_run: bool, installer: F) -> Result<Outcome>
where
    F: FnOnce() -> Fut,
    Fut: Future<Output = Result<()>>,
{
    ensure!(
        report.is_ready(),
        "Setup stopped at preflight. No installation changes were made; fix the failed checks and retry."
    );
    if dry_run {
        return Ok(Outcome::DryRun);
    }
    if !report.needs_install {
        return Ok(Outcome::Reused);
    }
    installer().await?;
    Ok(Outcome::Installed)
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doctor::{Check, Status};
    use std::{cell::Cell, fs};

    fn report(root: &Path, needs_install: bool, ready: bool) -> Report {
        Report {
            schema_version: 1,
            root: root.into(),
            selected_version: "2.1.302".into(),
            installed_version: (!needs_install).then(|| "2.1.302".into()),
            needs_install,
            patchelf_path: None,
            checks: vec![Check {
                id: "fixture",
                status: if ready { Status::Ok } else { Status::Error },
                detail: "test preflight".into(),
                remedy: None,
            }],
        }
    }
    #[tokio::test]
    async fn failed_preflight_never_runs_installer_or_changes_user_files() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let file = directory.path().join("personal.txt");
        fs::write(&file, "keep")?;
        let invoked = Cell::new(false);
        let outcome = apply(&report(directory.path(), true, false), false, || async {
            invoked.set(true);
            fs::remove_file(&file)?;
            Ok(())
        })
        .await;
        assert!(outcome.is_err());
        assert!(!invoked.get());
        assert_eq!(fs::read_to_string(file)?, "keep");
        Ok(())
    }
    #[tokio::test]
    async fn dry_run_and_healthy_reuse_do_not_invoke_installer() -> Result<()> {
        for (needs_install, dry_run, expected) in [
            (true, true, Outcome::DryRun),
            (false, false, Outcome::Reused),
        ] {
            let directory = tempfile::tempdir()?;
            let root = directory.path().join("absent");
            let invoked = Cell::new(false);
            let outcome = apply(&report(&root, needs_install, true), dry_run, || async {
                invoked.set(true);
                fs::create_dir(&root)?;
                Ok(())
            })
            .await?;
            assert_eq!(outcome, expected);
            assert!(!invoked.get());
            assert!(!root.exists());
        }
        Ok(())
    }
    #[tokio::test]
    async fn first_install_uses_the_installer_once() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let invoked = Cell::new(0);
        let outcome = apply(&report(directory.path(), true, true), false, || async {
            invoked.set(invoked.get() + 1);
            Ok(())
        })
        .await?;
        assert_eq!(outcome, Outcome::Installed);
        assert_eq!(invoked.get(), 1);
        Ok(())
    }
    #[tokio::test]
    async fn download_failure_is_reported_and_repeated_attempt_can_retry() -> Result<()> {
        let directory = tempfile::tempdir()?;
        let state = report(directory.path(), true, true);
        let invoked = Cell::new(0);
        let failed = apply(&state, false, || async {
            invoked.set(invoked.get() + 1);
            anyhow::bail!("simulated download failure")
        })
        .await;
        assert!(failed.is_err());
        assert_eq!(fs::read_dir(directory.path())?.count(), 0);
        assert_eq!(
            apply(&state, false, || async {
                invoked.set(invoked.get() + 1);
                Ok(())
            })
            .await?,
            Outcome::Installed
        );
        assert_eq!(invoked.get(), 2);
        Ok(())
    }
    #[test]
    fn printed_commands_quote_paths_without_shell_expansion() {
        assert_eq!(shell_quote("/tmp/a b"), "'/tmp/a b'");
        assert_eq!(shell_quote("/tmp/a'b"), "'/tmp/a'\"'\"'b'");
        assert_eq!(shell_quote("$(touch sentinel)"), "'$(touch sentinel)'");
    }
}
