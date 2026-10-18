use super::{Check, Options, Report, Status};
use crate::{
    DEFAULT_CLAUDE_VERSION,
    artifacts::npm,
    installation::{self, InstallationState},
};
use anyhow::{Context, Result};
use nix::{
    sys::statvfs::{FsFlags, statvfs},
    unistd::{AccessFlags, access},
};
use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
};

/// Includes headroom above the current archive plus two executable copies.
const REQUIRED_BYTES: u64 = 768 * 1024 * 1024;

struct Host {
    os: &'static str,
    architecture: &'static str,
    shell: PathBuf,
    certificates: PathBuf,
    search_paths: Vec<PathBuf>,
}
impl Host {
    fn current() -> Self {
        Self {
            os: std::env::consts::OS,
            architecture: std::env::consts::ARCH,
            shell: PathBuf::from("/data/data/com.termux/files/usr/bin/bash"),
            certificates: PathBuf::from("/data/data/com.termux/files/usr/etc/tls/cert.pem"),
            search_paths: std::env::var_os("PATH")
                .map(|p| std::env::split_paths(&p).collect())
                .unwrap_or_default(),
        }
    }
}

pub(super) fn examine(root: &Path, options: &Options) -> Result<Report> {
    examine_host(root, options, &Host::current())
}

fn examine_host(root: &Path, options: &Options, host: &Host) -> Result<Report> {
    if let Some(version) = &options.requested_version {
        npm::validate_version(version)?;
    }
    let mut checks = Vec::new();
    let platform_ok = host.os == "android" && host.architecture == "aarch64";
    checks.push(Check::new("platform",if platform_ok{Status::Ok}else{Status::Error},format!("{} / {}",host.os,host.architecture),if platform_ok{None}else{Some("This release supports ARM64 (aarch64) Termux on Android only; a Linux build can run tests but cannot install Claude.")}));
    let shell_ok = executable(&host.shell);
    checks.push(Check::new(
        "shell",
        if shell_ok { Status::Ok } else { Status::Error },
        if shell_ok {
            "Termux Bash is available."
        } else {
            "Termux Bash is missing or not executable."
        },
        if shell_ok {
            None
        } else {
            Some("Install Bash in the standard com.termux environment: pkg install bash")
        },
    ));
    let certificates_ok = fs::File::open(&host.certificates)
        .and_then(|file| file.metadata())
        .is_ok_and(|m| m.is_file() && m.len() > 0);
    checks.push(Check::new(
        "certificates",
        if certificates_ok {
            Status::Ok
        } else {
            Status::Error
        },
        if certificates_ok {
            "CA bundle is readable; use --network to test certificate trust."
        } else {
            "Termux CA bundle is missing, empty or unreadable."
        },
        if certificates_ok {
            None
        } else {
            Some("Install trusted certificates: pkg install ca-certificates")
        },
    ));

    let state = installation::inspect_state_readonly(root);
    let installed_version = match &state {
        Ok(InstallationState::Healthy(manifest)) => Some(manifest.claude_version.clone()),
        _ => None,
    };
    let selected_version = options
        .requested_version
        .clone()
        .or_else(|| installed_version.clone())
        .unwrap_or_else(|| DEFAULT_CLAUDE_VERSION.into());
    let needs_install = installed_version.as_deref() != Some(selected_version.as_str());
    checks.push(match state {
        Ok(InstallationState::Healthy(_))=>Check::new("installation",Status::Ok,format!("Claude {} passed installed-file integrity checks.",installed_version.as_deref().unwrap_or("unknown")),None),
        Ok(InstallationState::Missing|InstallationState::Empty)=>Check::new("installation",Status::Warning,"No active installation; setup can create one.",None),
        Ok(InstallationState::Incomplete)=>Check::new("installation",Status::Warning,"Managed directory has no active release; setup can retry a previous incomplete install.",None),
        Err(error)=>Check::new("installation",Status::Error,format!("Installation cannot be safely reused: {error:#}"),Some("Keep existing files. Retry if an installer is running; otherwise inspect the directory or select a new empty --root. Setup will not erase or repair it.")),
    });
    let patchelf_path = find_executable(&options.patchelf, &host.search_paths);
    checks.push(match &patchelf_path {
        Some(path)=>Check::new("patchelf",Status::Ok,format!("Executable found at {} (not run by doctor).",path.display()),None),
        None=>Check::new("patchelf",if needs_install{Status::Error}else{Status::Warning},if needs_install{"Required to install this release but not found."}else{"Not found; the verified active release can still be reused."},Some("Install the trusted Termux package: pkg install patchelf; or provide --patchelf /path/to/a/trusted/executable.")),
    });
    checks.push(match storage(root, needs_install) {
        Ok(check) => check,
        Err(error) => Check::new(
            "storage",
            if needs_install {
                Status::Error
            } else {
                Status::Warning
            },
            format!("Could not assess storage: {error:#}"),
            Some("Check access to the installation directory and available storage."),
        ),
    });
    Ok(Report {
        schema_version: 1,
        root: root.to_path_buf(),
        selected_version,
        installed_version,
        needs_install,
        patchelf_path,
        checks,
    })
}

