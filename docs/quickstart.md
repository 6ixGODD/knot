# Run the Speaker example

Use the internally supplied Windows x64 or Linux x64 SDK. The example prints `282800` followed by `159900` with either shared or static linkage.

1. Obtain the internal SDK for your platform and matching C++ ABI/configuration. Extract it as `C:/sdk/knot` (or a corresponding absolute path on Linux). The package contains `include`, `lib`, `bin`, `contracts`, `examples`, `docs` and `licenses`.
2. Use the complete `examples/CMakeLists.txt` below with `examples/c/smoke.c` and `examples/cpp/speaker.cpp` from the same package. CMake 3.23 syntax is used; Windows package consumption was tested with CMake 3.29.5 and MSVC 19.41. Choose Release and the package's dynamic CRT on MSVC.
3. Build and run using a trusted DLL directory. Consumers need a C/C++ compiler and CMake on PATH, not Rust, a rule-engine installation or a network download during CMake. Check `cmake --version` first. If Visual Studio's bundled CMake is not on PATH, use its installed `cmake.exe` absolute path in place of `cmake` below.

```powershell
cmake -S C:/sdk/knot/examples -B build -DCMAKE_PREFIX_PATH=C:/sdk/knot -DKNOT_LINKAGE=shared
cmake --build build --config Release
$env:PATH = "C:/sdk/knot/bin;" + $env:PATH
./build/Release/speaker.exe C:/sdk/knot/contracts/examples/speaker.json
```

On a single-configuration Linux generator, also pass `-DCMAKE_BUILD_TYPE=Release` and run `./build/speaker`. Set `LD_LIBRARY_PATH` to the package's absolute `lib` directory, or deploy with an application-specific runpath. Do not search the current directory for libraries.

The shared C++ target is `Knot::CppShared`. [Static integration](integration.md) uses `Knot::CppStatic`.

<!-- source: examples/CMakeLists.txt -->
```cmake
cmake_minimum_required(VERSION 3.23)
project(knot_examples LANGUAGES C CXX)
find_package(Knot CONFIG REQUIRED)
set(KNOT_LINKAGE shared CACHE STRING "shared or static")
set_property(CACHE KNOT_LINKAGE PROPERTY STRINGS shared static)
if(KNOT_LINKAGE STREQUAL shared)
    set(c_target Knot::Shared)
    set(cpp_target Knot::CppShared)
elseif(KNOT_LINKAGE STREQUAL static)
    set(c_target Knot::Static)
    set(cpp_target Knot::CppStatic)
else()
    message(FATAL_ERROR "KNOT_LINKAGE must be shared or static")
endif()
add_executable(speaker cpp/speaker.cpp)
target_link_libraries(speaker PRIVATE ${cpp_target})
add_executable(c_smoke c/smoke.c)
target_link_libraries(c_smoke PRIVATE ${c_target})
```

<!-- source: examples/cpp/speaker.cpp -->
```cpp
#include <knot/knot.hpp>
#include <iostream>

int main(int argc, char **argv) {
    if (argc != 2) {
        std::cerr << "Usage: speaker <speaker.json>\n";
        return 2;
    }
    try {
        auto model = knot::Model::compile_file(argv[1]);
        auto result = model.initial();
        auto set = [&](const std::string &id, knot::Answer value) {
            result = model.apply(
                result.state, knot::Event::set_answer(id, std::move(value), result.state.revision));
        };
        set("speaker_type", std::string("active"));
        set("material", std::string("aluminum"));
        set("panel_color", std::string("#315A79"));
        set("power", std::string("200w"));
        set("extras", std::vector<std::string>{"led"});
        auto display = [&] {
            const auto &price = result.snapshot.price;
            if (price.total_minor)
                std::cout << *price.total_minor << '\n';
            else
                std::cout << "unknown\n";
        };
        display();
        set("material", std::string("walnut"));
        display();
        // Save the whole returned State; restore with this same model.
        const auto saved = knot::to_json(result.state);
        result = model.evaluate(knot::state_from_json(saved));
        return 0;
    } catch (const knot::Error &error) {
        std::cerr << error.code << " at " << error.path << ": " << error.what() << '\n';
        return 3;
    } catch (const std::exception &error) {
        std::cerr << error.what() << '\n';
        return 2;
    }
}
```

All six events match [speaker-events.json](../contracts/examples/speaker-events.json). The first five answers produce totalMinor `282800`; changing material to walnut produces `159900` and clears invalid downstream answers. The [model](../contracts/examples/speaker.json), [first expected result](../contracts/golden/T12/expected-result.json) and [changed expected result](../contracts/golden/T10/expected-result.json) are shipped together.

Save the complete returned State and replace the complete Snapshot on success. On failure, keep your old State. Unknown price is null, not zero; optional questions still need an answer or explicit skip. A final price is not payment authorization. Details: [state machine](state-machine.md).
