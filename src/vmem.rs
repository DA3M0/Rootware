//! Four-level x86_64 paging with a real CR3 switch.

use core::ptr::{read_volatile, write_volatile};

pub const PAGE_SIZE: u64 = 4096;
pub const PRESENT: u64 = 1;
pub const WRITABLE: u64 = 1 << 1;
pub const USER: u64 = 1 << 2;
const HUGE_PAGE: u64 = 1 << 7;
const ADDRESS_MASK: u64 = 0x000f_ffff_ffff_f000;

#[repr(C, align(4096))]
pub struct PageTable {
    entries: [u64; 512],
}

impl PageTable {
    const fn empty() -> Self {
        Self { entries: [0; 512] }
    }

    fn clear(&mut self) {
        for entry in &mut self.entries {
            *entry = 0;
        }
    }
}

#[unsafe(link_section = ".data")]
static mut PML4: PageTable = PageTable::empty();
#[unsafe(link_section = ".data")]
static mut PDP: PageTable = PageTable::empty();
#[unsafe(link_section = ".data")]
static mut PD: [PageTable; 2] = [PageTable::empty(), PageTable::empty()];
#[unsafe(link_section = ".data")]
static mut PT: PageTable = PageTable::empty();

fn index(virt: u64, shift: u32) -> usize {
    ((virt >> shift) & 0x1ff) as usize
}

fn aligned_page(address: u64) -> bool {
    address & (PAGE_SIZE - 1) == 0
}

fn canonical(virt: u64) -> bool {
    virt < (1 << 48)
}

unsafe fn table_address(table: *const PageTable) -> u64 {
    table as u64 & ADDRESS_MASK
}

unsafe fn load_cr3() {
    core::arch::asm!(
        "mov cr3, {root}",
        root = in(reg) table_address(&raw const PML4),
        options(nostack, preserves_flags)
    );
}

unsafe fn target_pt() -> Option<*mut PageTable> {
    let pml4 = read_volatile((*&raw const PML4).entries.as_ptr().add(0));
    if pml4 & PRESENT == 0 {
        return None;
    }
    let pdp = &raw mut PDP;
    let pdp_entry = read_volatile((*pdp).entries.as_ptr().add(1));
    if pdp_entry & PRESENT == 0 {
        return None;
    }
    let pd = &raw mut PD[1];
    let pd_entry = read_volatile((*pd).entries.as_ptr().add(0));
    if pd_entry & PRESENT == 0 || pd_entry & HUGE_PAGE != 0 {
        return None;
    }
    Some(&raw mut PT)
}

pub fn init() {
    unsafe {
        (*&raw mut PML4).clear();
        (*&raw mut PDP).clear();
        (*&raw mut PD[0]).clear();
        (*&raw mut PD[1]).clear();
        (*&raw mut PT).clear();

        // Keep all kernel code, stack, Multiboot data, and the test physical
        // page identity-mapped before replacing GRUB's page tables.
        (*&raw mut PML4).entries[0] =
            table_address(&raw const PDP) | PRESENT | WRITABLE;
        (*&raw mut PDP).entries[0] =
            table_address(&raw const PD[0]) | PRESENT | WRITABLE;
        (*&raw mut PD[0]).entries[0] = PRESENT | WRITABLE | HUGE_PAGE;
        (*&raw mut PD[0]).entries[1] =
            0x0020_0000 | PRESENT | WRITABLE | HUGE_PAGE;

        // 0x4000_0000 uses PDP index 1 and PD index 0.
        (*&raw mut PDP).entries[1] =
            table_address(&raw const PD[1]) | PRESENT | WRITABLE;
        (*&raw mut PD[1]).entries[0] =
            table_address(&raw const PT) | PRESENT | WRITABLE;

        load_cr3();
    }
    crate::serial_println!("[VMEM] CR3 switched to Rootware PML4");
}

pub fn map_page(virt: u64, phys: u64, flags: u64) -> Result<(), ()> {
    if !canonical(virt) || !aligned_page(virt) || !aligned_page(phys) {
        return Err(());
    }
    unsafe {
        let pt = target_pt().ok_or(())?;
        if index(virt, 39) != 0 || index(virt, 30) != 1 || index(virt, 21) != 0 {
            return Err(());
        }
        (*pt).entries[index(virt, 12)] = (phys & ADDRESS_MASK) | (flags & 0xfff) | PRESENT;
    }
    crate::serial_println!("[VMEM] page mapped: {:#x} -> {:#x}", virt, phys);
    Ok(())
}

pub fn unmap_page(virt: u64) -> Result<(), ()> {
    if !canonical(virt) || !aligned_page(virt) {
        return Err(());
    }
    unsafe {
        let pt = target_pt().ok_or(())?;
        (*pt).entries[index(virt, 12)] = 0;
        core::arch::asm!("invlpg [{}]", in(reg) virt, options(nostack, preserves_flags));
    }
    crate::serial_println!("[VMEM] page unmapped: {:#x}", virt);
    Ok(())
}

pub fn translate(virt: u64) -> Option<u64> {
    if !canonical(virt) {
        return None;
    }
    unsafe {
        let pml4e = read_volatile((*&raw const PML4).entries.as_ptr().add(index(virt, 39)));
        if pml4e & PRESENT == 0 {
            return None;
        }
        let pdpe = read_volatile((*&raw const PDP).entries.as_ptr().add(index(virt, 30)));
        if pdpe & PRESENT == 0 {
            return None;
        }
        let pd = if index(virt, 30) == 0 {
            &raw const PD[0]
        } else {
            &raw const PD[1]
        };
        let pde = read_volatile((*pd).entries.as_ptr().add(index(virt, 21)));
        if pde & PRESENT == 0 {
            return None;
        }
        if pde & HUGE_PAGE != 0 {
            return Some((pde & 0x000f_ffff_ffe0_0000) | (virt & 0x1f_ffff));
        }
        let entry = read_volatile((*&raw const PT).entries.as_ptr().add(index(virt, 12)));
        (entry & PRESENT != 0).then_some((entry & ADDRESS_MASK) | (virt & 0xfff))
    }
}

pub fn test() {
    const VIRT: u64 = 0x4000_0000;
    const PHYS: u64 = 0x0020_0000;
    const VALUE: u64 = 0xfeed_face_cafe_beef;

    map_page(VIRT, PHYS, WRITABLE).expect("VMEM map failed");
    assert_eq!(translate(VIRT + 8), Some(PHYS + 8));
    unsafe {
        write_volatile(VIRT as *mut u64, VALUE);
        assert_eq!(read_volatile(PHYS as *const u64), VALUE);
    }
    crate::serial_println!("[VMEM] virtual write -> physical read passed");
    unmap_page(VIRT).expect("VMEM unmap failed");
    assert_eq!(translate(VIRT), None);
}
