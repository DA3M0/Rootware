//! Kernel heap: a first-fit free-list allocator over statically reserved
//! memory.
//!
//! The kernel is single-core, but host unit tests share this allocator
//! across test threads, so a tiny spinlock guards the free list.
//! Alignments up to 4096 are honored by padding; page tables come from
//! the frame allocator, not the heap.

use core::alloc::{GlobalAlloc, Layout};
use core::mem::size_of;
use core::sync::atomic::{AtomicBool, Ordering};

/// 4 MiB of heap, reserved in the kernel image.
const HEAP_SIZE: usize = 4 * 1024 * 1024;
/// Minimum block size: header plus 16 usable bytes keeps splitting sane.
const MIN_BLOCK: usize = size_of::<Block>() + 16;
const HDR: usize = size_of::<Block>();
/// Largest alignment served by the heap.
const MAX_ALIGN: usize = 4096;

#[repr(C)]
struct Block {
    size: usize,
    next: *mut Block,
}

#[repr(C, align(16))]
struct HeapArea([u8; HEAP_SIZE]);

#[unsafe(link_section = ".bss")]
static mut HEAP_AREA: HeapArea = HeapArea([0; HEAP_SIZE]);
static mut FREE_LIST: *mut Block = core::ptr::null_mut();
static mut INITIALIZED: bool = false;
static HEAP_LOCK: AtomicBool = AtomicBool::new(false);

struct HeapGuard;

impl HeapGuard {
    fn acquire() -> Self {
        while HEAP_LOCK
            .compare_exchange_weak(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            core::hint::spin_loop();
        }
        HeapGuard
    }
}

impl Drop for HeapGuard {
    fn drop(&mut self) {
        HEAP_LOCK.store(false, Ordering::Release);
    }
}

unsafe fn block_at(ptr: *mut Block) -> &'static mut Block {
    unsafe { &mut *ptr }
}

fn align_up(value: usize, align: usize) -> usize {
    (value + align - 1) & !(align - 1)
}

pub struct KernelHeap;

impl KernelHeap {
    unsafe fn reset() {
        unsafe {
            let first = &raw mut HEAP_AREA as *mut Block;
            (*first).size = HEAP_SIZE - size_of::<Block>();
            (*first).next = core::ptr::null_mut();
            FREE_LIST = first;
            INITIALIZED = true;
        }
    }

    /// Lazily initialize on first use when `init` was not called yet (host
    /// tests, early boot).
    pub(crate) fn ensure_init() {
        let _guard = HeapGuard::acquire();
        unsafe {
            if !INITIALIZED {
                Self::reset();
            }
        }
    }
}

/// Reserve the heap once at boot. Idempotent: early boot code (module
/// relocation) may have forced the lazy initialization already, and a
/// second reset would discard its allocations.
pub fn init() {
    KernelHeap::ensure_init();
    crate::serial_println!("[HEAP] kernel heap ready (4 MiB)");
}

unsafe impl GlobalAlloc for KernelHeap {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        unsafe {
            Self::ensure_init();
            if layout.align() > MAX_ALIGN {
                return core::ptr::null_mut();
            }
            let _guard = HeapGuard::acquire();
            let align = layout.align().max(16);
            let want = core::cmp::max((layout.size() + 15) & !15, MIN_BLOCK - HDR);

