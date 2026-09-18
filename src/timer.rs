//! 时钟中断模块
//!
//! 使用本地 APIC 定时器产生时钟中断。
//! 频率：100Hz（每 10ms 一次）
//!
//! 注意：APIC 基址暂时用默认值 0xFEE00000。
//! 后续从 ACPI MADT 表读取正确基址。

use core::ptr::{read_volatile, write_volatile};
use core::sync::atomic::{AtomicU64, Ordering};

/// APIC 默认基址
const APIC_BASE: u64 = 0xFEE00000;

/// APIC 寄存器偏移
const APIC_LVT_TIMER: u64 = 0x320;
const APIC_TIMER_INIT_COUNT: u64 = 0x380;
const APIC_TIMER_CURRENT_COUNT: u64 = 0x390;
const APIC_TIMER_DIVIDE: u64 = 0x3E0;
const APIC_EOI: u64 = 0xB0;
const APIC_SPURIOUS: u64 = 0xF0;

/// 目标频率：100Hz
const TARGET_FREQUENCY: u64 = 100;

/// APIC 定时器基础频率（需要校准，这里先用一个估算值）
const APIC_TIMER_FREQUENCY: u64 = 100_000_000;

static TICKS: AtomicU64 = AtomicU64::new(0);

/// 初始化本地 APIC 定时器
pub fn init() {
    unsafe {
        // 1. 启用 APIC（设置 Spurious Interrupt Vector Register）
        // 位 8 是 APIC 软件使能位，向量 0xFF 是伪中断向量
        let spurious = read_volatile((APIC_BASE + APIC_SPURIOUS) as *const u32);
        write_volatile(
            (APIC_BASE + APIC_SPURIOUS) as *mut u32,
            spurious | 0x100 | 0xFF,
        );

        // 2. 设置定时器分频（除以 16）
        write_volatile((APIC_BASE + APIC_TIMER_DIVIDE) as *mut u32, 0x03);

        // 3. 设置 LVT Timer 寄存器
        // 位 17: 周期模式（1 = 周期，0 = 单次）
        // 位 16: 掩码（0 = 不掩码）
        // 位 0-7: 中断向量（32）
        write_volatile((APIC_BASE + APIC_LVT_TIMER) as *mut u32, 0x20000 | 32);

        // 4. 设置初始计数值
        let count = APIC_TIMER_FREQUENCY / TARGET_FREQUENCY;
        write_volatile(
            (APIC_BASE + APIC_TIMER_INIT_COUNT) as *mut u32,
            count as u32,
        );
    }
}

/// 每次时钟中断调用
pub fn tick() {
    let count = TICKS.fetch_add(1, Ordering::Relaxed) + 1;

    // 每 100 次（1 秒）输出一次
    if count % 100 == 0 {
        crate::serial_println!("[TIMER] {} seconds", count / 100);
    }
}

/// 发送 EOI（End of Interrupt）给 APIC
pub fn send_eoi() {
    unsafe {
        write_volatile((APIC_BASE + APIC_EOI) as *mut u32, 0);
    }
}

/// 获取当前 tick 数
pub fn ticks() -> u64 {
    TICKS.load(Ordering::Relaxed)
}
