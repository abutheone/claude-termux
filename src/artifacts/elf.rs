use anyhow::{Context, Result, ensure};
use goblin::elf::{
    Elf,
    header::{ELFCLASS64, ELFDATA2LSB, EM_AARCH64, ET_DYN, ET_EXEC},
    program_header::PT_INTERP,
};
use serde::{Deserialize, Serialize};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Report {
    pub elf_type: u16,
    pub interpreter: Option<String>,
    pub needed_libraries: Vec<String>,
}

pub fn inspect(path: &Path) -> Result<Report> {
    let bytes = std::fs::read(path)?;
    let elf = Elf::parse(&bytes)?;
    ensure!(
        elf.header.e_machine == EM_AARCH64
            && matches!(elf.header.e_type, ET_DYN | ET_EXEC)
            && elf.header.e_ident[4] == ELFCLASS64
            && elf.little_endian,
        "expected a little-endian AArch64 executable ELF binary"
    );
    ensure!(
        elf.program_headers
            .iter()
            .filter(|p| p.p_type == PT_INTERP)
            .count()
            <= 1,
        "multiple ELF interpreters"
    );
    Ok(Report {
        elf_type: elf.header.e_type,
        interpreter: elf.interpreter.map(str::to_owned),
        needed_libraries: elf.libraries.iter().map(|s| (*s).to_owned()).collect(),
    })
}

pub fn require_supported_binary(report: &Report) -> Result<()> {
    ensure!(
        report.interpreter.as_deref() == Some("/lib/ld-musl-aarch64.so.1"),
        "unexpected original binary interpreter"
    );
    // Fail closed on future dependency changes; do not pull arbitrary shared libs.
    ensure!(
        report.needed_libraries == ["libc.musl-aarch64.so.1"],
        "unsupported native dependencies: {:?}",
        report.needed_libraries
    );
    Ok(())
}

/// Read only the ELF interpreter path without loading the entire file into
/// memory. Reads under 1 KB instead of the full binary.
pub fn read_interpreter(path: &Path) -> Result<Option<String>> {
    let mut file = std::fs::File::open(path)?;
    let file_size = file.metadata()?.len();

    let mut hdr = [0u8; 64];
    file.read_exact(&mut hdr)?;
    ensure!(&hdr[0..4] == b"\x7fELF", "not an ELF file");
    ensure!(hdr[4] == ELFCLASS64, "not a 64-bit ELF");
    ensure!(hdr[5] == ELFDATA2LSB, "not a little-endian ELF");
    let e_type = u16::from_le_bytes(hdr[16..18].try_into().context("e_type")?);
    let e_machine = u16::from_le_bytes(hdr[18..20].try_into().context("e_machine")?);
    ensure!(
        e_machine == EM_AARCH64 && matches!(e_type, ET_DYN | ET_EXEC),
        "expected a little-endian AArch64 executable ELF binary"
    );
    let e_phoff = u64::from_le_bytes(hdr[32..40].try_into().context("e_phoff")?);
    let ph_size = u16::from_le_bytes(hdr[54..56].try_into().context("e_phentsize")?) as usize;
    let ph_count = u16::from_le_bytes(hdr[56..58].try_into().context("e_phnum")?) as usize;

    ensure!(ph_size >= 56, "ELF64 program header entry too small");
    let ph_total = ph_size
        .checked_mul(ph_count)
        .context("program header table size overflow")?;
    ensure!(
        e_phoff
            .checked_add(ph_total as u64)
            .is_some_and(|end| end <= file_size),
        "program headers extend past end of file"
    );

    file.seek(SeekFrom::Start(e_phoff))?;
    let mut ph_buf = vec![0u8; ph_total];
    file.read_exact(&mut ph_buf)?;

    for i in 0..ph_count {
        let base = i * ph_size;
        let p_type = u32::from_le_bytes(
            ph_buf
                .get(base..base + 4)
                .context("p_type out of range")?
                .try_into()
                .context("p_type conversion")?,
        );
        if p_type == PT_INTERP {
            let p_offset = u64::from_le_bytes(
                ph_buf
                    .get(base + 8..base + 16)
                    .context("p_offset out of range")?
                    .try_into()
                    .context("p_offset conversion")?,
            );
            let p_filesz = u64::from_le_bytes(
                ph_buf
                    .get(base + 32..base + 40)
                    .context("p_filesz out of range")?
                    .try_into()
                    .context("p_filesz conversion")?,
            );
            ensure!(
                p_filesz > 0 && p_filesz < 4096,
                "interpreter path length out of range"
            );
            ensure!(
                p_offset
                    .checked_add(p_filesz)
                    .is_some_and(|end| end <= file_size),
                "interpreter segment extends past end of file"
            );

            file.seek(SeekFrom::Start(p_offset))?;
            let mut interp = vec![0u8; p_filesz as usize];
            file.read_exact(&mut interp)?;
            while interp.last() == Some(&0) {
                interp.pop();
            }
            return Ok(Some(
                String::from_utf8(interp).context("interpreter path is not valid UTF-8")?,
            ));
        }
    }

    Ok(None)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn refuses_new_runtime_dependencies() {
        let report = Report {
            elf_type: ET_EXEC,
            interpreter: Some("/lib/ld-musl-aarch64.so.1".into()),
            needed_libraries: vec!["libc.musl-aarch64.so.1".into(), "libstdc++.so.6".into()],
        };
        assert!(require_supported_binary(&report).is_err());
    }
}
