//! Immutable releases plus one atomic activation record. No PATH or npm mutations.
use crate::artifacts::{self, alpine, download, elf, npm};
use anyhow::{Context, Result, ensure};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::{
    fs::{self, File, OpenOptions},
    io::Write,
    os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    time::Duration,
};
use tokio::{process::Command, time::timeout};

const OWNER: &str = "claude-termux-install-v1\n";
const SCHEMA: u32 = 1;

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub schema: u32,
    pub tool_version: String,
    pub claude_version: String,
    pub alpine_version: String,
    pub npm_integrity: String,
    pub original_sha256: String,
    pub binary_sha256: String,
    pub loader_sha256: String,
    pub original_elf: elf::Report,
    pub patched_elf: elf::Report,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Activation {
    schema: u32,
    current: String,
    previous: Option<String>,
}

pub fn resolve_root(root: Option<PathBuf>) -> Result<PathBuf> {
    let root = match root {
        Some(path) => path,
        None => PathBuf::from(std::env::var_os("HOME").context("HOME is not set; specify --root")?)
            .join(".local/share/claude-termux"),
    };
    let absolute = if root.is_absolute() {
        root
    } else {
        std::env::current_dir()?.join(root)
    };
    ensure!(
        absolute.file_name().is_some()
            && !absolute
                .components()
                .any(|c| matches!(c, std::path::Component::ParentDir)),
        "invalid installation root"
    );
    Ok(absolute)
}

pub(crate) fn private_directory(path: &Path) -> Result<()> {
    if path.exists() {
        let metadata = fs::symlink_metadata(path)?;
        ensure!(
            metadata.is_dir() && !metadata.file_type().is_symlink(),
            "managed directory must not be a symlink"
        );
        ensure!(
            metadata.permissions().mode() & 0o077 == 0,
            "managed directory must be private (mode 0700)"
        );
    } else {
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(path)?;
    }
    Ok(())
}

fn regular_file(path: &Path) -> Result<File> {
    ensure!(
        fs::symlink_metadata(path)?.is_file(),
        "expected a regular managed file"
    );
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)?;
    ensure!(
        file.metadata()?.is_file(),
        "expected a regular managed file"
    );
    Ok(file)
}
fn read_json<T: serde::de::DeserializeOwned>(path: &Path) -> Result<T> {
    Ok(serde_json::from_slice(&artifacts::read_bounded(
        regular_file(path)?,
        1024 * 1024,
    )?)?)
}
fn check_owner(root: &Path) -> Result<()> {
    private_directory_existing(root)?;
    let owner = artifacts::read_bounded(regular_file(&root.join(".owner"))?, 128)?;
    ensure!(
        owner == OWNER.as_bytes(),
        "not a managed claude-termux installation"
    );
    Ok(())
}
fn private_directory_existing(path: &Path) -> Result<()> {
    ensure!(path.exists(), "installation does not exist");
    private_directory(path)
}

/// A live session keeps a shared lock. Mutation fails rather than replacing a
/// release while a process may re-execute its own binary or loader.
pub fn lock(root: &Path, exclusive: bool) -> Result<File> {
    check_owner(root)?;
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW)
        .open(root.join(".lock"))?;
    if exclusive {
        FileExt::try_lock_exclusive(&file)
    } else {
        FileExt::try_lock_shared(&file)
    }
    .context("installation is busy; close active sessions and retry")?;
    Ok(file)
}

fn initialize(root: &Path) -> Result<()> {
    private_directory(root)?;
    let marker = root.join(".owner");
    if !marker.exists() {
        ensure!(
            fs::read_dir(root)?.next().is_none(),
            "refusing to adopt a nonempty directory"
        );
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(marker)?;
        file.write_all(OWNER.as_bytes())?;
        file.sync_all()?;
    }
    check_owner(root)?;
    private_directory(&root.join("releases"))?;
    Ok(())
}

fn write_json(path: &Path, value: &impl Serialize) -> Result<()> {
    let parent = path.parent().context("missing parent directory")?;
    let mut staged = tempfile::NamedTempFile::new_in(parent)?;
    staged
        .as_file_mut()
        .write_all(&serde_json::to_vec_pretty(value)?)?;
    staged.as_file_mut().write_all(b"\n")?;
    staged.as_file().sync_all()?;
    staged.persist(path).map_err(|e| e.error)?;
    File::open(parent)?.sync_all()?;
    Ok(())
}

