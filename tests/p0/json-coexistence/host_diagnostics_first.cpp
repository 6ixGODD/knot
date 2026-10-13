// Same version, incompatible configuration: the second include must not be
// silently replaced by the already-loaded host configuration.
#include "headers.hpp"
#define JSON_DIAGNOSTICS 1
#include BUNDLED_JSON_HEADER
using host_json = nlohmann::json;
static constexpr auto host_size = sizeof(host_json);
static constexpr int host_diagnostics = JSON_DIAGNOSTICS;
#undef JSON_DIAGNOSTICS
#define JSON_DIAGNOSTICS 0
#include BUNDLED_JSON_HEADER
using bundled_json = nlohmann::json;
#include <cstdio>

int main() {
    // With diagnostics enabled upstream adds a parent pointer to each value.
    // Changing the macro after a guarded include cannot change the existing type.
    std::printf("host_diagnostics=%d bundled_macro=%d host_size=%zu bundled_size=%zu\n",
                host_diagnostics, JSON_DIAGNOSTICS, host_size, sizeof(bundled_json));
    return host_diagnostics == 1 && JSON_DIAGNOSTICS == 0 &&
                   host_size > sizeof(bundled_json)
               ? 0 : 1;
}
