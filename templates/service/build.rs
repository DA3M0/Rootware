// Rootware 用户程序共享构建逻辑(见 sdk/build/user-program.rs)。
include!("../../sdk/build/user-program.rs");

fn main() {
    rootware_user_build();
}
