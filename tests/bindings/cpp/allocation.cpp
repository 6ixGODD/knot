#include "test.hpp"
#include <json.hpp>
#include <new>
namespace {
std::size_t allocations = 0, fail_at = 0, last_size = 0;
std::size_t label_size = 0, label_allocations = 0, fail_label_at = 0;
} // namespace
void *operator new(std::size_t n) {
    ++allocations;
    last_size = n;
    if (n == label_size)
        ++label_allocations;
    if ((fail_at && allocations == fail_at) ||
        (fail_label_at && n == label_size && label_allocations == fail_label_at))
        throw std::bad_alloc();
    if (auto *p = std::malloc(n ? n : 1))
        return p;
    throw std::bad_alloc();
}
void operator delete(void *p) noexcept { std::free(p); }
void operator delete(void *p, std::size_t) noexcept { std::free(p); }
void *operator new[](std::size_t n) { return ::operator new(n); }
void operator delete[](void *p) noexcept { ::operator delete(p); }
void operator delete[](void *p, std::size_t) noexcept { ::operator delete(p); }
int main() {
    knot_test_reset();
    auto model = knot::Model::compile("{}");
    model.initial(); // Warm lazy regex/runtime caches before injection.
    for (std::size_t point : {1, 2}) {
        auto before = knot_test_results_freed();
        bool caught = false;
        allocations = 0;
        fail_at = point;
        try {
            model.initial();
        } catch (const std::bad_alloc &) {
            caught = true;
        }
        fail_at = 0;
        CHECK(caught);
        CHECK(knot_test_results_freed() == before + 1);
    }
    // Distinguish an owning DTO string allocation from parser/destructor work.
    // Learn this standard library's string allocation size (no CRT assumptions).
    const std::string label(9001, 'L');
    {
        std::string copy(label);
        label_size = last_size;
        CHECK(copy == label);
    }
    CHECK(label_size >= label.size());
    auto fixture = nlohmann::json::parse(load("golden/T10/expected-result.json"));
    fixture["snapshot"]["flow"]["currentQuestion"]["label"] = label;
    respond(fixture.dump());
    label_allocations = 0;
    {
        auto r = model.initial();
        CHECK(r.snapshot.flow.current_question->label == label);
    }
    const auto copies = label_allocations;
    CHECK(copies >= 2); // JSON storage, then the final owning CurrentQuestion label.
    auto before = knot_test_results_freed();
    bool caught = false;
    label_allocations = 0;
    fail_label_at = copies;
    try {
        model.initial();
    } catch (const std::bad_alloc &) {
        caught = true;
    }
    fail_label_at = 0;
    CHECK(caught);
    CHECK(knot_test_results_freed() == before + 1);
    CHECK(knot_test_live_models() == 1);
    CHECK(knot_test_live_buffers() == 0);
}
