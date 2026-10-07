#!/usr/bin/env bash
# Rootware 单组件构建器。
#
# 用法: sdk/build-program.sh <组件目录>(如 user/zero-driver)
#
# 清单 program.toml(兼容遗留 rkm.toml;纯 Rust 程序可无清单):
#   [program] name / kind(program|service|driver|lib) / entry(rust|c) / version
#   [c] [cpp] [zig] sources = [...]  includes = [...]
#   [link] libs = [...]              # 引用 lib 组件(产物 lib<name>.a)
#
# 构建语义:
#   entry = rust : cargo 主导链接(bin;kind=lib 时为 staticlib)。
#                  非 Rust 源由程序 build.rs(共享 sdk/build/user-program.rs)
#                  从 c/ cpp/ zig/ 约定目录编译并链接。
#   entry = c    : 本脚本主导 —— shim + 各语言对象 + 可选 Rust staticlib,
#                  cc 终链(user/linker.ld)。C 程序入口用 RKM_PROGRAM(fn),
#                  RKM 驱动用 RKM_DEVICE(ops)(见 compat/linux/rkm.h)。
#   kind  = lib  : 产物为 lib<name>.a;entry=rust → staticlib 改名,
#                  entry=c → 对象 ar 打包。
#
# 编译旗标与 sdk/build/user-program.rs 保持一致,修改时两处同步。
set -e
cd "$(dirname "$0")/.."
export PATH="$HOME/.cargo/bin:$PATH"

RELEASE_DIR="target/x86_64-unknown-none/release"
SHIM_DIR="compat/linux"
BARE_TARGET="x86_64-unknown-none"
C_FLAGS=(-O2 -ffreestanding -fno-pic -fno-pie -fno-stack-protector
    -fno-asynchronous-unwind-tables -mno-red-zone -Wall -Wextra)
CPP_FLAGS=(-std=c++20 -fno-exceptions -fno-rtti)
ZIG_TARGET="x86_64-freestanding-none"

fail() { echo "FAIL: $*" >&2; exit 1; }
need_tool() { command -v "$1" >/dev/null 2>&1 || fail "构建 '$2' 需要 $1 但未安装或不在 PATH"; }

# ---- 清单解析 ----
dir="${1%/}"
[ -d "$dir" ] || fail "组件目录不存在:$dir"
default_name="$(basename "$dir")"
manifest=""
for m in "$dir/program.toml" "$dir/rkm.toml"; do
    [ -f "$m" ] && manifest="$m" && break
done

name="$default_name" kind="program" entry="" version="0.1.0"
manifest_scalar() { # $1=键:容忍行尾注释,取第一个带引号的值
    sed -n "s/^$1 = \"\([^\"]*\)\".*/\1/p" "$manifest" | head -1
}
if [ -n "$manifest" ]; then
    v="$(manifest_scalar name)";     [ -n "$v" ] && name="$v"
    v="$(manifest_scalar kind)";     [ -n "$v" ] && kind="$v"
    v="$(manifest_scalar entry)";    [ -n "$v" ] && entry="$v"
    v="$(manifest_scalar version)";  [ -n "$v" ] && version="$v"
fi
[ -z "$entry" ] && { if [ -f "$dir/Cargo.toml" ]; then entry="rust"; else entry="c"; fi; }
case "$kind" in program|service|driver|lib) ;; *) fail "未知 kind:$kind";; esac
case "$entry" in rust|c) ;; *) fail "未知 entry:$entry";; esac

# ---- 语言源收集:清单段优先,回退约定目录(c/ cpp/ zig/ src/*.*)----
manifest_list() { # $1=段名 $2=键 → 引号内字符串逐行输出(无匹配时安全返回空)
    [ -n "$manifest" ] || return 0
    sed -n "/^\[$1\]/,/^\[/p" "$manifest" | sed -n "s/^$2 = //p" | grep -oE '"[^"]+"' | tr -d '"' || true
}
lang_sources() { # $1=段名 其余=默认 glob 模式
    local section="$1"; shift
    local listed pattern f
    listed="$(manifest_list "$section" sources)"
    if [ -n "$listed" ]; then
        while IFS= read -r pattern; do
            [ -n "$pattern" ] || continue
            for f in "$dir"/$pattern; do [ -f "$f" ] && echo "$f"; done
        done <<EOF
$listed
EOF
        return 0
    fi
    for pattern in "$@"; do
        for f in "$dir"/$pattern; do [ -f "$f" ] && echo "$f"; done
    done
    return 0   # glob 无匹配时最后的 [ -f ] 失败,不能让函数返回非零
}
lang_includes() { # $1=段名 → -I 旗标
    local inc
    while IFS= read -r inc; do
        [ -n "$inc" ] && echo "-I$dir/$inc"
    done < <(manifest_list "$1" includes)
}

work="target/components/$name"
rm -rf "$work"
mkdir -p "$work"

