// Rootware 用户程序共享构建逻辑(build.rs 主体)。
//
// 程序 crate 的 build.rs 只需两行:
//
//   include!("../../sdk/build/user-program.rs");   // 路径按目录深度调整
//   fn main() { rootware_user_build(); }
//
// 职责(仅 target = x86_64-unknown-none;host 构建直接跳过):
//   1. 传入共享的 user/linker.ld(用户区基址 0x40000000);
//   2. 约定目录 c/、cpp/、zig/ 下的源文件逐个编译成 freestanding 对象
//      并链接进最终 ELF —— C ABI 是跨语言边界,Rust 通过
//      `extern "C"` 调用它们;
//   3. program.toml `[link] libs` 引用的 lib 组件(.a)一并链接。
//
// 旗标与 sdk/build-program.sh 保持一致;修改时两处同步。
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

const C_FLAGS: &[&str] = &[
    "-O2",
    "-ffreestanding",
    "-fno-pic",
    "-fno-pie",
    "-fno-stack-protector",
    "-fno-asynchronous-unwind-tables",
    "-mno-red-zone",
    "-Wall",
    "-Wextra",
];
const CPP_FLAGS: &[&str] = &["-std=c++20", "-fno-exceptions", "-fno-rtti"];
const ZIG_TARGET: &str = "x86_64-freestanding-none";
const BARE_METAL_TARGET: &str = "x86_64-unknown-none";

fn fail(message: &str) -> ! {
    eprintln!("build.rs: {message}");
    std::process::exit(1);
}

fn run_tool(tool: &str, args: &[String]) {
    let status = Command::new(tool)
        .args(args)
        .status()
        .unwrap_or_else(|_| fail(&format!("需要 {tool} 但未安装或不在 PATH")));
    if !status.success() {
        fail(&format!("{tool} 编译失败:{args:?}"));
    }
}

fn collect_sources(dir: &Path, sources: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_sources(&path, sources);
        } else {
            sources.push(path);
        }
    }
}

fn language_of(extension: &str) -> Option<&'static str> {
    match extension {
        "c" | "s" | "S" => Some("c"),
        "cpp" | "cc" | "cxx" => Some("cpp"),
        "zig" => Some("zig"),
        _ => None,
    }
}

fn compile_source(source: &Path, out_dir: &Path, flavor: &str) -> PathBuf {
    let extension = source
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default();
    let flavor = if flavor == "auto" {
        language_of(extension)
            .unwrap_or_else(|| fail(&format!("无法识别的源文件类型:{}", source.display())))
    } else {
        flavor
    };
    let stem = source
        .with_extension("")
        .to_string_lossy()
        .replace(['/', '.', '-'], "_");
    let object = out_dir.join(format!("{stem}.o"));

    match flavor {
        "c" => {
            let mut args: Vec<String> = C_FLAGS.iter().map(|f| f.to_string()).collect();
            args.push("-c".into());
            args.push(source.to_string_lossy().into_owned());
            args.push("-o".into());
            args.push(object.to_string_lossy().into_owned());
            run_tool("cc", &args);
        }
        "cpp" => {
            let mut args: Vec<String> = C_FLAGS
                .iter()
                .chain(CPP_FLAGS.iter())
                .map(|f| f.to_string())
                .collect();
            args.push("-c".into());
            args.push(source.to_string_lossy().into_owned());
            args.push("-o".into());
            args.push(object.to_string_lossy().into_owned());
            run_tool("c++", &args);
        }
        "zig" => {
            // zig 0.16 起缓存目录只经环境变量控制(无 CLI 参数)。
            let status = Command::new("zig")
                .args([
                    "build-obj",
                    "-OReleaseSafe",
                    "-target",
                    ZIG_TARGET,
                    &format!("-femit-bin={}", object.display()),
                    source.to_str().unwrap_or_default(),
                ])
                .env("ZIG_LOCAL_CACHE_DIR", out_dir.join("zig-cache"))
                .env("ZIG_GLOBAL_CACHE_DIR", out_dir.join("zig-global-cache"))
                .status()
                .unwrap_or_else(|_| fail("需要 zig 但未安装或不在 PATH"));
            if !status.success() {
                fail(&format!("zig 编译失败:{}", source.display()));
            }
        }
        _ => unreachable!(),
    }
    object
}

