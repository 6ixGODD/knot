# Golden contract corpus

T01–T28 retain their original scenario numbers. Named `B-*` cases add protocol
boundaries. Each directory supplies `model.json`, `initial-state.json`,
`event.json`, `expected-result.json` and `case.json`. Null state/event files mean
that the selected operation does not take that argument. T01 has zero model
bytes; T27 contains deliberately invalid UTF-8; T28 contains truncated JSON.
Read these files as bytes, never normalize hostile input before the API call.

`case.json` names the operation, expected ABI status and intentional validation
failure phase. Success compares the entire result, ignoring only object key
order. Arrays, numbers, revisions, changes and rule IDs are exact. Error `message`
is human-readable and not a machine contract; check its type/nonempty bound and
compare every other error field exactly. No expected error contains a state.

The expected snapshots were explicitly authored from the protocol, not recorded
from a KNOT implementation. Schema tests check complete output shapes, typed
roundtrips, the bundled currency digest and RFC8785 model identities; they do not
execute these product operations. Full runtime and language parity execution
belongs to the integrated product tests. Independent review remains required.

Harness instructions in cases are requirements, not evidence of execution:
`roundtripState` serializes and reloads input before evaluating; `threads` and
`repetitions` require shared-model calls with isolated results (T22 alternates
with T03 on that same model); `bindings` identifies compared consumers;
`releaseOriginalBufferTwice` tests the API's clearing of the original descriptor,
never freeing a copied descriptor. A runner must fail unsupported operations or
unavailable bindings, rather than silently treating them as passed.

Review arithmetic: T12 = 129900 + 50000 + 90000 + 12900 = 282800.
T10 = 129900 + 30000 = 159900; panel color survives, power/extras disappear,
LED returns to false and the next question is power. T05 intentionally submits
multi IDs in reverse order; output uses declaration order and totals 302700.
T13 skips extras: the amount stays 269900 but price status changes to final.

Source identities for unnamed conditions are `rule:` plus their Product JSON
Pointer. In suffix cleanup without a deciding condition, `ruleIds` is empty;
branch switching identifies the condition selecting the former branch. Explicit
visual slot IDs remain intact in conflict and missing-value errors.
