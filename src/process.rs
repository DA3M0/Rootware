//! Process table, cooperative scheduler, and user program spawning.
//!
//! Processes own an address space, a kernel stack and (for user images) a
//! Ring 3 entry point plus user stack. Scheduling is cooperative and
//! round-robin: a process runs until it blocks on IPC, yields, or exits.
//! Context switches happen through a callee-saved switch on per-process
//! kernel stacks; entering a user image iretqs to Ring 3 from its kernel
//! stack, and every syscall re-enters the kernel through that stack.

use core::arch::global_asm;
use rootware_abi::ErrorCode;

use crate::vmem::{self, AddressSpace, USER_LIMIT};

pub const MAX_PROCESSES: usize = 8;
const KSTACK_SIZE: usize = 16 * 1024;
const USTACK_PAGES: u64 = 4;

pub type EntryPoint = extern "C" fn() -> !;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Context {
    pub rsp: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum State {
    Free,
    Ready,
    Blocked,
}

#[derive(Clone, Copy)]
struct Process {
    id: u16,
    state: State,
    space: AddressSpace,
    context: Context,
    is_user: bool,
    /// Ring 3 entry point (user processes only).
    entry: u64,
    /// Top of the mapped user stack (user processes only).
    stack_top: u64,
    /// Receiver id this process waits on while Blocked.
    waiting_for: u16,
}

impl Process {
    const fn free() -> Self {
        Process {
            id: 0,
            state: State::Free,
            space: AddressSpace {
                pml4: 0,
                pdp: 0,
                user_pd: 0,
            },
            context: Context { rsp: 0 },
            is_user: false,
            entry: 0,
            stack_top: 0,
            waiting_for: 0,
        }
    }
}

global_asm!(
    ".global rootware_context_switch",
    "rootware_context_switch:",
    "push rbx",
    "push rbp",
    "push r12",
    "push r13",
    "push r14",
    "push r15",
    "mov [rdi], rsp",
    "mov rsp, [rsi]",
    "pop r15",
    "pop r14",
    "pop r13",
    "pop r12",
    "pop rbp",
    "pop rbx",
    "ret",
);

unsafe extern "C" {
    fn rootware_context_switch(old: *mut Context, new: *const Context);
}

#[unsafe(link_section = ".bss")]
static mut KSTACKS: [[u8; KSTACK_SIZE]; MAX_PROCESSES] = [[0; KSTACK_SIZE]; MAX_PROCESSES];
static mut PROCESSES: [Process; MAX_PROCESSES] = [Process::free(); MAX_PROCESSES];
static mut CURRENT: isize = -1;
static mut CURSOR: isize = -1;
static mut BOOT_CONTEXT: Context = Context { rsp: 0 };

fn slot_of(pid: u16) -> Option<usize> {
    if pid == 0 || pid as usize > MAX_PROCESSES {
        None
    } else {
        Some(pid as usize - 1)
    }
}

unsafe fn kstack_top(slot: usize) -> u64 {
    unsafe { (&raw const KSTACKS[slot] as u64) + KSTACK_SIZE as u64 }
}

unsafe fn prepare_context(slot: usize, entry: usize) {
    unsafe {
        let stack = &raw mut KSTACKS[slot] as *mut u8;
        let top = stack.add(KSTACK_SIZE) as usize & !0xf;
        let frame = (top - 56) as *mut u64;
        for offset in 0..6 {
            frame.add(offset).write(0);
        }
        frame.add(6).write(entry as u64);
        PROCESSES[slot].context.rsp = frame as u64;
    }
}

pub fn current_id() -> u16 {
    unsafe {
        if CURRENT >= 0 {
            PROCESSES[CURRENT as usize].id
        } else {
            0
        }
    }
}

fn lowest_free_pid() -> Option<u16> {
    unsafe {
        (1..=MAX_PROCESSES as u16)
            .find(|pid| PROCESSES[(*pid - 1) as usize].state == State::Free)
    }
}

/// Register a kernel task (runs in the kernel address space).
pub fn spawn_kernel_task(pid: u16, entry: EntryPoint) -> Result<(), ErrorCode> {
    let slot = slot_of(pid).ok_or(ErrorCode::InvalidArgument)?;
    unsafe {
        if PROCESSES[slot].state != State::Free {
            return Err(ErrorCode::QueueFull);
        }
        PROCESSES[slot] = Process {
            id: pid,
            state: State::Ready,
            space: vmem::kernel_space(),
            context: Context { rsp: 0 },
            is_user: false,
            entry: 0,
            stack_top: 0,
            waiting_for: 0,
        };
        prepare_context(slot, entry as usize);
    }
    Ok(())
}

/// Load a user program from the boot module registry into a fresh address
/// space.
pub fn spawn_user_by_name(name: &str, pid: u16) -> Result<(), ErrorCode> {
    let module = crate::memory::find_module(name).ok_or(ErrorCode::NotFound)?;
    let slot = slot_of(pid).ok_or(ErrorCode::InvalidArgument)?;
    let bytes = crate::memory::module_bytes(&module);

    let space = vmem::create_address_space().ok_or(ErrorCode::QueueFull)?;
    let entry = match crate::elf::load(&space, bytes) {
        Ok(entry) => entry,
        Err(error) => {
            vmem::destroy_address_space(&space);
            return Err(error);
        }
    };

    // User stack at the top of the user region.
    let stack_top = USER_LIMIT;
    let stack_bottom = USER_LIMIT - USTACK_PAGES * vmem::PAGE_SIZE;
    if let Err(error) = map_user_stack(&space, stack_bottom, USTACK_PAGES) {
        vmem::destroy_address_space(&space);
        return Err(error);
    }

    unsafe {
        if PROCESSES[slot].state != State::Free {
            vmem::destroy_address_space(&space);
            return Err(ErrorCode::QueueFull);
        }
        PROCESSES[slot] = Process {
            id: pid,
            state: State::Ready,
            space,
            context: Context { rsp: 0 },
            is_user: true,
            entry,
            stack_top,
            waiting_for: 0,
        };
        prepare_context(slot, user_entry_trampoline as *const () as usize);
    }
    crate::serial_println!(
        "[PROC] spawned '{}' as pid {} (entry {:#x})",
        name,
        pid,
        entry
    );
    Ok(())
}

/// Spawn by name from a syscall: first free pid, any user process may call.
pub fn spawn_by_name(name: &str) -> Result<u16, ErrorCode> {
    let pid = lowest_free_pid().ok_or(ErrorCode::QueueFull)?;
    spawn_user_by_name(name, pid)?;
    Ok(pid)
}

fn map_user_stack(
    space: &AddressSpace,
    bottom: u64,
    pages: u64,
) -> Result<(), ErrorCode> {
    for page in 0..pages {
        let frame = crate::memory::alloc_frame().ok_or(ErrorCode::QueueFull)?;
        vmem::map_user_page(
            space,
            bottom + page * vmem::PAGE_SIZE,
            frame,
            vmem::USER | vmem::WRITABLE,
        )?;
    }
    Ok(())
}

/// Ring 3 entry: runs on the process kernel stack with its address space
/// active, then iretqs into the user image.
extern "C" fn user_entry_trampoline() -> ! {
    unsafe {
        let slot = CURRENT as usize;
        let entry = PROCESSES[slot].entry;
        let stack = PROCESSES[slot].stack_top;
        core::arch::asm!(
            "push {user_data}",
            "push {stack}",
            "pushfq",
            "push {user_code}",
            "push {entry}",
            "iretq",
            user_data = const crate::gdt::USER_DATA,
            user_code = const crate::gdt::USER_CODE,
            entry = in(reg) entry,
            stack = in(reg) stack,
            options(noreturn)
        );
    }
}

/// Mark the current process blocked waiting for IPC from/for `receiver`
/// and switch away. Returns when woken and scheduled again.
pub fn block_current(receiver: u16) {
    unsafe {
        let slot = CURRENT;
        PROCESSES[slot as usize].state = State::Blocked;
        PROCESSES[slot as usize].waiting_for = receiver;
    }
    schedule();
}

/// Wake one process blocked on `receiver`, if any.
pub fn wake_receiver(receiver: u16) {
    unsafe {
        let table = &raw mut PROCESSES;
        for process in (*table).iter_mut() {
            if process.state == State::Blocked && process.waiting_for == receiver {
                process.state = State::Ready;
                return;
            }
        }
    }
}

/// Terminate the current process, freeing its address space.
pub fn exit_current(code: i64) -> ! {
    unsafe {
        let slot = CURRENT as usize;
        let id = PROCESSES[slot].id;
        // A departing driver's RKM entry survives as a Stopped record.
        crate::rkm::on_process_exit(id);
        if PROCESSES[slot].is_user {
            let space = PROCESSES[slot].space;
            vmem::destroy_address_space(&space);
        }
        PROCESSES[slot] = Process::free();
        crate::serial_println!("[PROC] pid {} exited (code {})", id, code);
    }
    schedule();
    unreachable!("a freed process is never scheduled again");
}

/// Kill the current process after a Ring 3 fault.
pub fn kill_current() -> ! {
    exit_current(-9)
}

fn pick_ready() -> Option<usize> {
    unsafe {
        let start = (CURSOR + 1) as usize;
        for offset in 0..MAX_PROCESSES {
            let slot = (start + offset) % MAX_PROCESSES;
            if PROCESSES[slot].state == State::Ready {
                return Some(slot);
            }
        }
        None
    }
}

/// Cooperative round-robin switch. Returns when this process is scheduled
/// again; from the boot context it returns only via `unreachable` paths.
pub fn schedule() {
    let current = unsafe { CURRENT };
    match pick_ready() {
        Some(slot) => {
            #[cfg(target_os = "none")]
            crate::serial_println!(
                "[DBG] switch {:+?} -> slot {} (pid {}, user {})",
                current,
                slot,
                unsafe { (*(&raw const PROCESSES))[slot].id },
                unsafe { (*(&raw const PROCESSES))[slot].is_user }
            );
            unsafe {
                CURSOR = slot as isize;
                CURRENT = slot as isize;
                let next = &raw mut PROCESSES[slot];
                if (*next).is_user {
                    gdt_stack_for(slot);
                    vmem::switch_to(&(*next).space);
                } else {
                    vmem::switch_to_kernel();
                }
                // Interrupts stay on across switches: every kernel context
                // expects them enabled.
                core::arch::asm!("sti", options(nostack, preserves_flags));
                let old_context = if current >= 0 {
                    &raw mut PROCESSES[current as usize].context
                } else {
                    &raw mut BOOT_CONTEXT
                };
                rootware_context_switch(old_context, &raw const (*next).context);
            }
        }
        None => idle(),
    }
}

unsafe fn gdt_stack_for(slot: usize) {
    unsafe {
        let top = kstack_top(slot);
        crate::gdt::set_kernel_stack(top);
    }
}

pub fn yield_now() {
    schedule();
}

fn idle() -> ! {
    unsafe {
        let table = &raw const PROCESSES;
        let runnable = (*table).iter().any(|p| p.state == State::Ready);
        let blocked = (*table).iter().any(|p| p.state == State::Blocked);
        if blocked {
            crate::serial_println!("[BOOT] idle: all remaining processes blocked on IPC");
        } else if !runnable {
            crate::serial_println!("[BOOT] idle: no runnable processes remain");
        }
        // QEMU test harness: exit through the isa-debug-exit port. The
        // status distinguishes a clean boot-time selftest run from a
        // failed one; on real hardware the write is harmless and the
        // kernel keeps halting.
        let status: u8 = if crate::selftest::failed() { 0x20 } else { 0x10 };
        core::arch::asm!(
            "out dx, al",
            in("dx") 0xf4u16,
            in("al") status,
            options(nomem, nostack)
        );
        loop {
            core::arch::asm!("sti; hlt");
        }
    }
}

/// Boot-time startup: register boot programs (or the kernel demo) and
/// start scheduling. Never returns.
///
/// The boot module list IS the program list: pinned system slots spawn
/// first, then every remaining ELF module is launched automatically in
/// boot order — adding a program to the image is all it takes to run
/// it. Non-ELF or failing modules only log an error.
pub fn init_and_run() -> ! {
    let echo = crate::memory::find_module("echo-service").is_some();
    let client = crate::memory::find_module("ipc-client").is_some();

    // Boot pids are pinned by the default permission table: the client is
    // pid 1, the echo service is pid 2. Everything else spawns after them.
    if echo {
        match spawn_user_by_name("echo-service", 2) {
            Ok(()) => {}
            Err(error) => crate::serial_println!("[PROC] echo spawn failed: {:?}", error),
        }
    }
    if client {
        match spawn_user_by_name("ipc-client", 1) {
            Ok(()) => {}
            Err(error) => crate::serial_println!("[PROC] client spawn failed: {:?}", error),
        }
    }

    let count = crate::memory::module_count();
    for index in 0..count {
        let Some(info) = crate::memory::module(index) else {
            continue;
        };
        let Some(name) = info.name_str() else {
            continue;
        };
        if name == "echo-service" || name == "ipc-client" {
            continue;
        }
        match lowest_free_pid() {
            Some(pid) => {
                if let Err(error) = spawn_user_by_name(name, pid) {
                    crate::serial_println!("[PROC] {} spawn failed: {:?}", name, error);
                }
            }
            None => {
                crate::serial_println!("[PROC] process table full; {} not spawned", name);
                break;
            }
        }
    }

    if !echo && !client && count == 0 {
        // No user modules on the boot medium: run the kernel demo pair so
        // the IPC path is exercised on every boot.
        crate::serial_println!("[PROC] no user modules; running kernel demo");
        let _ = spawn_kernel_task(1, demo_producer);
        let _ = spawn_kernel_task(2, demo_consumer);
    }

    crate::serial_println!("[PROC] scheduler starting");
    schedule();
    unreachable!("boot context is never resumed");
}

extern "C" fn demo_producer() -> ! {
    for _ in 0..2 {
        crate::serial_println!("[SCHED] task A running");
        let _ = crate::ipc::send(crate::ipc::test_message());
        yield_now();
    }
    exit_current(0)
}

extern "C" fn demo_consumer() -> ! {
    let mut received = 0;
    while received < 2 {
        if crate::ipc::recv(2).is_ok() {
            crate::serial_println!("[IPC] task B received: hello from A");
            received += 1;
        }
        yield_now();
    }
    exit_current(0)
}
