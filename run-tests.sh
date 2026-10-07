#!/usr/bin/env bash
# Rootware 1.0 stability gate: host unit tests plus a QEMU boot that must
# run the kernel selftest suite and the full user-space round trip.
set -e
cd "$(dirname "$0")"

export PATH="$HOME/.cargo/bin:$PATH"

echo "==> Host unit tests (whole workspace)"
cargo test --workspace --quiet

echo "==> Building release image"
./build.sh iso

SERIAL_LOG="$(mktemp)"
trap 'rm -f "$SERIAL_LOG"' EXIT

echo "==> Booting image in QEMU"
set +e
timeout 90 qemu-system-x86_64 \
    -cdrom rootware.iso \
    -boot order=d,menu=off \
    -serial file:"$SERIAL_LOG" \
    -display none \
    -m 128M \
    -no-reboot \
    -device isa-debug-exit,iobase=0xf4
STATUS=$?
set -e

fail() {
    echo "FAIL: $1"
    echo "--- serial output tail ---"
    tail -30 "$SERIAL_LOG"
    exit 1
}

[ "$STATUS" -eq 33 ] || fail "QEMU exit status $STATUS (want 33 = clean selftest run; 65 = selftest failure; 124 = hang/crash)"
grep -q "ping from client v1.0" "$SERIAL_LOG" || fail "IPC echo round trip did not complete"
grep -q "hello from Rootware userspace" "$SERIAL_LOG" || fail "hello program did not print"
grep -q "\[PROC\] pid 1 exited (code 0)" "$SERIAL_LOG" || fail "client did not exit cleanly"
grep -q "kernel ABI v4 handshake ok" "$SERIAL_LOG" || fail "ABI version handshake failed"
grep -q "PASS rkm-registry" "$SERIAL_LOG" || fail "rkm registry selftest did not pass"
grep -q "\[RKM\] module 'rkm-driver' v0.1.0 (native) registered as pid" "$SERIAL_LOG" || fail "native RKM driver did not register"
grep -q "\[RKM\] module 'zero-driver' v0.1.0 (linux) registered as pid" "$SERIAL_LOG" || fail "Linux-compat RKM driver did not register"
grep -q "zero-driver read ok: 8 zero bytes" "$SERIAL_LOG" || fail "zero-driver end-to-end read failed"
grep -q "mixed: c=17 cpp=33 zig=51" "$SERIAL_LOG" || fail "multi-language mixed-demo did not run"
if grep -q "\[SELFTEST\] FAIL" "$SERIAL_LOG"; then fail "a kernel selftest failed"; fi
grep -q "\[SELFTEST\] suite complete" "$SERIAL_LOG" || fail "selftest suite did not run to completion"
if grep -qi "PANIC" "$SERIAL_LOG"; then fail "a panic occurred during boot"; fi

echo "==> Stability gate passed: selftests + user-space round trip OK"
