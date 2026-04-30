# vesper

A small RISC-V 64 kernel written in Rust, following Stephen Marz's
[osblog tutorial](https://osblog.stephenmarz.com/).

## Current Status

Implemented so far:

- M-mode boot path (`_start`) with PMP setup and handoff to Rust
- Sv39 paging with identity mapping of kernel memory and MMIO regions
- M-mode trap handling (timer + external + syscall path)
- PLIC + UART interrupt-driven console input
- Simple round-robin kernel-thread scheduler
- VirtIO legacy MMIO probe + VirtIO block driver (Chapter 9 scope)
- Boot-time block read smoke test from `hdd.dsk`
- Minix 3 filesystem reader: superblock, inodes, direct/indirect zone
  walk, directory entries (Chapter 10 scope)
- Syscall #63 (`read(dev, inode, buf, size, offset)`) routed through
  the Minix 3 reader
- ELF64 header + program-header parser, RISC-V/EXEC validation, and a
  boot-time `test_elf` path that loads + dumps a binary by inode
  (Chapter 11 scope)

Known gaps relative to Chapter 11: the ELF loader parses and validates
binaries, but actually executing them in U-mode requires per-process
`satp` switching in the M-mode trap vector — that wiring is not in
yet, so the demo currently only summarises program headers rather
than `mret`-ing into the loaded entry point.

## Toolchain

```bash
rustup target add riscv64gc-unknown-none-elf
```

The crate uses Rust 2024 edition (Rust 1.85+).

## QEMU

You need `qemu-system-riscv64`:

```bash
# Debian / Ubuntu
sudo apt install qemu-system-misc
# Arch / Manjaro
sudo pacman -S qemu-system-riscv
# macOS
brew install qemu
```

## Build

```bash
cargo build              # debug
cargo build --release    # optimised
```

The target and `-T` linker flag are baked into [.cargo/config.toml](./.cargo/config.toml),
so a plain `cargo build` cross-compiles to RISC-V automatically.

## Disk Image

The QEMU runner is configured with a `virtio-blk-device` backed by
`hdd.dsk`. Create it first if missing:

```bash
chmod +x ./mkhdd.sh
./mkhdd.sh
```

`mkhdd.sh` allocates a 32 MiB image and, if `mkfs.minix` is on
`PATH`, formats it as a Minix 3 filesystem so the chapter-10 reader
can mount it on boot.

## Run

```bash
cargo run
```

That invokes the runner in `.cargo/config.toml`:

```
qemu-system-riscv64 \
    -machine virt -cpu rv64 -smp 4 -m 128M \
    -display none -serial stdio \
    -bios none \
    -drive file=hdd.dsk,if=none,format=raw,id=hd0 \
    -device virtio-blk-device,drive=hd0 \
    -kernel target/riscv64gc-unknown-none-elf/debug/vesper
```
