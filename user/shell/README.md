# shell

Rootware 交互式串口 shell(`kind = "program"`,2.0 随镜像启动)。

- 提示符 `rootware> `;输入经内核 `SYS_CONSOLE_READ`(APIC 时钟
  100Hz 轮询串口 → 内核环形缓冲),SDK `console::read_line` 提供回显、
  退格擦除,方向键等 CSI 终端序列被整段吞掉。
- 命令:`help` / `echo <text>` / `clear` / `abi` / `modules` /
  `run <name>` / `exit`(`exit` 后全部进程阻塞,内核走关机路径,
  QEMU 以 exit 33 退出)。
- ABI 握手走兼容窗口 `[ABI_COMPAT_MIN, ABI_VERSION]`:废弃的 v4
  内核仍可通过,新代码目标 v5。

构建:`./build.sh program shell`;加入镜像后开机自动拉起。
稳定性门禁 run-tests.sh 会通过串口管道自动驱动本 shell 并断言输出。
