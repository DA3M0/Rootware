//! Four-level x86_64 paging with per-process address spaces.
//!
//! Layout shared by every address space:
//! - PML4 entry 0 -> a per-space page directory pointer, whose entry 0
//!   points at the *shared* kernel directory chain (supervisor-only) and
//!   whose entry 1 holds the per-space user region `0x4000_0000..0x8000_0000`.
//! - The kernel identity-maps all usable RAM below the Multiboot2 memory
//!   map top with 2 MiB supervisor pages, so any physical frame (including
//!   page-table frames) is directly writable from kernel code no matter
//!   which address space is active.
//!
//! Every page-table frame comes from the frame allocator.

use rootware_abi::ErrorCode;

use crate::memory::{self, FRAME_SIZE};

pub const PAGE_SIZE: u64 = FRAME_SIZE;
pub const PRESENT: u64 = 1;
pub const WRITABLE: u64 = 1 << 1;
pub const USER: u64 = 1 << 2;
pub const HUGE_PAGE: u64 = 1 << 7;
const HUGE_ALIGN: u64 = 1 << 21;
/// Physical base of the local APIC MMIO region.
const APIC_MMIO: u64 = 0xFEE0_0000;
const ADDRESS_MASK: u64 = 0x000f_ffff_ffff_f000;
const HUGE_ADDRESS_MASK: u64 = 0x000f_ffff_ffe0_0000;

/// Exclusive start of the user region (PML4 entry 0, directory entry 1).
pub const USER_BASE: u64 = 0x4000_0000;
/// Exclusive end of the user region (one directory entry of address space).
pub const USER_LIMIT: u64 = 0x8000_0000;

/// Static kernel page directories covering up to 4 GiB of identity-mapped
/// RAM (1 GiB each, filled with 2 MiB pages).
const KERNEL_PDS: usize = 4;

#[repr(C, align(4096))]
#[derive(Clone, Copy)]
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

#[unsafe(link_section = ".bss")]
static mut PML4: PageTable = PageTable::empty();
#[unsafe(link_section = ".bss")]
static mut PDP: PageTable = PageTable::empty();
#[unsafe(link_section = ".bss")]
static mut KERNEL_PD: [PageTable; KERNEL_PDS] = [PageTable::empty(); KERNEL_PDS];

/// Physical address of the currently loaded PML4.
static mut CURRENT_PML4: u64 = 0;

/// An independent address space for one process. Frames stay owned by the
/// space until [`destroy_address_space`].
#[derive(Clone, Copy, Debug)]
pub struct AddressSpace {
    pub pml4: u64,
    pub pdp: u64,
    pub user_pd: u64,
}

fn index(virt: u64, shift: u32) -> usize {
    ((virt >> shift) & 0x1ff) as usize
}

fn aligned_page(address: u64) -> bool {
    address & (PAGE_SIZE - 1) == 0
}

fn canonical(virt: u64) -> bool {
    virt < (1 << 48)
}

/// Interpret a physical table address as a writable table. Only valid
/// because the kernel identity-maps all usable RAM.
unsafe fn table_at(phys: u64) -> &'static mut PageTable {
    unsafe { &mut *((phys & ADDRESS_MASK) as *mut PageTable) }
}

pub fn current_pml4() -> u64 {
    unsafe { CURRENT_PML4 }
}

unsafe fn load_cr3(root: u64) {
    unsafe {
        core::arch::asm!(
            "mov cr3, {root}",
            root = in(reg) root,
            options(nostack, preserves_flags)
        );
    }
}

