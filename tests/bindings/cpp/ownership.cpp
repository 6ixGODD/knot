#include "test.hpp"
#include <type_traits>
int main() {
    static_assert(!std::is_copy_constructible_v<knot::Model>);
    static_assert(std::is_nothrow_move_constructible_v<knot::Model>);
    static_assert(std::is_nothrow_move_assignable_v<knot::Model>);
    static_assert(std::is_nothrow_destructible_v<knot::Model>);
    knot_test_reset();
    {
        auto a = knot::Model::compile("{}");
        CHECK(knot_test_results_freed() == 1);
        auto r = a.initial();
        CHECK(knot_test_results_freed() == 2);
        auto b = std::move(a);
        fails([&] { a.initial(); });
        b = std::move(b);
        CHECK(knot_test_live_models() == 1);
        auto c = knot::Model::compile("{}");
        c = std::move(b);
        CHECK(knot_test_models_freed() == 1);
        c.apply(r.state,
                knot::Event::set_answer("material", std::string("walnut"), r.state.revision));
        CHECK(std::string(knot_test_last_event()).find("walnut") != std::string::npos);
        c.evaluate(r.state);
        CHECK(knot_test_results_freed() == 5);
        respond("{");
        fails([&] { c.initial(); });
        CHECK(knot_test_results_freed() == 6);
        respond("{}");
        fails([&] { c.initial(); });
        CHECK(knot_test_results_freed() == 7);
    }
    CHECK(knot_test_live_models() == 0);
    CHECK(knot_test_models_freed() == 2);
    knot_test_reset();
    respond("{");
    knot_test_create_response(1);
    fails([] { knot::Model::compile("{}"); });
    CHECK(knot_test_live_models() == 0);
    CHECK(knot_test_models_freed() == 1);
    CHECK(knot_test_results_freed() == 1);
}
