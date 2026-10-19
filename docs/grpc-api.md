# gRPC API

[简体中文](grpc-api.zh-CN.md)

Use `knot.v1.ConfigurationService` to drive a product configurator: ask the next question, accept an answer, and return the updated configuration, price and visual properties. The service is stateless and uses the same engine as the native SDK. Your application owns the UI, session storage and payment flow.

## Try a real configuration

### Interactive SVG configurator

The [browser example](../examples/grpc/speaker_web.py) renders four SVG layers (base, cabinet, panel and LED ring) through a small Go HTTP-to-gRPC bridge. Prices, available options, answer clearing and visual properties come from KNOT. This loopback-only bridge belongs to the example, not the server's public API.

With Docker Linux containers, Python 3.9+, Go 1.26.4 and the internal image loaded, run from the repository root:

```powershell
$image = Read-Host 'Local image tag with Quote support'
$out = Join-Path $env:TEMP ('knot-speaker-web-' + [guid]::NewGuid().ToString('N'))
python examples/grpc/speaker_web.py --image $image --out $out --port 8765
```

Open `http://127.0.0.1:8765`. Choose active, aluminum, a panel color, 200W and LED. Each selection calls Apply and updates the SVG immediately; Quote then supplies the complete-product price range and each option's whole-product price. Previous/next navigation preserves answers and revision. Changing aluminum to walnut clears incompatible answers and hides the LED ring. Reloading calls Evaluate, then Quote, with saved State. A failed restore leaves the stored configuration intact. Press Ctrl+C to stop the example and remove its container; logs remain in `$out`. Use a new output directory and a supplied image that includes Quote; older three-method images cannot serve this UI.

The UI and SVG sources are in [speaker-web](../examples/grpc/go/cmd/speaker-web/). Restart the launcher after editing to rebuild embedded assets. This demo uses localStorage and has no accounts, shared database, checkout or payment processing. Use non-sensitive demo data; production applications must provide session and authorization handling.

### Scripted scenarios

The [Speaker demo](../examples/grpc/speaker_demo.py) uses the existing [product model](../contracts/examples/speaker.json), a real container and the [generated Go client](../examples/grpc/go/main.go). It runs three purchase/edit scenarios plus a rejected option, and retains full responses so you can inspect the behavior.

Requirements: Docker with Linux containers, Python 3.9+ and Go 1.26.4 (see [go.mod](../examples/grpc/go/go.mod)). The first client build needs the Go modules; no Rust, protoc or native SDK is required. Import the internally supplied image with `docker load --input <archive>`. Use the tag reported by that command.

From the repository root in PowerShell:

```powershell
$image = Read-Host 'Imported local image tag'
$out = Join-Path $env:TEMP ('knot-speaker-' + [guid]::NewGuid().ToString('N'))
python examples/grpc/speaker_demo.py --image $image --out $out
Get-Content (Join-Path $out 'summary.json')
```

The script builds the Go client, mounts only `speaker.json`, starts a read-only container on an unused loopback port, checks health, runs the scenarios and removes its container. Output stays in `$out`; use a new directory for each run. Plaintext is explicitly enabled for this local demo. For a remote deployment, use [mTLS](deployment.md).

| Scenario | Choices and behavior | Expected result |
|---|---|---|
| `premium` | Active speaker → aluminum → blue panel → 200W → LED ring | CNY 2,828.00 (`282800` minor units), `final`, flow `complete`; LED visible |
| `change-material` | Start with that completed configuration, then change aluminum to walnut | CNY 1,599.00 (`159900`), `estimated`; clears `power` and `extras`, hides LED, asks `power` again |
| `passive` | Passive speaker → walnut → blue panel → banana connector | CNY 1,199.00 (`119900`), `final`, flow `complete`; no power or extras questions |
| `disabled-power` | Active speaker → walnut → blue panel → attempt 200W | `INVALID_OPTION`; the rejected request yields no replacement State |

