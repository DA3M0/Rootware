// Rootware 用户程序共享构建逻辑(见 sdk/build/user-program.rs):
// 传入 user/linker.ld,并自动编译 c/ cpp/ zig/ 约定目录下的源。
include!("../../sdk/build/user-program.rs");

fn main() {
    rootware_user_build();
}