/// Build the kernel address space: identity-map all usable RAM with
/// supervisor 2 MiB pages and load CR3.
pub fn init() {
    let top = memory::usable_top();
    unsafe {
        (*&raw mut PML4).clear();
        (*&raw mut PDP).clear();
        let kernel_pds = &raw mut KERNEL_PD;
        for pd in (*kernel_pds).iter_mut() {
            pd.clear();
        }

        (*&raw mut PML4).entries[0] =
            table_address(&raw const PDP) | PRESENT | WRITABLE | USER;
        for i in 0..KERNEL_PDS {
            let gigabyte = (i as u64) << 30;
            if gigabyte >= top {
                break;
            }
            (*&raw mut PDP).entries[i] =
                table_address(&raw const KERNEL_PD[i]) | PRESENT | WRITABLE;
            // Fill the directory with 2 MiB pages up to the usable top.
            let mut slot = 0usize;
            while slot < 512 {
                let page_base = gigabyte + ((slot as u64) << 21);
                if page_base >= top {
                    break;
                }
                (*&raw mut KERNEL_PD[i]).entries[slot] = page_base | PRESENT | WRITABLE | HUGE_PAGE;
                slot += 1;
            }
        }

        // Device MMIO needed by the kernel: the local APIC.
        let apic_page = APIC_MMIO & !(HUGE_ALIGN - 1);
        let pd_index = (apic_page >> 30) as usize;
        let slot = ((apic_page >> 21) & 0x1ff) as usize;
        (*&raw mut PDP).entries[pd_index] =
            table_address(&raw const KERNEL_PD[pd_index]) | PRESENT | WRITABLE;
        (*&raw mut KERNEL_PD[pd_index]).entries[slot] =
            apic_page | PRESENT | WRITABLE | HUGE_PAGE;

        CURRENT_PML4 = table_address(&raw const PML4);
        load_cr3(CURRENT_PML4);
    }
    crate::serial_println!(
        "[VMEM] kernel space ready: identity map up to {:#x}",
        top
    );
}

fn table_address(table: *const PageTable) -> u64 {
    table as u64 & ADDRESS_MASK
}

/// The boot kernel address space; shared by all kernel tasks.
pub fn kernel_space() -> AddressSpace {
    AddressSpace {
        pml4: table_address(&raw const PML4),
        pdp: table_address(&raw const PDP),
        user_pd: 0,
    }
}

/// Allocate frames and wire up a fresh address space whose kernel half is
/// shared with the kernel mapping.
pub fn create_address_space() -> Option<AddressSpace> {
    let pml4 = memory::alloc_frame()?;
    let pdp = memory::alloc_frame()?;
    let user_pd = memory::alloc_frame()?;
    unsafe {
        table_at(pml4).clear();
        table_at(pdp).clear();
        table_at(user_pd).clear();

        table_at(pml4).entries[0] = pdp | PRESENT | WRITABLE | USER;
        // Kernel half: identical to the kernel PDP entry 0 chain.
        table_at(pdp).entries[0] = table_address(&raw const KERNEL_PD[0]) | PRESENT | WRITABLE;
        for i in 1..KERNEL_PDS {
            table_at(pdp).entries[i] = table_address(&raw const KERNEL_PD[i]) | PRESENT | WRITABLE;
        }
        // User half.
        table_at(pdp).entries[1] = user_pd | PRESENT | WRITABLE | USER;
    }
    Some(AddressSpace {
        pml4,
        pdp,
        user_pd,
    })
}

/// Free every frame owned by the space (user data, page tables) and the
/// space's own table frames. The shared kernel directory chain survives.
pub fn destroy_address_space(space: &AddressSpace) {
    unsafe {
        let user_pd = table_at(space.user_pd);
        for pd_entry in user_pd.entries.iter_mut() {
            if *pd_entry & PRESENT == 0 || *pd_entry & HUGE_PAGE != 0 {
                continue;
            }
            let pt_phys = *pd_entry & ADDRESS_MASK;
            let pt = table_at(pt_phys);
            for pt_entry in pt.entries.iter_mut() {
                if *pt_entry & PRESENT != 0 {
                    let _ = memory::free_frame(*pt_entry & ADDRESS_MASK);
                }
            }
            let _ = memory::free_frame(pt_phys);
            *pd_entry = 0;
        }
        let _ = memory::free_frame(space.user_pd);
        let _ = memory::free_frame(space.pdp);
        let _ = memory::free_frame(space.pml4);
    }
}

