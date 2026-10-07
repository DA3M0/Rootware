// Rootware 用户程序共享构建逻辑(见 sdk/build/user-program.rs):
// 自动编译 c/ cpp/ zig/ 约定目录下的源并链接进最终 ELF。
include!("../../sdk/build/user-program.rs");

fn main() {
    rootware_user_build();
}
