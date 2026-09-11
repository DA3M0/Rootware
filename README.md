# Rootware

A microkernel operating system written in Rust.

## Status

Alpha 1 — Minimal bootable kernel with serial output "Hello from Rootware!"

## What is this

The first runnable prototype of the Rootware root system (microkernel).
It does exactly three things: boot, print, and prove the design works.

## Tech Stack

- Rust (stable, 2024 Edition)
- No third-party libraries
- GRUB2 + Multiboot2
- x86_64-unknown-none

## Build & Run

```bash
cargo build --release
./build.sh