fn is_user_page(virt: u64) -> bool {
    virt >= USER_BASE && virt < USER_LIMIT
}

/// Map one 4 KiB page into the space's user region, allocating the
/// covering page table when needed. `virt` must be page-aligned.
pub fn map_user_page(space: &AddressSpace, virt: u64, phys: u64, flags: u64) -> Result<(), ErrorCode> {
    if !aligned_page(virt) || !aligned_page(phys) || !is_user_page(virt) {
        return Err(ErrorCode::InvalidArgument);
    }
    unsafe {
        let pt_phys = ensure_user_page_table(space, virt)?;
        table_at(pt_phys).entries[index(virt, 12)] =
            (phys & ADDRESS_MASK) | (flags & 0xfff) | PRESENT;
        if space.pml4 == current_pml4() {
            core::arch::asm!(
                "invlpg [{virt}]",
                virt = in(reg) virt,
                options(nostack, preserves_flags)
            );
        }
    }
    Ok(())
}

/// Map `pages` consecutive physical frames starting at `virt`/`phys`.
#[allow(dead_code)] // public mapping helper for future loaders
pub fn map_user_range(
    space: &AddressSpace,
    virt: u64,
    phys: u64,
    pages: u64,
    flags: u64,
) -> Result<(), ErrorCode> {
    for offset in 0..pages {
        map_user_page(
            space,
            virt + offset * PAGE_SIZE,
            phys + offset * PAGE_SIZE,
            flags,
        )?;
    }
    Ok(())
}

unsafe fn ensure_user_page_table(space: &AddressSpace, virt: u64) -> Result<u64, ErrorCode> {
    unsafe {
        let pd = table_at(space.user_pd);
        let slot = index(virt, 21);
        if pd.entries[slot] & PRESENT != 0 {
            if pd.entries[slot] & HUGE_PAGE != 0 {
                return Err(ErrorCode::InvalidArgument);
            }
            return Ok(pd.entries[slot] & ADDRESS_MASK);
        }
        let frame = memory::alloc_frame().ok_or(ErrorCode::QueueFull)?;
        table_at(frame).clear();
        pd.entries[slot] = frame | PRESENT | WRITABLE | USER;
        Ok(frame)
    }
}

/// Resolve a virtual address in the *currently loaded* space to its
/// physical address. Reads page tables through the kernel identity map,
/// so it is valid from any address space.
pub fn translate(virt: u64) -> Option<u64> {
    if !canonical(virt) {
        return None;
    }
    unsafe {
        let pml4 = table_at(current_pml4());
        let pml4e = pml4.entries[index(virt, 39)];
        if pml4e & PRESENT == 0 {
            return None;
        }
        let pdp = table_at(pml4e & ADDRESS_MASK);
        let pdpe = pdp.entries[index(virt, 30)];
        if pdpe & PRESENT == 0 {
            return None;
        }
        let pd = table_at(pdpe & ADDRESS_MASK);
        let pde = pd.entries[index(virt, 21)];
        if pde & PRESENT == 0 {
            return None;
        }
        if pde & HUGE_PAGE != 0 {
            return Some((pde & HUGE_ADDRESS_MASK) | (virt & 0x1f_ffff));
        }
        let pt = table_at(pde & ADDRESS_MASK);
        let entry = pt.entries[index(virt, 12)];
        (entry & PRESENT != 0).then_some((entry & ADDRESS_MASK) | (virt & 0xfff))
    }
}

