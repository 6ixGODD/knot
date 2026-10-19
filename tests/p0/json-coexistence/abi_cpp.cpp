#include "knot.h"
#include <type_traits>
static_assert(std::is_standard_layout_v<knot_result_t>);
static_assert(std::is_same_v<decltype(&knot_abi_version), uint32_t (*)(void)>);
static_assert(std::is_same_v<decltype(&knot_model_create), knot_result_t (*)(const uint8_t *, size_t, knot_model_t **)>);
static_assert(std::is_same_v<decltype(&knot_initial), knot_result_t (*)(const knot_model_t *)>);
static_assert(std::is_same_v<decltype(&knot_apply), knot_result_t (*)(const knot_model_t *, const uint8_t *, size_t, const uint8_t *, size_t)>);
static_assert(std::is_same_v<decltype(&knot_evaluate), knot_result_t (*)(const knot_model_t *, const uint8_t *, size_t)>);
static_assert(std::is_same_v<decltype(&knot_result_free), void (*)(knot_result_t *)>);
static_assert(std::is_same_v<decltype(&knot_model_free), void (*)(knot_model_t *)>);
int main() { return 0; }
