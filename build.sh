#!/usr/bin/env bash
# Rootware 构建入口:自动发现 user/ 下的组件(程序/服务/驱动/库),
# 多语言(Rust/C/C++/Zig)静态编译链接为启动模块,并驱动各种构建目标。
#
# 用法: ./build.sh [命令]
#   (默认)            全流程:构建内核 + 全部组件 + ISO + QEMU 运行
#   build              构建内核与全部组件(含 ISO)
#   program <名字>     构建单个组件(按清单 kind 分派)
#   driver|service|lib <名字>
#                      类型化目标:构建并校验清单 kind 匹配
#   new <kind> <名字>  从模板实例化新组件到 user/<名字>(program|service|driver|lib)
#   iso                = build(构建 + 打包,不运行)
#   run                运行现有 rootware.iso
#   test               cargo test --workspace + run-tests.sh 稳定性门禁
#   list               列出发现的组件(kind/entry/语言组成)
#   clean              清理 target/ iso/ rootware.iso
#   --no-run           = iso(兼容旧入口)
set -e
cd "$(dirname "$0")"
export PATH="$HOME/.cargo/bin:$PATH"

RELEASE_DIR="target/x86_64-unknown-none/release"
KERNEL_ELF="$RELEASE_DIR/rootware"
ISO_DIR="iso"
BARE_TARGET="x86_64-unknown-none"