            let mut prev: *mut Block = core::ptr::null_mut();
            let mut current = FREE_LIST;
            while !current.is_null() {
                let block = block_at(current);
                let payload = align_up(current as usize + HDR, align);
                let pad = payload - (current as usize + HDR);
                if block.size >= pad + want {
                    // Split the tail when the remainder can stand alone.
                    if block.size >= pad + want + MIN_BLOCK {
                        let remainder = (current as *mut u8).add(HDR + pad + want) as *mut Block;
                        let rest = block_at(remainder);
                        rest.size = block.size - pad - want - HDR;
                        rest.next = block.next;
                        block.size = pad + want;
                        block.next = remainder;
                    }
                    // Unlink the allocated block.
                    let next = block.next;
                    if prev.is_null() {
                        FREE_LIST = next;
                    } else {
                        block_at(prev).next = next;
                    }
                    // Record the padding in the word before the payload so
                    // dealloc can walk back to the header.
                    (payload as *mut usize).sub(1).write(pad);
                    return payload as *mut u8;
                }
                prev = current;
                current = block.next;
            }
            core::ptr::null_mut()
        }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe {
            if ptr.is_null() {
                return;
            }
            let _guard = HeapGuard::acquire();
            let want = core::cmp::max((layout.size() + 15) & !15, MIN_BLOCK - HDR);
            let payload = ptr as usize;
            let pad = (payload as *const usize).sub(1).read();
            let block = (payload - pad - HDR) as *mut Block;
            (*block).size = want;

            // Insert sorted by address so adjacent blocks coalesce.
            let mut prev: *mut Block = core::ptr::null_mut();
            let mut current = FREE_LIST;
            while !current.is_null() && (current as usize) < (block as usize) {
                prev = current;
                current = block_at(current).next;
            }
            (*block).next = current;
            if prev.is_null() {
                FREE_LIST = block;
            } else {
                block_at(prev).next = block;
            }

            // Coalesce with the following block.
            let block_end = (block as usize) + HDR + (*block).size;
            if !current.is_null() && block_end == current as usize {
                block_at(block).size += HDR + block_at(current).size;
                block_at(block).next = block_at(current).next;
            }
            // Coalesce with the preceding block.
            if !prev.is_null() {
                let prev_end = (prev as usize) + HDR + block_at(prev).size;
                if prev_end == block as usize {
                    block_at(prev).size += HDR + block_at(block).size;
                    block_at(prev).next = block_at(block).next;
                }
            }
        }
    }
}

#[global_allocator]
static KERNEL_ALLOCATOR: KernelHeap = KernelHeap;

#[cfg(test)]
mod tests {
    use alloc::vec::Vec;
    use core::alloc::Layout;

    #[test]
    fn allocates_frees_and_reuses() {
        let mut a: Vec<u32> = Vec::new();
        let mut b: Vec<u64> = Vec::new();
        for i in 0..1000u32 {
            a.push(i);
            b.push(i as u64 * 7);
        }
        assert_eq!(a[999], 999);
        assert_eq!(b[999], 6993);
        drop(a);
        let mut c: Vec<u8> = Vec::new();
        c.resize(64 * 1024, 0xAB);
        assert_eq!(c[0], 0xAB);
        assert_eq!(c[c.len() - 1], 0xAB);
    }

    #[test]
    fn live_blocks_survive_later_traffic() {
        // Mirrors the boot-time pattern: three leaked module images at the
        // heap base, then Vec growth/drop traffic. A later 8 KiB fill of
        // 0xA5 must never touch the live blocks.
        let sizes = [0x19B8usize, 0x3E68, 0x28E0];
        let copies: Vec<(*mut u8, core::alloc::Layout)> = sizes
            .iter()
            .map(|size| {
                let layout = Layout::from_size_align(*size, 16).unwrap();
                let ptr = unsafe { alloc::alloc::alloc(layout) };
                assert!(!ptr.is_null());
                unsafe { core::ptr::write_bytes(ptr, 0x77, *size) };
                (ptr, layout)
            })
            .collect();

        let mut values: Vec<u64> = Vec::new();
        for index in 0..2000u64 {
            values.push(index);
        }
        drop(values);
        let mut block: Vec<u8> = Vec::new();
        block.resize(8192, 0xA5);
        assert_eq!(block[0], 0xA5);

        for (ptr, layout) in &copies {
            let bytes = unsafe { core::slice::from_raw_parts(*ptr, layout.size()) };
            assert!(
                bytes.iter().all(|byte| *byte == 0x77),
                "live allocation was overwritten"
            );
        }
    }

    #[test]
    fn honors_high_alignments() {
        let layout = Layout::from_size_align(512, 512).unwrap();
        let ptr = unsafe { alloc::alloc::alloc(layout) };
        assert!(!ptr.is_null());
        assert_eq!(ptr as usize % 512, 0);
        unsafe { alloc::alloc::dealloc(ptr, layout) };
    }
}
