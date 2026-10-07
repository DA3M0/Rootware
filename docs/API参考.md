# librootware API 参考

- `rootware_abi::ABI_VERSION`：**ABI v4**（1.0 正式版冻结）。`Message` 为
  `#[repr(C)]`、44 字节；标准消息类型、能力令牌和广播接收方常量保持稳定。
  1.0.x 只允许增量扩展；破坏性变更需要新的主版本。
- `ipc::Message`：固定消息布局，包含发送方、接收方、类型、能力令牌和 32 字节
  payload。在内核与用户程序之间通过裸指针传递，字段顺序不可调整。
- `ipc::Transport`：发送、接收、回复的传输抽象；`InMemoryTransport`
  （`std` feature）可用于 Linux 单元测试。
- `syscall::SyscallTransport`：Rootware x86_64 上通过 `syscall` 指令进入内核的
  IPC 传输；Linux 等其他目标会明确返回 `ErrorCode::Unsupported`。接收在
  Rootware 上是阻塞的（队列为空时进程挂起，被唤醒后重试），在模拟器上立即
  返回 `QueueEmpty`。
- `console::write_str` / `console::write_bytes` / `console::write_args`：
  通过 `SYS_CONSOLE_WRITE` 输出到内核串口控制台，格式化不分配内存。
  `log::info!` / `log::warn!` 在 no_std 下也接同一个控制台。
- `sys::kernel_abi_version` / `sys::check_abi_version`：运行时版本握手，
  程序启动时应先校验内核 ABI 与 SDK 一致。
- `process::exit(code)`：终止当前进程；`process::spawn(name)`：按名称加载
  启动模块并返回新进程 id。
- `capability::request_capability(kind)`：通过 `SYS_CAP_REQUEST` 向内核请求
  能力，由内核的启动策略白名单决定是否授予（1.0 只对 pid 1/2 开放
  `IPC_SEND`）。`CapabilityProvider` / `InMemoryCapabilities` 仍是纯模拟实现。
- `service::Service`：服务的 `name`、`init`、`handle` 和 `stop` 生命周期接口。
  `ServiceRegistry`（`std` feature）负责注册、启动、停止和按名称分发；
  `EchoService` 是可用于入门和测试的最小服务示例。`reply` 统一以
  `RESPONSE` 类型回包，路由表据此把应答送回请求方。
- `rkm::register(name, version, kind)`：把调用进程注册为 RKM 模块
  （`SYS_MODULE_REGISTER`），返回内核回填后的描述符（含分配的 pid）。
  `rkm::list(&mut [RkmModule])` 枚举注册表（`SYS_MODULE_LIST`）。
  `rkm::Driver` trait 与 `Service` 镜像。原生 Rust 驱动示例见
  `user/rkm-driver`；Linux 兼容 C 驱动（`compat/linux` shim + ops 表）
  示例见 `user/zero-driver`。
- `ipc::message_type`：标准 `REQUEST`、`RESPONSE`、`EVENT`、`BROADCAST` 类型；
  `RouteRule` 描述类型到接收方的路由。
- `ipc::BROADCAST_RECEIVER`：广播消息的特殊接收方；内核按路由表复制到多个
  目标队列。
- `capability::Capability`：`#[repr(C)]` 的 32 位能力令牌；
  `capability_kind` 标准命名空间：`IPC_SEND = 1`、`FS_READ = 2`、
  `FS_WRITE = 3`（FS 类在 1.0 保留，内核不会授予）。
- 安全模型：发送方身份由内核强制（`SYS_IPC_SEND` 会覆盖用户声明的 sender）；
  `SYS_IPC_RECEIVE` 只允许读取调用者自己进程的队列；权限表与能力令牌由内核
  在投递时校验。
- `ErrorCode` / `Error`：统一错误码 `InvalidArgument=1`、`PermissionDenied=2`、
  `QueueFull=3`、`QueueEmpty=4`、`NotFound=5`、`Unsupported=6`、`Transport=7`；
  syscall 以 `-code` 形式在 `rax` 返回。
- 内核侧 `PermissionRule` / `PermissionConfig` / `RouteConfig` / `AuditEntry`
  是内核内部策略结构，不属于用户 SDK。

## 冻结的 syscall 表（ABI v4）

| 号 | 名称 | rdi | rsi | 返回 |
|---|------|-----|-----|------|
| 1 | IPC_SEND | `*const Message` | — | 0 或错误码 |
| 2 | IPC_RECEIVE | 接收方 id（须等于调用者 pid） | `*mut Message` | 0、`QueueEmpty`、`PermissionDenied` |
| 3 | IPC_REPLY | `*const Message` 请求 | `*const [u8; 32]` | 0 或错误码 |
| 4 | VERSION | — | — | ABI 版本（正数） |
| 5 | CONSOLE_WRITE | `*const u8` | 长度 | 0 或 `InvalidArgument` |
| 6 | EXIT | 退出码 | — | 不返回 |
| 7 | SPAWN | `*const u8` 程序名 | 名字长度 | 新 pid 或错误 |
| 8 | CAP_REQUEST | 能力 kind | — | 0 或 `PermissionDenied` |
| 9 | MODULE_REGISTER | `*mut RkmModule`（in/out） | — | 0 或错误码 |
| 10 | MODULE_LIST | `*mut RkmModule` 输出数组 | 数组容量（条目） | 注册总数（正数）或错误码 |

`RkmModule` 是冻结的 32 字节描述符（`rkm::RkmModule`）：调用者填
`name`（≤16 字节）、`version`（≤8 字节）、`kind`（`NATIVE=1` /
`LINUX=2`），内核校验后回填 `pid` 与 `state`（`ACTIVE=1` / `STOPPED=2`）。
注册即授权：模块获得 `IPC_SEND` 能力，权限表自动开通 pid 1 与驱动之间
的双向通道。驱动请求使用消息类型 `DRIVER_REQUEST=5`（直投驱动 pid），
payload 约定见开发者指南。

库默认启用 `std` 以便服务和模拟器使用；内核适配可通过
`default-features = false` 使用无标准库构建（该模式下 SDK 提供 crt0 与
panic handler，用户程序只需实现 `#[no_mangle] extern "C" fn rootware_main() -> i32`）。
