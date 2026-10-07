#!/usr/bin/env bash
set -e

# Prefer the rustup toolchain: the bare-metal target's rust-std lives there.
export PATH="$HOME/.cargo/bin:$PATH"

RELEASE_DIR="target/x86_64-unknown-none/release"
KERNEL_ELF="$RELEASE_DIR/rootware"
ISO_DIR="iso"
USER_PROGRAMS="hello echo-service ipc-client rkm-driver"
SHIM_DIR="compat/linux"

echo "==> Building kernel..."
cargo build --release --target x86_64-unknown-none -p rootware

echo "==> Building user programs..."
for program in $USER_PROGRAMS; do
    cargo build --release --target x86_64-unknown-none --target-dir target \
        --manifest-path "user/$program/Cargo.toml"
done

# RKM Linux-compat drivers: every user/* directory whose rkm.toml
# declares kind = "linux" is compiled with the C shim into a boot
# module. The manifest is the single source for name/version/sources.
build_rkm_linux_driver() {
    local manifest="$1" dir name version kind source work objects obj
    dir="$(dirname "$manifest")"
    kind="$(sed -n 's/^kind = "\(.*\)"$/\1/p' "$manifest" | head -1)"
    [ "$kind" = "linux" ] || return 0
    name="$(sed -n 's/^name = "\(.*\)"$/\1/p' "$manifest" | head -1)"
    version="$(sed -n 's/^version = "\(.*\)"$/\1/p' "$manifest" | head -1)"
    if [ -z "$name" ] || [ -z "$version" ]; then
        echo "FAIL: $manifest must declare name and version" >&2
        return 1
    fi
    if ! command -v cc >/dev/null 2>&1; then
        echo "FAIL: Linux-compat driver '$name' needs a C compiler (cc)" >&2
        return 1
    fi
    echo "==>   $name v$version (Linux compat)"
    work="target/rkm-linux/$name"
    rm -rf "$work"
    mkdir -p "$work"

    local cflags=(-c -O2 -ffreestanding -fno-pic -fno-pie -fno-stack-protector
        -fno-asynchronous-unwind-tables -mno-red-zone -Wall -Wextra
        -I"$SHIM_DIR"
        -DRKM_MODULE_NAME="\"$name\"" -DRKM_MODULE_VERSION="\"$version\"")

    cc "${cflags[@]}" "$SHIM_DIR/rkm.c" -o "$work/rkm.o"
    objects="$work/rkm.o"
    while IFS= read -r source; do
        [ -n "$source" ] || continue
        obj="$work/$(basename "$source" .c).o"
        cc "${cflags[@]}" "$dir/$source" -o "$obj"
        objects="$objects $obj"
    done <<EOF
$(sed -n '/^\[linux\]/,$p' "$manifest" | grep -oE '"[^"]+\.c"' | tr -d '"')
EOF
    cc $objects -nostdlib -static -no-pie -Wl,--build-id=none \
        -T user/linker.ld -o "$RELEASE_DIR/$name"
    RKM_LINUX_DRIVERS="$RKM_LINUX_DRIVERS $name"
}

echo "==> Building RKM Linux-compat drivers..."
RKM_LINUX_DRIVERS=""
for manifest in user/*/rkm.toml; do
    [ -f "$manifest" ] || continue
    build_rkm_linux_driver "$manifest"
done

echo "==> Creating ISO..."
rm -rf "$ISO_DIR"
mkdir -p "$ISO_DIR/boot/grub"
cp "$KERNEL_ELF" "$ISO_DIR/boot/"
for program in $USER_PROGRAMS; do
    cp "target/x86_64-unknown-none/release/$program" "$ISO_DIR/boot/"
done
for driver in $RKM_LINUX_DRIVERS; do
    cp "$RELEASE_DIR/$driver" "$ISO_DIR/boot/"
done
cp grub.cfg "$ISO_DIR/boot/grub/"
grub2-mkrescue -o rootware.iso "$ISO_DIR"

if [ "${1:-}" = "--no-run" ]; then
    echo "==> Image ready: rootware.iso"
    exit 0
fi

echo "==> Running in QEMU..."
qemu-system-x86_64 \
    -cdrom rootware.iso \
    -boot order=d,menu=off \
    -serial stdio \
    -display none \
    -m 128M
