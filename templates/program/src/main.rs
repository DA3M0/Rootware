//! {{project_name}}:Rootware 用户程序骨架。
//!
//! 构建: ./build.sh program {{project_name}}
//! 多语言: 把 C/C++/Zig 源放进 c/ cpp/ zig/ 目录即自动编译链接
//! (经 C ABI 调用),参考 user/mixed-demo。

#![no_std]
#![no_main]

use librootware::console;

#[unsafe(no_mangle)]
extern "C" fn rootware_main() -> i32 {
    let _ = console::write_str("hello from {{project_name}}\n");
    0
}
