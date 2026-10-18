//! Black-box checks work on both supported Termux and unsupported Linux CI.
//! Unsupported hosts still produce useful JSON and must never initialize state.
use anyhow::{Context, Result};
use std::{
    fs,
    path::Path,
    process::{Command, Output},
};

fn invoke(root: &Path, arguments: &[&str]) -> Result<Output> {
    let home = root
        .parent()
        .context("test parent missing")?
        .join("home-not-created");
    Ok(Command::new(env!("CARGO_BIN_EXE_claude-termux"))
        .arg("--root")
        .arg(root)
        .args(arguments)
        .env("HOME", &home)
        .output()?)
}

#[test]
fn offline_doctor_json_does_not_create_installation_or_home() -> Result<()> {
    let directory = tempfile::tempdir()?;
    let root = directory.path().join("installation");
    let output = invoke(
        &root,
        &[
            "doctor",
            "--json",
            "--patchelf",
            "/definitely/missing/patchelf",
        ],
    )?;
    let report: serde_json::Value = serde_json::from_slice(&output.stdout)?;
    assert_eq!(report["schema_version"], 1);
    assert_eq!(report["needs_install"], true);
    let checks = report["checks"].as_array().context("checks missing")?;
    assert!(
        checks
            .iter()
            .any(|c| c["id"] == "network" && c["status"] == "skipped")
    );
    assert!(
        checks
            .iter()
            .any(|c| c["id"] == "patchelf" && c["status"] == "error")
    );
    assert!(!output.status.success());
    assert!(!root.exists());
    assert!(!directory.path().join("home-not-created").exists());
    Ok(())
}

#[test]
fn dry_run_and_invalid_version_never_initialize_state() -> Result<()> {
    for arguments in [
        vec![
            "setup",
            "--dry-run",
            "--patchelf",
            "/definitely/missing/patchelf",
        ],
        vec!["setup", "--version", "latest"],
    ] {
        let directory = tempfile::tempdir()?;
        let root = directory.path().join("installation");
        let output = invoke(&root, &arguments)?;
        assert!(!output.status.success());
        assert!(!root.exists());
        assert!(!directory.path().join("home-not-created").exists());
    }
    Ok(())
}

#[test]
fn setup_preserves_an_unmanaged_directory() -> Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let directory = tempfile::tempdir()?;
    let root = directory.path().join("installation");
    fs::create_dir(&root)?;
    fs::set_permissions(&root, fs::Permissions::from_mode(0o700))?;
    fs::write(root.join("personal.txt"), "keep me")?;
    let output = invoke(&root, &["setup"])?;
    assert!(!output.status.success());
    assert_eq!(fs::read_to_string(root.join("personal.txt"))?, "keep me");
    assert_eq!(fs::read_dir(&root)?.count(), 1);
    Ok(())
}
