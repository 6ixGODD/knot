# gRPC API reference

[简体中文](grpc-api.zh-CN.md) · [Service guide](grpc/README.md) · [Quickstart](grpc/quickstart.md) · [Language clients](grpc/clients.md)

`knot.v1.ConfigurationService` is a stateless product configuration service. All four RPCs are unary. Requests select an exact model and include the complete state where required. This page describes the network contract; the [browser demo](../examples/grpc/README.md) is documented separately.

## Which method to call

| RPC | Application use | State effect |
|---|---|---|
| Initial | Start a new configuration | Returns revision 0 and no answers; creates no server session |
| Apply | Submit, clear or skip an answer | Validates the event, reconciles dependent answers and returns complete new State/Snapshot |
| Evaluate | Restore saved configuration or rebuild its view | Validates State without repairing it or increasing revision; empty Changes |
| Quote | Initial display, successful answer update, question navigation or candidate preview | Complete-product range and candidate totals; no State mutation |

### Initial

Send only `model_digest`. The successful TransitionResult contains the `state` to save, `snapshot.flow` and `snapshot.visual` to render. Call Quote with that State for the complete-product range. Do not call Initial on every interaction; it starts over.

### Apply

Send `model_digest`, complete `state_json` and one `event_json`. Event `expectedRevision` must equal the supplied State revision. `set_answer` sets an answer; for multi questions send the complete option set, not one appended option. `clear_answer` removes an answer; `skip_answer` skips an eligible question. Exact fields are in the [Event Schema](../contracts/schemas/event.schema.json).

A valid change may clear later answers that depended on the old selection. Replace the entire returned State rather than merging only the clicked answer locally. A no-op does not increase revision. Domain errors provide no replacement State. Do not edit revision, answers or skipped fields to bypass Apply.

### Evaluate

Send complete saved State to rebuild its Snapshot; no Event is required. Evaluate does not repair invalid State, migrate models or increase revision. Preserve a saved configuration when restoration fails instead of silently overwriting it with Initial.

### Quote

Send outer `model_digest` and `quote_json`. The JSON contains `schemaVersion`, complete `state`, optional `focusQuestionId` and optional `candidateValue`. Omitted focus uses the engine's current question; explicit focus can inspect an earlier question without a navigation Event. Numeric/color previews require explicit focus and do not submit an answer. Success is a separate QuoteResult, not TransitionResult; full pricing semantics follow below.

## Calls and payloads

The wire contract is [configuration.proto](../contracts/grpc/knot/v1/configuration.proto). Generate a client from it or use the included Go example. Each method is unary; its full path is `/knot.v1.ConfigurationService/<method>`.

| Unary method | Request | Successful payload |
|---|---|---|
| Initial | `model_digest` | Complete initial TransitionResult |
| Apply | `model_digest`, `state_json`, `event_json` | Complete transition, including State, Snapshot and Changes |
| Evaluate | `model_digest`, `state_json` | `result_json`: recomputed transition without advancing revision |
| Quote | `model_digest`, `quote_json` | Independent `quote_json`: complete-product range and focus candidates |

