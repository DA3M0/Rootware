# {{project_name}}

RKM driver skeleton. The `rkm.toml` manifest declares the module
metadata (`name` / `version` / `kind`); it is the single source of
truth consumed by the build system.

- `kind = "native"` — Rust driver: register through the librootware SDK
  (`librootware::rkm::register`) and serve requests over IPC. Full
  example: `user/rkm-driver`.
- `kind = "linux"` — C driver: implement `struct rkm_device_operations`
  (probe/read/write) from `compat/linux/rkm.h`, declare the sources in
  the `[linux]` section, and `build.sh` compiles them with the shim
  into a boot module. Full example: `user/zero-driver`.

Both paths share the kernel registry, the `SYS_MODULE_REGISTER` /
`SYS_MODULE_LIST` syscalls and the driver payload convention documented
in `docs/开发者指南.md`.

## Build modes (native Rust path)

- Host simulation (default): `cargo build` / `cargo test` — runs the
  driver logic on Linux with the in-memory capability provider.
- Rootware image: `cargo build --release --no-default-features --target
  x86_64-unknown-none` — no_std build that registers with the kernel.

