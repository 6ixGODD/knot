#include "headers.hpp"
#define JSON_DIAGNOSTICS 0
#include BUNDLED_JSON_HEADER

int bundled_parse() {
    const auto value = nlohmann::json::parse(R"({"value":42})");
    return value.at("value").get<int>() + NLOHMANN_JSON_VERSION_MINOR;
}
