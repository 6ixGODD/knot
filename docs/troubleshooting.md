# Troubleshooting

Use error code, path and ruleIds, not message text. Paths are JSON Pointers (escape `~` as `~0`, `/` as `~1`). Native error status is nonzero and never carries a replacement State. The [error catalog](../contracts/errors.json) defines status and details.

| Symptom | Code | Cause | One action |
|---|---|---|---|
| 200W option disabled | INVALID_OPTION | current upstream material does not allow it | choose an enabled power or change material first |
| stale edit | REVISION_CONFLICT | event revision differs from supplied State | reload stored State and rebuild the event |
| restore fails after product update | MODEL_MISMATCH | modelDigest differs | restore with the original model or start a new configuration |
| input or work budget too large | RESOURCE_LIMIT | byte/count/depth/cost cap exceeded | inspect details.limit and reduce that input dimension |
| no progress after optional question | none | optional was neither answered nor skipped | send skip_answer with current revision |
| price has no number | none, status unknown | pricing is not resolved | display unknown and continue configuration |
| inactive question edit | INACTIVE_QUESTION | question is outside reachable prefix | edit the current or a reached upstream question |
| wrong numeric representation | BAD_SCHEMA or INVALID_ANSWER | decimal passed as number or integer outside safe domain | send a bounded decimal string or safe integer as required |
| missing visual value source | MISSING_VALUE | a matching visual case references an absent answer | answer the source question before activating that case |
| multiple matching cases in one visual slot | VISUAL_CONFLICT | more than one case condition is true | make that slot's case conditions mutually exclusive |
| multiple slots declare the same target/property | INVALID_MODEL | duplicate writers are invalid even with exclusive conditions | combine the cases into one slot for that target/property |
| backend failure or empty error JSON | INTERNAL | native failure; model may be invalidated | release and rebuild the model, retaining diagnostic metadata |
| DLL load failure | host loader error | wrong architecture, missing file or dependency | load the matching package from an explicit absolute path |
| unresolved imports in static build | linker error | import bridge used with static kernel | link only Knot::CppStatic |

C++ file access errors are filesystem exceptions, distinct from knot::Error. Python file errors remain OSError. A bridge decode error indicates an incompatible or malformed native response, not a business success. Do not retry indefinitely on INTERNAL. See [limits](limits.md) and [integration](integration.md).
