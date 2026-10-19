# Standard model benchmark

The generator fixes 100 questions, 500 condition roots, 100 visual properties and 100 price components. It does not relax the shared call budget.

Generate inputs in an external output directory, then run the production CLI:

```text
python tests/benchmark/generate.py --out <output>
knot validate <output>/model.json
knot simulate <output>/model.json --events <output>/events.json
```

`tools/verify/benchmark.py --out <output>` builds the Release measurement executable and records latency samples, memory, Rust allocations, parallel throughput and tool versions. Set an absolute `CARGO_TARGET_DIR` before running it. Any resource rejection or incomplete sequence fails the gate; preserve its log instead of reducing the model or reporting partial measurements as success.
