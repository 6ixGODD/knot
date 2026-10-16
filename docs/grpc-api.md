# gRPC API

The stateless `knot.v1.ConfigurationService` calls the same compiler and configuration core as the native SDK. See [configuration.proto](../contracts/grpc/knot/v1/configuration.proto) and [deployment](deployment.md).

| Unary method | Request | Successful `result_json` |
|---|---|---|
| Initial | `model_digest` | Complete initial TransitionResult |
| Apply | `model_digest`, `state_json`, `event_json` | Complete transition, including State, Snapshot and Changes |
| Evaluate | `model_digest`, `state_json` | Recomputed transition without advancing revision |

All request fields have protobuf presence and are required by the service. Send JSON as raw UTF-8 bytes, without an extra JSON string or Base64 layer. Parsing retains duplicate-key, unknown-field and exact-number checks. A digest must match `sha256:[0-9a-f]{64}` and select a model loaded at startup.

Every successful gRPC response has exactly one outcome: `result_json` or `error_json`. The latter contains the complete `{schemaVersion,error}` domain envelope. gRPC OK does not mean business success. Reject a missing outcome or invalid JSON envelope; never save State from a failure.

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

The [pure-Go example](../examples/grpc/go) uses generated protobuf code, mTLS and explicit outcome handling. It needs Go, but no cgo, native SDK, Rust or protoc at run time. Its `--digest` comes from the protocol package's `models/manifest.json`; `--events` accepts the complete packaged Speaker event array. It calls Initial, Apply and Evaluate, then checks a revision-conflict error branch.

Standard `grpc.health.v1.Health` is available for the service name `knot.v1.ConfigurationService`. There is no reflection, REST, model upload, model listing or remote administrative API.
