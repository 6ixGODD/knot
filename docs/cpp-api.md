# C++17 API

Include `<knot/knot.hpp>` and link `Knot::CppShared` (or `Knot::CppStatic`). The header exposes only KNOT and standard-library types; JSON parsing is inside a precompiled bridge. Exceptions are required. Hosts built with exceptions disabled should use the [C ABI](c-api.md).

`Model::compile_file(path)` reads at most 4 MiB; I/O failures throw `std::filesystem::filesystem_error`, oversize input throws `knot::Error` with RESOURCE_LIMIT. `Model::compile(json)` passes a bounded UTF-8 Product to the native compiler and checks ABI version 1. `info()` returns owning model identity and diagnostics. `initial()`, `apply(State, Event)` and `evaluate(State)` return owning `Result` values. State and Event are borrowed for the call and never mutated.

Model uniquely owns its handle, cannot be copied, and supports noexcept move/destruction. Move-assignment releases the previous handle; self-move is safe. Using a moved-from Model throws NULL_ARGUMENT. Do not move or destroy a model while a call uses it. Each native result is freed exactly once, including decoding and native-error exceptions.

```cpp
#include <knot/knot.hpp>
#include <iostream>
int main() {
    try {
        auto model = knot::Model::compile_file("speaker.json");
        auto result = model.initial();
        result = model.apply(result.state,
            knot::Event::set_answer("speaker_type", std::string("active"), result.state.revision));
        const auto stored = knot::to_json(result.state);
        result = model.evaluate(knot::state_from_json(stored));
        return 0;
    } catch (const knot::Error& error) {
        std::cerr << error.status << " " << error.code << " " << error.path << '\n';
        return 1;
    }
}
```

`Error` exposes `status`, `code`, `path`, `rule_ids` and `what()`. Branch on code, not message. An INTERNAL result without JSON still becomes an Error. Malformed native JSON becomes BAD_JSON or BAD_SCHEMA; no JSON-library exception escapes. Standard allocation and filesystem exceptions remain distinguishable, never converted to success.

State fields: schema_version, product_id, model_digest, revision, answers, skipped. Answers are `variant<string,int64_t,vector<string>>`; decimal strings remain exact. `to_json` and `state_from_json` serialize/validate without repairing semantic state. Event factories are `set_answer(id, Answer, revision)`, `clear_answer(id, revision)`, `skip_answer(id, revision)`; wire version is always 1.0.

Snapshot contains typed price, flow, visual and validation. Price includes a closed status enum, currency, nullable int64 total_minor and ordered components. Flow includes a closed status, ordered path, optional next_question_id and CurrentQuestion. CurrentQuestion carries common id/label/type/required fields plus choice, integer, decimal or color constraints; absent numeric bounds remain optional. Visual values are bool/string/double/three-double-array; integer and money domains never use double. Validation contains valid and ordered issues. Changes contains answer_changes (question_id, kind, reason, optional rule_ids), price_changed, visual_changed_targets, flow_changed. Result also retains schema_version and ordered diagnostics. See the [public header](../bindings/cpp/include/knot/knot.hpp).

C++ ABI is stricter than the C ABI. Native-probe baselines: Windows MSVC 19.41 locally and 19.44 on Server 2022, x64 Release and dynamic CRT; Ubuntu 24.04 GNU 13.3/libstdc++; macOS 15.7.9 AppleClang 17/libc++ on arm64. Final packages must identify their exact compiler, standard-library ABI, architecture, Release configuration, CRT and deployment target. Do not mix Debug/Release, /MT and /MD, or arbitrary standard-library ABIs. Earlier macOS versions and arbitrary compiler combinations have not been certified; client/package validation on Linux/macOS is still pending.
