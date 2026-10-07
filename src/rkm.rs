//! Rootware Kernel Module (RKM) registry.
//!
//! User-space drivers register themselves through SYS_MODULE_REGISTER:
//! the kernel validates the descriptor, binds it to the calling process
//! id and marks the entry Stopped when the owner exits. Native Rust
//! drivers and Linux-compatible C drivers share this one table. This
//! module is pure bookkeeping (host-testable); the syscall boundary in
//! `crate::syscall` logs registrations.

use rootware_abi::ErrorCode;
use rootware_abi::rkm::{self, RkmModule, module_kind, module_state};

pub const MAX_MODULES: usize = 8;

static mut MODULES: [Option<RkmModule>; MAX_MODULES] = [None; MAX_MODULES];

/// Validates the caller-provided descriptor fields. `pid`, `state` and
/// padding are kernel-owned and cleared here; name and version must be
/// valid UTF-8 (trailing NUL padding allowed) with a non-empty name.
fn sanitize(descriptor: &mut RkmModule) -> Result<(), ErrorCode> {
    if descriptor.kind != module_kind::NATIVE && descriptor.kind != module_kind::LINUX {
        return Err(ErrorCode::InvalidArgument);
    }
    let name_len = descriptor
        .name
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(rkm::MAX_MODULE_NAME);
    if name_len == 0 || core::str::from_utf8(&descriptor.name[..name_len]).is_err() {
        return Err(ErrorCode::InvalidArgument);
    }
    let version_len = descriptor
        .version
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(rkm::MAX_MODULE_VERSION);
    if core::str::from_utf8(&descriptor.version[..version_len]).is_err() {
        return Err(ErrorCode::InvalidArgument);
    }
    descriptor.pid = 0;
    descriptor.state = 0;
    descriptor._pad = [0; 4];
    Ok(())
}

/// Registers a module for `pid`. Re-registration of an already active
/// pid is rejected; a stopped entry of the same pid is refreshed in
/// place, otherwise the first free slot wins and, when the table is
/// full, the oldest stopped entry is recycled. Returns the descriptor
/// as stored (pid and state filled in).
pub fn register(pid: u16, mut descriptor: RkmModule) -> Result<RkmModule, ErrorCode> {
    if pid == 0 {
        return Err(ErrorCode::InvalidArgument);
    }
    sanitize(&mut descriptor)?;
    unsafe {
        let modules = &raw const MODULES;
        let mut own_stopped: Option<usize> = None;
        let mut free_slot: Option<usize> = None;
        let mut any_stopped: Option<usize> = None;
        for index in 0..MAX_MODULES {
            match (*modules)[index] {
                Some(entry) if entry.pid == pid && entry.state == module_state::ACTIVE => {
                    return Err(ErrorCode::InvalidArgument);
                }
                Some(entry) if entry.pid == pid && own_stopped.is_none() => {
                    own_stopped = Some(index);
                }
                Some(_) => {}
                None if free_slot.is_none() => free_slot = Some(index),
                None => {}
            }
        }
        if any_stopped.is_none() {
            for index in 0..MAX_MODULES {
                if (*modules)[index].is_some_and(|entry| entry.state == module_state::STOPPED) {
                    any_stopped = Some(index);
                    break;
                }
            }
        }
        let slot = own_stopped
            .or(free_slot)
            .or(any_stopped)
            .ok_or(ErrorCode::QueueFull)?;

        descriptor.pid = pid;
        descriptor.state = module_state::ACTIVE;
        let modules = &raw mut MODULES;
        (*modules)[slot] = Some(descriptor);
    }
    Ok(descriptor)
}

/// Number of entries currently stored, including stopped ones.
pub fn count() -> usize {
    unsafe {
        let modules = &raw const MODULES;
        (*modules).iter().filter(|entry| entry.is_some()).count()
    }
}

/// Copy of the entry at `index` in slot order.
pub fn entry(index: usize) -> Option<RkmModule> {
    unsafe {
        let modules = &raw const MODULES;
        (*modules).get(index).copied().flatten()
    }
}

/// Marks every active entry owned by `pid` as stopped. Called from the
/// process exit path so the registry reflects live drivers only.
pub fn on_process_exit(pid: u16) {
    unsafe {
        let modules = &raw mut MODULES;
        for entry in (*modules).iter_mut().flatten() {
            if entry.pid == pid && entry.state == module_state::ACTIVE {
                entry.state = module_state::STOPPED;
            }
        }
    }
}

/// Clears the registry. Only the boot selftest uses this; real boots
/// start from an all-zero .bss table.
pub fn reset() {
    unsafe {
        MODULES = [None; MAX_MODULES];
    }
}

pub fn init() {
    reset();
    crate::serial_println!("[RKM] module registry initialized (capacity {})", MAX_MODULES);
}

#[cfg(test)]
mod tests {
    use super::{MAX_MODULES, count, entry, on_process_exit, register, reset};
    use rootware_abi::ErrorCode;
    use rootware_abi::rkm::{RkmModule, module_kind, module_state};

    // 注册表是全局 static,全部场景放在一个测试里顺序执行,
    // 避免 cargo test 并行线程互相踩踏。
    #[test]
    fn registry_lifecycle_and_recycling() {
        // --- 生命周期:注册 / 拒绝 / 停止 / 原位刷新 ---
        reset();
        let native = register(7, RkmModule::new("test-native", "1.0", module_kind::NATIVE))
            .expect("register native");
        assert_eq!(native.pid, 7);
        assert_eq!(native.state, module_state::ACTIVE);
        let linux = register(8, RkmModule::new("test-linux", "0.1", module_kind::LINUX))
            .expect("register linux");
        assert_eq!(linux.kind, module_kind::LINUX);
        assert_eq!(count(), 2);

        // Duplicate active registration for the same pid is rejected.
        assert_eq!(
            register(7, RkmModule::new("again", "1.0", module_kind::NATIVE)),
            Err(ErrorCode::InvalidArgument)
        );
        // Unknown kinds never enter the table.
        assert_eq!(
            register(6, RkmModule::new("bad", "1.0", 9)),
            Err(ErrorCode::InvalidArgument)
        );
        assert_eq!(count(), 2);

        // Exiting marks the module stopped without dropping the entry.
        on_process_exit(7);
        assert_eq!(entry(0).unwrap().state, module_state::STOPPED);
        assert_eq!(count(), 2);

        // Re-registration after a stop refreshes the same slot.
        let again = register(7, RkmModule::new("test-native", "1.1", module_kind::NATIVE))
            .expect("re-register");
        assert_eq!(again.state, module_state::ACTIVE);
        assert_eq!(count(), 2);

        // --- 满表:覆盖已停止条目而非拒绝新模块 ---
        reset();
        for pid in 1..=MAX_MODULES as u16 {
            register(
                pid,
                RkmModule::new("filler", "1.0", module_kind::NATIVE),
            )
            .expect("fill table");
        }
        assert_eq!(
            register(20, RkmModule::new("extra", "1.0", module_kind::NATIVE)),
            Err(ErrorCode::QueueFull)
        );
        on_process_exit(1);
        assert!(register(20, RkmModule::new("extra", "1.0", module_kind::NATIVE)).is_ok());
        assert_eq!(count(), MAX_MODULES);

        reset();
        assert_eq!(count(), 0);
    }
}
