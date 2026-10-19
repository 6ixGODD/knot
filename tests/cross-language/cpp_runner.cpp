#include <atomic>
#include <fstream>
#include <iostream>
#include <json.hpp>
#include <knot.h>
#include <knot/knot.hpp>
#include <thread>
using J = nlohmann::json;
template <class T> J optional(const std::optional<T> &value) {
  return value ? J(*value) : J(nullptr);
}
J diagnostics(const std::vector<knot::Diagnostic> &values) {
  J out = J::array();
  for (const auto &v : values)
    out.push_back({{"level", std::array{"info", "warning", "error"}.at(
                                 static_cast<size_t>(v.level))},
                   {"code", v.code},
                   {"path", v.path},
                   {"ruleIds", v.rule_ids}});
  return out;
}
J encode(const knot::Result &value) {
  const auto &p = value.snapshot.price;
  J components = J::array();
  for (const auto &c : p.components)
    components.push_back({{"id", c.id}, {"amountMinor", c.amount_minor}});
  J price = {{"status", std::array{"unknown", "estimated", "final"}.at(
                            static_cast<size_t>(p.status))},
             {"currency", p.currency},
             {"totalMinor", optional(p.total_minor)},
             {"components", components}};
  const auto &f = value.snapshot.flow;
  J current = nullptr;
  if (f.current_question) {
    const auto &q = *f.current_question;
    current = {{"id", q.id},
               {"label", q.label},
               {"type",
                std::array{"single", "multi", "integer", "decimal", "color"}.at(
                    static_cast<size_t>(q.type))},
               {"required", q.required}};
    if (const auto *c = std::get_if<knot::ChoiceQuestion>(&q.constraints)) {
      current["options"] = J::array();
      for (const auto &o : c->options)
        current["options"].push_back(
            {{"id", o.id}, {"label", o.label}, {"enabled", o.enabled}});
    } else if (const auto *integer =
                   std::get_if<knot::IntegerQuestion>(&q.constraints)) {
      if (integer->min)
        current["min"] = *integer->min;
      if (integer->max)
        current["max"] = *integer->max;
      if (integer->step)
        current["step"] = *integer->step;
    } else if (const auto *decimal =
                   std::get_if<knot::DecimalQuestion>(&q.constraints)) {
      if (decimal->min)
        current["min"] = *decimal->min;
      if (decimal->max)
        current["max"] = *decimal->max;
      if (decimal->step)
        current["step"] = *decimal->step;
      current["scale"] = decimal->scale;
    }
  }
  J flow = {{"status", std::array{"in_progress", "complete", "blocked"}.at(
                           static_cast<size_t>(f.status))},
            {"path", f.path},
            {"nextQuestionId", optional(f.next_question_id)},
            {"currentQuestion", current}};
  J targets = J::array();
  for (const auto &t : value.snapshot.visual.targets) {
    J properties = J::object();
    for (const auto &[key, v] : t.properties)
      std::visit([&](const auto &x) { properties[key] = x; }, v);
    targets.push_back(
        {{"id", t.id}, {"order", t.order}, {"properties", properties}});
  }
  J issues = J::array();
  for (const auto &i : value.snapshot.validation.issues)
    issues.push_back({{"code", i.code},
                      {"questionId", optional(i.question_id)},
                      {"path", i.path},
                      {"ruleIds", i.rule_ids}});
  J changes = J::array();
  for (const auto &c : value.changes.answer_changes) {
    J change = {{"questionId", c.question_id},
                {"kind", std::array{"set", "cleared", "skipped"}.at(
                             static_cast<size_t>(c.kind))},
                {"reason",
                 std::array{"user", "became_disabled", "branch_unreachable"}.at(
                     static_cast<size_t>(c.reason))}};
    if (c.rule_ids)
      change["ruleIds"] = *c.rule_ids;
    changes.push_back(change);
  }
  return {
      {"schemaVersion", value.schema_version},
      {"state", J::parse(knot::to_json(value.state))},
      {"snapshot",
       {{"price", price},
        {"flow", flow},
        {"visual", {{"targets", targets}}},
        {"validation",
         {{"valid", value.snapshot.validation.valid}, {"issues", issues}}}}},
      {"changes",
       {{"answerChanges", changes},
        {"priceChanged", value.changes.price_changed},
        {"visualChangedTargets", value.changes.visual_changed_targets},
        {"flowChanged", value.changes.flow_changed}}},
      {"diagnostics", diagnostics(value.diagnostics)}};
}
std::string read(const char *path) {
  std::ifstream in(path, std::ios::binary);
  if (!in)
    throw std::runtime_error("file read failed");
  return {std::istreambuf_iterator<char>(in), std::istreambuf_iterator<char>()};
}
// Inputs outside the typed Answer domain still exercise the real C ABI from
// this C++ consumer. Keep the original JSON bytes, including exact decimals.
J apply_wire(const std::string &product, const std::string &state,
             const std::string &event) {
  knot_model_t *handle = nullptr;
  auto created =
      knot_model_create(reinterpret_cast<const std::uint8_t *>(product.data()),
                        product.size(), &handle);
  struct ModelOwner {
    knot_model_t *value;
    ~ModelOwner() { knot_model_free(value); }
  } model{handle};
  struct ResultOwner {
    knot_result_t value;
    ~ResultOwner() { knot_result_free(&value); }
  } result{created};
  if (result.value.status == 0) {
    knot_result_free(&result.value);
    result.value = knot_apply(
        handle, reinterpret_cast<const std::uint8_t *>(state.data()),
        state.size(), reinterpret_cast<const std::uint8_t *>(event.data()),
        event.size());
  }
  const auto &raw = result.value;
  J payload = nullptr;
  if (raw.json.data)
    payload = J::parse(raw.json.data, raw.json.data + raw.json.len);
  return {
      {"status", raw.status}, {"result", payload}, {"interface", "raw-abi"}};
}
int main(int argc, char **argv) {
  if (argc != 5)
    return 2;
  try {
    auto model = knot::Model::compile(read(argv[2]));
    J result;
    const std::string operation = argv[1];
    if (operation == "model_create") {
      const auto &info = model.info();
      result = {{"schemaVersion", info.schema_version},
                {"productId", info.product_id},
                {"productVersion", info.product_version},
                {"modelDigest", info.model_digest},
                {"diagnostics", diagnostics(info.diagnostics)}};
    } else if (operation == "initial")
      result = encode(model.initial());
    else {
      auto state = knot::state_from_json(read(argv[3]));
      if (operation == "evaluate" || operation == "threads") {
        result = encode(model.evaluate(state));
        if (operation == "threads") {
          const auto other = model.initial().state;
          const auto expected_other = encode(model.evaluate(other));
          std::atomic<bool> failed{false};
          std::vector<std::thread> threads;
          for (int t = 0; t < 8; ++t)
            threads.emplace_back([&]() {
              try {
                for (int i = 0; i < 100; ++i) {
                  if (encode(model.evaluate(i % 2 ? other : state)) !=
                      (i % 2 ? expected_other : result))
                    failed = true;
                }
              } catch (...) {
                failed = true;
              }
            });
          for (auto &thread : threads)
            thread.join();
          if (failed)
            return 1;
        }
      } else if (operation == "apply") {
        const auto event_bytes = read(argv[4]);
        const auto event = J::parse(event_bytes);
        const auto id = event.at("questionId").get<std::string>();
        const auto revision = event.at("expectedRevision").get<std::int64_t>();
        const auto type = event.at("type").get<std::string>();
        auto e = knot::Event::clear_answer(id, revision);
        if (type == "skip_answer")
          e = knot::Event::skip_answer(id, revision);
        else if (type == "set_answer") {
          const auto &v = event.at("value");
          knot::Answer answer;
          if (v.is_string())
            answer = v.get<std::string>();
          else if (v.is_array())
            answer = v.get<std::vector<std::string>>();
          else if (v.is_number_integer())
            answer = v.get<std::int64_t>();
          else {
            std::cout
                << apply_wire(read(argv[2]), read(argv[3]), event_bytes).dump()
                << '\n';
            return std::cout ? 0 : 1;
          }
          e = knot::Event::set_answer(id, answer, revision);
        } else if (type != "clear_answer")
          return 2;
        result = encode(model.apply(state, e));
      } else
        return 2;
    }
    std::cout << J({{"status", 0}, {"result", result}}).dump() << '\n';
  } catch (const knot::Error &e) {
    std::cout << J({{"status", e.status},
                    {"result",
                     {{"schemaVersion", "1.0"},
                      {"error",
                       {{"code", e.code},
                        {"path", e.path},
                        {"ruleIds", e.rule_ids}}}}}})
                     .dump()
              << '\n';
  } catch (const std::exception &e) {
    std::cerr << e.what() << '\n';
    return 2;
  }
  return std::cout ? 0 : 1;
}
