# vesper

A small RISC-V 64 kernel written in Rust, following Stephen Marz's
[osblog tutorial](https://osblog.stephenmarz.com/).

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
    -kernel target/riscv64gc-unknown-none-elf/debug/vesper
```

`-bios none` skips OpenSBI so our `_start` runs first in M-mode at
`0x80000000`. `-display none -serial stdio` wires the UART straight to
your terminal — no monitor multiplexing — so keystrokes go to the
kernel and `Ctrl-A x` quits.

To exit QEMU: **`Ctrl-A` then `x`**.
