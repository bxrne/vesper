//! Executable loading.
//!
//! Currently the only supported format is statically-linked RISC-V
//! ELF64 produced by an `riscv64-unknown-elf-*` toolchain.

pub mod elf;