# ---- 各语言对象编译(entry=c 与 kind=lib 需要;entry=rust 由 build.rs 负责)----
# 注意:在数组里先收集源文件再循环编译,保证 set -e 对编译失败立即生效
# (process substitution 子壳中的失败不会中断主流程)。
compile_objects() {
    local -n out_objects=$1   # 调用者数组引用(nameref),编译失败即刻中断
    local file obj
    local c_includes cpp_includes
    c_includes="$(lang_includes c | tr '\n' ' ') -I$SHIM_DIR"
    cpp_includes="$(lang_includes cpp | tr '\n' ' ') -I$SHIM_DIR"

    local c_sources cpp_sources zig_sources
    c_sources="$(lang_sources c "c/*.c" "src/*.c")"
    cpp_sources="$(lang_sources cpp "cpp/*.cpp" "cpp/*.cc" "src/*.cpp")"
    zig_sources="$(lang_sources zig "zig/*.zig" "src/*.zig")"

    while IFS= read -r file; do
        [ -n "$file" ] || continue
        need_tool cc "$name"
        obj="$work/c_$(basename "${file%.*}").o"
        # shellcheck disable=SC2086
        cc "${C_FLAGS[@]}" $c_includes -c "$file" -o "$obj"
        out_objects+=("$obj")
    done <<<"$c_sources"

    while IFS= read -r file; do
        [ -n "$file" ] || continue
        need_tool "c++" "$name"
        obj="$work/cpp_$(basename "${file%.*}").o"
        # shellcheck disable=SC2086
        c++ "${C_FLAGS[@]}" "${CPP_FLAGS[@]}" $cpp_includes -c "$file" -o "$obj"
        out_objects+=("$obj")
    done <<<"$cpp_sources"

    while IFS= read -r file; do
        [ -n "$file" ] || continue
        need_tool zig "$name"
        obj="$work/zig_$(basename "${file%.*}").o"
        # zig 0.16 起缓存目录只经环境变量控制(无 CLI 参数)。
        ZIG_LOCAL_CACHE_DIR="$work/zig-cache" \
        ZIG_GLOBAL_CACHE_DIR="$work/zig-global-cache" \
        zig build-obj -OReleaseSafe -target "$ZIG_TARGET" \
            -femit-bin="$obj" \
            "$file"
        out_objects+=("$obj")
    done <<<"$zig_sources"
}

# 语言组成描述(list 展示用)
describe_langs() {
    local langs=""
    [ -f "$dir/Cargo.toml" ] && langs="rust"
    [ -d "$dir/c" ] || [ -n "$(manifest_list c sources)" ] && langs="${langs:+$langs }c"
    [ -d "$dir/cpp" ] || [ -n "$(manifest_list cpp sources)" ] && langs="${langs:+$langs }cpp"
    [ -d "$dir/zig" ] || [ -n "$(manifest_list zig sources)" ] && langs="${langs:+$langs }zig"
    if [ "$entry" = "c" ] && ls "$dir"/src/*.c >/dev/null 2>&1; then
        [[ "$langs" == *c* ]] || langs="${langs:+$langs }c"
    fi
    echo "${langs:-—}"
}

rust_package_name() {
    sed -n 's/^name = "\(.*\)"$/\1/p' "$dir/Cargo.toml" | head -1
}

build_rust_bin() {
    local pkg
    pkg="$(rust_package_name)"
    cargo build --release --target "$BARE_TARGET" --target-dir target \
        --manifest-path "$dir/Cargo.toml" --quiet
    if [ "$pkg" != "$name" ] && [ -f "$RELEASE_DIR/$pkg" ]; then
        cp -f "$RELEASE_DIR/$pkg" "$RELEASE_DIR/$name"
    fi
}

# 构建 Rust staticlib,产物归一化为 $RELEASE_DIR/lib$name.a 并回传路径。
build_rust_staticlib() {
    [ -f "$dir/Cargo.toml" ] || return 0
    local marker="$work/.marker"
    touch "$marker"
    cargo rustc --release --target "$BARE_TARGET" --target-dir target \
        --manifest-path "$dir/Cargo.toml" --crate-type staticlib --quiet
    local produced
    produced="$(find "$RELEASE_DIR" -maxdepth 1 -name 'lib*.a' -newer "$marker" | head -1)"
    [ -n "$produced" ] || fail "staticlib 构建未产出 .a($dir)"
    cp -f "$produced" "$RELEASE_DIR/lib$name.a"
    echo "$RELEASE_DIR/lib$name.a"
}

echo "==>   $name v$version (kind=$kind entry=$entry langs=$(describe_langs))"
mkdir -p "$RELEASE_DIR"

case "$kind:$entry" in
    lib:rust)
        build_rust_staticlib >/dev/null
        ;;
    lib:c)
        need_tool ar "$name"
        need_tool cc "$name"
        objects=()
        compile_objects objects
        [ "${#objects[@]}" -gt 0 ] || fail "lib 组件 '$name' 没有可编译的源"
        ar rcs "$RELEASE_DIR/lib$name.a" "${objects[@]}"
        ;;
    *:rust)
        # 非 Rust 语言的编译链接由 build.rs 完成(c/ cpp/ zig/ 目录 + [link] libs)
        build_rust_bin
        ;;
    *:c)
        need_tool cc "$name"
        # 驱动与 C 程序共用 shim:_start 与 syscall 封装在 rkm.o。
        cc "${C_FLAGS[@]}" -I"$SHIM_DIR" \
            -DRKM_MODULE_NAME="\"$name\"" -DRKM_MODULE_VERSION="\"$version\"" \
            -c "$SHIM_DIR/rkm.c" -o "$work/rkm.o"
        objects=()
        compile_objects objects
        link_libs=()
        while IFS= read -r lib; do
            [ -n "$lib" ] || continue
            [ -f "$RELEASE_DIR/lib$lib.a" ] || fail "[link] libs 引用的组件 '$lib' 尚未构建:先运行 ./build.sh lib $lib"
            link_libs+=("$RELEASE_DIR/lib$lib.a")
        done < <(manifest_list link libs)
        cc "$work/rkm.o" "${objects[@]}" "${link_libs[@]}" \
            -nostdlib -static -no-pie -Wl,--build-id=none \
            -T user/linker.ld -o "$RELEASE_DIR/$name"
        ;;
esac
