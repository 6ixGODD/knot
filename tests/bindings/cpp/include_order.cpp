#define JSON_DIAGNOSTICS HOST_DIAGNOSTICS
#define JSON_USE_IMPLICIT_CONVERSIONS 1
#define JSON_USE_GLOBAL_UDLS 1
#if HEADER_FIRST
#include <knot/knot.hpp>
#include HOST_HEADER
#else
#include HOST_HEADER
#include <knot/knot.hpp>
#endif
#include "test.hpp"
int main() {
    static_assert(NLOHMANN_JSON_VERSION_MINOR == HOST_MINOR);
    static_assert(JSON_DIAGNOSTICS == HOST_DIAGNOSTICS);
    auto host = nlohmann::json::parse("{\"value\":42}");
    int n = host["value"];
    CHECK(n == 42);
    knot_test_reset();
    auto m = knot::Model::compile("{}");
    CHECK(m.initial().state.revision == 5);
    respond("{");
    fails([&] { m.initial(); });
    CHECK(host.dump() == "{\"value\":42}");
}
