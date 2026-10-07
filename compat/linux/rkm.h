/* Rootware Linux 驱动兼容层(C shim)。
 *
 * Linux 驱动逻辑只需提供 rkm_device_operations(probe/read/write,
 * 语义对齐 file_operations),经 RKM_DEVICE 宏 + rkm_driver_main
 * 驱动核心即成为常驻 Rootware 用户态 RKM 模块。所有系统调用走 ABI
 * v4 冻结调用号;结构体布局与 abi 下的 Rust 定义逐字段一致。
 *
 * 覆盖面:IPC 收发/回复、控制台、模块注册与枚举。
 * 明确不做:中断、DMA、总线枚举、内核内存分配 —— Rootware 的驱动
 * 运行在用户态,这些设施由内核或未来的总线服务提供。
 */
#ifndef RKM_H
#define RKM_H

#include <stddef.h>
#include <stdint.h>

/* 冻结的模块描述符限制(abi/src/rkm.rs)。 */
#define RKM_MAX_NAME 16
#define RKM_MAX_VERSION 8

#define RKM_KIND_NATIVE 1
#define RKM_KIND_LINUX 2

#define RKM_STATE_ACTIVE 1
#define RKM_STATE_STOPPED 2

/* 驱动请求 payload 约定:
 *   payload[0]  操作码(READ/WRITE)
 *   payload[1]  数据长度 L(READ 为请求长度,WRITE 为实际长度,L ≤ 30)
 *   payload[2..2+L]  数据
 * 回复:payload[1] = 0 成功 / 0xFF 失败,READ 的数据同布局。 */
#define RKM_OP_READ 1
#define RKM_OP_WRITE 2
#define RKM_STATUS_OK 0
#define RKM_STATUS_ERROR 0xFF

/* 冻结的 syscall 号(abi/src/syscall.rs)。 */
#define RKM_SYS_IPC_SEND 1
#define RKM_SYS_IPC_RECEIVE 2
#define RKM_SYS_IPC_REPLY 3
#define RKM_SYS_VERSION 4
#define RKM_SYS_CONSOLE_WRITE 5
#define RKM_SYS_EXIT 6
#define RKM_SYS_SPAWN 7
#define RKM_SYS_CAP_REQUEST 8
#define RKM_SYS_MODULE_REGISTER 9
#define RKM_SYS_MODULE_LIST 10

#define RKM_MESSAGE_TYPE_REQUEST 1
#define RKM_MESSAGE_TYPE_RESPONSE 2
/* 驱动请求走 5(DRIVER_REQUEST):路由表没有它的规则,
 * 内核按请求接收方直投到驱动自己的 pid。 */
#define RKM_MESSAGE_TYPE_DRIVER_REQUEST 5

/* 与 abi/src/ipc.rs 的 44 字节冻结布局逐字段一致。 */
struct rkm_message {
    uint16_t sender;
    uint16_t receiver;
    uint16_t message_type;
    uint32_t capability;
    uint8_t payload[32];
};

/* 与 abi/src/rkm.rs 的 32 字节冻结描述符逐字段一致。 */
struct rkm_module {
    char name[RKM_MAX_NAME];
    char version[RKM_MAX_VERSION];
    uint16_t pid;
    uint8_t state;
    uint8_t kind;
    uint8_t _pad[4];
};

_Static_assert(sizeof(struct rkm_message) == 44, "rkm_message must match the frozen 44-byte ABI");
_Static_assert(sizeof(struct rkm_module) == 32, "rkm_module must match the frozen 32-byte ABI");

/* Linux 惯用设备操作表。read/write 的返回值语义对齐 ssize_t:
 * 成功为已处理字节数,失败为负值。 */
struct rkm_device_operations {
    /* 注册成功后调用一次;返回非 0 视为初始化失败,驱动退出。 */
    int (*probe)(void);
    /* 读至多 len 字节到 buf,返回实际读取数(<0 为错误)。 */
    long (*read)(char *buf, unsigned long len);
    /* 消费 buf 中 len 字节,返回已消费数(<0 为错误)。 */
    long (*write)(const char *buf, unsigned long len);
};

/* 驱动核心:注册(名字/版本由构建系统经 RKM_MODULE_NAME/
 * RKM_MODULE_VERSION 注入)→ probe → 常驻 IPC 分发循环。
 * 致命错误时返回非 0 退出码;正常情况不返回。 */
int rkm_driver_main(const struct rkm_device_operations *ops);

/* 生成驱动入口并把设备交给驱动核心(ops 为结构体名,自动取地址)。 */
#define RKM_DEVICE(ops)                          \
    int rkm_driver_start(void)                   \
    {                                            \
        return rkm_driver_main(&(ops));          \
    }

/* 底层 syscall 封装,也可直接使用。返回 rax 原始值(0 成功,
 * 负的 ErrorCode 为失败;MODULE_LIST 返回条目数)。 */
long rkm_syscall(long number, long first, long second);

/* 以 kind 种类注册本驱动;成功回填 *out(含内核分配的 pid)。 */
int rkm_register(const char *name, const char *version, uint8_t kind,
                 struct rkm_module *out);

/* 阻塞接收发往 receiver 的消息;0 成功,负值失败。 */
int rkm_receive(uint16_t receiver, struct rkm_message *out);

/* 以 RESPONSE 类型回复请求;payload 为 32 字节。 */
int rkm_reply(const struct rkm_message *request, const uint8_t payload[32]);

/* 输出到内核串口控制台。 */
void rkm_console_write(const char *buf, unsigned long len);

/* 内核 ABI 版本握手。 */
uint32_t rkm_abi_version(void);

#endif /* RKM_H */
