use std::env;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=src/boot.asm");
    println!("cargo:rerun-if-changed=linker.ld");

    // Host builds (used by `cargo test`) only compile the kernel logic;
    // the boot assembly and bare-metal linker script belong to the
    // embedded target alone.
    if env::var("TARGET").as_deref() != Ok("x86_64-unknown-none") {
        return;
    }

    let out_dir = env::var("OUT_DIR").unwrap();
    let boot_obj = format!("{}/boot.o", out_dir);

    let status = Command::new("nasm")
        .args(["-f", "elf64", "src/boot.asm", "-o", &boot_obj])
        .status()
        .expect("nasm failed");
    assert!(status.success(), "nasm returned {status}");

    println!("cargo:rustc-link-arg={}", boot_obj);
    println!("cargo:rustc-link-arg=-Tlinker.ld");
}
