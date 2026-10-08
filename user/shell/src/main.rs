//! shell:Rootware 交互式命令行。
//!
//! 随镜像启动,通过内核 `SYS_CONSOLE_READ`(时钟中断轮询串口)
//! 逐字节读取输入,自带回显与行编辑(退格;方向键等终端序列被
//! 吞掉)。命令分发见 `dispatch`。

#![no_std]
#![no_main]

use librootware::console;
use librootware::process;
use librootware::rkm;
use librootware::rkm::{module_kind, module_state};
use librootware::sys;
use librootware::RkmModule;

const MAX_LINE: usize = 128;
const MAX_MODULES: usize = 8;

#[unsafe(no_mangle)]
extern "C" fn rootware_main() -> i32 {
    let _ = console::write_str("\n");
    let _ = console::write_str("Rootware shell v0.1.0\n");
    // Handshake over the compatibility window; deprecated v4 kernels
    // still pass, v5 is the supported target.
    match sys::kernel_abi_version() {
        Ok(version)
            if version >= librootware::ABI_COMPAT_MIN
                && version <= librootware::ABI_VERSION => {}
        Ok(version) => {
            let _ = console::write_args(format_args!(
                "shell: kernel ABI v{} outside compat window, continuing anyway\n",
                version
            ));
        }
        Err(_) => {
            let _ = console::write_str("shell: ABI handshake syscall failed, continuing anyway\n");
        }
    }
    let _ = console::write_str("[SHELL] ready - type 'help' for commands\n");

    let mut line = [0u8; MAX_LINE];
    loop {
        let _ = console::write_str("rootware> ");
        match console::read_line(&mut line) {
            Ok(len) => {
                if !dispatch(&line[..len]) {
                    return 0;
                }
            }
            Err(_) => {
                let _ = console::write_str("shell: console read failed\n");
                return 1;
            }
        }
    }
}

/// Runs one command line. Returns `false` only for `exit`.
fn dispatch(line: &[u8]) -> bool {
    let (cmd, args) = split(line);
    if cmd == b"help" {
        help();
    } else if cmd == b"echo" {
        echo(trim(args));
    } else if cmd == b"clear" {
        let _ = console::write_str("\x1b[2J\x1b[H");
    } else if cmd == b"abi" {
        abi_cmd();
    } else if cmd == b"modules" {
        modules_cmd();
    } else if cmd == b"run" {
        run_cmd(trim(args));
    } else if cmd == b"exit" {
        let _ = console::write_str("bye\n");
        return false;
    } else if cmd.is_empty() {
        // Empty line: just show a fresh prompt.
    } else {
        let _ = console::write_args(format_args!(
            "unknown command: {} (try 'help')\n",
            as_str(cmd)
        ));
    }
    true
}

fn help() {
    let _ = console::write_str(
        "commands: help echo clear abi modules run exit\n\
         help             - this text\n\
         echo <text>      - print text back\n\
         clear            - clear the screen\n\
         abi              - kernel ABI version handshake\n\
         modules          - list registered RKM driver modules\n\
         run <name>       - spawn a boot module program by name\n\
         exit             - leave the shell and power off\n",
    );
}

fn echo(args: &[u8]) {
    if args.is_empty() {
        return;
    }
    let _ = console::write_str(as_str(args));
    let _ = console::write_str("\n");
}

fn abi_cmd() {
    match sys::kernel_abi_version() {
        Ok(version)
            if version >= librootware::ABI_COMPAT_MIN
                && version <= librootware::ABI_VERSION =>
        {
            let _ = console::write_args(format_args!("kernel ABI v{} handshake ok\n", version));
        }
        Ok(version) => {
            let _ = console::write_args(format_args!(
                "kernel ABI v{} outside compat window [v{}, v{}]\n",
                version,
                librootware::ABI_COMPAT_MIN,
                librootware::ABI_VERSION
            ));
        }
        Err(_) => {
            let _ = console::write_str("abi: version syscall failed\n");
        }
    }
}

fn modules_cmd() {
    let mut table = [zero_module(); MAX_MODULES];
    match rkm::list(&mut table) {
        Ok(0) => {
            let _ = console::write_str("no RKM modules registered\n");
        }
        Ok(count) => {
            for module in &table[..count] {
                let _ = console::write_args(format_args!(
                    "pid {:<3} {:<12} v{:<8} {:<7} {}\n",
                    module.pid,
                    module.name_str().unwrap_or("?"),
                    module.version_str().unwrap_or("?"),
                    state_name(module.state),
                    kind_name(module.kind),
                ));
            }
        }
        Err(error) => {
            let _ = console::write_args(format_args!("modules: list failed: {:?}\n", error));
        }
    }
}

fn run_cmd(name: &[u8]) {
    if name.is_empty() {
        let _ = console::write_str("usage: run <name>\n");
        return;
    }
    match core::str::from_utf8(name) {
        Ok(name) => match process::spawn(name) {
            Ok(pid) => {
                let _ = console::write_args(format_args!("spawned {} as pid {}\n", name, pid));
            }
            Err(error) => {
                let _ = console::write_args(format_args!("run: {} failed: {:?}\n", name, error));
            }
        },
        Err(_) => {
            let _ = console::write_str("run: name must be ASCII\n");
        }
    }
}

fn split(line: &[u8]) -> (&[u8], &[u8]) {
    match line.iter().position(|byte| *byte == b' ') {
        Some(index) => (&line[..index], &line[index..]),
        None => (line, &[]),
    }
}

fn trim(args: &[u8]) -> &[u8] {
    let mut start = 0;
    while start < args.len() && args[start] == b' ' {
        start += 1;
    }
    &args[start..]
}

/// read_line only accepts printable ASCII, so this is always valid
/// UTF-8; the fallback just avoids relying on that invariant.
fn as_str(bytes: &[u8]) -> &str {
    core::str::from_utf8(bytes).unwrap_or("?")
}

fn state_name(state: u8) -> &'static str {
    match state {
        module_state::ACTIVE => "active",
        module_state::STOPPED => "stopped",
        _ => "unknown",
    }
}

fn kind_name(kind: u8) -> &'static str {
    match kind {
        module_kind::NATIVE => "native",
        module_kind::LINUX => "linux",
        _ => "unknown",
    }
}

const fn zero_module() -> RkmModule {
    RkmModule {
        name: [0; 16],
        version: [0; 8],
        pid: 0,
        state: 0,
        kind: 0,
        _pad: [0; 4],
    }
}
