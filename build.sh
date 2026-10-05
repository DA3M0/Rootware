#!/usr/bin/env bash
set -e

# Prefer the rustup toolchain: the bare-metal target's rust-std lives there.
export PATH="$HOME/.cargo/bin:$PATH"

RELEASE_DIR="target/x86_64-unknown-none/release"
KERNEL_ELF="$RELEASE_DIR/rootware"
ISO_DIR="iso"
USER_PROGRAMS="hello echo-service ipc-client"

echo "==> Building kernel..."
cargo build --release --target x86_64-unknown-none -p rootware

echo "==> Building user programs..."
for program in $USER_PROGRAMS; do
    cargo build --release --target x86_64-unknown-none --target-dir target \
        --manifest-path "user/$program/Cargo.toml"
done

echo "==> Creating ISO..."
rm -rf "$ISO_DIR"
mkdir -p "$ISO_DIR/boot/grub"
cp "$KERNEL_ELF" "$ISO_DIR/boot/"
for program in $USER_PROGRAMS; do
    cp "target/x86_64-unknown-none/release/$program" "$ISO_DIR/boot/"
done
cp grub.cfg "$ISO_DIR/boot/grub/"
grub2-mkrescue -o rootware.iso "$ISO_DIR"

echo "==> Running in QEMU..."
qemu-system-x86_64 \
    -cdrom rootware.iso \
    -boot order=d,menu=off \
    -serial stdio \
    -display none \
    -m 128M
