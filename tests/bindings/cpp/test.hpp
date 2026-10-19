#pragma once
#include <knot/knot.hpp>
#include "mock.h"
#include <fstream>
#include <iostream>
#include <sstream>
#include <cstdlib>
#define CHECK(x)                                                                                   \
    do {                                                                                           \
        if (!(x)) {                                                                                \
            std::cerr << __FILE__ << ":" << __LINE__ << ": " #x "\n";                              \
            std::exit(1);                                                                          \
        }                                                                                          \
    } while (false)
inline std::string load(const std::string &relative) {
    std::ifstream f(std::string(CONTRACTS) + "/" + relative, std::ios::binary);
    CHECK(f.good());
    std::ostringstream b;
    b << f.rdbuf();
    return b.str();
}
inline void respond(const std::string &s, int code = 0) {
    knot_test_response(s.data(), s.size(), code);
}
template <class F> inline void fails(F f) {
    bool caught = false;
    try {
        f();
    } catch (const knot::Error &) {
        caught = true;
    }
    CHECK(caught);
}
