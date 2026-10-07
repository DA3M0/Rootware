//! 多语言混合程序示例:一个 Rootware 启动模块,
//! 由 Rust(入口与 IPC)+ C + C++ + Zig(算法函数)+ C 静态库
//! (libcrc,经 [link] libs)共同静态链接而成。

#![no_std]
#![no_main]

use librootware::console;

// 各语言经 C ABI 导出的函数(见 c/ cpp/ zig/ 目录与 libcrc 组件)。
unsafe extern "C" {
    fn lang_c_value() -> i32;
    fn lang_cpp_value() -> i32;
    fn lang_zig_value() -> i32;
    fn crc32(data: *const u8, len: usize) -> u32;
}

#[unsafe(no_mangle)]
extern "C" fn rootware_main() -> i32 {
    const MESSAGE: &[u8] = b"rootware";
    let crc = unsafe { crc32(MESSAGE.as_ptr(), MESSAGE.len()) };
    let _ = console::write_args(format_args!(
        "mixed: c={} cpp={} zig={} crc32(\"rootware\")={:#010x}\n",
        unsafe { lang_c_value() },
        unsafe { lang_cpp_value() },
        unsafe { lang_zig_value() },
        crc,
    ));
    0
}
