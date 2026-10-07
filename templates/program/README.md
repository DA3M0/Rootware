# {{project_name}}

Rootware program skeleton (`kind = "program"`).

- `program.toml` — 组件清单:名称、类型、链接主导方(entry)。
- `src/main.rs` — Rust 入口(`rootware_main`)。
- `c/` `cpp/` `zig/` — 可选:放入源文件即自动静态编译链接(C ABI 边界)。
- `[link] libs` — 可选:引用 lib 类型组件(见 user/libcrc)。

构建:`./build.sh program {{project_name}}`;加入镜像后开机自动拉起。
