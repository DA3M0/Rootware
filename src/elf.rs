//! Minimal ELF64 static executable loader.
//!
//! Supports what the userspace toolchain emits: little-endian ET_EXEC
//! images with PT_LOAD segments. Segments are copied into freshly
//! allocated frames mapped into the target address space; the tail
//! beyond `p_filesz` (BSS) is zeroed.

use rootware_abi::ErrorCode;

use crate::vmem::{self, AddressSpace, USER, USER_BASE, USER_LIMIT, WRITABLE};

const PT_LOAD: u32 = 1;
const ET_EXEC: u16 = 2;

/// A parsed program ready to be mapped.
#[derive(Clone, Copy, Debug)]
pub struct ProgramHeader {
    pub offset: u64,
    pub vaddr: u64,
    pub filesz: u64,
    pub memsz: u64,
}

/// Parse ELF headers and return the entry point with all PT_LOAD
/// segments. Pure function, no side effects — host-testable.
pub fn parse(bytes: &[u8]) -> Result<(u64, alloc::vec::Vec<ProgramHeader>), ErrorCode> {
    let ehdr = bytes
        .get(..64)
        .ok_or(ErrorCode::NotFound)?;
    if ehdr[..4] != [0x7F, b'E', b'L', b'F'] {
        return Err(ErrorCode::NotFound);
    }
    // 64-bit, little-endian, executable, SysV ABI.
    if ehdr[4] != 2 || ehdr[5] != 1 {
        return Err(ErrorCode::NotFound);
    }
    let e_type = u16::from_le_bytes([ehdr[16], ehdr[17]]);
    if e_type != ET_EXEC {
        return Err(ErrorCode::NotFound);
    }
    let entry = u64::from_le_bytes(ehdr[24..32].try_into().unwrap());
    let phoff = u64::from_le_bytes(ehdr[32..40].try_into().unwrap());
    let phentsize = u16::from_le_bytes(ehdr[54..56].try_into().unwrap()) as usize;
    let phnum = u16::from_le_bytes(ehdr[56..58].try_into().unwrap()) as usize;

    if entry < USER_BASE || entry >= USER_LIMIT {
        return Err(ErrorCode::InvalidArgument);
    }
    if phentsize < 56 || phnum > 32 {
        return Err(ErrorCode::InvalidArgument);
    }

    let mut segments = alloc::vec::Vec::new();
    for index in 0..phnum {
        let start = phoff as usize + index * phentsize;
        let phdr = bytes
            .get(start..start + 56)
            .ok_or(ErrorCode::NotFound)?;
        if u32::from_le_bytes(phdr[0..4].try_into().unwrap()) != PT_LOAD {
            continue;
        }
        let segment = ProgramHeader {
            offset: u64::from_le_bytes(phdr[8..16].try_into().unwrap()),
            vaddr: u64::from_le_bytes(phdr[16..24].try_into().unwrap()),
            filesz: u64::from_le_bytes(phdr[32..40].try_into().unwrap()),
            memsz: u64::from_le_bytes(phdr[40..48].try_into().unwrap()),
        };
        if segment.memsz == 0 {
            continue;
        }
        if segment.filesz > segment.memsz {
            return Err(ErrorCode::InvalidArgument);
        }
        if segment
            .offset
            .checked_add(segment.filesz)
            .is_none_or(|end| end > bytes.len() as u64)
        {
            return Err(ErrorCode::InvalidArgument);
        }
        if segment.vaddr < USER_BASE
            || segment
                .vaddr
                .checked_add(segment.memsz)
                .is_none_or(|end| end > USER_LIMIT)
        {
            return Err(ErrorCode::InvalidArgument);
        }
        segments.push(segment);
    }
    if segments.is_empty() {
        return Err(ErrorCode::NotFound);
    }
    Ok((entry, segments))
}

/// Load an ELF image into `space` and return the entry point.
pub fn load(space: &AddressSpace, bytes: &[u8]) -> Result<u64, ErrorCode> {
    let (entry, segments) = parse(bytes)?;
    for segment in &segments {
        map_segment(space, segment, bytes)?;
    }
    Ok(entry)
}

