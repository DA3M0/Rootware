//! IDT（中断描述符表）模块
//!
//! 每个向量都有完整栈帧的处理器：所有通用寄存器被保存，带错误码的
//! 异常与不带错误码的异常使用统一的帧布局。双重故障运行在独立 IST
//! 栈上。syscall 入口切换到每进程内核栈，STAR/FMASK 与新 GDT 的
//! Ring 3 描述符配套。

use core::arch::global_asm;
use core::mem;

global_asm!(
    ".global rootware_syscall_entry",
    "rootware_syscall_entry:",
    // The `syscall` instruction leaves rsp on the user stack and clobbers
    // only rcx (return rip) and r11 (rflags). Stash those two on the user
    // stack, bridge the user rsp through a static, then switch to the
    // current process's kernel stack. Every register the kernel handler
    // may clobber (SysV caller-saved: rdi/rsi/rdx/r8-r11) is saved around
    // the call so the user keeps its register state across syscalls.
    "push rcx",                         // user stack: return rip
    "push r11",                         // user stack: rflags
    "mov [rip + KSTACK_SAVE], rsp",     // remember user rsp
    "mov rsp, [rip + KSTACK_TOP]",      // switch to the kernel stack
    "push qword ptr [rip + KSTACK_SAVE]",
    "push rdi",
    "push rsi",
    "push rdx",
    "push r8",
    "push r9",
    "push r10",
    "mov rdx, rsi",
    "mov rsi, rdi",
    "mov rdi, rax",
    "call rootware_syscall_handler",
    "pop r10",
    "pop r9",
    "pop r8",
    "pop rdx",
    "pop rsi",
    "pop rdi",
    "pop rsp",                          // back to the user stack
    "pop r11",                          // rflags
    "pop rcx",                          // return rip
    "sysretq",
);

/// Kernel stack top used by the syscall entry (per current process).
#[unsafe(no_mangle)]
static mut KSTACK_TOP: u64 = 0;

