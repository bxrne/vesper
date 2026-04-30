#!/bin/sh
# Build a 32 MiB disk image for the virtio block device and (if
# `mkfs.minix` is available) format it as a Minix 3 filesystem so the
# chapter-10 reader has something real to mount. `cargo run` attaches
# this file as the kernel's only disk; if it doesn't exist, QEMU
# refuses to start.
set -eu

dd if=/dev/zero of=hdd.dsk bs=1M count=32

if command -v mkfs.minix >/dev/null 2>&1; then
    mkfs.minix -3 hdd.dsk
else
    echo "mkfs.minix not found; leaving disk image unformatted." >&2
    echo "Install util-linux to format as Minix 3."             >&2
fi
