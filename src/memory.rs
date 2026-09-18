//! 物理内存管理模块

/// 初始化物理内存管理
pub fn init(multiboot_info: u64) {
    unsafe {
        let info = multiboot_info as usize;
        if info == 0 || info & 7 != 0 {
            crate::serial::write_str("[MEMORY] invalid Multiboot2 address\n");
            return;
        }

        let total_size = core::ptr::read_unaligned(info as *const u32) as usize;
        if total_size < 16 || total_size > 16 * 1024 * 1024 {
            crate::serial::write_str("[MEMORY] invalid Multiboot2 size\n");
            return;
        }

        let end = match info.checked_add(total_size) {
            Some(end) => end,
            None => {
                crate::serial::write_str("[MEMORY] Multiboot2 range overflow\n");
                return;
            }
        };
        let mut tag = info + 8;

        while tag.checked_add(8).is_some_and(|header_end| header_end <= end) {
            let typ = core::ptr::read_unaligned(tag as *const u32);
            let size = core::ptr::read_unaligned((tag + 4) as *const u32) as usize;

            if size < 8 {
                crate::serial::write_str("[MEMORY] invalid tag size\n");
                return;
            }

            if typ == 6 {
                crate::serial::write_str("[MEMORY] found memory map\n");
            }

            let next = match tag.checked_add((size + 7) & !7) {
                Some(next) if next > tag && next <= end => next,
                _ => {
                    crate::serial::write_str("[MEMORY] invalid tag range\n");
                    return;
                }
            };

            if typ == 0 {
                crate::serial::write_str("[MEMORY] done\n");
                return;
            }
            tag = next;
        }

        crate::serial::write_str("[MEMORY] missing end tag\n");
    }
}
