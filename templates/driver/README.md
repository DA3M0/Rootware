# {{project_name}}

RKM driver skeleton (`kind = "driver"`). The `program.toml` manifest
declares the component metadata (`name` / `kind` / `entry` / `version`);
it is the single source of truth consumed by the build system.

- `entry = "rust"` — native Rust driver: register through the
  librootware SDK (`librootware::rkm::register`) and serve requests over
  IPC. Full example: `user/rkm-driver`.
- `entry = "c"` — Linux-compatible C driver: implement
  `struct rkm_device_operations` (probe/read/write) from
  `compat/linux/rkm.h`, declare the C sources in the `[c]` section, and
  the build compiles them with the shim into a boot module. Full
  example: `user/zero-driver`.

Both paths share the kernel registry, the `SYS_MODULE_REGISTER` /
`SYS_MODULE_LIST` syscalls and the driver payload convention documented
in `docs/开发者指南.md`.

## Build modes (entry = "rust")

- Rootware (default): `./build.sh driver {{project_name}}` — no_std
  build that registers with the kernel and stays resident.
- Host simulation: `cargo build --features host-sim` / `cargo test` —
  runs the driver logic on Linux with the in-memory capability provider.
