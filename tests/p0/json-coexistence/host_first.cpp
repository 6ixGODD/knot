// Dependency inclusion probe, not a KNOT product wrapper.
#include "headers.hpp"
#include HOST_JSON_HEADER
static constexpr int host_version = NLOHMANN_JSON_VERSION_MINOR;
#include BUNDLED_JSON_HEADER
static constexpr int bundled_version = NLOHMANN_JSON_VERSION_MINOR;
#include <cstdio>

int main() {
    const auto result = nlohmann::json::parse(R"({"value":42})");
    std::printf("host_minor=%d bundled_minor=%d expected_host=11 expected_bundled=12\n",
                host_version, bundled_version);
    return host_version == 11 && bundled_version == 12 &&
                   result.at("value").get<int>() == 42
               ? 0 : 1;
}
