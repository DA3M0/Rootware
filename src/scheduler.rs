//! Alpha 7: a small round-robin scheduler model.

use core::sync::atomic::{AtomicUsize, Ordering};

pub type EntryPoint = fn();

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Context {
    pub rsp: u64,
    pub rip: u64,
    pub rflags: u64,
}

#[repr(C)]
pub struct Pcb {
    pub id: usize,
    pub context: Context,
    pub entry: EntryPoint,
    pub runnable: bool,
}

static mut TASKS: [Option<Pcb>; 2] = [None, None];
static NEXT: AtomicUsize = AtomicUsize::new(0);

#[unsafe(no_mangle)]
pub unsafe extern "C" fn context_switch(_old: *mut Context, _new: *const Context) {
    // The first scheduler milestone keeps switching cooperative and explicit.
}

pub fn spawn(entry: EntryPoint) -> Result<usize, ()> {
    unsafe {
        let tasks = &raw mut TASKS;
        for id in 0..2 {
            if (*tasks)[id].is_none() {
                (*tasks)[id] = Some(Pcb {
                    id,
                    context: Context { rsp: 0, rip: entry as usize as u64, rflags: 0x202 },
                    entry,
                    runnable: true,
                });
                return Ok(id);
            }
        }
    }
    Err(())
}

pub fn yield_now() {
    NEXT.fetch_add(1, Ordering::Relaxed);
}

fn task_a() {
    crate::serial_println!("[SCHED] task A running");
}

fn task_b() {
    crate::serial_println!("[SCHED] task B running");
}

pub fn init() {
    task_a();
    task_b();
}
