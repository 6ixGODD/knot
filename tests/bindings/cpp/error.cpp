#include "test.hpp"
int main() {
    knot_test_reset();
    auto model = knot::Model::compile("{}");
    respond(
        R"({"schemaVersion":"1.0","error":{"code":"INVALID_OPTION","message":"disabled","path":"/value","ruleIds":["rule:/questions/0"],"details":{}}})",
        1007);
    try {
        model.initial();
        CHECK(false);
    } catch (const knot::Error &e) {
        CHECK(e.status == 1007);
        CHECK(e.code == "INVALID_OPTION");
        CHECK(e.path == "/value");
        CHECK(e.rule_ids.size() == 1);
    }
    CHECK(knot_test_results_freed() == 2);
    knot_test_response(nullptr, 0, 1999);
    try {
        model.initial();
        CHECK(false);
    } catch (const knot::Error &e) {
        CHECK(e.code == "INTERNAL");
        CHECK(e.status == 1999);
    }
    CHECK(knot_test_results_freed() == 3);
    knot_test_response(nullptr, 0, 0);
    fails([&] { model.initial(); });
    CHECK(knot_test_results_freed() == 4);
    try {
        knot::Model::compile_file("this-product-does-not-exist.json");
        CHECK(false);
    } catch (const std::filesystem::filesystem_error &) {
    }
    fails([] { knot::Model::compile(std::string(4194305, ' ')); });
}
