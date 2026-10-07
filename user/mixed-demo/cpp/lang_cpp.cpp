// C++ 组件:freestanding(无异常、无 RTTI),经 C ABI 导出。
// 注意:不要使用异常、RTTI、new/delete —— 无 libstdc++ 可链。
extern "C" int lang_cpp_value() {
    return 33;
}
