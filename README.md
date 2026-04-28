# vesper

A small RISC-V 64 kernel written in Rust, following Stephen Marz's
[osblog tutorial](https://osblog.stephenmarz.com/).

## Toolchain setup

You need a recent Rust toolchain and the `riscv64gc-unknown-none-elf` target:

```bash
rustup target add riscv64gc-unknown-none-elf
```

The crate uses Rust 2024 edition (Rust 1.85+).

## QEMU setup

You need `qemu-system-riscv64`. On Debian / Ubuntu:

```bash
sudo apt install qemu-system-misc
```

On Arch / Manjaro:

```bash
sudo pacman -S qemu-system-riscv
```

On macOS (Homebrew):

```bash
brew install qemu
```

Verify:

```bash
qemu-system-riscv64 --version
```

## Build

```bash
cargo build              # debug
cargo build --release    # optimised
```

The target and linker flags are baked into [.cargo/config.toml](./.cargo/config.toml),
so a plain `cargo build` cross-compiles to RISC-V automatically.

## Run under QEMU

`cargo run` invokes the runner defined in `.cargo/config.toml`:

```bash
cargo run
```

That expands to:

```
qemu-system-riscv64 \
    -machine virt \
    -cpu rv64 \
    -smp 4 \
    -m 128M \
    -nographic \
    -serial mon:stdio \
    -bios none \
    -kernel target/riscv64gc-unknown-none-elf/debug/vesper
```

`-bios none` disables OpenSBI so our `_start` is the first thing the CPU
runs in M-mode at `0x80000000`. With `-nographic` the QEMU monitor and
kernel share stdio.

To exit QEMU: press `Ctrl-A` then `x`.