All request fields have protobuf presence and are required by the service. `model_digest` is a string matching `sha256:[0-9a-f]{64}` and must select a model loaded at startup. `state_json` and `event_json` are bytes: pass raw UTF-8 JSON to a generated client, without double-encoding it as a JSON string or manually Base64-encoding it. Tools displaying [ProtoJSON](https://protobuf.dev/programming-guides/json/) represent bytes as Base64; that is a tooling representation, not another encoding inside the protobuf bytes.

The first Speaker answer is this complete Event:

<!-- schema: event expect: valid -->
```json
{"schemaVersion":"1.0","type":"set_answer","questionId":"speaker_type","value":"active","expectedRevision":0}
```

1. Call Initial with the deployed digest. Decode `result_json`, keep the entire `state`, and display `snapshot.flow.currentQuestion`.
2. Build an Event from the user's choice, using the returned State's `revision` as `expectedRevision`.
3. Call Apply with that same digest, the complete State and the Event. Keep JSON numbers exact; do not round-trip through a lossy floating-point map.
4. On success, replace State and Snapshot with the returned objects. Re-render the flow and Visual immediately, then call Quote for the displayed whole-product price. Changes explains what changed, but is not a replacement State.
5. To resume a saved configuration, call Evaluate with its original digest and complete State. It recomputes Snapshot without advancing revision or repairing invalid State.

Result JSON follows [result.schema.json](../contracts/schemas/result.schema.json); State and Event follow [state.schema.json](../contracts/schemas/state.schema.json) and [event.schema.json](../contracts/schemas/event.schema.json). A result contains `schemaVersion`, `state`, `snapshot`, `changes` and `diagnostics`. Prices are integer minor units: `282800` CNY is ¥2,828.00. `estimated` is the subtotal for answered items, not a completion quote; even `final` is not a payment authorization. Visual properties describe assets/colors/visibility; the service does not render images.

## Outcomes and persistence

Initial, Apply and Evaluate responses have exactly one outcome: `result_json` or `error_json`. Quote uses `quote_json` or `error_json`. The latter contains the complete `{schemaVersion,error}` domain envelope. gRPC OK does not mean business success. Reject a missing outcome or invalid JSON envelope; never save State from a failure.

Save the complete returned State only after a successful domain result. Use database CAS or a transaction in the calling application. `expectedRevision` checks the supplied State; the service stores no session and cannot enforce a database revision. Retry with the original inputs only under the caller's retry policy.

| Condition | gRPC result |
|---|---|
| Missing envelope field, malformed digest/protobuf | INVALID_ARGUMENT |
| Well-formed digest not deployed | NOT_FOUND |
| Network message or admission capacity exceeded | RESOURCE_EXHAUSTED |
| Request deadline exceeded | DEADLINE_EXCEEDED |
| Graceful drain | UNAVAILABLE |
| Internal failure or captured panic | INTERNAL; the process retires |
| Domain validation, revision, rule or resource error | OK with complete `error_json` |

Internal JSON limits remain 4 MiB Product, 512 KiB State and 64 KiB Event. Business protobuf messages are capped at 1 MiB inbound and 4 MiB + 1 KiB outbound. Deadlines do not interrupt running synchronous computation; the compute slot remains occupied until that work exits.

For mTLS, follow the [language client setup](grpc/clients.md#remote-connections-and-validation). Use a client identity and an endpoint hostname matching the server certificate SAN. The [deployment guide](deployment.md) covers server certificates, health and shutdown.

Standard `grpc.health.v1.Health` is available for the service name `knot.v1.ConfigurationService`. There is no reflection, REST, model upload, model listing or remote administrative API.


## Completion quotes

Send raw UTF-8 [QuoteRequest JSON](../contracts/schemas/quote-request.schema.json) in `QuoteRequest.quote_json`. Its fields are `schemaVersion`, full `state`, optional `focusQuestionId` and optional `candidateValue`. Explicit null is invalid. A candidate preview requires an explicit color/integer/decimal focus. The RPC returns a separate `QuoteResponse` oneof: `quote_json` or the existing `error_json` envelope. Validate the [QuoteResult schema](../contracts/schemas/quote-result.schema.json); reject empty/unknown outcomes.

`price` describes all legal complete configurations retaining current answers and skips. `range` has exact reachable `minMinor`/`maxMinor`; unfinished equal bounds are still a range. `final` means the input is complete and both bounds equal its final total. `no_completion` means no legal completion exists. `unavailable` means an exact result cannot be proved within the supported domain or search budget (`domain_limit`/`search_limit`). Both latter states have null amounts; never fall back to the legacy subtotal. Domain/model errors remain errors, not zero-priced or omitted branches.

`focus` contains the effective question, answer, skipped flag, option candidates and optional preview. Single candidates select that option. Multi candidates toggle it in the full current set. Each valid candidate's `price` is the **whole-product total after choosing it**, including reconciliation and remaining required choices; do not label it as a fixed surcharge. Disabled options have no price. An invalid candidate has a full `error` envelope; a valid candidate with no continuation has `price.status=no_completion`.

Quotes preserve input revision and do not authorize checkout. Bind displayed responses to modelDigest, revision, focus and a request sequence. Navigation changes only local focus; Apply is serialized and its Visual replaces the preview before Quote finishes. Aborting fetch does not prove the server stopped computing.

Limits: request 768 KiB, nested State 512 KiB, output 4 MiB, 256 explicit candidates, 10,000 search nodes and one shared 2,000,000-cost budget. Numeric domains over 10,000 legal grid points are unavailable. Colors use proven condition-equivalence classes. If the remaining budget cannot encode a complete response, the call returns `RESOURCE_LIMIT`.

For the unchanged Speaker model, the initial complete range is CNY 1,199–3,027; active/walnut is CNY 1,599–2,327. Active/aluminum/200W/LED completes at CNY 2,828. These figures are model examples, not client-side pricing rules.
