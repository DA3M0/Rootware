//! IDT（中断描述符表）模块
//!
//! IDT 定义了 256 个中断处理程序的入口。
//! 每个描述符 16 字节，指向一个处理函数。

use core::mem;
use core::arch::global_asm;

global_asm!(
    ".global rootware_syscall_entry",
    "rootware_syscall_entry:",
    "push rcx",
    "push r11",
    "mov rdx, rsi",
    "mov rsi, rdi",
    "mov rdi, rax",
    "call rootware_syscall_handler",
    "pop r11",
    "pop rcx",
    "sysretq",
);

unsafe extern "C" {
    fn rootware_syscall_entry();
}

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
        let lstar = rootware_syscall_entry as *const () as u64;
        core::arch::asm!(
            "wrmsr",
            in("ecx") 0xc0000082u32,
            in("eax") lstar as u32,
            in("edx") (lstar >> 32) as u32,
            options(nostack, preserves_flags)
        );
        core::arch::asm!(
            "wrmsr",
            in("ecx") 0xc0000081u32,
            in("eax") 0x0008u32,
            in("edx") 0x000b_0008u32,
            options(nostack, preserves_flags)
        );
        let mut efer_low: u32;
        let mut efer_high: u32;
        core::arch::asm!(
            "rdmsr",
            in("ecx") 0xc0000080u32,
            out("eax") efer_low,
            out("edx") efer_high,
            options(nostack)
        );
        efer_low |= 1;
        core::arch::asm!(
            "wrmsr",
            in("ecx") 0xc0000080u32,
            in("eax") efer_low,
            in("edx") efer_high,
            options(nostack, preserves_flags)
        );
        // 设置除零异常（向量 0）
        IDT[0] = IdtEntry::new(divide_by_zero_handler as *const () as u64, 0x08, 0x8E);

        // 设置页错误（向量 14）
        IDT[14] = IdtEntry::new(page_fault_handler as *const () as u64, 0x08, 0x8E);

        // 设置双重故障（向量 8）
        IDT[8] = IdtEntry::new(double_fault_handler as *const () as u64, 0x08, 0x8E);

        // 设置时钟中断（向量 32）
        IDT[32] = IdtEntry::new(timer_interrupt_handler as *const () as u64, 0x08, 0x8E);
        IDT[128] = IdtEntry::new(rootware_syscall_entry as *const () as u64, 0x08, 0xEE);

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

    #[unsafe(no_mangle)]
    extern "C" fn rootware_syscall_handler(number: u64, first: u64, second: u64) -> i64 {
        const IPC_SEND: u64 = 1;
        const IPC_RECEIVE: u64 = 2;
        const IPC_REPLY: u64 = 3;

        match number {
            IPC_SEND => {
                if first == 0 {
                    return -1;
                }
                // User pages are mapped by the service loader before entering
                // Ring 3. The syscall ABI uses a fixed, Copy message layout.
                let message = unsafe { core::ptr::read(first as *const crate::ipc::Message) };
                crate::ipc::send(message).map_or(-7, |_| 0)
            }
            IPC_RECEIVE => {
                if second == 0 {
                    return -1;
                }
                match crate::ipc::recv(first as u16) {
                    Some(message) => {
                        unsafe {
                            core::ptr::write(second as *mut crate::ipc::Message, message);
                        }
                        0
                    }
                    None => -4,
                }
            }
            IPC_REPLY => {
                if first == 0 || second == 0 {
                    return -1;
                }
                let request = unsafe { core::ptr::read(first as *const crate::ipc::Message) };
                if request.receiver == 0 || request.sender == 0 {
                    return -1;
                }
                let payload = unsafe {
                    core::ptr::read(second as *const [u8; crate::ipc::PAYLOAD_SIZE])
                };
                let response = crate::ipc::Message {
                    sender: request.receiver,
                    receiver: request.sender,
                    message_type: request.message_type,
                    capability: request.capability,
                    payload,
                };
                crate::ipc::send(response).map_or(-7, |_| 0)
            }
            _ => -6,
        }
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
