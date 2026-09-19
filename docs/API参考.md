# librootware API 参考

- `ipc::Message`：Beta 3 ABI 的固定消息，包含发送方、接收方、类型、能力令牌和 32 字节 payload。
- `ipc::PermissionRule` / `ipc::PermissionConfig`：内核 IPC 的发送方-接收方权限表；配置会在 IPC 初始化时加载，也可通过 `load_permissions` 更新。
- `ipc::Transport`：发送、接收、回复的传输抽象；`InMemoryTransport` 可用于 Linux 测试。
- `capability::Capability`：`#[repr(C)]` 的 32 位能力令牌；`CapabilityProvider` 请求和检查能力，模拟器提供内存实现。
- IPC 发送要求消息携带的能力令牌同时被发送方持有，并匹配权限规则中的所需能力；无能力或无效令牌会被拒绝。
- `log::log`、`info!`、`warn!`：统一日志接口。
- `ErrorCode` / `Error`：统一错误码，包含权限、队列、参数和传输错误。
- Beta 3 ABI 版本为 `2`；消息新增能力令牌字段，Beta 7 冻结前仍可能扩展。

库默认启用 `std` 以便服务和模拟器使用；内核适配可通过
`default-features = false` 使用无标准库构建。
