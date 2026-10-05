//! Physical memory: Multiboot2 information parsing, a bitmap frame
//! allocator, and the boot module registry used to load user programs.
//!
//! Frames below 1 MiB, the kernel image itself, the Multiboot2
//! information block and every loaded module are permanently reserved.
//! The allocator covers up to 2 GiB of physical memory; regions beyond
//! that are ignored (and never identity-mapped).

use rootware_abi::ErrorCode;

pub const FRAME_SIZE: u64 = 4096;
/// Bitmap capacity: 2 GiB / 4 KiB frames, one bit each.
const MAX_FRAMES: usize = 2 * 1024 * 1024 * 1024 / FRAME_SIZE as usize;
const BITMAP_WORDS: usize = MAX_FRAMES / 64;
/// Maximum number of boot modules (user programs) tracked.
pub const MAX_MODULES: usize = 8;
const MAX_MODULE_NAME: usize = 16;

const MMAP_TAG: u32 = 6;
const MODULE_TAG: u32 = 3;
const MMAP_ENTRY_AVAILABLE: u32 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ModuleInfo {
    /// NUL-terminated program name copied from the module cmdline.
    pub name: [u8; MAX_MODULE_NAME],
    /// Physical [start, end) of the module image (an ELF binary).
    pub start: u64,
    pub end: u64,
}

impl ModuleInfo {
    pub fn name_str(&self) -> Option<&str> {
        let len = self.name.iter().position(|&b| b == 0).unwrap_or(self.name.len());
        core::str::from_utf8(&self.name[..len]).ok()
    }
}

static mut BITMAP: [u64; BITMAP_WORDS] = [u64::MAX; BITMAP_WORDS];
static mut FREE_FRAMES: usize = 0;
static mut USABLE_TOP: u64 = 0;
static mut MODULES: [Option<ModuleInfo>; MAX_MODULES] = [None; MAX_MODULES];
static mut MODULE_COUNT: usize = 0;

// --- Bitmap primitives (pure, host-testable) ---

fn mark(bitmap: &mut [u64; BITMAP_WORDS], frame: usize, used: bool) {
    let word = frame / 64;
    let bit = 1u64 << (frame % 64);
    if used {
        bitmap[word] |= bit;
    } else {
        bitmap[word] &= !bit;
    }
}

fn is_free(bitmap: &[u64; BITMAP_WORDS], frame: usize) -> bool {
    bitmap[frame / 64] & (1u64 << (frame % 64)) == 0
}

fn first_free(bitmap: &[u64; BITMAP_WORDS]) -> Option<usize> {
    for (word, bits) in bitmap.iter().enumerate() {
        let free = !bits;
        if free != 0 {
            return Some(word * 64 + free.trailing_zeros() as usize);
        }
    }
    None
}

fn mark_range_free(bitmap: &mut [u64; BITMAP_WORDS], start_frame: usize, end_frame: usize) {
    for frame in start_frame..end_frame {
        mark(bitmap, frame, false);
    }
}

fn mark_range_used(bitmap: &mut [u64; BITMAP_WORDS], start_frame: usize, end_frame: usize) {
    for frame in start_frame..end_frame {
        if !is_free(bitmap, frame) {
            continue;
        }
        mark(bitmap, frame, true);
    }
}

// --- Frame allocator ---

pub fn alloc_frame() -> Option<u64> {
    unsafe {
        let frame = first_free(&*(&raw const BITMAP))?;
        mark(&mut *(&raw mut BITMAP), frame, true);
        FREE_FRAMES -= 1;
        Some(frame as u64 * FRAME_SIZE)
    }
}

pub fn free_frame(addr: u64) -> Result<(), ErrorCode> {
    if addr % FRAME_SIZE != 0 {
        return Err(ErrorCode::InvalidArgument);
    }
    let frame = (addr / FRAME_SIZE) as usize;
    if frame >= MAX_FRAMES {
        return Err(ErrorCode::InvalidArgument);
    }
    unsafe {
        if is_free(&*(&raw const BITMAP), frame) {
            return Err(ErrorCode::InvalidArgument);
        }
        mark(&mut *(&raw mut BITMAP), frame, false);
        FREE_FRAMES += 1;
    }
    Ok(())
}

pub fn free_count() -> usize {
    unsafe { FREE_FRAMES }
}

/// Highest usable physical address discovered at boot; the virtual memory
/// subsystem identity-maps everything below it.
pub fn usable_top() -> u64 {
    unsafe { USABLE_TOP }
}

pub fn module_count() -> usize {
    unsafe { MODULE_COUNT }
}

pub fn module(index: usize) -> Option<ModuleInfo> {
    unsafe { (*(&raw const MODULES))[index] }
}

