#pragma once
#include "probe.h"
#include <string>
// Test-only precompiled C++ bridge. No third-party types or headers cross this boundary.
bool p0_cpp_evaluate(const p0_probe* model,const std::string& input);
