#!/bin/sh
# Build a 32 MiB zero-filled disk image for the virtio block device.
# `cargo run` attaches this file as the kernel's only disk; if it
# doesn't exist, QEMU refuses to start.
dd if=/dev/zero of=hdd.dsk bs=1M count=32
