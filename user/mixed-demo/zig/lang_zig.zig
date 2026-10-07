//! Zig 组件:以 x86_64-freestanding 目标编译,经 C ABI 导出。
export fn lang_zig_value() c_int {
    return 51;
}