fn activation(root: &Path) -> Result<Activation> {
    let record: Activation = read_json(&root.join("active.json"))?;
    ensure!(record.schema == SCHEMA, "unsupported activation schema");
    npm::validate_version(&record.current)?;
    if let Some(previous) = &record.previous {
        npm::validate_version(previous)?;
    }
    Ok(record)
}
pub fn release_path(root: &Path, version: &str) -> Result<PathBuf> {
    npm::validate_version(version)?;
    Ok(root.join("releases").join(version))
}
fn verify_release(root: &Path, version: &str) -> Result<Manifest> {
    let release = release_path(root, version)?;
    private_directory_existing(&release)?;
    let manifest: Manifest = read_json(&release.join("manifest.json"))?;
    ensure!(
        manifest.schema == SCHEMA
            && manifest.claude_version == version
            && manifest.alpine_version == alpine::VERSION,
        "release identity mismatch"
    );
    let binary = release.join("claude");
    let loader = release.join("loader");
    let original = release.join("claude.original");
    for path in [&binary, &loader, &original] {
        regular_file(path)?;
    }
    let (binary_hash, loader_hash, original_hash) = std::thread::scope(|s| {
        let h1 = s.spawn(|| artifacts::sha256_file(&binary));
        let h2 = s.spawn(|| artifacts::sha256_file(&loader));
        let h3 = s.spawn(|| artifacts::sha256_file(&original));
        let r1 = h1
            .join()
            .map_err(|_| anyhow::anyhow!("integrity check thread panicked"))?;
        let r2 = h2
            .join()
            .map_err(|_| anyhow::anyhow!("integrity check thread panicked"))?;
        let r3 = h3
            .join()
            .map_err(|_| anyhow::anyhow!("integrity check thread panicked"))?;
        Ok::<_, anyhow::Error>((r1?, r2?, r3?))
    })?;
    ensure!(
        binary_hash == manifest.binary_sha256,
        "installed Claude binary changed"
    );
    ensure!(
        loader_hash == manifest.loader_sha256,
        "installed musl loader changed"
    );
    ensure!(
        original_hash == manifest.original_sha256,
        "original Claude binary changed"
    );
    let absolute_loader = fs::canonicalize(&loader)?;
    let expected = absolute_loader
        .to_str()
        .context("loader path is not UTF-8")?;
    ensure!(
        elf::read_interpreter(&binary)?.as_deref() == Some(expected),
        "patched interpreter mismatch"
    );
    Ok(manifest)
}
pub fn verify_active(root: &Path) -> Result<Manifest> {
    let _lock = lock(root, false)?;
    verify_release(root, &activation(root)?.current)
}

/// Read-only installation states used by doctor and setup preflight.
#[derive(Debug)]
pub enum InstallationState {
    Missing,
    Empty,
    Incomplete,
    Healthy(Box<Manifest>),
}

/// Never initializes a directory or creates a lock file. A present lock is
/// acquired shared through a read-only descriptor before inspecting metadata.
pub fn inspect_state_readonly(root: &Path) -> Result<InstallationState> {
    match fs::symlink_metadata(root) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(InstallationState::Missing);
        }
        result => {
            result?;
        }
    }
    private_directory_existing(root)?;
    if fs::read_dir(root)?.next().is_none() {
        return Ok(InstallationState::Empty);
    }
    check_owner(root)?;
    let lock_path = root.join(".lock");
    let guard = match fs::symlink_metadata(&lock_path) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        result => {
            result?;
            let file = regular_file(&lock_path)?;
            FileExt::try_lock_shared(&file)
                .context("installation is busy; retry after the installer finishes")?;
            Some(file)
        }
    };
    match fs::symlink_metadata(root.join("active.json")) {
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(InstallationState::Incomplete);
        }
        result => {
            result?;
        }
    }
    ensure!(
        guard.is_some(),
        "active installation has no lock file; preserve it and inspect before retrying"
    );
    Ok(InstallationState::Healthy(Box::new(verify_release(
        root,
        &activation(root)?.current,
    )?)))
}

