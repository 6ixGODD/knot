# Shared interface baseline

The wire fields are defined by the six schemas and the `knot-schema::wire`
structures. Bindings preserve those fields and reject missing required fields;
they do not supply success-shaped defaults or perform rule evaluation.

## Rust ownership and execution

The future Core entries are `initial(&CompiledModel)`,
`apply(&CompiledModel, &ConfigurationState, &ConfigurationEvent)` and
`evaluate(&CompiledModel, &ConfigurationState)`, all returning
`Result<TransitionResult, KnotError>`. Inputs are borrowed and immutable.
One new `CallBudget` belongs to each complete operation and is reused across
every backend call and reconciliation round. Initial/evaluate changes are empty.

`RuleCompiler::compile(fields, conditions, sources, &mut CompileBudget)` returns
`Arc<dyn RuleProgram>`. Sources are ordered by question/option conditions, flow
edges, price components, then visual slot/case conditions, each in declaration
order. Each `RuleSource` retains both its JSON Pointer and diagnostic rule ID.
Price and slot conditions retain their explicit definition IDs; other conditions
use `rule:` followed by the pointer. Multiple cases can share a diagnostic ID;
they cannot share a source pointer. The vector position is the `ConditionId`.

`RuleProgram::evaluate(&Facts, &[ConditionId], &mut CallBudget)` returns exactly
one `(ConditionId, bool)` per requested ID, in that order. Missing values are
represented by absence, not null answers or default scalars. Errors propagate.
`CompiledModel` owns normalized Product data, indices, ordered edges, transitive
successor sets, target order, sources and the shared program with prepaid cost
bounds. Its constructor checks storage consistency; public JSON compilation is
a separate operation. Condition lookup uses the source pointer, not the rule ID.

## C++17 values

The public namespace is `knot`; public headers contain only standard-library and
KNOT types. `Model` uniquely owns the C handle, is move-only and has noexcept
move/destruction. Its compile_file/initial/apply/evaluate methods preserve the
wire results, with Event::set_answer and clear/skip factories. Result, State,
Snapshot, Price, Flow, Visual and Changes own their strings/collections.

| Wire domain | C++ representation requirement |
|---|---|
| Integer/revision/amount/order | `std::int64_t`, validated safe-integer domain |
| Decimal answer | Exact decimal `std::string`, never `double` |
| Answer | Variant of string, int64 and vector of option-ID strings |
| State answers/skipped | Owning ID-to-Answer map and ordered vector of IDs |
| Nullable totalMinor | `std::optional<std::int64_t>`; unknown is null, not zero |
| Nullable currentQuestion/nextQuestionId | Explicit optional typed question/ID |
| Visual scalar/vector | Typed bool/string/double/array-of-three variant |
| Price/flow/change states | Closed enum matching the schema strings |
| Diagnostic ruleIds | Ordered vector of strings, preserving generated pointers |

`knot::Error` exposes status, code, path and ruleIds. JSON decode failures become
KNOT errors; no third-party JSON type or exception escapes. A local result guard
releases the original C result exactly once, including exceptional decode paths.
No-JSON INTERNAL errors still construct a minimal error. File I/O errors remain
distinguishable from kernel errors. The two static bridge variants bind to the
matching dynamic/static kernel and do not expose their private JSON dependency.

## CLI envelopes

The formal executable is supplied by integration, not by the P0 probe.
`validate <product>` returns the model-create result; `explain <product> --state
<state>` returns a TransitionResult. `inspect <product>` returns a versioned model
summary containing the model digest, question count, flow nodes/edges, rule count,
visual targets and price components from that same compiled model.

`simulate <product> --events <events>` consumes a bounded Event array. Each stdout
line is exactly `{"schemaVersion":"1.0","step":N,"result":R}`. Step 0 is the
initial result; step N>0 belongs to input event N-1. R is a complete
TransitionResult or the complete error envelope. Stop at the first error;
previous lines describe completed steps, not a transaction over the whole file.
There is no progress chatter on stdout. I/O/usage errors go to stderr.

Exit codes: 0 all operations succeeded; 2 usage/file error; 3 model/state/event
failure; 4 internal failure. Output-stream failure stops execution and returns a
nonzero code. No command downloads models, schemas, currencies or resources.
