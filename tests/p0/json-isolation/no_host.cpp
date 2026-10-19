#include "probe.hpp"
#if defined(INCLUDE_NLOHMANN_JSON_HPP_) || defined(JSON_DIAGNOSTICS) || \
    defined(JSON_USE_IMPLICIT_CONVERSIONS) || defined(nlohmann)
#error Public probe header requires or leaks JSON
#endif

int main() {
    const auto value = p0_json_probe::parse(R"({"value":42,"text":"standalone"})");
    return value.number == 42 && value.text == "standalone" ? 0 : 1;
}
