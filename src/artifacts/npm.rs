//! npm's registry signature authenticates name, version and SHA-512 metadata.
use super::download;
use anyhow::{Context, Result, bail, ensure};
use base64::{Engine, engine::general_purpose::STANDARD};
use flate2::read::GzDecoder;
use openssl::{hash::MessageDigest, pkey::PKey, sign::Verifier};
use serde::Deserialize;
use sha2::{Digest, Sha512};
use std::{
    fs::{File, OpenOptions},
    io::{self, Read},
    path::Path,
};

pub const PACKAGE: &str = "@anthropic-ai/claude-code-linux-arm64-musl";
const TRUST: &str = include_str!("../../trust/npm-registry-keys.json");

#[derive(Debug, Deserialize)]
pub struct Metadata {
    pub name: String,
    pub version: String,
    pub dist: Distribution,
}
#[derive(Debug, Deserialize)]
pub struct Distribution {
    pub tarball: String,
    pub integrity: String,
    pub signatures: Vec<RegistrySignature>,
}
#[derive(Debug, Deserialize)]
pub struct RegistrySignature {
    keyid: String,
    sig: String,
}
#[derive(Deserialize)]
struct Keyring {
    keys: Vec<RegistryKey>,
}
#[derive(Deserialize)]
struct RegistryKey {
    keyid: String,
    key: String,
    expires: Option<String>,
}

pub fn validate_version(version: &str) -> Result<()> {
    let parts: Vec<_> = version.split('.').collect();
    ensure!(
        parts.len() == 3
            && parts
                .iter()
                .all(|s| !s.is_empty() && s.len() <= 9 && s.bytes().all(|b| b.is_ascii_digit())),
        "use an exact numeric version, such as 2.1.302"
    );
    Ok(())
}

pub fn verify_metadata(bytes: &[u8], version: &str) -> Result<Metadata> {
    validate_version(version)?;
    let metadata: Metadata = serde_json::from_slice(bytes).context("invalid npm metadata")?;
    ensure!(
        metadata.name == PACKAGE && metadata.version == version,
        "npm package identity mismatch"
    );
    let expected = format!(
        "https://registry.npmjs.org/{PACKAGE}/-/claude-code-linux-arm64-musl-{version}.tgz"
    );
    ensure!(
        metadata.dist.tarball == expected,
        "unexpected npm tarball location"
    );
    download::validate_url(&metadata.dist.tarball, "registry.npmjs.org")?;
    let digest = metadata
        .dist
        .integrity
        .strip_prefix("sha512-")
        .context("SHA-512 integrity is required")?;
    ensure!(
        STANDARD.decode(digest)?.len() == 64,
        "invalid SHA-512 metadata"
    );
    let keyring: Keyring = serde_json::from_str(TRUST)?;
    let message = format!(
        "{}@{}:{}",
        metadata.name, metadata.version, metadata.dist.integrity
    );
    for signature in &metadata.dist.signatures {
        // Only keys with no expiration are currently trusted. Expiring keys need a
        // reviewed trust-file change rather than silently extending their lifetime.
        let Some(key) = keyring
            .keys
            .iter()
            .find(|k| k.keyid == signature.keyid && k.expires.is_none())
        else {
            continue;
        };
        let public = PKey::public_key_from_der(&STANDARD.decode(&key.key)?)?;
        let mut verifier = Verifier::new(MessageDigest::sha256(), &public)?;
        verifier.update(message.as_bytes())?;
        if verifier.verify(&STANDARD.decode(&signature.sig)?)? {
            return Ok(metadata);
        }
    }
    bail!("no valid signature from a pinned, non-expiring npm registry key")
}

pub fn verify_tarball(path: &Path, integrity: &str) -> Result<()> {
    let wanted = STANDARD.decode(
        integrity
            .strip_prefix("sha512-")
            .context("SHA-512 is required")?,
    )?;
    let mut hash = Sha512::new();
    io::copy(&mut File::open(path)?, &mut hash)?;
    ensure!(
        hash.finalize().as_slice() == wanted,
        "npm tarball SHA-512 mismatch"
    );
    Ok(())
}

/// Extract only the expected regular file; never unpack arbitrary archive paths.
pub fn extract_binary(tarball: &Path, destination: &Path) -> Result<()> {
    let decoder = GzDecoder::new(File::open(tarball)?);
    let mut archive = tar::Archive::new(decoder.take(1024 * 1024 * 1024));
    let mut found = false;
    for entry in archive.entries()? {
        let mut entry = entry?;
        if entry.path()?.as_ref() != Path::new("package/claude") {
            continue;
        }
        ensure!(
            !found && entry.header().entry_type().is_file(),
            "duplicate or non-regular Claude binary"
        );
        let size = entry.size();
        ensure!(
            size > 0 && size <= 768 * 1024 * 1024,
            "unexpected Claude binary size"
        );
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(destination)?;
        let copied = io::copy(&mut entry, &mut file)?;
        ensure!(copied == size, "truncated Claude binary");
        file.sync_all()?;
        found = true;
    }
    ensure!(found, "package contains no package/claude binary");
    Ok(())
}

pub async fn metadata(client: &reqwest::Client, version: &str) -> Result<Metadata> {
    validate_version(version)?;
    let url = format!("https://registry.npmjs.org/{PACKAGE}/{version}");
    let bytes = download::bytes(client, &url, 1024 * 1024).await?;
    verify_metadata(&bytes, version)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_versions_that_can_change_urls_or_paths() {
        for version in [
            "latest",
            "../../x",
            "2.1.3?x",
            "2.1",
            "2.1.3-beta",
            "2.1.3/4",
        ] {
            assert!(validate_version(version).is_err());
        }
    }
    #[test]
    fn official_signature_and_tampered_metadata() -> Result<()> {
        let bytes = include_bytes!("../../tests/fixtures/npm-2.1.302.json");
        verify_metadata(bytes, "2.1.302")?;
        let mut value: serde_json::Value = serde_json::from_slice(bytes)?;
        value["dist"]["integrity"] = format!("sha512-{}", STANDARD.encode([0; 64])).into();
        assert!(verify_metadata(&serde_json::to_vec(&value)?, "2.1.302").is_err());
        value["dist"]["signatures"] = serde_json::json!([]);
        assert!(verify_metadata(&serde_json::to_vec(&value)?, "2.1.302").is_err());
        Ok(())
    }
    #[test]
    fn archive_symlink_cannot_replace_the_expected_binary() -> Result<()> {
        use std::io::Write;
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("package.tgz");
        let encoder =
            flate2::write::GzEncoder::new(File::create(&path)?, flate2::Compression::default());
        let mut builder = tar::Builder::new(encoder);
        let mut header = tar::Header::new_gnu();
        header.set_entry_type(tar::EntryType::Symlink);
        header.set_size(0);
        header.set_mode(0o777);
        builder.append_link(&mut header, "package/claude", "/etc/passwd")?;
        let mut encoder = builder.into_inner()?;
        encoder.flush()?;
        encoder.finish()?;
        let destination = directory.path().join("claude");
        assert!(extract_binary(&path, &destination).is_err());
        assert!(!destination.exists());
        Ok(())
    }
    #[test]
    fn rejects_corrupted_tarball() -> Result<()> {
        let file = tempfile::NamedTempFile::new()?;
        std::fs::write(file.path(), b"corrupted")?;
        assert!(
            verify_tarball(file.path(), &format!("sha512-{}", STANDARD.encode([0; 64]))).is_err()
        );
        Ok(())
    }
}