/// 从 program.toml(或遗留 rkm.toml)读 `[link] libs` 组件名列表。
fn manifest_libs(manifest_dir: &Path) -> Vec<String> {
    let mut libs = Vec::new();
    for candidate in ["program.toml", "rkm.toml"] {
        let path = manifest_dir.join(candidate);
        if !path.is_file() {
            continue;
        }
        println!("cargo:rerun-if-changed={}", path.display());
        let text = fs::read_to_string(&path).unwrap_or_default();
        let mut in_link = false;
        for line in text.lines() {
            let line = line.trim();
            if line.starts_with('[') {
                in_link = line == "[link]";
                continue;
            }
            if !in_link {
                continue;
            }
            if let Some(rest) = line.strip_prefix("libs") {
                let list = rest.trim_start().strip_prefix('=').unwrap_or("");
                // 取引号之间的内容:"a", "b" → a b
                let quoted: Vec<&str> = list.split('"').collect();
                for part in quoted.iter().skip(1).step_by(2) {
                    if !part.trim().is_empty() {
                        libs.push(part.trim().to_string());
                    }
                }
            }
        }
        break; // 只读第一个存在的清单
    }
    libs
}

fn link_manifest_libs(manifest_dir: &Path, out_dir: &Path) {
    let libs = manifest_libs(manifest_dir);
    if libs.is_empty() {
        return;
    }
    // 目标目录探测:OUT_DIR 形如 <target>/<triple>/release/build/<pkg>/out,
    // 向上找到 triple 层再取其父;另有 CARGO_TARGET_DIR 与仓库布局两个候选。
    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Ok(env_dir) = env::var("CARGO_TARGET_DIR") {
        candidates.push(PathBuf::from(env_dir));
    }
    for ancestor in out_dir.ancestors() {
        if ancestor.ends_with(BARE_METAL_TARGET) {
            if let Some(target_dir) = ancestor.parent() {
                candidates.push(target_dir.to_path_buf());
            }
            break;
        }
    }
    candidates.push(
        manifest_dir
            .join("../../target")
            .components()
            .collect::<PathBuf>(),
    );

    for name in libs {
        let artifact = candidates
            .iter()
            .map(|dir| dir.join(BARE_METAL_TARGET).join("release").join(format!("lib{name}.a")))
            .find(|path| path.is_file());
        let Some(artifact) = artifact else {
            fail(&format!(
                "[link] libs 引用的组件 '{name}' 尚未构建:先运行 ./build.sh lib {name}"
            ));
        };
        println!("cargo:rustc-link-arg={}", artifact.display());
        println!("cargo:rerun-if-changed={}", artifact.display());
    }
}

/// 程序 build.rs 的唯一入口。
pub fn rootware_user_build() {
    println!("cargo:rerun-if-changed=build.rs");
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let linker = manifest_dir.parent().unwrap().join("linker.ld");
    println!("cargo:rerun-if-changed={}", linker.display());

    let target = env::var("TARGET").unwrap_or_default();
    if target != BARE_METAL_TARGET {
        // host 构建(host-sim 模拟模式):不需要 freestanding 对象与链接脚本。
        return;
    }
    println!("cargo:rustc-link-arg=-T{}", linker.display());

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    let mut objects = Vec::new();
    for label in ["c", "cpp", "zig"] {
        let dir = manifest_dir.join(label);
        if !dir.is_dir() {
            continue;
        }
        println!("cargo:rerun-if-changed={}", dir.display());
        let mut sources = Vec::new();
        collect_sources(&dir, &mut sources);
        sources.sort();
        for source in sources {
            let extension = source
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or_default();
            if language_of(extension).is_none() {
                continue;
            }
            println!("cargo:rerun-if-changed={}", source.display());
            objects.push(compile_source(&source, &out_dir, "auto"));
        }
    }
    for object in &objects {
        println!("cargo:rustc-link-arg={}", object.display());
    }

    link_manifest_libs(&manifest_dir, &out_dir);
}
