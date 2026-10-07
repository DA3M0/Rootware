/* zero 驱动:RKM Linux 兼容路径示例。
 *
 * 设备逻辑移植自 Linux drivers/char/mem.c 的 /dev/zero:读返回全零,
 * 写入即丢弃。这里不依赖任何 Linux 内核头,只面向 rkm.h 的
 * probe/read/write ops 表 —— 演示一个 Linux 惯用写法的 C 驱动如何
 * 经 compat/linux shim 编译为 Rootware 用户态 RKM 模块。
 */

#include "rkm.h"

static int zero_probe(void) {
    return 0;
}

static long zero_read(char *buf, unsigned long len) {
    for (unsigned long i = 0; i < len; i++) {
        buf[i] = 0;
    }
    return (long)len;
}

static long zero_write(const char *buf, unsigned long len) {
    (void)buf; /* 写入 /dev/zero 的内容直接丢弃。 */
    return (long)len;
}

static const struct rkm_device_operations zero_ops = {
    .probe = zero_probe,
    .read = zero_read,
    .write = zero_write,
};

RKM_DEVICE(zero_ops)