pub fn find_module(name: &str) -> Option<ModuleInfo> {
    unsafe {
        (0..MODULE_COUNT)
            .filter_map(|i| (*(&raw const MODULES))[i])
            .find(|m| m.name_str() == Some(name))
    }
}

// --- Boot-time initialization ---

/// Parse the Multiboot2 information block, reserve kernel-owned ranges and
/// build the free frame bitmap.
pub fn init(multiboot_info: u64) {
    let info = multiboot_info as usize;
    if info == 0 || info & 7 != 0 {
        crate::serial_println!("[MEMORY] invalid Multiboot2 address");
        return;
    }
    let total_size = unsafe { core::ptr::read_unaligned(info as *const u32) } as usize;
    if total_size < 16 || total_size > 16 * 1024 * 1024 {
        crate::serial_println!("[MEMORY] invalid Multiboot2 size");
        return;
    }

    let kernel_end = extern_global_kernel_end();
    let mut top: u64 = 0;

    unsafe {
        let end = info + total_size;
        // Two passes: the memory map frees frames before module ranges are
        // reserved, whichever order GRUB placed the tags in. A single pass
        // would let the allocator hand out frames that still hold boot
        // module images.
        for pass in 0..2 {
            let mut tag = info + 8;
            while tag + 8 <= end {
                let typ = core::ptr::read_unaligned(tag as *const u32);
                let size = core::ptr::read_unaligned((tag + 4) as *const u32) as usize;
                if size < 8 {
                    crate::serial_println!("[MEMORY] invalid tag size");
                    return;
                }
                let mmap_pass = typ == MMAP_TAG && pass == 0;
                let module_pass = typ == MODULE_TAG && pass == 1;
                if !mmap_pass && !module_pass {
                    let next = tag + ((size + 7) & !7);
                    if next <= tag || next > end || typ == 0 {
                        break;
                    }
                    tag = next;
                    continue;
                }

                match typ {
                    MMAP_TAG => {
                        // mmap tag: type(4) size(4) entry_size(4) entry_version(4)
                        let entry_size =
                            core::ptr::read_unaligned((tag + 8) as *const u32) as usize;
                        if entry_size < 24 {
                            crate::serial_println!("[MEMORY] invalid mmap entry size");
                            return;
                        }
                        let entries_end = tag + size;
                        let mut entry = tag + 16;
                        while entry + entry_size <= entries_end {
                            let base = core::ptr::read_unaligned(entry as *const u64);
                            let length = core::ptr::read_unaligned((entry + 8) as *const u64);
                            let region_type =
                                core::ptr::read_unaligned((entry + 16) as *const u32);
                            if region_type == MMAP_ENTRY_AVAILABLE && length > 0 {
                                let region_start = (base + FRAME_SIZE - 1) & !(FRAME_SIZE - 1);
                                let region_end =
                                    base.saturating_add(length) & !(FRAME_SIZE - 1);
                                if region_end > top {
                                    top = region_end;
                                }
                                add_available_region(region_start, region_end, kernel_end);
                            }
                            entry += entry_size;
                        }
                    }
                    MODULE_TAG => {
                        let mod_start = core::ptr::read_unaligned((tag + 8) as *const u32) as u64;
                        let mod_end = core::ptr::read_unaligned((tag + 12) as *const u32) as u64;
                        register_module(tag + 16, tag + size, mod_start, mod_end);
                    }
                    _ => {}
                }

                let next = tag + ((size + 7) & !7);
                if next <= tag || next > end || typ == 0 {
                    break;
                }
                tag = next;
            }
        }

        // Keep the Multiboot2 block itself out of the allocator.
        reserve_range(info as u64, end as u64);
        USABLE_TOP = top;
        FREE_FRAMES = count_free(&*(&raw const BITMAP));
    }

    crate::serial_println!(
        "[MEMORY] frame allocator ready: {} free frames, usable top {:#x}, {} module(s)",
        free_count(),
        usable_top(),
        module_count()
    );
}

/// First frame the allocator may hand out: just past the kernel image.
/// The symbol comes from the linker script on the bare-metal target.
#[cfg(target_os = "none")]
fn extern_global_kernel_end() -> u64 {
    unsafe extern "C" {
        static kernel_end: u8;
    }
    unsafe { &kernel_end as *const u8 as u64 }
}

#[cfg(not(target_os = "none"))]
fn extern_global_kernel_end() -> u64 {
    // Host tests have no kernel image; nothing is pre-reserved.
    0
}

fn add_available_region(start: u64, end: u64, kernel_end: u64) {
    if end <= start {
        return;
    }
    let first = (start / FRAME_SIZE) as usize;
    let last = (end / FRAME_SIZE) as usize;
    if first >= MAX_FRAMES {
        return;
    }
    unsafe {
        mark_range_free(&mut *(&raw mut BITMAP), first, last.min(MAX_FRAMES));
    }
    // Permanently reserved: everything below 1 MiB and the kernel image.
    let reserved_end = kernel_end.max(0x100000);
    reserve_range(0, reserved_end);
}

