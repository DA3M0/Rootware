//! {{project_name}}:Rootware 常驻服务骨架(kind = "service")。
//!
//! 通过 RKM 注册拿到内核分配的 pid(== 自己的 IPC 接收方 id),
//! 然后永久驻留应答请求(REQUEST → RESPONSE 回显)。
//! 主机端(Linux)的服务逻辑开发见 sdk/simulator 与
//! librootware::service::ServiceRegistry。

#![no_std]
#![no_main]

use librootware::ipc::{Transport, message_type};
use librootware::rkm::{self, module_kind};
use librootware::syscall::SyscallTransport;

#[unsafe(no_mangle)]
extern "C" fn rootware_main() -> i32 {
    let mut transport = SyscallTransport::new();
    let descriptor = match rkm::register("{{project_name}}", "0.1.0", module_kind::NATIVE) {
        Ok(descriptor) => descriptor,
        Err(_) => return 1,
    };

    loop {
        let request = match transport.receive(descriptor.pid) {
            Ok(request) => request,
            Err(_) => return 2,
        };
        if request.message_type != message_type::REQUEST {
            continue;
        }
        if transport.reply(&request, request.payload).is_err() {
            return 3;
        }
    }
}
