#include "test.hpp"
#include <json.hpp>
using J = nlohmann::json;
int main() {
    knot_test_reset();
    auto model = knot::Model::compile("{}");
    const auto base = J::parse(load("golden/T12/expected-result.json"));
    respond(base.dump());
    auto r = model.initial();
    CHECK(r.state.revision == 5);
    CHECK(r.snapshot.price.total_minor == 282800);
    CHECK(std::get<std::vector<std::string>>(r.state.answers.at("extras")) ==
          std::vector<std::string>{"led"});
    auto verify_bad = [&](J j) {
        auto before = knot_test_results_freed();
        respond(j.dump());
        fails([&] { model.initial(); });
        CHECK(knot_test_results_freed() == before + 1);
    };
    for (auto it = base.begin(); it != base.end(); ++it) {
        auto j = base;
        j.erase(it.key());
        verify_bad(j);
    }
    for (const auto &ptr : {"/state", "/snapshot", "/snapshot/price", "/snapshot/flow",
                            "/snapshot/visual", "/snapshot/validation", "/changes"}) {
        auto object = base.at(J::json_pointer(ptr));
        for (auto it = object.begin(); it != object.end(); ++it) {
            auto j = base;
            j.at(J::json_pointer(ptr)).erase(it.key());
            verify_bad(j);
        }
        auto j = base;
        j.at(J::json_pointer(ptr))["unknownField"] = 1;
        verify_bad(j);
    }
    for (auto v : {J(-1), J(9007199254740992LL), J(true), J("5"), J(nullptr)}) {
        auto j = base;
        j["state"]["revision"] = v;
        verify_bad(j);
    }
    auto j = base;
    j["snapshot"]["price"]["status"] = "unknown";
    verify_bad(j);
    j["snapshot"]["price"]["totalMinor"] = nullptr;
    j["snapshot"]["price"]["components"] = J::array();
    respond(j.dump());
    CHECK(!model.initial().snapshot.price.total_minor);
    for (auto token :
         {"9007199254740991.1", "1.0000000000000000001", "9.007199254740992e15", "1e309"}) {
        auto text = base.dump();
        auto at = text.find("\"revision\":5");
        CHECK(at != std::string::npos);
        text.replace(at, 12, std::string("\"revision\":") + token);
        respond(text);
        fails([&] { model.initial(); });
    }
    auto text = base.dump();
    auto at = text.find("\"revision\":5");
    text.replace(at, 12, "\"revision\":1.0");
    respond(text);
    CHECK(model.initial().state.revision == 1);
    text = base.dump();
    text.insert(1, "\"schemaVersion\":\"1.0\",");
    respond(text);
    fails([&] { model.initial(); });
    respond(base.dump() + " {}");
    fails([&] { model.initial(); });
    j = base;
    j["state"]["answers"]["decimal"] = "9007199254740990.123456789";
    respond(j.dump());
    CHECK(std::get<std::string>(model.initial().state.answers.at("decimal")) ==
          "9007199254740990.123456789");
    j["snapshot"]["visual"]["targets"][0]["properties"]["opacity"] = 0.12345678901234568;
    respond(j.dump());
    CHECK(model.initial().snapshot.visual.targets.size() > 0);
    j["snapshot"]["visual"]["targets"][0]["properties"]["opacity"] = 2;
    verify_bad(j);
    auto s = r.state;
    s.answers["bad"] = std::int64_t(9007199254740992LL);
    fails([&] { knot::to_json(s); });
    CHECK(knot::state_from_json(knot::to_json(r.state)).revision == r.state.revision);
    // Limits count UTF-8 bytes, not code points, on both codec directions.
    std::string unicode;
    for (int i = 0; i < 5461; ++i)
        unicode += "中";
    s = r.state;
    s.answers["unicode"] = unicode + "a";
    CHECK(std::get<std::string>(knot::state_from_json(knot::to_json(s)).answers.at("unicode")) ==
          unicode + "a");
    s.answers["unicode"] = unicode + "ab";
    fails([&] { knot::to_json(s); });
    CHECK(!knot::to_json(knot::Event::set_answer("q", unicode + "a", 0)).empty());
    fails([&] { knot::to_json(knot::Event::set_answer("q", unicode + "ab", 0)); });
    CHECK(J::parse(knot::to_json(knot::Event::clear_answer("q", 2)))["type"] == "clear_answer");
    CHECK(J::parse(knot::to_json(knot::Event::skip_answer("q", 2)))["type"] == "skip_answer");
    // Decode all frozen expected fixtures, without evaluating any business rules.
    auto errors = J::parse(load("errors.json"));
    int cases = 0;
    for (const auto &entry :
         std::filesystem::directory_iterator(std::string(CONTRACTS) + "/golden")) {
        auto path = entry.path() / "expected-result.json";
        if (!std::filesystem::exists(path))
            continue;
        std::ifstream f(path);
        J expected;
        f >> expected;
        ++cases;
        if (expected.contains("error")) {
            int status = 0;
            for (const auto &e : errors["errors"])
                if (e["code"] == expected["error"]["code"])
                    status = e["status"];
            CHECK(status != 0);
            respond(expected.dump(), status);
            try {
                model.initial();
                CHECK(false);
            } catch (const knot::Error &error) {
                CHECK(error.status == status);
                CHECK(error.code == expected["error"]["code"]);
                CHECK(error.path == expected["error"]["path"]);
                CHECK(error.rule_ids ==
                      expected["error"]["ruleIds"].get<std::vector<std::string>>());
            }
        } else if (expected.contains("state")) {
            respond(expected.dump());
            model.initial();
        } else {
            respond(expected.dump());
            knot_test_create_response(1);
            {
                auto compiled = knot::Model::compile("{}");
                CHECK(compiled.info().model_digest == expected["modelDigest"]);
            }
            knot_test_create_response(0);
        }
    }
    CHECK(cases == 43);
}
