use core::mem;

#[repr(C, packed)]
#[derive(Clone, Copy)]
struct Descriptor {
    limit_low: u16,
    base_low: u16,
    base_middle: u8,
    access: u8,
    granularity: u8,
    base_high: u8,
}

impl Descriptor {
    const fn null() -> Self {
        Descriptor {
            limit_low: 0,
            base_low: 0,
            base_middle: 0,
            access: 0,
            granularity: 0,
            base_high: 0,
        }
    }

    const fn kernel_code() -> Self {
        Descriptor {
            limit_low: 0xFFFF,
            base_low: 0,
            base_middle: 0,
            access: 0x9A,
            granularity: 0xAF,
            base_high: 0,
        }
    }

    const fn kernel_data() -> Self {
        Descriptor {
            limit_low: 0xFFFF,
            base_low: 0,
            base_middle: 0,
            access: 0x92,
            granularity: 0xCF,
            base_high: 0,
        }
    }
}

#[repr(C, packed)]
struct GdtPointer {
    size: u16,
    offset: u64,
}

static GDT: [Descriptor; 3] = [
    Descriptor::null(),
    Descriptor::kernel_code(),
    Descriptor::kernel_data(),
];

pub fn init() {
    let gdt_ptr = GdtPointer {
        size: (mem::size_of::<[Descriptor; 3]>() - 1) as u16,
        offset: GDT.as_ptr() as u64,
    };

    unsafe {
        // 加载 GDT
        core::arch::asm!(
            "lgdt [{}]",
            in(reg) &gdt_ptr,
            options(nostack)
        );

        // 只重载数据段寄存器
        core::arch::asm!(
            "mov ax, 0x10",
            "mov ds, ax",
            "mov es, ax",
            "mov fs, ax",
            "mov gs, ax",
            "mov ss, ax",
            options(nostack)
        );
    }
}