/// True when every byte of `[virt, virt+len)` resolves to a present,
/// user-accessible page in the current space.
pub fn is_user_accessible(virt: u64, len: u64) -> bool {
    if len == 0 || !is_user_page(virt) {
        return false;
    }
    let end = match virt.checked_add(len) {
        Some(end) if end <= USER_LIMIT => end,
        _ => return false,
    };
    let mut page = virt & !(PAGE_SIZE - 1);
    while page < end {
        match translate(page) {
            Some(phys) => {
                // Re-read the flag bits through the walk: translate() drops
                // them, so check the last-level entry directly.
                if !last_level_user(page) {
                    return false;
                }
                let _ = phys;
            }
            None => return false,
        }
        page += PAGE_SIZE;
    }
    true
}

fn last_level_user(virt: u64) -> bool {
    unsafe {
        let pml4 = table_at(current_pml4());
        let pml4e = pml4.entries[index(virt, 39)];
        if pml4e & PRESENT == 0 {
            return false;
        }
        let pdp = table_at(pml4e & ADDRESS_MASK);
        let pdpe = pdp.entries[index(virt, 30)];
        if pdpe & PRESENT == 0 {
            return false;
        }
        let pd = table_at(pdpe & ADDRESS_MASK);
        let pde = pd.entries[index(virt, 21)];
        if pde & PRESENT == 0 {
            return false;
        }
        if pde & HUGE_PAGE != 0 {
            return pde & USER != 0;
        }
        let pt = table_at(pde & ADDRESS_MASK);
        let entry = pt.entries[index(virt, 12)];
        entry & PRESENT != 0 && entry & USER != 0
    }
}

/// Load a process address space (no-op when it is already active).
pub fn switch_to(space: &AddressSpace) {
    unsafe {
        if CURRENT_PML4 != space.pml4 {
            CURRENT_PML4 = space.pml4;
            load_cr3(space.pml4);
        }
    }
}

/// Switch back to the kernel address space.
pub fn switch_to_kernel() {
    unsafe {
        let kernel = table_address(&raw const PML4);
        if CURRENT_PML4 != kernel {
            CURRENT_PML4 = kernel;
            load_cr3(kernel);
        }
    }
}

pub fn selftest() {
    let before = memory::free_count();
    let space = create_address_space().expect("selftest: create space");
    let frame = memory::alloc_frame().expect("selftest: alloc frame");

    map_user_page(&space, USER_BASE, frame, USER | WRITABLE).expect("selftest: map");
    assert_eq!(
        current_pml4(),
        table_address(&raw const PML4),
        "selftest: kernel space must stay active"
    );

    // Write through the kernel identity map, resolve through the space.
    unsafe {
        core::ptr::write_volatile(frame as *mut u64, 0xfeed_face_cafe_beef);
    }
    assert_eq!(translate_in_space(&space, USER_BASE), Some(frame));

    // Temporarily switch, access through the user address, switch back.
    switch_to(&space);
    unsafe {
        let value = core::ptr::read_volatile(USER_BASE as *const u64);
        assert_eq!(value, 0xfeed_face_cafe_beef);
    }
    switch_to_kernel();

    destroy_address_space(&space);
    let _ = memory::free_frame(frame);
    assert_eq!(
        memory::free_count(),
        before,
        "selftest: every frame must be returned"
    );
    crate::serial_println!("[VMEM] address space selftest passed");
}

pub(crate) fn translate_in_space(space: &AddressSpace, virt: u64) -> Option<u64> {
    unsafe {
        let pml4 = table_at(space.pml4);
        let pml4e = pml4.entries[index(virt, 39)];
        if pml4e & PRESENT == 0 {
            return None;
        }
        let pdp = table_at(pml4e & ADDRESS_MASK);
        let pdpe = pdp.entries[index(virt, 30)];
        if pdpe & PRESENT == 0 {
            return None;
        }
        let pd = table_at(pdpe & ADDRESS_MASK);
        let pde = pd.entries[index(virt, 21)];
        if pde & PRESENT == 0 {
            return None;
        }
        let pt = table_at(pde & ADDRESS_MASK);
        let entry = pt.entries[index(virt, 12)];
        (entry & PRESENT != 0).then_some((entry & ADDRESS_MASK) | (virt & 0xfff))
    }
}
