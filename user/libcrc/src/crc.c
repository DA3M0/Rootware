/* crc 组件(kind = lib):标准的 IEEE 802.3 CRC-32,无查表、无依赖。
 * 消费方在其 program.toml 的 [link] libs 中加入 "libcrc" 即可调用。 */
#include <stdint.h>
#include <stddef.h>

uint32_t crc32(const uint8_t *data, size_t len) {
    uint32_t crc = 0xFFFFFFFFu;
    for (size_t i = 0; i < len; i++) {
        crc ^= data[i];
        for (int bit = 0; bit < 8; bit++) {
            uint32_t mask = -(crc & 1u);
            crc = (crc >> 1) ^ (0xEDB88320u & mask);
        }
    }
    return ~crc;
}
