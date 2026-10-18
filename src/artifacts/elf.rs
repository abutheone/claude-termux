use anyhow::{Result, ensure};
use goblin::elf::{
    Elf,
    header::{ELFCLASS64, EM_AARCH64, ET_DYN, ET_EXEC},
    program_header::PT_INTERP,
};
use serde::{Deserialize, Serialize};
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
