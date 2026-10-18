//! A deliberately narrow verifier for the pinned musl APK v2 package.
//! Signature -> compressed control metadata -> SHA-256 of compressed payload.
use super::{download, read_bounded};
use anyhow::{Context, Result, ensure};
use flate2::bufread::GzDecoder;
use openssl::{hash::MessageDigest, pkey::PKey, sign::Verifier};
use sha2::{Digest, Sha256};
use std::{io::Cursor, path::Path};

pub const VERSION: &str = "1.2.6-r2";
pub const URL: &str = "https://dl-cdn.alpinelinux.org/alpine/v3.24/main/aarch64/musl-1.2.6-r2.apk";
const KEY_NAME: &str = "alpine-devel@lists.alpinelinux.org-616ae350.rsa.pub";
const KEY: &[u8] =
    include_bytes!("../../trust/alpine-devel@lists.alpinelinux.org-616ae350.rsa.pub");
const MAX_SEGMENT: u64 = 16 * 1024 * 1024;

struct Segment<'a> {
    compressed: &'a [u8],
    decoded: Vec<u8>,
}
fn segment<'a>(remaining: &mut &'a [u8]) -> Result<Segment<'a>> {
    let original = *remaining;
    let mut decoder = GzDecoder::new(original);
    let decoded = read_bounded(&mut decoder, MAX_SEGMENT)?;
    let tail = decoder.into_inner();
    let consumed = original.len() - tail.len();
    ensure!(consumed > 0, "missing gzip segment");
    *remaining = tail;
    Ok(Segment {
        compressed: &original[..consumed],
        decoded,
    })
}

fn member(bytes: &[u8], wanted: &str, limit: u64) -> Result<Vec<u8>> {
    let mut archive = tar::Archive::new(Cursor::new(bytes));
    let mut found = None;
    // APK signature/control segments can omit tar end markers.
    archive.set_ignore_zeros(true);
    for entry in archive.entries()? {
        let entry = entry?;
        if entry.path()?.as_ref() == Path::new(wanted) {
            ensure!(
                found.is_none() && entry.header().entry_type().is_file(),
                "duplicate or non-regular APK member"
            );
            found = Some(read_bounded(entry, limit)?);
        }
    }
    found.context("required APK member missing")
}

pub fn verify_and_extract(bytes: &[u8]) -> Result<Vec<u8>> {
    let mut remaining = bytes;
    let signature = segment(&mut remaining)?;
    let control = segment(&mut remaining)?;
    let signature = member(&signature.decoded, &format!(".SIGN.RSA.{KEY_NAME}"), 8192)?;
    let public = PKey::public_key_from_pem(KEY)?;
    let mut verifier = Verifier::new(MessageDigest::sha1(), &public)?;
    verifier.update(control.compressed)?;
    ensure!(
        verifier.verify(&signature)?,
        "Alpine control signature mismatch"
    );
    let metadata = String::from_utf8(member(&control.decoded, ".PKGINFO", 64 * 1024)?)?;
    for (name, expected) in [
        ("pkgname", "musl"),
        ("pkgver", VERSION),
        ("arch", "aarch64"),
    ] {
        ensure!(
            field(&metadata, name)? == expected,
            "Alpine package identity mismatch: {name}"
        );
    }
    let digest = field(&metadata, "datahash")?;
    ensure!(
        hex::encode(Sha256::digest(remaining)) == digest,
        "Alpine payload SHA-256 mismatch"
    );
    let payload = segment(&mut remaining)?;
    ensure!(remaining.is_empty(), "unexpected trailing APK data");
    member(
        &payload.decoded,
        "lib/ld-musl-aarch64.so.1",
        2 * 1024 * 1024,
    )
}

fn field<'a>(metadata: &'a str, name: &str) -> Result<&'a str> {
    let prefix = format!("{name} = ");
    let mut values = metadata
        .lines()
        .filter_map(|line| line.strip_prefix(&prefix));
    let value = values.next().context("required Alpine metadata missing")?;
    ensure!(values.next().is_none(), "duplicate Alpine metadata field");
    Ok(value)
}

pub async fn fetch(client: &reqwest::Client) -> Result<Vec<u8>> {
    download::validate_url(URL, "dl-cdn.alpinelinux.org")?;
    let bytes = download::bytes(client, URL, 4 * 1024 * 1024).await?;
    verify_and_extract(&bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn verifies_official_package_and_rejects_mutation() -> Result<()> {
        let bytes = include_bytes!("../../tests/fixtures/musl-1.2.6-r2.apk");
        let loader = verify_and_extract(bytes)?;
        assert!(loader.starts_with(b"\x7fELF"));
        let mut changed = bytes.to_vec();
        let last = changed.len() - 1;
        changed[last] ^= 1;
        assert!(verify_and_extract(&changed).is_err());
        assert!(verify_and_extract(&bytes[..bytes.len() / 2]).is_err());
        let mut trailing = bytes.to_vec();
        trailing.push(0);
        assert!(verify_and_extract(&trailing).is_err());
        Ok(())
    }
    #[test]
    fn rejects_valid_gzip_with_forged_control_or_payload() -> Result<()> {
        use std::io::Write;
        let bytes = include_bytes!("../../tests/fixtures/musl-1.2.6-r2.apk");
        let mut remaining = bytes.as_slice();
        let signature = segment(&mut remaining)?;
        let control = segment(&mut remaining)?;
        let payload = segment(&mut remaining)?;
        for change_control in [true, false] {
            let source = if change_control {
                &control.decoded
            } else {
                &payload.decoded
            };
            let mut altered = source.clone();
            let position = altered
                .iter()
                .position(|b| *b != 0)
                .ok_or_else(|| anyhow::anyhow!("empty fixture"))?;
            altered[position] ^= 1;
            let mut encoder =
                flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::default());
            encoder.write_all(&altered)?;
            let changed = encoder.finish()?;
            let mut forged = signature.compressed.to_vec();
            forged.extend_from_slice(if change_control {
                &changed
            } else {
                control.compressed
            });
            forged.extend_from_slice(if change_control {
                payload.compressed
            } else {
                &changed
            });
            let error = verify_and_extract(&forged)
                .err()
                .ok_or_else(|| anyhow::anyhow!("forged APK was accepted"))?;
            let message = error.to_string();
            assert!(
                message.contains(if change_control {
                    "signature mismatch"
                } else {
                    "SHA-256 mismatch"
                }),
                "{message}"
            );
        }
        Ok(())
    }
    #[test]
    fn rejects_duplicate_security_metadata() {
        assert!(field("datahash = one\ndatahash = two", "datahash").is_err());
    }
}
