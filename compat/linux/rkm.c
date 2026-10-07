/* Rootware Linux 驱动兼容层实现。见 rkm.h。 */

#include "rkm.h"

/* 名字与版本来自 rkm.toml,由 build.sh 以 -D 注入,清单是唯一来源。 */
#ifndef RKM_MODULE_NAME
#define RKM_MODULE_NAME "unnamed"
#endif
#ifndef RKM_MODULE_VERSION
#define RKM_MODULE_VERSION "0.0.0"
#endif

long rkm_syscall(long number, long first, long second) {
    long ret;
    __asm__ volatile("syscall"
                     : "=a"(ret)
                     : "a"(number), "D"(first), "S"(second)
                     : "rcx", "r11", "memory");
    return ret;
}

/* 无 libc:手动清零与限长拷贝。 */
static void mem_zero(void *dst, unsigned long len) {
    uint8_t *out = dst;
    for (unsigned long i = 0; i < len; i++) out[i] = 0;
}

static unsigned long str_copy(char *dst, const char *src, unsigned long cap) {
    unsigned long used = 0;
    while (src[used] != 0 && used < cap) {
        dst[used] = src[used];
        used++;
    }
    return used;
}

int rkm_register(const char *name, const char *version, uint8_t kind,
                 struct rkm_module *out) {
    struct rkm_module descriptor;
    mem_zero(&descriptor, sizeof(descriptor));
    str_copy(descriptor.name, name, RKM_MAX_NAME);
    str_copy(descriptor.version, version, RKM_MAX_VERSION);
    descriptor.kind = kind;
    if (rkm_syscall(RKM_SYS_MODULE_REGISTER, (long)&descriptor, 0) != 0) {
        return -1;
    }
    if (out != 0) {
        *out = descriptor;
    }
    return 0;
}

int rkm_receive(uint16_t receiver, struct rkm_message *out) {
    return rkm_syscall(RKM_SYS_IPC_RECEIVE, receiver, (long)out) == 0 ? 0 : -1;
}

int rkm_reply(const struct rkm_message *request, const uint8_t payload[32]) {
    return rkm_syscall(RKM_SYS_IPC_REPLY, (long)request, (long)payload) == 0 ? 0 : -1;
}

void rkm_console_write(const char *buf, unsigned long len) {
    (void)rkm_syscall(RKM_SYS_CONSOLE_WRITE, (long)buf, (long)len);
}

uint32_t rkm_abi_version(void) {
    return (uint32_t)rkm_syscall(RKM_SYS_VERSION, 0, 0);
}

/* 内核经 iretq 进入 _start 时 rsp % 16 == 8,与 SysV 调用约定一致;
 * 显式对齐后再调用驱动核心。核心返回即视为驱动退出(6 = SYS_EXIT)。 */
__asm__(".section .text._start\n"
        ".global _start\n"
        "_start:\n"
        "  xor %ebp, %ebp\n"
        "  and $-16, %rsp\n"
        "  call rkm_driver_start\n"
        "  mov %eax, %edi\n"
        "  mov $6, %eax\n"
        "  xor %esi, %esi\n"
        "  syscall\n"
        "1: jmp 1b\n");

int rkm_driver_main(const struct rkm_device_operations *ops) {
    struct rkm_module self;
    if (rkm_register(RKM_MODULE_NAME, RKM_MODULE_VERSION, RKM_KIND_LINUX, &self) != 0) {
        return 1;
    }
    if (ops->probe != 0 && ops->probe() != 0) {
        return 2;
    }
    for (;;) {
        struct rkm_message request;
        if (rkm_receive(self.pid, &request) != 0) {
            return 3;
        }
        if (request.message_type != RKM_MESSAGE_TYPE_REQUEST &&
            request.message_type != RKM_MESSAGE_TYPE_DRIVER_REQUEST) {
            continue;
        }
        uint8_t reply[32];
        mem_zero(reply, sizeof(reply));
        reply[0] = request.payload[0];
        unsigned long len = request.payload[1];
        if (len > 30) {
            len = 30;
        }
        switch (request.payload[0]) {
        case RKM_OP_READ: {
            long got = ops->read != 0 ? ops->read((char *)&reply[2], len) : -1;
            reply[1] = got < 0 ? RKM_STATUS_ERROR : RKM_STATUS_OK;
            break;
        }
        case RKM_OP_WRITE: {
            long done = ops->write != 0
                            ? ops->write((const char *)&request.payload[2], len)
                            : -1;
            reply[1] = done < 0 ? RKM_STATUS_ERROR : RKM_STATUS_OK;
            break;
        }
        default:
            reply[1] = RKM_STATUS_ERROR;
            break;
        }
        if (rkm_reply(&request, reply) != 0) {
            return 4;
        }
    }
}
