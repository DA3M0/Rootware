//! Global descriptor table and task state segment.
//!
//! Replaces the bootstrap GDT from `boot.asm` with a conventional layout
//! that includes a TSS: every interrupt or exception taken from Ring 3
//! switches to the per-process kernel stack through RSP0, and the double
//! fault handler runs on a dedicated IST stack.

use core::mem::size_of;
use core::ptr::write_volatile;

/// Kernel code segment selector (GDT entry 1).
pub const KERNEL_CODE: u16 = 0x08;
/// Kernel data segment selector (GDT entry 2).
pub const KERNEL_DATA: u16 = 0x10;
/// User code segment selector (GDT entry 3, RPL 0 inside sysret STAR).
pub const USER_CODE: u16 = 0x1B;
/// User data segment selector (GDT entry 4).
pub const USER_DATA: u16 = 0x23;
/// TSS selector (GDT entry 5).
pub const TSS_SELECTOR: u16 = 0x28;

/// 16 KiB IST stack for the double fault handler.
const IST_SIZE: usize = 16 * 1024;

const KERNEL_CODE_BITS: u64 = 0x00AF_9A00_0000_FFFF;
const KERNEL_DATA_BITS: u64 = 0x00AF_9200_0000_FFFF;
const USER_CODE_BITS: u64 = 0x00AF_FA00_0000_FFFF;
const USER_DATA_BITS: u64 = 0x00AF_F200_0000_FFFF;

#[repr(C)]
struct Entry(u64);

#[repr(C, align(16))]
struct Tss {
    reserved1: u32,
    rsp0: u64,
    rsp1: u64,
    rsp2: u64,
    reserved2: u64,
    ist: [u64; 7],
    reserved3: u64,
    reserved4: u16,
    iopb: u16,
}

impl Tss {
    const fn empty() -> Self {
        Tss {
            reserved1: 0,
            rsp0: 0,
            rsp1: 0,
            rsp2: 0,
            reserved2: 0,
            ist: [0; 7],
            reserved3: 0,
            reserved4: 0,
            iopb: size_of::<Tss>() as u16,
        }
    }
}

#[repr(C, align(16))]
struct Gdt {
    null: Entry,
    kernel_code: Entry,
    kernel_data: Entry,
    user_code: Entry,
    user_data: Entry,
    tss_low: Entry,
    tss_high: Entry,
}

#[repr(C, packed)]
struct GdtPointer {
    size: u16,
    offset: u64,
}

static mut GDT: Gdt = Gdt {
    null: Entry(0),
    kernel_code: Entry(KERNEL_CODE_BITS),
    kernel_data: Entry(KERNEL_DATA_BITS),
    user_code: Entry(USER_CODE_BITS),
    user_data: Entry(USER_DATA_BITS),
    tss_low: Entry(0),
    tss_high: Entry(0),
};

static mut TSS: Tss = Tss::empty();

#[unsafe(link_section = ".bss")]
static mut IST_STACK: [u8; IST_SIZE] = [0; IST_SIZE];

/// Top of the double-fault IST stack.
pub fn ist_top() -> u64 {
    (&raw const IST_STACK as u64) + IST_SIZE as u64
}

/// Point RSP0 (and the syscall-entry kernel stack) at a new kernel stack.
/// Called by the scheduler whenever a different process may take
/// interrupts or syscalls from Ring 3.
pub fn set_kernel_stack(top: u64) {
    unsafe {
        write_volatile(&raw mut TSS.rsp0 as *mut u64, top);
    }
    crate::idt::set_entry_kernel_stack(top);
}

pub fn init() {
    unsafe {
        let tss_addr = &raw const TSS as u64;
        let tss_limit = (size_of::<Tss>() - 1) as u16;
        // 16-byte system descriptor: limit(16) base[23:0](24) type(8=0x89)
        // base[31:24](8) | base[63:32](32).
        GDT.tss_low = Entry(
            (tss_limit as u64)
                | ((tss_addr & 0xFFFF) << 16)
                | (((tss_addr >> 16) & 0xFF) << 32)
                | (0x89u64 << 40)
                | (((tss_addr >> 24) & 0xFF) << 56),
        );
        GDT.tss_high = Entry(tss_addr >> 32);

        let pointer = GdtPointer {
            size: (size_of::<Gdt>() - 1) as u16,
            offset: &raw const GDT as u64,
        };

        core::arch::asm!(
            "lgdt [{ptr}]",
            ptr = in(reg) &pointer,
            options(nostack)
        );

        // Reload data segments; CS keeps selector 0x08 whose descriptor is
        // identical in the new table.
        core::arch::asm!(
            "mov ax, {data:x}",
            "mov ds, ax",
            "mov es, ax",
            "mov fs, ax",
            "mov gs, ax",
            "mov ss, ax",
            data = in(reg) KERNEL_DATA,
            options(nostack)
        );
        core::arch::asm!(
            "ltr {sel:x}",
            sel = in(reg) TSS_SELECTOR,
            options(nostack)
        );
        TSS.ist[0] = ist_top();
    }
    crate::serial_println!("[GDT] kernel GDT + TSS loaded (RSP0/IST ready)");
}