This CLI scenario retains the legacy cumulative `snapshot.price`; `estimated` is a subtotal, not the complete-product quote displayed by the browser. The demo asserts these subtotals/final prices, revisions and flow states. Every successful scenario also calls Evaluate and checks that State is unchanged, then replays an old event and requires `REVISION_CONFLICT`.

Inspect `*.events.json` for inputs, `*.result.json` for the final result, and `*.trace.jsonl` for every Initial/Apply/Evaluate outcome. In `change-material.trace.jsonl`, the last successful Apply contains the automatic answer changes; Evaluate has empty Changes. The trace contains complete answers: keep it local and use it only with non-sensitive demo data.

To try another configuration, edit a generated event file and call the built client against your running server:

```powershell
& (Join-Path $out 'grpc-example.exe') --plaintext --trace `
  --endpoint 127.0.0.1:50051 `
  --digest sha256:73e5998863973011c590d9883bb0a593f62d77fca5d10480b4f0029153ed1541 `
  --events (Join-Path $out 'premium.events.json')
```

This second command requires a server you started separately; the demo's temporary server has already stopped. Use its actual endpoint. The digest above identifies the unchanged Speaker model; it is not a raw JSON-file checksum. Other deployed models need their own compiled digest from the matching model manifest. The client prints the final result to stdout and optional RPC traces to stderr. Its sample workflow starts a fresh session and tests an old-revision rejection; it is not a session persistence client.

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

For mTLS, omit `--plaintext` and provide `--ca`, `--cert`, `--key` and `--server-name` to the Go client. Use a real client identity and the server's verified SAN. The [deployment guide](deployment.md) covers server certificates, health and shutdown.

Standard `grpc.health.v1.Health` is available for the service name `knot.v1.ConfigurationService`. There is no reflection, REST, model upload, model listing or remote administrative API.


## Completion quotes

Send raw UTF-8 [QuoteRequest JSON](../contracts/schemas/quote-request.schema.json) in `QuoteRequest.quote_json`. Its fields are `schemaVersion`, full `state`, optional `focusQuestionId` and optional `candidateValue`. Explicit null is invalid. A candidate preview requires an explicit color/integer/decimal focus. The RPC returns a separate `QuoteResponse` oneof: `quote_json` or the existing `error_json` envelope. Validate the [QuoteResult schema](../contracts/schemas/quote-result.schema.json); reject empty/unknown outcomes.

`price` describes all legal complete configurations retaining current answers and skips. `range` has exact reachable `minMinor`/`maxMinor`; unfinished equal bounds are still a range. `final` means the input is complete and both bounds equal its final total. `no_completion` means no legal completion exists. `unavailable` means an exact result cannot be proved within the supported domain or search budget (`domain_limit`/`search_limit`). Both latter states have null amounts; never fall back to the legacy subtotal. Domain/model errors remain errors, not zero-priced or omitted branches.

`focus` contains the effective question, answer, skipped flag, option candidates and optional preview. Single candidates select that option. Multi candidates toggle it in the full current set. Each valid candidate's `price` is the **whole-product total after choosing it**, including reconciliation and remaining required choices; do not label it as a fixed surcharge. Disabled options have no price. An invalid candidate has a full `error` envelope; a valid candidate with no continuation has `price.status=no_completion`.

Quotes preserve input revision and do not authorize checkout. Bind displayed responses to modelDigest, revision, focus and a request sequence. Navigation changes only local focus; Apply is serialized and its Visual replaces the preview before Quote finishes. Aborting fetch does not prove the server stopped computing.

Limits: request 768 KiB, nested State 512 KiB, output 4 MiB, 256 explicit candidates, 10,000 search nodes and one shared 2,000,000-cost budget. Numeric domains over 10,000 legal grid points are unavailable. Colors use proven condition-equivalence classes. If the remaining budget cannot encode a complete response, the call returns `RESOURCE_LIMIT`.

For the unchanged Speaker model, the initial complete range is CNY 1,199–3,027; active/walnut is CNY 1,599–2,327. Active/aluminum/200W/LED completes at CNY 2,828. These figures are model examples, not client-side pricing rules.
