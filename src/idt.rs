//! IDT（中断描述符表）模块
//!
//! IDT 定义了 256 个中断处理程序的入口。
//! 每个描述符 16 字节，指向一个处理函数。

use core::mem;

/// IDT 中的一个描述符（16 字节）
#[repr(C, packed)]
#[derive(Clone, Copy)]
struct IdtEntry {
    offset_low: u16,    // 处理函数地址低 16 位
    selector: u16,      // 代码段选择子（0x08）
    ist: u8,            // 中断栈表偏移（0 表示不用）
    type_attr: u8,      // 类型和属性
    offset_middle: u16, // 处理函数地址中间 16 位
    offset_high: u32,   // 处理函数地址高 32 位
    reserved: u32,      // 保留
}

impl IdtEntry {
    /// 创建一个空描述符
    const fn missing() -> Self {
        IdtEntry {
            offset_low: 0,
            selector: 0,
            ist: 0,
            type_attr: 0,
            offset_middle: 0,
            offset_high: 0,
            reserved: 0,
        }
    }

    /// 创建一个中断门描述符
    fn new(handler: u64, selector: u16, type_attr: u8) -> Self {
        IdtEntry {
            offset_low: (handler & 0xFFFF) as u16,
            selector,
            ist: 0,
            type_attr,
            offset_middle: ((handler >> 16) & 0xFFFF) as u16,
            offset_high: ((handler >> 32) & 0xFFFFFFFF) as u32,
            reserved: 0,
        }
    }
}

/// IDTR 寄存器的值
#[repr(C, packed)]
struct IdtPointer {
    size: u16,   // IDT 大小（字节数减 1）
    offset: u64, // IDT 地址
}

/// 我们的 IDT，包含 256 个描述符
static mut IDT: [IdtEntry; 256] = [IdtEntry::missing(); 256];

/// 初始化 IDT
pub fn init() {
    unsafe {
        // 设置除零异常（向量 0）
        IDT[0] = IdtEntry::new(divide_by_zero_handler as *const () as u64, 0x08, 0x8E);

        // 设置页错误（向量 14）
        IDT[14] = IdtEntry::new(page_fault_handler as *const () as u64, 0x08, 0x8E);

        // 设置双重故障（向量 8）
        IDT[8] = IdtEntry::new(double_fault_handler as *const () as u64, 0x08, 0x8E);

        // 设置时钟中断（向量 32）
        IDT[32] = IdtEntry::new(timer_interrupt_handler as *const () as u64, 0x08, 0x8E);

        // 加载 IDT
        let idt_ptr = IdtPointer {
            size: (mem::size_of::<[IdtEntry; 256]>() - 1) as u16,
            offset: &raw const IDT as u64,
        };

        core::arch::asm!(
            "lidt [{}]",
            in(reg) &idt_ptr,
            options(nostack)
        );
    }
}

/// 除零异常处理
extern "C" fn divide_by_zero_handler() {
    unsafe {
        core::arch::asm!("cli");
        crate::serial_println!("[EXCEPTION] Divide by zero");
        loop {
            core::arch::asm!("hlt");
        }
    }
}

/// 页错误处理
extern "C" fn page_fault_handler() {
    unsafe {
        core::arch::asm!("cli");
        crate::serial_println!("[EXCEPTION] Page fault");
        loop {
            core::arch::asm!("hlt");
        }
    }
}

/// 双重故障处理
extern "C" fn double_fault_handler() {
    unsafe {
        core::arch::asm!("cli");
        crate::serial_println!("[EXCEPTION] Double fault");
        loop {
            core::arch::asm!("hlt");
        }
    }
}
/// 时钟中断处理
extern "C" fn timer_interrupt_handler() {
    crate::timer::tick();
    crate::timer::send_eoi();
}