fn executable(path: &Path) -> bool {
    fs::metadata(path).is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0)
}
fn find_executable(requested: &Path, search_paths: &[PathBuf]) -> Option<PathBuf> {
    if requested.is_absolute() || requested.components().count() > 1 {
        return executable(requested)
            .then(|| fs::canonicalize(requested).ok())
            .flatten();
    }
    search_paths
        .iter()
        .map(|directory| directory.join(requested))
        .find(|path| executable(path))
        .and_then(|path| fs::canonicalize(path).ok())
}
fn nearest_existing(root: &Path) -> Result<&Path> {
    let mut path = root;
    loop {
        match fs::metadata(path) {
            Ok(metadata) if metadata.is_dir() => return Ok(path),
            Ok(_) => anyhow::bail!("installation ancestor is not a directory"),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                path = path.parent().context("no existing installation ancestor")?
            }
            Err(error) => return Err(error.into()),
        }
    }
}
fn storage(root: &Path, needs_install: bool) -> Result<Check> {
    let parent = nearest_existing(root)?;
    let stats = statvfs(parent)?;
    if stats.flags().contains(FsFlags::ST_NOEXEC) {
        return Ok(Check::new(
            "storage",
            Status::Error,
            "Filesystem forbids executing binaries.",
            Some("Choose a private Termux home directory, not Android shared storage."),
        ));
    }
    if needs_install
        && (stats.flags().contains(FsFlags::ST_RDONLY)
            || access(parent, AccessFlags::W_OK | AccessFlags::X_OK).is_err())
    {
        return Ok(Check::new(
            "storage",
            Status::Error,
            "Installation ancestor is not writable/searchable.",
            Some("Choose a writable directory in your private Termux home."),
        ));
    }
    let available = stats
        .blocks_available()
        .saturating_mul(stats.fragment_size());
    Ok(space_check(available, needs_install))
}
fn space_check(available: u64, needs_install: bool) -> Check {
    let enough = available >= REQUIRED_BYTES;
    Check::new(
        "storage",
        if enough {
            Status::Ok
        } else if needs_install {
            Status::Error
        } else {
            Status::Warning
        },
        format!(
            "{} MiB available; {} MiB headroom required for a new install.",
            available / (1024 * 1024),
            REQUIRED_BYTES / (1024 * 1024)
        ),
        if enough {
            None
        } else {
            Some(
                "Free storage before installing another version. Reusing the healthy active version needs no new download.",
            )
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn host(directory: &Path) -> Result<Host> {
        let shell = directory.join("bash");
        fs::write(&shell, b"test shell, never executed")?;
        fs::set_permissions(&shell, fs::Permissions::from_mode(0o700))?;
        let certificates = directory.join("cert.pem");
        fs::write(&certificates, b"public CA presence fixture")?;
        Ok(Host {
            os: "android",
            architecture: "aarch64",
            shell,
            certificates,
            search_paths: vec![directory.into()],
        })
    }
    #[test]
    fn preflight_does_not_create_missing_root_or_execute_dependencies() -> Result<()> {
        let temporary = tempfile::tempdir()?;
        let host = host(temporary.path())?;
        let patcher = temporary.path().join("patchelf");
        fs::write(&patcher, b"#!/bin/sh\ntouch should-not-exist\n")?;
        fs::set_permissions(&patcher, fs::Permissions::from_mode(0o700))?;
        let root = temporary.path().join("new/install");
        let report = examine_host(&root, &Options::default(), &host)?;
        assert!(report.is_ready());
        assert!(report.needs_install);
        assert!(!root.exists());
        assert_eq!(report.selected_version, DEFAULT_CLAUDE_VERSION);
        assert!(!temporary.path().join("should-not-exist").exists());
        Ok(())
    }
    #[test]
    fn missing_patcher_has_an_actionable_error_without_changes() -> Result<()> {
        let temporary = tempfile::tempdir()?;
        let host = host(temporary.path())?;
        let root = temporary.path().join("absent");
        let report = examine_host(&root, &Options::default(), &host)?;
        let check = report
            .checks
            .iter()
            .find(|check| check.id == "patchelf")
            .context("patcher check missing")?;
        assert_eq!(check.status, Status::Error);
        assert!(
            check
                .remedy
                .as_ref()
                .is_some_and(|s| s.contains("pkg install patchelf"))
        );
        assert!(!root.exists());
        Ok(())
    }
    #[test]
    fn installation_space_threshold_does_not_block_reusing_healthy_release() {
        assert_eq!(space_check(REQUIRED_BYTES - 1, true).status, Status::Error);
        assert_eq!(space_check(REQUIRED_BYTES, true).status, Status::Ok);
        assert_eq!(space_check(0, false).status, Status::Warning);
    }
    #[test]
    fn foreign_files_are_preserved_and_block_setup() -> Result<()> {
        let temporary = tempfile::tempdir()?;
        let host = host(temporary.path())?;
        let root = temporary.path().join("existing");
        fs::create_dir(&root)?;
        fs::set_permissions(&root, fs::Permissions::from_mode(0o700))?;
        let personal = root.join("personal.txt");
        fs::write(&personal, "keep")?;
        let report = examine_host(&root, &Options::default(), &host)?;
        assert!(!report.is_ready());
        assert_eq!(fs::read_to_string(personal)?, "keep");
        assert!(!root.join(".lock").exists());
        Ok(())
    }
}
