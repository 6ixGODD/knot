// First include deliberately detects forced includes and external JSON macros.
#include "reject_json_pollution.hpp"
#include <knot/knot.hpp>
#include <algorithm>
#include <cmath>
#include <fstream>
#include <regex>
#include <set>
#include <utility>
#define JSON_DIAGNOSTICS 0
#define JSON_DIAGNOSTIC_POSITIONS 0
#define JSON_USE_IMPLICIT_CONVERSIONS 0
#define JSON_USE_GLOBAL_UDLS 0
#define nlohmann knot_private_json_v3_12_0
#include "json.hpp"
#undef nlohmann
#include "wire_schemas.hpp"

namespace knot {
namespace {
using Json = knot_private_json_v3_12_0::json;
constexpr std::size_t product_limit = 4194304, output_limit = 4194304;
[[noreturn]] void invalid(const std::string &message = "Invalid native JSON",
                          const std::string &path = "") {
    throw Error(KNOT_E_BAD_SCHEMA, "BAD_SCHEMA", message, path);
}
// Inspect the original token before binary64 conversion. Exact mathematical
// integers are rewritten to integer tokens; fractional tokens remain floats.
// Thus 1.0 is accepted, but 9007199254740991.1 never rounds into an integer.
std::string normalize_number(const std::string &token) {
    static const std::regex syntax(R"(-?(0|[1-9][0-9]*)(\.[0-9]+)?([eE][+-]?[0-9]+)?)");
    if (token.size() > 128 || !std::regex_match(token, syntax))
        invalid("Invalid number token");
    auto exp_pos = token.find_first_of("eE");
    auto mantissa = token.substr(0, exp_pos);
    int exponent = 0;
    if (exp_pos != std::string::npos) {
        auto e = token.substr(exp_pos + 1);
        bool negative = e.front() == '-';
        std::size_t i = (e.front() == '-' || e.front() == '+') ? 1 : 0;
        for (; i < e.size(); ++i) {
            exponent = exponent * 10 + (e[i] - '0');
            if (exponent > 308)
                invalid("Exponent exceeds limit");
        }
        if (negative)
            exponent = -exponent;
    }
    bool negative = mantissa.front() == '-';
    if (negative)
        mantissa.erase(0, 1);
    auto point = mantissa.find('.');
    if (point != std::string::npos) {
        exponent -= static_cast<int>(mantissa.size() - point - 1);
        mantissa.erase(point, 1);
    }
    while (mantissa.size() > 1 && mantissa.front() == '0')
        mantissa.erase(0, 1);
    if (mantissa == "0")
        return "0";
    while (exponent < 0 && mantissa.back() == '0') {
        mantissa.pop_back();
        ++exponent;
    }
    if (exponent < 0)
        return token;
    if (mantissa.size() + static_cast<std::size_t>(exponent) > 16)
        return token;
    mantissa.append(static_cast<std::size_t>(exponent), '0');
    if (mantissa.size() == 16 && mantissa > "9007199254740991")
        return token;
    return negative ? "-" + mantissa : mantissa;
}
Json parse(const std::string &text, std::size_t limit = output_limit) {
    if (text.size() > limit)
        throw Error(KNOT_E_RESOURCE_LIMIT, "RESOURCE_LIMIT", "JSON byte limit exceeded");
    std::string normalized;
    normalized.reserve(text.size());
    bool quoted = false, escape = false;
    for (std::size_t i = 0; i < text.size();) {
        char c = text[i];
        if (!quoted && (c == '-' || (c >= '0' && c <= '9'))) {
            auto start = i++;
            while (i < text.size() &&
                   std::string("0123456789.eE+-").find(text[i]) != std::string::npos)
                ++i;
            normalized += normalize_number(text.substr(start, i - start));
            continue;
        }
        normalized += c;
        ++i;
        if (quoted) {
            if (escape)
                escape = false;
            else if (c == '\\')
                escape = true;
            else if (c == '"')
                quoted = false;
        } else if (c == '"')
            quoted = true;
    }
    std::vector<std::set<std::string>> keys;
    return Json::parse(normalized, [&](int depth, Json::parse_event_t event, Json &value) {
        if (depth > 64)
            invalid("JSON nesting exceeds limit");
        if (event == Json::parse_event_t::object_start)
            keys.emplace_back();
        if (event == Json::parse_event_t::object_end)
            keys.pop_back();
        if (value.is_string() && value.get_ref<const std::string &>().size() > 16384)
            invalid("String exceeds limit");
        if (event == Json::parse_event_t::key &&
            !keys.back().insert(value.get<std::string>()).second)
            invalid("Duplicate JSON key");
        return true;
    });
}
bool id_valid(const std::string &s) {
    if (s.empty() || s.size() > 128)
        return false;
    for (std::size_t i = 0; i < s.size(); ++i) {
        auto c = static_cast<unsigned char>(s[i]);
        if (c < 32 || c == 127 ||
            (c == 194 && i + 1 < s.size() && static_cast<unsigned char>(s[i + 1]) < 160))
            return false;
    }
    return true;
}
bool decimal_valid(std::string s) {
    if (s.empty() || s.size() > 128)
        return false;
    if (s.front() == '-')
        s.erase(0, 1);
    auto p = s.find('.');
    auto whole = s.substr(0, p);
    auto frac = p == std::string::npos ? std::string{} : s.substr(p + 1);
    while (whole.size() > 1 && whole.front() == '0')
        whole.erase(0, 1);
    while (!frac.empty() && frac.back() == '0')
        frac.pop_back();
    return frac.size() <= 9 &&
           (whole.size() < 16 ||
            (whole.size() == 16 &&
             (whole < "9007199254740991" || (whole == "9007199254740991" && frac.empty()))));
}
// Evaluates only the vocabulary used by the frozen wire schemas. The build
// generator rejects any unimplemented keyword or external reference.
bool matches(const Json &v, const Json &s, const Json &root) {
    if (s.is_boolean())
        return s.get<bool>();
    if (s.contains("$ref"))
        return matches(v, root.at(Json::json_pointer(s.at("$ref").get<std::string>().substr(1))),
                       root);
    for (const auto *keyword : {"oneOf", "anyOf", "allOf"})
        if (s.contains(keyword)) {
            std::size_t count = 0;
            for (const auto &branch : s.at(keyword))
                if (matches(v, branch, root))
                    ++count;
            if ((std::string(keyword) == "oneOf" && count != 1) ||
                (std::string(keyword) == "anyOf" && count == 0) ||
                (std::string(keyword) == "allOf" && count != s.at(keyword).size()))
                return false;
        }
    if (s.contains("if")) {
        auto branch = matches(v, s.at("if"), root) ? "then" : "else";
        if (s.contains(branch) && !matches(v, s.at(branch), root))
            return false;
    }
    if (s.contains("const") && v != s.at("const"))
        return false;
    if (s.contains("enum") &&
        std::find(s.at("enum").begin(), s.at("enum").end(), v) == s.at("enum").end())
        return false;
    if (s.contains("type")) {
        auto t = s.at("type").get<std::string>();
        if (!((t == "object" && v.is_object()) || (t == "array" && v.is_array()) ||
              (t == "string" && v.is_string()) || (t == "boolean" && v.is_boolean()) ||
              (t == "null" && v.is_null()) || (t == "number" && v.is_number()) ||
              (t == "integer" && v.is_number_integer())))
            return false;
    }
    if (v.is_number()) {
        if (s.contains("minimum") && v < s.at("minimum"))
            return false;
        if (s.contains("maximum") && v > s.at("maximum"))
            return false;
    }
    if (v.is_string()) {
        const auto &str = v.get_ref<const std::string &>();
        if (str.size() > 16384)
            return false;
        auto length =
            std::count_if(str.begin(), str.end(), [](unsigned char c) { return (c & 192) != 128; });
        if (s.contains("minLength") && length < s.at("minLength").get<std::ptrdiff_t>())
            return false;
        if (s.contains("maxLength") && length > s.at("maxLength").get<std::ptrdiff_t>())
            return false;
        if (s.value("format", "") == "knot-id") {
            if (!id_valid(str))
                return false;
        } else if (s.contains("pattern") &&
                   !std::regex_search(str, std::regex(s.at("pattern").get<std::string>())))
            return false;
        if (s.value("format", "") == "knot-decimal" && !decimal_valid(str))
            return false;
    }
    if (v.is_array()) {
        if (s.contains("minItems") && v.size() < s.at("minItems").get<std::size_t>())
            return false;
        if (s.contains("maxItems") && v.size() > s.at("maxItems").get<std::size_t>())
            return false;
        if (s.value("uniqueItems", false)) {
            std::set<Json> unique(v.begin(), v.end());
            if (unique.size() != v.size())
                return false;
        }
        if (s.contains("items"))
            for (const auto &item : v)
                if (!matches(item, s.at("items"), root))
                    return false;
    }
    if (v.is_object()) {
        if (s.contains("maxProperties") && v.size() > s.at("maxProperties").get<std::size_t>())
            return false;
        if (s.contains("required"))
            for (const auto &key : s.at("required"))
                if (!v.contains(key.get<std::string>()))
                    return false;
        for (auto it = v.begin(); it != v.end(); ++it) {
            if (s.contains("propertyNames") && !matches(it.key(), s.at("propertyNames"), root))
                return false;
            if (s.contains("properties") && s.at("properties").contains(it.key())) {
                if (!matches(it.value(), s.at("properties").at(it.key()), root))
                    return false;
            } else if (s.contains("additionalProperties") &&
                       !matches(it.value(), s.at("additionalProperties"), root))
                return false;
        }
    }
    return true;
}
const Json &schemas() {
    static const auto s = Json::parse(wire_schemas);
    return s;
}
void validate(const Json &value, const char *kind) {
    const auto &s = schemas().at(kind);
    if (!matches(value, s, s))
        invalid(std::string("Native ") + kind + " violates wire schema");
}
template <class F> auto translate(F &&f) -> decltype(f()) {
    try {
        return f();
    } catch (const Json::exception &e) {
        throw Error(KNOT_E_BAD_JSON, "BAD_JSON", e.what());
    }
}
template <class T> std::optional<T> optional(const Json &o, const char *key) {
    if (!o.contains(key) || o.at(key).is_null())
        return std::nullopt;
    return o.at(key).get<T>();
}
template <class E>
E enumeration(const Json &v, std::initializer_list<std::pair<const char *, E>> values) {
    for (const auto &pair : values)
        if (v == pair.first)
            return pair.second;
    invalid("Unknown enum");
}
Answer answer(const Json &j) {
    if (j.is_string())
        return j.get<std::string>();
    if (j.is_array())
        return j.get<std::vector<std::string>>();
    return j.get<std::int64_t>();
}
Json answer_json(const Answer &a) {
    return std::visit([](const auto &v) { return Json(v); }, a);
}
State state(const Json &j) {
    State s{j.at("schemaVersion").get<std::string>(),
            j.at("productId").get<std::string>(),
            j.at("modelDigest").get<std::string>(),
            j.at("revision").get<std::int64_t>(),
            {},
            j.at("skipped").get<std::vector<std::string>>()};
    for (auto it = j.at("answers").begin(); it != j.at("answers").end(); ++it)
        s.answers.emplace(it.key(), answer(it.value()));
    return s;
}
std::vector<Diagnostic> diagnostics(const Json &j) {
    std::vector<Diagnostic> out;
    for (const auto &d : j)
        out.push_back(
            {enumeration<DiagnosticLevel>(d.at("level"), {{"info", DiagnosticLevel::info},
                                                          {"warning", DiagnosticLevel::warning},
                                                          {"error", DiagnosticLevel::error}}),
             d.at("code").get<std::string>(), d.at("path").get<std::string>(),
             d.at("ruleIds").get<std::vector<std::string>>()});
    return out;
}
CurrentQuestion question(const Json &q) {
    CurrentQuestion out;
    out.id = q.at("id").get<std::string>();
    out.label = q.at("label").get<std::string>();
    out.required = q.at("required").get<bool>();
    auto type = q.at("type").get<std::string>();
    out.type = enumeration<QuestionType>(q.at("type"), {{"single", QuestionType::single},
                                                        {"multi", QuestionType::multi},
                                                        {"integer", QuestionType::integer},
                                                        {"decimal", QuestionType::decimal},
                                                        {"color", QuestionType::color}});
    if (type == "single" || type == "multi") {
        ChoiceQuestion choices;
        for (const auto &o : q.at("options"))
            choices.options.push_back({o.at("id").get<std::string>(),
                                       o.at("label").get<std::string>(),
                                       o.at("enabled").get<bool>()});
        out.constraints = std::move(choices);
    } else if (type == "integer")
        out.constraints =
            IntegerQuestion{optional<std::int64_t>(q, "min"), optional<std::int64_t>(q, "max"),
                            optional<std::int64_t>(q, "step")};
    else if (type == "decimal")
        out.constraints =
            DecimalQuestion{optional<std::string>(q, "min"), optional<std::string>(q, "max"),
                            optional<std::string>(q, "step"), q.at("scale").get<std::int64_t>()};
    else
        out.constraints = ColorQuestion{};
    return out;
}
Result result(const Json &j) {
    validate(j, "result");
    if (!j.contains("state"))
        invalid("Expected transition result");
    Result out;
    out.schema_version = j.at("schemaVersion").get<std::string>();
    out.state = state(j.at("state"));
    const auto &s = j.at("snapshot");
    const auto &p = s.at("price");
    const auto &f = s.at("flow");
    const auto &c = j.at("changes");
    out.snapshot.price = {
        enumeration<PriceStatus>(p.at("status"), {{"unknown", PriceStatus::unknown},
                                                  {"estimated", PriceStatus::estimated},
                                                  {"final", PriceStatus::final}}),
        p.at("currency").get<std::string>(),
        optional<std::int64_t>(p, "totalMinor"),
        {}};
    for (const auto &item : p.at("components"))
        out.snapshot.price.components.push_back(
            {item.at("id").get<std::string>(), item.at("amountMinor").get<std::int64_t>()});
    out.snapshot.flow = {
        enumeration<FlowStatus>(f.at("status"), {{"in_progress", FlowStatus::in_progress},
                                                 {"complete", FlowStatus::complete},
                                                 {"blocked", FlowStatus::blocked}}),
        f.at("path").get<std::vector<std::string>>(), optional<std::string>(f, "nextQuestionId"),
        std::nullopt};
    if (!f.at("currentQuestion").is_null())
        out.snapshot.flow.current_question = question(f.at("currentQuestion"));
    for (const auto &t : s.at("visual").at("targets")) {
        VisualTarget target{t.at("id").get<std::string>(), t.at("order").get<std::int64_t>(), {}};
        for (auto it = t.at("properties").begin(); it != t.at("properties").end(); ++it) {
            const auto &v = it.value();
            if (v.is_boolean())
                target.properties.emplace(it.key(), v.get<bool>());
            else if (v.is_string())
                target.properties.emplace(it.key(), v.get<std::string>());
            else if (v.is_array())
                target.properties.emplace(it.key(), v.get<std::array<double, 3>>());
            else
                target.properties.emplace(it.key(), v.get<double>());
        }
        out.snapshot.visual.targets.push_back(std::move(target));
    }
    out.snapshot.validation.valid = s.at("validation").at("valid").get<bool>();
    for (const auto &issue : s.at("validation").at("issues"))
        out.snapshot.validation.issues.push_back(
            {issue.at("code").get<std::string>(), optional<std::string>(issue, "questionId"),
             issue.at("path").get<std::string>(),
             issue.at("ruleIds").get<std::vector<std::string>>()});
    out.changes = {{},
                   c.at("priceChanged").get<bool>(),
                   c.at("visualChangedTargets").get<std::vector<std::string>>(),
                   c.at("flowChanged").get<bool>()};
    for (const auto &a : c.at("answerChanges"))
        out.changes.answer_changes.push_back(
            {a.at("questionId").get<std::string>(),
             enumeration<ChangeKind>(a.at("kind"), {{"set", ChangeKind::set},
                                                    {"cleared", ChangeKind::cleared},
                                                    {"skipped", ChangeKind::skipped}}),
             enumeration<ChangeReason>(a.at("reason"),
                                       {{"user", ChangeReason::user},
                                        {"became_disabled", ChangeReason::became_disabled},
                                        {"branch_unreachable", ChangeReason::branch_unreachable}}),
             optional<std::vector<std::string>>(a, "ruleIds")});
    out.diagnostics = diagnostics(j.at("diagnostics"));
    return out;
}
struct ResultGuard {
    knot_result_t value;
    ~ResultGuard() noexcept { knot_result_free(&value); }
    explicit ResultGuard(knot_result_t r) noexcept : value(r) {}
    ResultGuard(const ResultGuard &) = delete;
    ResultGuard &operator=(const ResultGuard &) = delete;
};
Json response(const ResultGuard &guard) {
    const auto &r = guard.value;
    if (!r.json.data || r.json.len == 0) {
        if (r.status == KNOT_E_INTERNAL && !r.json.data && r.json.len == 0)
            throw Error(r.status, "INTERNAL", "Native internal failure (no JSON)");
        invalid("Native result contains no JSON");
    }
    if (r.json.len > output_limit)
        throw Error(KNOT_E_RESOURCE_LIMIT, "RESOURCE_LIMIT", "Native output exceeds limit");
    auto value = parse(std::string(reinterpret_cast<const char *>(r.json.data), r.json.len));
    if (r.status != KNOT_OK) {
        validate(value, "error");
        const auto &e = value.at("error");
        auto code = e.at("code").get<std::string>();
        if (schemas().at("statuses").at(code).get<knot_status_t>() != r.status)
            invalid("Native status/code mismatch");
        throw Error(r.status, code, e.at("message").get<std::string>(),
                    e.at("path").get<std::string>(),
                    e.at("ruleIds").get<std::vector<std::string>>());
    }
    return value;
}
const std::uint8_t *bytes(const std::string &s) {
    return reinterpret_cast<const std::uint8_t *>(s.data());
}
} // namespace

Error::Error(knot_status_t s, std::string c, std::string m, std::string p,
             std::vector<std::string> r)
    : std::runtime_error(std::move(m)), status(s), code(std::move(c)), path(std::move(p)),
      rule_ids(std::move(r)) {}
Event Event::set_answer(std::string id, Answer a, std::int64_t r) {
    return {Type::set_answer, std::move(id), r, std::move(a)};
}
Event Event::clear_answer(std::string id, std::int64_t r) {
    return {Type::clear_answer, std::move(id), r, std::nullopt};
}
Event Event::skip_answer(std::string id, std::int64_t r) {
    return {Type::skip_answer, std::move(id), r, std::nullopt};
}
std::string to_json(const State &s) {
    return translate([&] {
        Json a = Json::object();
        for (const auto &pair : s.answers)
            a[pair.first] = answer_json(pair.second);
        Json j = {{"schemaVersion", s.schema_version},
                  {"productId", s.product_id},
                  {"modelDigest", s.model_digest},
                  {"revision", s.revision},
                  {"answers", a},
                  {"skipped", s.skipped}};
        validate(j, "state");
        auto text = j.dump();
        if (text.size() > 524288)
            throw Error(1011, "RESOURCE_LIMIT", "State exceeds limit");
        return text;
    });
}
std::string to_json(const Event &e) {
    return translate([&] {
        std::string type;
        switch (e.type) {
        case Event::Type::set_answer:
            type = "set_answer";
            break;
        case Event::Type::clear_answer:
            type = "clear_answer";
            break;
        case Event::Type::skip_answer:
            type = "skip_answer";
            break;
        default:
            invalid("Invalid Event type");
        }
        Json j = {{"schemaVersion", "1.0"},
                  {"type", type},
                  {"questionId", e.question_id},
                  {"expectedRevision", e.expected_revision}};
        if (e.value)
            j["value"] = answer_json(*e.value);
        validate(j, "event");
        auto text = j.dump();
        if (text.size() > 65536)
            throw Error(1011, "RESOURCE_LIMIT", "Event exceeds limit");
        return text;
    });
}
State state_from_json(const std::string &text) {
    return translate([&] {
        auto j = parse(text, 524288);
        validate(j, "state");
        return state(j);
    });
}
Model::~Model() noexcept {
    if (handle_)
        knot_model_free(handle_);
}
Model::Model(Model &&other) noexcept
    : handle_(std::exchange(other.handle_, nullptr)), info_(std::move(other.info_)) {}
Model &Model::operator=(Model &&other) noexcept {
    if (this != &other) {
        if (handle_)
            knot_model_free(handle_);
        handle_ = std::exchange(other.handle_, nullptr);
        info_ = std::move(other.info_);
    }
    return *this;
}
void Model::require_open() const {
    if (!handle_)
        throw Error(1001, "NULL_ARGUMENT", "Model was moved from");
}
Model Model::compile(const std::string &text) {
    return translate([&] {
        if (text.size() > product_limit)
            throw Error(1011, "RESOURCE_LIMIT", "Product exceeds limit");
        if (knot_abi_version() != KNOT_ABI_VERSION)
            throw Error(1999, "INTERNAL", "Unsupported native ABI version");
        knot_model_t *handle = nullptr;
        ResultGuard guard(knot_model_create(bytes(text), text.size(), &handle));
        Model model(handle); // Acquire before any decoding or allocation can throw.
        auto j = response(guard);
        validate(j, "result");
        if (!handle || !j.contains("productId"))
            invalid("Invalid model-create result");
        model.info_ = {j.at("schemaVersion").get<std::string>(),
                       j.at("productId").get<std::string>(),
                       j.at("productVersion").get<std::string>(),
                       j.at("modelDigest").get<std::string>(), diagnostics(j.at("diagnostics"))};
        return model;
    });
}
Model Model::compile_file(const std::filesystem::path &path) {
    std::ifstream file(path, std::ios::binary);
    if (!file)
        throw std::filesystem::filesystem_error("Cannot open Product", path,
                                                std::make_error_code(std::errc::io_error));
    std::string text;
    std::array<char, 8192> block{};
    while (file) {
        file.read(block.data(), static_cast<std::streamsize>(block.size()));
        auto n = static_cast<std::size_t>(file.gcount());
        if (text.size() + n > product_limit)
            throw Error(1011, "RESOURCE_LIMIT", "Product exceeds limit");
        text.append(block.data(), n);
    }
    if (!file.eof())
        throw std::filesystem::filesystem_error("Cannot read Product", path,
                                                std::make_error_code(std::errc::io_error));
    return compile(text);
}
Result Model::initial() const {
    require_open();
    return translate([&] {
        ResultGuard r(knot_initial(handle_));
        return result(response(r));
    });
}
Result Model::apply(const State &s, const Event &e) const {
    require_open();
    return translate([&] {
        auto sj = to_json(s), ej = to_json(e);
        ResultGuard r(knot_apply(handle_, bytes(sj), sj.size(), bytes(ej), ej.size()));
        return result(response(r));
    });
}
Result Model::evaluate(const State &s) const {
    require_open();
    return translate([&] {
        auto sj = to_json(s);
        ResultGuard r(knot_evaluate(handle_, bytes(sj), sj.size()));
        return result(response(r));
    });
}
} // namespace knot
