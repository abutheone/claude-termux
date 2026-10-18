//! Downloaded bytes stay untrusted until signatures and payload hashes pass.
pub mod alpine;
pub mod download;
pub mod elf;
pub mod npm;

use anyhow::{Result, bail};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{self, Read},
    path::Path,
};

pub fn sha256_file(path: &Path) -> Result<String> {
    let mut file = File::open(path)?;
    let mut hash = Sha256::new();
    io::copy(&mut file, &mut hash)?;
    Ok(hex::encode(hash.finalize()))
}

pub fn read_bounded(reader: impl Read, maximum: u64) -> Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader
        .take(maximum.saturating_add(1))
        .read_to_end(&mut bytes)?;
    if bytes.len() as u64 > maximum {
        bail!("input exceeds the {maximum}-byte limit");
    }
    Ok(bytes)
}
