#!/usr/bin/env bash
# Rootware 2.x stability gate: host unit tests plus a QEMU boot that must
# run the kernel selftest suite, the full user-space round trip, and the
# interactive shell (driven over the serial pipe).
set -e
cd "$(dirname "$0")"

export PATH="$HOME/.cargo/bin:$PATH"

echo "==> Host unit tests (whole workspace)"
cargo test --workspace --quiet

echo "==> Building release image"
./build.sh iso

SERIAL_LOG="$(mktemp)"
COMDIR="$(mktemp -d)"
mkfifo "$COMDIR/com.in" "$COMDIR/com.out"
trap 'rm -f "$SERIAL_LOG"; rm -rf "$COMDIR"' EXIT

# Serial pipe: QEMU writes com.out and reads com.in. The background cat
# mirrors output into the log (line-buffered so the gate driver can grep
# it while the VM runs); fd 3 holds the input side open for the driver.
# FIFO opens block until their peer appears, so QEMU must start in the
# background BEFORE the input side is opened.
stdbuf -oL cat "$COMDIR/com.out" > "$SERIAL_LOG" &
CAT_PID=$!

echo "==> Booting image in QEMU"
set +e
timeout 90 qemu-system-x86_64 \
    -cdrom rootware.iso \
    -boot order=d,menu=off \
    -serial pipe:"$COMDIR/com" \
    -display none \
    -m 128M \
    -no-reboot \
    -device isa-debug-exit,iobase=0xf4 &
QEMU_PID=$!
exec 3>"$COMDIR/com.in"

# Gate driver: wait for the shell banner, then exercise the command set
# (including SYS_SPAWN through 'run hello') and leave through 'exit'.
(
    for _ in $(seq 120); do
        grep -q "\[SHELL\] ready" "$SERIAL_LOG" 2>/dev/null && break
        sleep 0.5
    done
    printf 'help\r' >&3
    sleep 1
    printf 'modules\r' >&3
    sleep 1
    printf 'run hello\r' >&3
    sleep 1
    printf 'abi\r' >&3
    sleep 1
    printf 'exit\r' >&3
) &
DRIVER_PID=$!

wait "$QEMU_PID"
STATUS=$?
set -e
kill "$CAT_PID" "$DRIVER_PID" 2>/dev/null || true
exec 3>&-
wait 2>/dev/null || true

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
grep -q "kernel ABI v5 handshake ok" "$SERIAL_LOG" || fail "ABI version handshake failed"
grep -q "PASS rkm-registry" "$SERIAL_LOG" || fail "rkm registry selftest did not pass"
grep -q "\[RKM\] module 'rkm-driver' v0.1.0 (native) registered as pid" "$SERIAL_LOG" || fail "native RKM driver did not register"
grep -q "\[RKM\] module 'zero-driver' v0.1.0 (linux) registered as pid" "$SERIAL_LOG" || fail "Linux-compat RKM driver did not register"
grep -q "zero-driver read ok: 8 zero bytes" "$SERIAL_LOG" || fail "zero-driver end-to-end read failed"
grep -q "mixed: c=17 cpp=33 zig=51" "$SERIAL_LOG" || fail "multi-language mixed-demo did not run"
grep -q "\[SHELL\] ready" "$SERIAL_LOG" || fail "shell did not announce readiness"
grep -q "commands: help echo clear abi modules run exit" "$SERIAL_LOG" || fail "shell help output missing"
grep -Eq "rkm-driver +v0\.1\.0 +active +native" "$SERIAL_LOG" || fail "shell modules listing did not show the native driver"
grep -Eq "zero-driver +v0\.1\.0 +active +linux" "$SERIAL_LOG" || fail "shell modules listing did not show the linux driver"
grep -q "spawned hello as pid" "$SERIAL_LOG" || fail "shell run command did not spawn hello"
if grep -q "\[SELFTEST\] FAIL" "$SERIAL_LOG"; then fail "a kernel selftest failed"; fi
grep -q "\[SELFTEST\] suite complete" "$SERIAL_LOG" || fail "selftest suite did not run to completion"
if grep -qi "PANIC" "$SERIAL_LOG"; then fail "a panic occurred during boot"; fi

echo "==> Stability gate passed: selftests + user-space round trip + shell OK"
