//! Cooperative round-robin scheduler with a real callee-saved context switch.

use core::arch::global_asm;
use core::sync::atomic::{AtomicUsize, Ordering};

pub type EntryPoint = extern "C" fn() -> !;
const STACK_SIZE: usize = 16 * 1024;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Context {
    pub rsp: u64,
}

#[repr(C)]
pub struct Pcb {
    pub id: usize,
    pub context: Context,
    pub entry: EntryPoint,
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

#[unsafe(link_section = ".data")]
static mut STACKS: [[u8; STACK_SIZE]; 2] = [[0; STACK_SIZE]; 2];
static mut TASKS: [Pcb; 2] = [
    Pcb { id: 0, context: Context { rsp: 0 }, entry: task_a },
    Pcb { id: 1, context: Context { rsp: 0 }, entry: task_b },
];
static mut BOOT_CONTEXT: Context = Context { rsp: 0 };
static CURRENT: AtomicUsize = AtomicUsize::new(0);
static ROUNDS: AtomicUsize = AtomicUsize::new(0);

fn prepare_context(id: usize) {
    unsafe {
        let stack = &raw mut STACKS[id] as *mut u8;
        let top = stack.add(STACK_SIZE) as usize & !0xf;
        let frame = (top - 56) as *mut u64;
        for slot in 0..6 {
            frame.add(slot).write(0);
        }
        frame.add(6).write(TASKS[id].entry as usize as u64);
        TASKS[id].context.rsp = frame as u64;
    }
}

pub fn spawn(entry: EntryPoint) -> Result<usize, ()> {
    unsafe {
        for id in 0..2 {
            if TASKS[id].entry == task_a && id == 1 {
                TASKS[id].entry = entry;
                prepare_context(id);
                return Ok(id);
            }
        }
    }
    Err(())
}

pub fn yield_now() {
    let old = CURRENT.load(Ordering::Relaxed);
    let next = (old + 1) % 2;
    CURRENT.store(next, Ordering::Relaxed);
    unsafe {
        rootware_context_switch(
            &raw mut TASKS[old].context,
            &raw const TASKS[next].context,
        );
    }
}

extern "C" fn task_a() -> ! {
    loop {
        crate::serial_println!("[SCHED] task A running");
        let _ = crate::ipc::send(crate::ipc::test_message());
        yield_now();
    }
}

extern "C" fn task_b() -> ! {
    loop {
        crate::serial_println!("[SCHED] task B running");
        if crate::ipc::recv(2).is_some() {
            crate::serial_println!("[IPC] task B received: hello from A");
            if ROUNDS.fetch_add(1, Ordering::Relaxed) >= 2 {
                crate::service::init();
            }
        }
        yield_now();
    }
}

pub fn init() -> ! {
    prepare_context(0);
    prepare_context(1);
    crate::serial_println!("[SCHED] starting cooperative round-robin");
    unsafe {
        rootware_context_switch(
            &raw mut BOOT_CONTEXT,
            &raw const TASKS[0].context,
        );
    }
    loop {
        core::hint::spin_loop();
    }
}
