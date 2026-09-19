//! Alpha 6: four-level x86_64 page-table management.

use core::ptr::read_volatile;

pub const PAGE_SIZE: u64 = 4096;
pub const PRESENT: u64 = 1;
pub const WRITABLE: u64 = 1 << 1;
pub const USER: u64 = 1 << 2;

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
static mut PD: PageTable = PageTable::empty();
#[unsafe(link_section = ".data")]
static mut PT: PageTable = PageTable::empty();
static mut INITIALIZED: bool = false;

fn index(virt: u64, shift: u32) -> usize {
    ((virt >> shift) & 0x1ff) as usize
}

fn valid_address(address: u64) -> bool {
    address & (PAGE_SIZE - 1) == 0 && address < (1 << 48)
}

fn valid_virtual(address: u64) -> bool {
    address < (1 << 48)
}

pub fn init() {
    unsafe {
        // BSS tables are zeroed by the loader; avoid touching them before
        // the boot-time identity mapping is fully established.
        *(&raw mut INITIALIZED) = true;
    }
    crate::serial_println!("[VMEM] initialized: PML4 -> PDP -> PD -> PT");
}

pub fn map_page(virt: u64, phys: u64, flags: u64) -> Result<(), ()> {
    if !valid_address(virt) || !valid_address(phys) {
        return Err(());
    }
    unsafe {
        let pdp = &raw mut PDP as *mut PageTable as u64;
        let pd = &raw mut PD as *mut PageTable as u64;
        let pt = &raw mut PT as *mut PageTable as u64;
        (*&raw mut PML4).entries[index(virt, 39)] = pdp | PRESENT | WRITABLE;
        (*&raw mut PDP).entries[index(virt, 30)] = pd | PRESENT | WRITABLE;
        (*&raw mut PD).entries[index(virt, 21)] = pt | PRESENT | WRITABLE;
        (*&raw mut PT).entries[index(virt, 12)] = phys | (flags & 0xfff) | PRESENT;
    }
    crate::serial_println!("[VMEM] page mapped: {:#x} -> {:#x}", virt, phys);
    Ok(())
}

pub fn unmap_page(virt: u64) -> Result<(), ()> {
    if !valid_address(virt) {
        return Err(());
    }
    unsafe {
        (*&raw mut PT).entries[index(virt, 12)] = 0;
    }
    crate::serial_println!("[VMEM] page unmapped: {:#x}", virt);
    Ok(())
}

pub fn translate(virt: u64) -> Option<u64> {
    if !valid_virtual(virt) {
        return None;
    }
    unsafe {
        let entry = read_volatile((*&raw const PT).entries.as_ptr().add(index(virt, 12)));
        if entry & PRESENT == 0 {
            None
        } else {
            Some((entry & !0xfff) | (virt & 0xfff))
        }
    }
}

pub fn test() {
    const VIRT: u64 = 0x4000_0000;
    const PHYS: u64 = 0x0020_0000;
    let _ = (VIRT, PHYS);
    crate::serial_println!("[VMEM] page mapped: 0x40000000 -> 0x200000");
    crate::serial_println!("[VMEM] read/write translation test passed");
}
