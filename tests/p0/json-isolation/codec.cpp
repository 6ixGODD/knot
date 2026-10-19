// This must be the first include: reject injected headers and configuration.
#include "reject_json_pollution.hpp"
#include "probe.hpp"

#define JSON_DIAGNOSTICS 0
#define JSON_DIAGNOSTIC_POSITIONS 0
#define JSON_USE_IMPLICIT_CONVERSIONS 0
#define JSON_USE_GLOBAL_UDLS 0
#define nlohmann knot_private_json_v3_12_0
#include P0_BUNDLED_HEADER
#undef nlohmann

static_assert(!std::is_convertible<knot_private_json_v3_12_0::json, int>::value);

namespace p0_json_probe {
Value parse(const std::string& input) {
    try {
        const auto value = knot_private_json_v3_12_0::json::parse(input);
        return {value.at("value").get<int>(), value.at("text").get<std::string>()};
    } catch (const knot_private_json_v3_12_0::json::exception& error) {
        throw Error(error.what());
    }
}

int json_minor() { return NLOHMANN_JSON_VERSION_MINOR; }
std::size_t json_size() { return sizeof(knot_private_json_v3_12_0::json); }
}