pub async fn install(root: &Path, version: &str, patchelf: &Path) -> Result<()> {
    ensure!(
        std::env::consts::ARCH == "aarch64" && std::env::consts::OS == "android",
        "installation currently targets aarch64 Termux only"
    );
    npm::validate_version(version)?;
    initialize(root)?;
    let _lock = lock(root, true)?;
    let root = fs::canonicalize(root)?;
    let destination = release_path(&root, version)?;
    if destination.exists() {
        verify_release(&root, version)?;
        activate(&root, version)?;
        println!("Activated existing verified Claude Code {version}.");
        return Ok(());
    }
    let stage = tempfile::Builder::new()
        .prefix(".stage-")
        .tempdir_in(&root)?;
    let client = download::client()?;
    eprintln!("Authenticating npm metadata for {version}...");
    let metadata = npm::metadata(&client, version).await?;
    eprintln!("Verifying the pinned Alpine musl package...");
    let loader = alpine::fetch(&client).await?;
    let stage_loader = stage.path().join("loader");
    fs::write(&stage_loader, &loader)?;
    let loader_report = elf::inspect(&stage_loader)?;
    ensure!(
        loader_report.needed_libraries.is_empty() && loader_report.interpreter.is_none(),
        "unexpected loader dependencies"
    );
    let archive = stage.path().join("package.tgz");
    eprintln!("Downloading the official Claude Code package...");
    download::file(&client, &metadata.dist.tarball, &archive, 256 * 1024 * 1024).await?;
    npm::verify_tarball(&archive, &metadata.dist.integrity)?;
    let original = stage.path().join("claude.original");
    npm::extract_binary(&archive, &original)?;
    fs::remove_file(&archive)?;
    let original_elf = elf::inspect(&original)?;
    elf::require_supported_binary(&original_elf)?;
    let binary = stage.path().join("claude");
    fs::copy(&original, &binary)?;
    let final_loader = destination.join("loader");
    let patch = Command::new(patchelf)
        .arg("--set-interpreter")
        .arg(&final_loader)
        .arg(&binary)
        .kill_on_drop(true)
        .output();
    let output = timeout(Duration::from_secs(30), patch)
        .await
        .context("patchelf timed out")??;
    ensure!(
        output.status.success(),
        "patchelf failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    for path in [&binary, &stage_loader] {
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    fs::set_permissions(&original, fs::Permissions::from_mode(0o600))?;
    let patched_elf = elf::inspect(&binary)?;
    ensure!(
        patched_elf.interpreter.as_deref() == final_loader.to_str()
            && patched_elf.needed_libraries == original_elf.needed_libraries,
        "unexpected ELF patch result"
    );
    let manifest = Manifest {
        schema: SCHEMA,
        tool_version: env!("CARGO_PKG_VERSION").into(),
        claude_version: version.into(),
        alpine_version: alpine::VERSION.into(),
        npm_integrity: metadata.dist.integrity,
        original_sha256: artifacts::sha256_file(&original)?,
        binary_sha256: artifacts::sha256_file(&binary)?,
        loader_sha256: artifacts::sha256_file(&stage_loader)?,
        original_elf,
        patched_elf,
    };
    write_json(&stage.path().join("manifest.json"), &manifest)?;
    for path in [&binary, &stage_loader, &original] {
        File::open(path)?.sync_all()?;
    }
    fs::rename(stage.path(), &destination)?;
    // The interpreter names the final immutable release directory. Test it there,
    // but do not activate it until execution and file verification both succeed.
    let result = async {
        let config = tempfile::Builder::new()
            .prefix(".check-")
            .tempdir_in(&root)?;
        let output = timeout(
            Duration::from_secs(30),
            Command::new(destination.join("claude"))
                .arg("--version")
                .env_remove("LD_PRELOAD")
                .env_remove("LD_LIBRARY_PATH")
                .env("CLAUDE_CONFIG_DIR", config.path())
                .env("DISABLE_UPDATES", "1")
                .env("CLAUDE_CODE_DISABLE_NONESSENTIAL_TRAFFIC", "1")
                .kill_on_drop(true)
                .output(),
        )
        .await
        .context("Claude version check timed out")??;
        let expected = format!("{version} (Claude Code)");
        ensure!(
            output.status.success() && String::from_utf8_lossy(&output.stdout).trim() == expected,
            "Claude failed its version check: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        verify_release(&root, version)?;
        Ok::<_, anyhow::Error>(())
    }
    .await;
    if result.is_err() {
        fs::remove_dir_all(&destination).context("failed to remove rejected release")?;
    }
    result?;
    File::open(root.join("releases"))?.sync_all()?;
    // Once activation may have committed, retain the verified candidate even if
    // a later durability sync fails. Never delete a possibly active release.
    activate(&root, version)?;
    println!("Verified and activated Claude Code {version}. No shell or npm settings changed.");
    Ok(())
}

fn activate(root: &Path, version: &str) -> Result<()> {
    let old = if root.join("active.json").exists() {
        Some(activation(root)?)
    } else {
        None
    };
    if old.as_ref().is_some_and(|r| r.current == version) {
        return Ok(());
    }
    write_json(
        &root.join("active.json"),
        &Activation {
            schema: SCHEMA,
            current: version.into(),
            previous: old.map(|r| r.current),
        },
    )
}
pub fn rollback(root: &Path) -> Result<()> {
    let _lock = lock(root, true)?;
    let old = activation(root)?;
    let previous = old.previous.context("no previous release to restore")?;
    verify_release(root, &previous)?;
    activate(root, &previous)?;
    println!("Activated previous verified release {previous}.");
    Ok(())
}
pub fn uninstall(root: &Path) -> Result<()> {
    let _lock = lock(root, true)?;
    // Refuse deletion when user-added files make ownership ambiguous.
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let name = entry.file_name();
        ensure!(
            [".owner", ".lock", "active.json", "releases"]
                .iter()
                .any(|n| name == *n),
            "installation contains an unrecognized file; inspect it before uninstalling"
        );
    }
    let releases = root.join("releases");
    private_directory_existing(&releases)?;
    for entry in fs::read_dir(&releases)? {
        let entry = entry?;
        let version = entry
            .file_name()
            .into_string()
            .map_err(|_| anyhow::anyhow!("invalid release name"))?;
        npm::validate_version(&version)?;
        private_directory_existing(&entry.path())?;
        for file in fs::read_dir(entry.path())? {
            let file = file?;
            ensure!(
                ["claude", "claude.original", "loader", "manifest.json"]
                    .iter()
                    .any(|n| file.file_name() == *n),
                "release contains an unrecognized file"
            );
            regular_file(&file.path())?;
        }
    }
    fs::remove_dir_all(root)?;
    println!("Removed managed releases. Claude credentials and the Rust toolchain were preserved.");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // tempfile uses the process umask by default; managed roots must be private
    // even on hosts whose default umask permits group or other access.
    fn private_test_directory() -> Result<tempfile::TempDir> {
        Ok(tempfile::Builder::new()
            .permissions(fs::Permissions::from_mode(0o700))
            .tempdir()?)
    }

    #[test]
    fn refuses_adopting_nonempty_directories() -> Result<()> {
        let root = private_test_directory()?;
        fs::write(root.path().join("personal.txt"), "keep me")?;
        assert!(initialize(root.path()).is_err());
        assert!(root.path().join("personal.txt").exists());
        Ok(())
    }
    #[test]
    fn uninstall_preserves_unrecognized_user_files() -> Result<()> {
        let root = private_test_directory()?;
        initialize(root.path())?;
        fs::write(root.path().join("personal.txt"), "keep me")?;
        assert!(uninstall(root.path()).is_err());
        assert!(root.path().join("personal.txt").exists());
        Ok(())
    }
    #[test]
    fn shared_session_lock_blocks_mutation() -> Result<()> {
        let root = private_test_directory()?;
        initialize(root.path())?;
        let _session = lock(root.path(), false)?;
        assert!(lock(root.path(), true).is_err());
        Ok(())
    }
    #[test]
    fn refuses_nonprivate_managed_directory() -> Result<()> {
        let root = private_test_directory()?;
        initialize(root.path())?;
        fs::set_permissions(root.path(), fs::Permissions::from_mode(0o755))?;
        assert!(initialize(root.path()).is_err());
        assert!(inspect_state_readonly(root.path()).is_err());
        Ok(())
    }
    #[test]
    fn refuses_symlink_roots() -> Result<()> {
        let directory = private_test_directory()?;
        let real = directory.path().join("real");
        fs::create_dir(&real)?;
        let link = directory.path().join("link");
        std::os::unix::fs::symlink(&real, &link)?;
        assert!(initialize(&link).is_err());
        Ok(())
    }
    #[test]
    fn readonly_inspection_never_creates_a_root_or_lock() -> Result<()> {
        let temporary = private_test_directory()?;
        let missing = temporary.path().join("absent");
        assert!(matches!(
            inspect_state_readonly(&missing)?,
            InstallationState::Missing
        ));
        assert!(!missing.exists());
        let managed = temporary.path().join("managed");
        initialize(&managed)?;
        assert!(!managed.join(".lock").exists());
        assert!(matches!(
            inspect_state_readonly(&managed)?,
            InstallationState::Incomplete
        ));
        assert!(!managed.join(".lock").exists());
        Ok(())
    }

    #[test]
    fn readonly_inspection_preserves_corrupt_state_and_reports_busy_installer() -> Result<()> {
        let temporary = private_test_directory()?;
        let root = temporary.path();
        initialize(root)?;
        let writer = lock(root, true)?;
        assert!(inspect_state_readonly(root).is_err());
        drop(writer);
        fs::write(root.join("active.json"), "broken metadata")?;
        assert!(inspect_state_readonly(root).is_err());
        assert_eq!(
            fs::read_to_string(root.join("active.json"))?,
            "broken metadata"
        );
        Ok(())
    }

    #[test]
    fn readonly_inspection_rejects_special_files_without_opening_them() -> Result<()> {
        let temporary = private_test_directory()?;
        nix::unistd::mkfifo(
            &temporary.path().join(".owner"),
            nix::sys::stat::Mode::S_IRUSR | nix::sys::stat::Mode::S_IWUSR,
        )?;
        assert!(inspect_state_readonly(temporary.path()).is_err());
        Ok(())
    }
}
