#if P0_HEADER_FIRST
#include "probe.hpp"
#if defined(INCLUDE_NLOHMANN_JSON_HPP_) || defined(JSON_DIAGNOSTICS) || \
    defined(JSON_USE_IMPLICIT_CONVERSIONS) || defined(nlohmann)
#error Public probe header leaked JSON state
#endif
#endif

#define JSON_DIAGNOSTICS P0_HOST_DIAGNOSTICS
#define JSON_USE_IMPLICIT_CONVERSIONS 1
#include P0_HOST_HEADER
constexpr auto host_size = sizeof(nlohmann::json);
constexpr int host_minor = NLOHMANN_JSON_VERSION_MINOR;

#if !P0_HEADER_FIRST
#include "probe.hpp"
#endif
#include <iostream>
#include <type_traits>

static_assert(NLOHMANN_JSON_VERSION_MINOR == P0_HOST_MINOR);
static_assert(JSON_DIAGNOSTICS == P0_HOST_DIAGNOSTICS);
static_assert(JSON_USE_IMPLICIT_CONVERSIONS == 1);
static_assert(sizeof(nlohmann::json) == host_size);
static_assert(std::is_convertible<nlohmann::json, int>::value);

int main() {
    const auto host = nlohmann::json::parse(R"({"value":7})");
    // Exercise implicit conversion, a difference absent from the 3.12 ABI tag.
    const int host_number = host.at("value");
    const auto result = p0_json_probe::parse(R"({"value":42,"text":"owned"})");
    int rejected = 0;
    for (const auto* invalid : {"[", R"({"value":1})", R"({"value":[],"text":"x"})"}) {
        try {
            p0_json_probe::parse(invalid);
        } catch (const p0_json_probe::Error&) {
            ++rejected;
        } catch (...) {
            return 2; // A private JSON exception must not escape the boundary.
        }
    }
    const bool sizes_ok = P0_HOST_DIAGNOSTICS
        ? host_size > p0_json_probe::json_size()
        : host_size == p0_json_probe::json_size();
    std::cout << "host_minor=" << host_minor << " private_minor=" << p0_json_probe::json_minor()
              << " sizes=" << host_size << '/' << p0_json_probe::json_size()
              << " rejected=" << rejected << '\n';
    return host_number == 7 && result.number == 42 && result.text == "owned" &&
        p0_json_probe::json_minor() == 12 && sizes_ok && rejected == 3 ? 0 : 1;
}
