//! Kernel console input path.
//!
//! The legacy PIC is fully masked, so serial RX has no interrupt of its
//! own. The APIC timer handler instead calls [`poll`] at 100 Hz to drain
//! the UART into a ring buffer; a process blocked in `SYS_CONSOLE_READ`
//! waits on the [`CONSOLE_WAIT`] sentinel and the poll wakes it once
//! data arrives.

use crate::process;

/// Sentinel `waiting_for` value marking a process blocked on console
/// input. Real receiver ids are 1..=MAX_PROCESSES, so this never
/// collides with an IPC wait.
pub const CONSOLE_WAIT: u16 = u16::MAX;

const RING_SIZE: usize = 256;

/// Single-producer (timer interrupt) / single-consumer (syscall) ring.
/// The producer only writes TAIL, the consumer only writes HEAD, so no
/// lock is needed beyond keeping both sides off the same index.
static mut RING: [u8; RING_SIZE] = [0; RING_SIZE];
static mut HEAD: usize = 0;
static mut TAIL: usize = 0;

fn ring_len() -> usize {
    unsafe {
        let head = HEAD;
        let tail = TAIL;
        if tail >= head {
            tail - head
        } else {
            RING_SIZE - head + tail
        }
    }
}

/// Pushes one byte, dropping it (and leaving the rest in the UART FIFO)
/// when the ring is full. Runs in interrupt context with IF off.
fn push(byte: u8) -> bool {
    unsafe {
        let head = HEAD;
        let tail = TAIL;
        let next = (tail + 1) % RING_SIZE;
        if next == head {
            return false;
        }
        RING[tail] = byte;
        TAIL = next;
        true
    }
}

/// Pops one byte, or `None` when the ring is empty. The caller must hold
/// interrupts disabled so a poll cannot interleave with the read.
pub fn pop() -> Option<u8> {
    if ring_len() == 0 {
        return None;
    }
    unsafe {
        let byte = RING[HEAD];
        HEAD = (HEAD + 1) % RING_SIZE;
        Some(byte)
    }
}

/// Timer-tick hook: drains pending UART bytes into the ring and wakes a
/// console waiter. Safe to call with no waiter and no input — both
/// checks are cheap and the gate image exercises this path every tick.
pub fn poll() {
    while let Some(byte) = crate::serial::read_byte() {
        if !push(byte) {
            break;
        }
    }
    if ring_len() > 0 {
        process::wake_receiver(CONSOLE_WAIT);
    }
}

/// True when some process is blocked waiting for console input. The
/// scheduler uses this to hlt-idle instead of powering off.
pub fn waiter_present() -> bool {
    process::waiter_for(CONSOLE_WAIT)
}

pub fn disable_interrupts() {
    unsafe {
        core::arch::asm!("cli", options(nostack));
    }
}

pub fn enable_interrupts() {
    unsafe {
        core::arch::asm!("sti", options(nostack));
    }
}
