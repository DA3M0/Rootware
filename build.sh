#!/usr/bin/env bash
set -e

RELEASE_DIR="target/x86_64-unknown-none/release"
KERNEL_ELF="$RELEASE_DIR/rootware"
ISO_DIR="iso"

echo "==> Building kernel..."
cargo build --release

echo "==> Creating ISO..."
rm -rf "$ISO_DIR"
mkdir -p "$ISO_DIR/boot/grub"
cp "$KERNEL_ELF" "$ISO_DIR/boot/"
cp grub.cfg "$ISO_DIR/boot/grub/"
grub2-mkrescue -o rootware.iso "$ISO_DIR"

echo "==> Running in QEMU..."
qemu-system-x86_64 \
    -cdrom rootware.iso \
    -boot order=d,menu=off \
    -serial stdio \
    -display none \
    -m 128M