fn map_segment(
    space: &AddressSpace,
    segment: &ProgramHeader,
    bytes: &[u8],
) -> Result<(), ErrorCode> {
    let first_page = segment.vaddr & !0xFFF;
    let last_page = (segment.vaddr + segment.memsz).div_ceil(0x1000) * 0x1000;
    let mut page = first_page;
    while page < last_page {
        // A previous segment may already map this page (lld can emit a
        // small trailing segment sharing the last rodata page). Reuse the
        // existing frame instead of shadowing the earlier mapping.
        let frame = match vmem::translate_in_space(space, page) {
            Some(phys) => phys & !(0x1000 - 1),
            None => {
                let frame = crate::memory::alloc_frame().ok_or(ErrorCode::QueueFull)?;
                // Zero the whole frame, then copy the file bytes overlapping it.
                unsafe {
                    core::ptr::write_bytes(frame as *mut u8, 0, 0x1000);
                }
                vmem::map_user_page(space, page, frame, USER | WRITABLE)?;
                frame
            }
        };

        let copy_start = page.max(segment.vaddr);
        let copy_end = (page + 0x1000).min(segment.vaddr + segment.filesz);
        if copy_end > copy_start {
            let src = (segment.offset + (copy_start - segment.vaddr)) as usize;
            let len = (copy_end - copy_start) as usize;
            let dst = (frame + (copy_start - page)) as *mut u8;
            unsafe {
                core::ptr::copy_nonoverlapping(bytes[src..src + len].as_ptr(), dst, len);
            }
        }
        page += 0x1000;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ehdr(entry: u64, phoff: u64, phentsize: u16, phnum: u16, e_type: u16) -> [u8; 64] {
        let mut e = [0u8; 64];
        e[..4].copy_from_slice(&[0x7F, b'E', b'L', b'F']);
        e[4] = 2; // 64-bit
        e[5] = 1; // LE
        e[16..18].copy_from_slice(&e_type.to_le_bytes());
        e[24..32].copy_from_slice(&entry.to_le_bytes());
        e[32..40].copy_from_slice(&phoff.to_le_bytes());
        e[54..56].copy_from_slice(&phentsize.to_le_bytes());
        e[56..58].copy_from_slice(&phnum.to_le_bytes());
        e
    }

    fn phdr(vaddr: u64, filesz: u64, memsz: u64, offset: u64) -> [u8; 56] {
        let mut p = [0u8; 56];
        p[0..4].copy_from_slice(&PT_LOAD.to_le_bytes());
        p[8..16].copy_from_slice(&offset.to_le_bytes());
        p[16..24].copy_from_slice(&vaddr.to_le_bytes());
        p[32..40].copy_from_slice(&filesz.to_le_bytes());
        p[40..48].copy_from_slice(&memsz.to_le_bytes());
        p
    }

    #[test]
    fn parses_a_minimal_image() {
        let mut image = alloc::vec::Vec::new();
        image.extend_from_slice(&ehdr(USER_BASE, 64, 56, 1, ET_EXEC));
        image.extend_from_slice(&phdr(USER_BASE, 4, 8, 64));
        image.extend_from_slice(&[0xAA, 0xBB, 0xCC, 0xDD]);

        let (entry, segments) = parse(&image).unwrap();
        assert_eq!(entry, USER_BASE);
        assert_eq!(segments.len(), 1);
        assert_eq!(segments[0].vaddr, USER_BASE);
        assert_eq!(segments[0].memsz, 8);
    }

    #[test]
    fn rejects_bad_magic() {
        let mut image = alloc::vec::Vec::new();
        image.extend_from_slice(&ehdr(USER_BASE, 64, 56, 0, ET_EXEC));
        image[0] = 0x42;
        assert!(matches!(parse(&image), Err(ErrorCode::NotFound)));
    }

    #[test]
    fn rejects_entry_outside_user_range() {
        let image = ehdr(0x1000, 64, 56, 0, ET_EXEC);
        assert!(matches!(parse(&image), Err(ErrorCode::InvalidArgument)));
    }

    #[test]
    fn rejects_segment_overrunning_the_file() {
        let mut image = alloc::vec::Vec::new();
        image.extend_from_slice(&ehdr(USER_BASE, 64, 56, 1, ET_EXEC));
        image.extend_from_slice(&phdr(USER_BASE, 64, 999, 64));
        assert!(matches!(parse(&image), Err(ErrorCode::InvalidArgument)));
    }

    #[test]
    fn rejects_relocated_images() {
        let image = ehdr(USER_BASE, 64, 56, 0, 3); // ET_DYN unsupported
        assert!(matches!(parse(&image), Err(ErrorCode::NotFound)));
    }
}
