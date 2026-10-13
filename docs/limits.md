# Hard limits

These values are generated from [limits.json](../contracts/limits.json). Units are bytes unless named as counts, depth or cost. Apply the limit before allocating or expanding the corresponding input. Counts within their individual maxima can still exceed the shared call cost budget.

| Limit | Maximum |
|---|---:|
| `productBytes` | 4194304 |
| `stateBytes` | 524288 |
| `eventBytes` | 65536 |
| `jsonDepth` | 64 |
| `stringBytes` | 16384 |
| `idBytes` | 128 |
| `numberTokenBytes` | 128 |
| `exponentMagnitude` | 308 |
| `questions` | 256 |
| `optionsPerQuestion` | 256 |
| `optionsTotal` | 8192 |
| `edges` | 2048 |
| `conditionRoots` | 4096 |
| `astNodes` | 32768 |
| `astDepth` | 32 |
| `inMembers` | 256 |
| `priceComponents` | 1024 |
| `visualTargets` | 256 |
| `visualSlots` | 2048 |
| `visualCases` | 4096 |
| `expressionBytes` | 65536 |
| `expressionsTotalBytes` | 8388608 |
| `callCost` | 2000000 |
| `outputBytes` | 4194304 |
| `diagnostics` | 1024 |
| `simulateEvents` | 10000 |
| `simulateBytes` | 16777216 |

Numbers are at most 128 source-token bytes; exponent magnitude is at most 308. Integer domains have absolute maximum 9007199254740991; decimal scale is 0–9 and absolute value is bounded by the same maximum. IDs are at most 128 UTF-8 bytes with no control characters. JSON is UTF-8, rejects duplicate keys, malformed Unicode, trailing content and unknown fields. Non-NULL empty input is BAD_JSON; required NULL pointers are NULL_ARGUMENT.

RESOURCE_LIMIT details include limit and maximum, and may include observed. Arithmetic errors are NUMERIC_OVERFLOW; revision overflow is REVISION_OVERFLOW. Reducing one count does not waive another cap. A byte/cost budget is not a guarantee against process-wide OOM or abort. [Troubleshooting](troubleshooting.md) provides actions.
