//! RKM 驱动骨架。
//!
//! Rootware 构建(--no-default-features):no_std,经内核注册。
//! 主机模拟(host-sim,默认):std,用内存能力表跑同一套逻辑。

#![cfg_attr(not(feature = "host-sim"), no_std)]
#![cfg_attr(not(feature = "host-sim"), no_main)]

use librootware::capability::capability_kind;

/// Rootware 入口:注册为 RKM 模块后开始服务。
#[cfg(not(feature = "host-sim"))]
#[unsafe(no_mangle)]
extern "C" fn rootware_main() -> i32 {
    use librootware::capability::request_capability;
    use librootware::rkm::{self, module_kind};

    if request_capability(capability_kind::IPC_SEND).is_err() {
        return 1;
    }
    // 注册成功时内核会打印统一的 [RKM] 日志(含分配的 pid)。
    let Ok(descriptor) = rkm::register("{{project_name}}", "0.1.0", module_kind::NATIVE) else {
        return 2;
    };
    let _ = descriptor;

    // TODO: 在这里实现请求循环 —— transport.receive(自己的 pid) →
    // 处理 → transport.reply,参考 user/rkm-driver。骨架直接退出,
    // 注册表会把本模块标记为 STOPPED。
    0
}

/// 主机模拟入口:没有内核,用内存能力表跑同一套驱动逻辑。
#[cfg(feature = "host-sim")]
fn main() {
    use librootware::capability::{Capability, CapabilityProvider, InMemoryCapabilities};
    use librootware::info;

    let mut capabilities = InMemoryCapabilities::default();
    capabilities
        .request(Capability { id: capability_kind::IPC_SEND })
        .expect("request driver capability");
    info!("{{project_name}} RKM driver initialized (host simulation)");
}
