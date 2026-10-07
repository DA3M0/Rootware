# {{project_name}}

Rootware static-library skeleton (`kind = "lib"`, entry = "c").

- 产物:`lib{{project_name}}.a`,不进 ISO、不参与启动。
- 消费:其他组件在 `program.toml` 写
  `[link] libs = ["{{project_name}}"]` 后即可调用导出的 C ABI 函数。
- Rust 组件:把 entry 换成 "rust" 并提供 `crate-type = ["staticlib"]`
  的 lib crate;Rust 程序之间共享代码更推荐直接用 cargo path 依赖。

构建:`./build.sh lib {{project_name}}`。
