/* {{project_name}}:lib 组件骨架。
 * 导出 C ABI 函数;消费方在 program.toml 的 [link] libs 里加入
 * "{{project_name}}" 即可链接调用(参考 user/mixed-demo)。 */
#include <stdint.h>

int32_t component_value(void) {
    return 42;
}