fn reserve_range(start: u64, end: u64) {
    if end <= start {
        return;
    }
    let first = (start / FRAME_SIZE) as usize;
    let last = ((end + FRAME_SIZE - 1) / FRAME_SIZE) as usize;
    if first >= MAX_FRAMES {
        return;
    }
    unsafe {
        mark_range_used(&mut *(&raw mut BITMAP), first, last.min(MAX_FRAMES));
    }
}

fn count_free(bitmap: &[u64; BITMAP_WORDS]) -> usize {
    bitmap
        .iter()
        .map(|word| (!word).count_ones() as usize)
        .sum()
}

fn register_module(cmdline: usize, tag_end: usize, start: u64, end: u64) {
    if start >= end || end > (MAX_FRAMES as u64) * FRAME_SIZE {
        return;
    }
    // grub.cfg loads modules as `module /boot/<name> [args]`; the program
    // name is the basename of the first cmdline token.
    let token = unsafe {
        let mut len = 0;
        while cmdline + len < tag_end && len < 64 {
            let byte = core::ptr::read_unaligned((cmdline + len) as *const u8);
            if byte == 0 || byte == b' ' {
                break;
            }
            len += 1;
        }
        core::slice::from_raw_parts(cmdline as *const u8, len)
    };
    let base = token
        .iter()
        .rposition(|&b| b == b'/')
        .map(|pos| &token[pos + 1..])
        .unwrap_or(token);

    let mut name = [0u8; MAX_MODULE_NAME];
    if base.len() >= MAX_MODULE_NAME {
        return;
    }
    name[..base.len()].copy_from_slice(base);

    unsafe {
        if MODULE_COUNT >= MAX_MODULES {
            return;
        }
        // grub may place module images inside the region the kernel's own
        // .bss occupies (the ELF load segments do not reflect the
        // linker-reserved heap and stacks, so its zeroing pass can eat
        // them). Relocate the image into the heap immediately, before
        // anything can overwrite it; the original range stays reserved
        // either way.
        let size = (end - start) as usize;
        let layout = core::alloc::Layout::from_size_align(size, 16).ok();
        let buffer = layout
            .map(|layout| alloc::alloc::alloc(layout))
            .filter(|pointer| !pointer.is_null());
        let Some(buffer) = buffer else {
            crate::serial_println!("[MEMORY] heap copy failed for module image");
            return;
        };
        core::ptr::copy_nonoverlapping(start as *const u8, buffer, size);
        let copy_start = buffer as u64;
        (*(&raw mut MODULES))[MODULE_COUNT] = Some(ModuleInfo {
            name,
            start: copy_start,
            end: copy_start + size as u64,
        });
        MODULE_COUNT += 1;
    }
}


/// Read the module image bytes (safe only while the module memory is
/// identity-mapped, which the kernel guarantees for all RAM).
pub fn module_bytes(info: &ModuleInfo) -> &[u8] {
    unsafe {
        core::slice::from_raw_parts(info.start as *const u8, (info.end - info.start) as usize)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bitmap_mark_alloc_and_free_roundtrip() {
        let mut bitmap = [u64::MAX; BITMAP_WORDS];
        assert_eq!(first_free(&bitmap), None);
        mark_range_free(&mut bitmap, 10, 13);
        assert_eq!(first_free(&bitmap), Some(10));
        mark(&mut bitmap, 10, true);
        assert_eq!(first_free(&bitmap), Some(11));
        assert!(is_free(&bitmap, 12));
        mark_range_used(&mut bitmap, 10, 13);
        assert_eq!(first_free(&bitmap), None);
    }

    #[test]
    fn bitmap_handles_word_boundaries() {
        let mut bitmap = [u64::MAX; BITMAP_WORDS];
        mark_range_free(&mut bitmap, 62, 67);
        assert_eq!(first_free(&bitmap), Some(62));
        assert!(is_free(&bitmap, 63));
        assert!(is_free(&bitmap, 64));
        assert!(is_free(&bitmap, 66));
        mark(&mut bitmap, 64, true);
        assert_eq!(first_free(&bitmap), Some(62));
        mark(&mut bitmap, 62, true);
        mark(&mut bitmap, 63, true);
        assert_eq!(first_free(&bitmap), Some(65));
    }

    #[test]
    fn module_names_parse() {
        let mut name = [0u8; 16];
        name[..4].copy_from_slice(b"echo");
        let info = ModuleInfo {
            name,
            start: 0x1000,
            end: 0x2000,
        };
        assert_eq!(info.name_str(), Some("echo"));
    }
}
