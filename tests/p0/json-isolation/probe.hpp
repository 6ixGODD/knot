#pragma once

#include <cstddef>
#include <stdexcept>
#include <string>

// Test-only boundary. This is not the production KNOT API or SDK.
namespace p0_json_probe {
struct Value {
    int number;
    std::string text;
};

class Error : public std::runtime_error {
public:
    using std::runtime_error::runtime_error;
};

Value parse(const std::string& input);
int json_minor();
std::size_t json_size();
}
