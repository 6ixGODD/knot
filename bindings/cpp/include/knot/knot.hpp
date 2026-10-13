#pragma once
#include <knot.h>
#include <array>
#include <cstdint>
#include <filesystem>
#include <map>
#include <optional>
#include <stdexcept>
#include <string>
#include <variant>
#include <vector>

namespace knot {
using Answer = std::variant<std::string, std::int64_t, std::vector<std::string>>;
using VisualValue = std::variant<bool, std::string, double, std::array<double, 3>>;
enum class PriceStatus { unknown, estimated, final };
enum class FlowStatus { in_progress, complete, blocked };
enum class DiagnosticLevel { info, warning, error };
enum class ChangeKind { set, cleared, skipped };
enum class ChangeReason { user, became_disabled, branch_unreachable };
enum class QuestionType { single, multi, integer, decimal, color };
struct Diagnostic {
    DiagnosticLevel level;
    std::string code, path;
    std::vector<std::string> rule_ids;
};
struct State {
    std::string schema_version = "1.0", product_id, model_digest;
    std::int64_t revision = 0;
    std::map<std::string, Answer> answers;
    std::vector<std::string> skipped;
};
struct AvailableOption {
    std::string id, label;
    bool enabled;
};
struct ChoiceQuestion {
    std::vector<AvailableOption> options;
};
struct IntegerQuestion {
    std::optional<std::int64_t> min, max, step;
};
struct DecimalQuestion {
    std::optional<std::string> min, max, step;
    std::int64_t scale;
};
struct ColorQuestion {};
struct CurrentQuestion {
    std::string id, label;
    QuestionType type;
    bool required;
    std::variant<ChoiceQuestion, IntegerQuestion, DecimalQuestion, ColorQuestion> constraints;
};
struct PricedComponent {
    std::string id;
    std::int64_t amount_minor;
};
struct Price {
    PriceStatus status;
    std::string currency;
    std::optional<std::int64_t> total_minor;
    std::vector<PricedComponent> components;
};
struct Flow {
    FlowStatus status;
    std::vector<std::string> path;
    std::optional<std::string> next_question_id;
    std::optional<CurrentQuestion> current_question;
};
struct VisualTarget {
    std::string id;
    std::int64_t order;
    std::map<std::string, VisualValue> properties;
};
struct Visual {
    std::vector<VisualTarget> targets;
};
struct ValidationIssue {
    std::string code;
    std::optional<std::string> question_id;
    std::string path;
    std::vector<std::string> rule_ids;
};
struct Validation {
    bool valid;
    std::vector<ValidationIssue> issues;
};
struct Snapshot {
    Price price;
    Flow flow;
    Visual visual;
    Validation validation;
};
struct AnswerChange {
    std::string question_id;
    ChangeKind kind;
    ChangeReason reason;
    std::optional<std::vector<std::string>> rule_ids;
};
struct Changes {
    std::vector<AnswerChange> answer_changes;
    bool price_changed;
    std::vector<std::string> visual_changed_targets;
    bool flow_changed;
};
struct Result {
    std::string schema_version;
    State state;
    Snapshot snapshot;
    Changes changes;
    std::vector<Diagnostic> diagnostics;
};
struct ModelInfo {
    std::string schema_version, product_id, product_version, model_digest;
    std::vector<Diagnostic> diagnostics;
};
class Error : public std::runtime_error {
  public:
    knot_status_t status;
    std::string code, path;
    std::vector<std::string> rule_ids;
    Error(knot_status_t status, std::string code, std::string message, std::string path = {},
          std::vector<std::string> rule_ids = {});
};
struct Event {
    enum class Type { set_answer, clear_answer, skip_answer };
    Type type;
    std::string question_id;
    std::int64_t expected_revision;
    std::optional<Answer> value;
    static Event set_answer(std::string id, Answer value, std::int64_t revision);
    static Event clear_answer(std::string id, std::int64_t revision);
    static Event skip_answer(std::string id, std::int64_t revision);
};
class Model {
    knot_model_t *handle_ = nullptr;
    ModelInfo info_;
    explicit Model(knot_model_t *handle) noexcept : handle_(handle) {}
    void require_open() const;

  public:
    static Model compile_file(const std::filesystem::path &path);
    static Model compile(const std::string &definition_json);
    ~Model() noexcept;
    Model(const Model &) = delete;
    Model &operator=(const Model &) = delete;
    Model(Model &&other) noexcept;
    Model &operator=(Model &&other) noexcept;
    const ModelInfo &info() const noexcept { return info_; }
    Result initial() const;
    Result apply(const State &state, const Event &event) const;
    Result evaluate(const State &state) const;
};
// Persistence helpers serialize only; they never evaluate or repair State.
std::string to_json(const State &state);
std::string to_json(const Event &event);
State state_from_json(const std::string &json);
} // namespace knot
