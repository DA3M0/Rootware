# {{project_name}}

Rootware resident IPC service skeleton (`kind = "service"`).

- 注册即定位:经 `librootware::rkm::register` 拿到内核分配的 pid
  (== 自己的 IPC 接收方 id),随后驻留应答请求。
- 构建:`./build.sh service {{project_name}}`;开机自动拉起。
- 主机端服务逻辑(Linux):`cargo build --features host-sim` 与
  `librootware::service::ServiceRegistry` / `sdk/simulator` 配合使用。
