use std::env;
use std::process::Command;

fn main() {
    let out_dir = env::var("OUT_DIR").unwrap();
    let boot_obj = format!("{}/boot.o", out_dir);

    Command::new("nasm")
        .args(["-f", "elf64", "src/boot.asm", "-o", &boot_obj])
        .status()
        .expect("nasm failed");

    println!("cargo:rustc-link-arg={}", boot_obj);
    println!("cargo:rustc-link-arg=-Tlinker.ld");
    println!("cargo:rerun-if-changed=src/boot.asm");
    println!("cargo:rerun-if-changed=linker.ld");
}
