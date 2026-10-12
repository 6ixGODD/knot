#include "headers.hpp"
#include BUNDLED_JSON_HEADER
static constexpr int bundled_version = NLOHMANN_JSON_VERSION_MINOR;
#include HOST_JSON_HEADER
static constexpr int host_version = NLOHMANN_JSON_VERSION_MINOR;
#include <cstdio>

int main() {
    const auto result = nlohmann::json::parse(R"({"value":42})");
    std::printf("bundled_minor=%d host_minor=%d expected_bundled=12 expected_host=11\n",
                bundled_version, host_version);
    return bundled_version == 12 && host_version == 11 &&
                   result.at("value").get<int>() == 42
               ? 0 : 1;
}
