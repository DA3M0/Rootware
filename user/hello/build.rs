fn main() {
    // Shared user linker script: one level above every program crate.
    let linker = concat!(env!("CARGO_MANIFEST_DIR"), "/../linker.ld");
    println!("cargo:rustc-link-arg=-T{linker}");
    println!("cargo:rerun-if-changed={linker}");
}