# ---- 组件自动发现 -------------------------------------------------------
# 每个组件 = user/<目录>/(可选 program.toml;兼容 rkm.toml;纯 Rust 可无清单)。
# 输出行:name|kind|entry|dir
discover_components() {
    local dir manifest name v
    for dir in user/*/; do
        [ -d "$dir" ] || continue
        manifest=""
        for m in "$dir/program.toml" "$dir/rkm.toml"; do
            [ -f "$m" ] && manifest="$m" && break
        done
        name="$(basename "$dir")"
        kind="program"; entry=""
        if [ -n "$manifest" ]; then
            v="$(sed -n 's/^name = "\([^\"]*\)".*/\1/p' "$manifest" | head -1)"
            [ -n "$v" ] && name="$v"
            v="$(sed -n 's/^kind = "\([^\"]*\)".*/\1/p' "$manifest" | head -1)"
            [ -n "$v" ] && kind="$v"
            v="$(sed -n 's/^entry = "\([^\"]*\)".*/\1/p' "$manifest" | head -1)"
            [ -n "$v" ] && entry="$v"
        fi
        [ -z "$entry" ] && { if [ -f "$dir/Cargo.toml" ]; then entry="rust"; else entry="c"; fi; }
        echo "$name|$kind|$entry|${dir%/}"
    done
}

find_component() { # $1=名字 → 输出 name|kind|entry|dir
    discover_components | grep -m1 "^$1|"
}

build_kernel() {
    echo "==> Building kernel..."
    cargo build --release --target "$BARE_TARGET" -p rootware
}

build_component() { # $1=期望 kind(program 表示任意) $2=名字
    local row name kind entry dir
    row="$(find_component "$2")" || { echo "FAIL: 未发现组件 '$2'(./build.sh list 查看)" >&2; exit 1; }
    IFS='|' read -r name kind entry dir <<<"$row"
    if [ "$1" != "program" ] && [ "$1" != "$kind" ]; then
        echo "FAIL: 组件 '$2' 的 kind 是 $kind,与目标 $1 不符" >&2
        exit 1
    fi
    ./sdk/build-program.sh "$dir"
}

build_all() {
    build_kernel
    echo "==> Building components..."
    # lib 组件先行,供 [link] libs 引用
    local row name kind entry dir
    while IFS='|' read -r name kind entry dir; do
        [ "$kind" = "lib" ] && ./sdk/build-program.sh "$dir"
    done < <(discover_components)
    while IFS='|' read -r name kind entry dir; do
        [ "$kind" = "lib" ] || ./sdk/build-program.sh "$dir"
    done < <(discover_components)
}

make_iso() {
    echo "==> Creating ISO..."
    rm -rf "$ISO_DIR"
    mkdir -p "$ISO_DIR/boot/grub"
    cp "$KERNEL_ELF" "$ISO_DIR/boot/"
    local row name kind entry dir
    while IFS='|' read -r name kind entry dir; do
        [ "$kind" = "lib" ] && continue
        cp "$RELEASE_DIR/$name" "$ISO_DIR/boot/"
    done < <(discover_components)
    {
        echo 'set timeout=0'
        echo 'set default=0'
        echo ''
        echo 'menuentry "Rootware" {'
        echo '    multiboot2 /boot/rootware'
        while IFS='|' read -r name kind entry dir; do
            [ "$kind" = "lib" ] && continue
            echo "    module2 /boot/$name $name"
        done < <(discover_components)
        echo '    boot'
        echo '}'
    } > "$ISO_DIR/boot/grub/grub.cfg"
    grub2-mkrescue -o rootware.iso "$ISO_DIR" >/dev/null 2>&1
    echo "==> Image ready: rootware.iso"
}

run_qemu() {
    [ -f rootware.iso ] || { echo "FAIL: rootware.iso 不存在,先 ./build.sh build" >&2; exit 1; }
    echo "==> Running in QEMU..."
    qemu-system-x86_64 \
        -cdrom rootware.iso \
        -boot order=d,menu=off \
        -serial stdio \
        -display none \
        -m 128M
}

new_component() { # $1=kind $2=名字
    local kind="$1" name="$2" file
    case "$kind" in program|service|driver|lib) ;; *) echo "FAIL: 未知类型 '$kind'(可选 program|service|driver|lib)" >&2; exit 1;; esac
    [ -d "templates/$kind" ] || { echo "FAIL: 缺少模板 templates/$kind" >&2; exit 1; }
    if [[ ! "$name" =~ ^[a-z0-9][a-z0-9_-]*$ ]]; then
        echo "FAIL: 名字 '$name' 只能含小写字母/数字/-/_" >&2
        exit 1
    fi
    [ -e "user/$name" ] && { echo "FAIL: user/$name 已存在" >&2; exit 1; }
    cp -r "templates/$kind" "user/$name"
    while IFS= read -r -d '' file; do
        sed -i "s/{{project_name}}/$name/g" "$file"
    done < <(find "user/$name" -type f -print0)
    echo "==> 已创建 user/$name (kind=$kind)"
    echo "    构建: ./build.sh $kind $name   列表: ./build.sh list"
}

list_components() {
    local row name kind entry dir langs
    printf '%-14s %-9s %-6s %-14s %s\n' NAME KIND ENTRY LANGS DIR
    while IFS='|' read -r name kind entry dir; do
        langs=""
        [ -f "$dir/Cargo.toml" ] && langs="rust"
        [ -d "$dir/c" ] && langs="$langs c"
        [ -d "$dir/cpp" ] && langs="$langs cpp"
        [ -d "$dir/zig" ] && langs="$langs zig"
        if [ "$entry" = "c" ] && ls "$dir"/src/*.c >/dev/null 2>&1 && [[ "$langs" != *c* ]]; then
            langs="$langs c"
        fi
        printf '%-14s %-9s %-6s %-14s %s\n' "$name" "$kind" "$entry" "${langs# }" "$dir"
    done < <(discover_components)
}

# ---- 命令分派 ----
cmd="${1:-all}"
case "$cmd" in
    all)
        build_all
        make_iso
        run_qemu
        ;;
    build|iso|--no-run)
        build_all
        make_iso
        ;;
    program)
        [ -n "${2:-}" ] || { echo "用法: ./build.sh program <名字>" >&2; exit 1; }
        build_component program "$2"
        ;;
    driver|service|lib)
        [ -n "${2:-}" ] || { echo "用法: ./build.sh $cmd <名字>" >&2; exit 1; }
        build_component "$cmd" "$2"
        ;;
    new)
        [ -n "${3:-}" ] || { echo "用法: ./build.sh new <program|service|driver|lib> <名字>" >&2; exit 1; }
        new_component "$2" "$3"
        ;;
    run)
        run_qemu
        ;;
    test)
        cargo test --workspace --quiet
        ./run-tests.sh
        ;;
    list)
        list_components
        ;;
    clean)
        rm -rf target iso rootware.iso
        echo "==> cleaned"
        ;;
    *)
        echo "未知命令:$cmd(见文件头用法注释)" >&2
        exit 1
        ;;
esac
