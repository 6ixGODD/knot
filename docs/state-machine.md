# State and transitions

The State schema is [state.schema.json](../contracts/schemas/state.schema.json). Required fields are schemaVersion=1.0, productId, modelDigest, revision, answers and skipped. Answers are raw choices; price, flow and visual are derived outputs. Never edit identity or revision to bypass a check.

`initial` creates revision 0. `apply` checks Event expectedRevision against the supplied State before applying the event. A real normalized change increments revision once; a normalized no-op retains it, including at the maximum safe revision. A changing event at the maximum fails REVISION_OVERFLOW. Both initial and evaluate return empty Changes arrays and false change flags. `evaluate` validates and derives without changing or repairing State.

`set_answer` supplies a typed value. `clear_answer` is allowed only for an answered question in the valid prefix and removes its answer. `skip_answer` is allowed only for the current unanswered optional question. Re-answering a skipped question removes its skip marker. Optional does not mean automatically skipped. An optional multi `[]` is an explicit answer; required multi cannot be empty. Multi answers and skipped IDs use declaration order. Duplicates, unknown IDs, required skips and answers/skipped overlap are invalid.

Only the active/reached prefix is operable. Changing an upstream answer reconciles downstream answers: disabled answers are cleared with became_disabled; unreachable branches are cleared with branch_unreachable. It does not silently choose another option. The returned Changes explain changes; persist full State instead of replaying the explanation.

Flow status is in_progress, complete or blocked. A blocked result can carry a valid recoverable State even though snapshot.validation.valid is false; evaluate may reproduce blocked and an upstream edit may resolve it. A successful in_progress result can have validation.valid=true without being complete. Completion and validity are both required for final price.

At every stage, nonzero ABI status returns only an error envelope, not a partial state or snapshot. Keep your old State on failure. State restoration requires matching productId/modelDigest and canonical answers; invalid state is rejected rather than repaired. See [CAS and recovery](integration.md).