/// Bridge slot carrying the user RSP across the stack switch. Single-core
/// kernel, so one slot suffices.
#[unsafe(no_mangle)]
static mut KSTACK_SAVE: u64 = 0;

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

    fn with_ist(mut self, ist: u8) -> Self {
        self.ist = ist & 0x07;
        self
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

/// syscall 入口与异常桩共用的完整中断帧（栈顶从 vector 开始）。
#[repr(C)]
pub struct ExceptionFrame {
    pub vector: u64,
    pub rax: u64,
    pub rbx: u64,
    pub rcx: u64,
    pub rdx: u64,
    pub rsi: u64,
    pub rdi: u64,
    pub rbp: u64,
    pub r8: u64,
    pub r9: u64,
    pub r10: u64,
    pub r11: u64,
    pub r12: u64,
    pub r13: u64,
    pub r14: u64,
    pub r15: u64,
    pub error: u64,
    pub rip: u64,
    pub cs: u64,
    pub rflags: u64,
    pub rsp: u64,
    pub ss: u64,
}

/// 生成带/不带错误码异常的统一桩：保存全部通用寄存器，压入向量号，
/// 调用公共分发器后按相反顺序恢复。
macro_rules! exception_stub {
    ($name:ident, $vector:expr, $error:tt) => {
        global_asm!(
            concat!(".global ", stringify!($name)),
            concat!(stringify!($name), ":"),
            $error,
            "push r15", "push r14", "push r13", "push r12", "push r11",
            "push r10", "push r9", "push r8", "push rbp", "push rdi",
            "push rsi", "push rdx", "push rcx", "push rbx", "push rax",
            concat!("push ", stringify!($vector)),
            "mov rdi, rsp",
            "call rootware_exception_dispatch",
            "add rsp, 8",
            "pop rax", "pop rbx", "pop rcx", "pop rdx", "pop rsi", "pop rdi",
            "pop rbp", "pop r8", "pop r9", "pop r10", "pop r11", "pop r12",
            "pop r13", "pop r14", "pop r15",
            "add rsp, 8",
            "iretq",
        );
    };
}

exception_stub!(rootware_exc_divide, 0, "push 0");
exception_stub!(rootware_exc_invalid_opcode, 6, "push 0");
exception_stub!(rootware_exc_double_fault, 8, "");
exception_stub!(rootware_exc_general_protection, 13, "");
exception_stub!(rootware_exc_page_fault, 14, "");
exception_stub!(rootware_exc_timer, 32, "push 0");
exception_stub!(rootware_exc_unexpected, 255, "push 0");

unsafe extern "C" {
    fn rootware_exc_divide();
    fn rootware_exc_invalid_opcode();
    fn rootware_exc_double_fault();
    fn rootware_exc_general_protection();
    fn rootware_exc_page_fault();
    fn rootware_exc_timer();
    fn rootware_exc_unexpected();
}

/// Update the kernel stack the syscall entry switches to. Must be called
/// before entering Ring 3 for a process.
pub fn set_entry_kernel_stack(top: u64) {
    unsafe {
        core::ptr::write_volatile(&raw mut KSTACK_TOP, top);
    }
}

/// Common dispatcher for every exception and device interrupt. Called from
/// the asm stubs with a pointer to the full frame.
#[unsafe(no_mangle)]
extern "C" fn rootware_exception_dispatch(frame: *mut ExceptionFrame) {
    let frame = unsafe { &mut *frame };
    match frame.vector {
        32 => {
            crate::timer::tick();
            // Drain the UART while interrupts are off: serial RX has no
            // interrupt of its own, the shell waits on this poll.
            crate::console::poll();
            crate::timer::send_eoi();
        }
        0..=31 => handle_exception(frame),
        _ => {
            // Unexpected device interrupt: acknowledge and continue.
            crate::timer::send_eoi();
        }
    }
}

fn handle_exception(frame: &mut ExceptionFrame) {
    let from_user = frame.cs & 3 == 3;
    // Double fault is unrecoverable regardless of the faulting ring.
    let fatal = !from_user || frame.vector == 8;

    if fatal {
        crate::serial_println!("[FAULT] kernel fault (vector {})", frame.vector);
        if frame.vector == 14 {
            let cr2: u64;
            unsafe {
                core::arch::asm!("mov {}, cr2", out(reg) cr2, options(nomem, nostack));
            }
            crate::serial_println!("[FAULT] page fault at {:#x}", cr2);
        }
        crate::serial_println!(
            "[FAULT] rip={:#x} rsp={:#x} cs={:#x} code={:#x}",
            frame.rip,
            frame.rsp,
            frame.cs,
            frame.error
        );
        loop {
            unsafe {
                core::arch::asm!("cli; hlt");
            }
        }
    }

    // Exception reached the kernel from Ring 3: report it, terminate the
    // offending process and keep the system alive. Never returns.
    crate::serial_println!(
        "[FAULT] process {} killed (vector {})",
        crate::process::current_id(),
        frame.vector
    );
    if frame.vector == 14 {
        let cr2: u64;
        unsafe {
            core::arch::asm!("mov {}, cr2", out(reg) cr2, options(nomem, nostack));
        }
        crate::serial_println!("[FAULT] page fault at {:#x}", cr2);
    }
    crate::process::kill_current();
}

/// 初始化 IDT
pub fn init() {
    unsafe {
        // --- syscall MSRs ---
        let lstar = rootware_syscall_entry as *const () as u64;
        core::arch::asm!(
            "wrmsr",
            in("ecx") 0xc0000082u32, // IA32_LSTAR
            in("eax") lstar as u32,
            in("edx") (lstar >> 32) as u32,
            options(nostack, preserves_flags)
        );
        // STAR: kernel CS 0x08 / SS 0x10; user base 0x18 so sysret yields
        // CS 0x1B and SS 0x23 (Ring 3 descriptors in the kernel GDT).
        core::arch::asm!(
            "wrmsr",
            in("ecx") 0xc0000081u32, // IA32_STAR
            in("eax") 0x0008u32,
            in("edx") 0x0018_0008u32,
            options(nostack, preserves_flags)
        );
        // FMASK: clear TF on syscall entry so user code cannot single-step
        // the kernel.
        core::arch::asm!(
            "wrmsr",
            in("ecx") 0xc0000084u32, // IA32_FMASK
            in("eax") 0x100u32,
            in("edx") 0u32,
            options(nostack, preserves_flags)
        );
        let mut efer_low: u32;
        let mut efer_high: u32;
        core::arch::asm!(
            "rdmsr",
            in("ecx") 0xc0000080u32, // IA32_EFER
            out("eax") efer_low,
            out("edx") efer_high,
            options(nostack)
        );
        efer_low |= 1; // SCE
        core::arch::asm!(
            "wrmsr",
            in("ecx") 0xc0000080u32,
            in("eax") efer_low,
            in("edx") efer_high,
            options(nostack, preserves_flags)
        );

        // --- exception gates ---
        IDT[0] = IdtEntry::new(rootware_exc_divide as *const () as u64, crate::gdt::KERNEL_CODE, 0x8E);
        IDT[6] = IdtEntry::new(rootware_exc_invalid_opcode as *const () as u64, crate::gdt::KERNEL_CODE, 0x8E);
        IDT[8] = IdtEntry::new(rootware_exc_double_fault as *const () as u64, crate::gdt::KERNEL_CODE, 0x8E)
            .with_ist(1);
        IDT[13] = IdtEntry::new(
            rootware_exc_general_protection as *const () as u64,
            0x08,
            0x8E,
        );
        IDT[14] = IdtEntry::new(rootware_exc_page_fault as *const () as u64, crate::gdt::KERNEL_CODE, 0x8E);

        // --- timer + catch-alls ---
        // Every vector gets a handler so stray device interrupts and
        // unimplemented exceptions report instead of faulting on a missing
        // gate.
        let unexpected = rootware_exc_unexpected as *const () as u64;
        let table = &raw mut IDT;
        for entry in (*table).iter_mut() {
            *entry = IdtEntry::new(unexpected, crate::gdt::KERNEL_CODE, 0x8E);
        }
        IDT[0] = IdtEntry::new(rootware_exc_divide as *const () as u64, crate::gdt::KERNEL_CODE, 0x8E);
        IDT[6] = IdtEntry::new(rootware_exc_invalid_opcode as *const () as u64, crate::gdt::KERNEL_CODE, 0x8E);
        IDT[8] = IdtEntry::new(rootware_exc_double_fault as *const () as u64, crate::gdt::KERNEL_CODE, 0x8E)
            .with_ist(1);
        IDT[13] = IdtEntry::new(
            rootware_exc_general_protection as *const () as u64,
            0x08,
            0x8E,
        );
        IDT[14] = IdtEntry::new(rootware_exc_page_fault as *const () as u64, crate::gdt::KERNEL_CODE, 0x8E);
        IDT[32] = IdtEntry::new(rootware_exc_timer as *const () as u64, crate::gdt::KERNEL_CODE, 0x8E);
        IDT[128] = IdtEntry::new(rootware_syscall_entry as *const () as u64, crate::gdt::KERNEL_CODE, 0xEE);

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
    crate::serial_println!("[IDT] 256 vectors + syscall gate ready");
}

#[unsafe(no_mangle)]
extern "C" fn rootware_syscall_handler(number: u64, first: u64, second: u64) -> i64 {
    crate::syscall::dispatch(number, first, second)
}
