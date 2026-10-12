#include "headers.hpp"
#define JSON_DIAGNOSTICS 1
#include HOST_JSON_HEADER

int host_parse() {
    const auto value = nlohmann::json::parse(R"({"value":42})");
    return value.at("value").get<int>() + NLOHMANN_JSON_VERSION_MINOR;
}
