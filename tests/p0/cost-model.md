# Candidate backend cost model

Units count bounded primitive work; they are not milliseconds, allocation bytes or
an operating-system sandbox. One caller-owned CallBudget has 2,000,000 units and
survives every coordination round. The adapter charges before building per-call
JSON/Variable inputs or polling ZEN. Failed charges do not replenish fuel.

| Template | Prepaid work | Dynamic bound / retained storage |
|---|---|---|
| presence, not, all, any | 16 per AST node | Each source AST depth <=32, <=32768 nodes total. Wide boolean groups are lowered as balanced binary expressions, avoiding a linear-depth backend tree. |
| eq/ne and ordered comparisons | 256 per scalar comparison plus node cost | IDs <=128 UTF-8 bytes; colors 7 bytes; integers <=16 decimal digits; decimals <=25 significant digits, scale <=9. ZEN compares bounded Decimal/string values. |
| between | both comparisons prepaid | No range enumeration; two scalar comparisons. |
| in | all members' scalar comparisons prepaid | <=256 constants; balanced disjunction, independent of short circuit. |
| contains | 65536 plus node cost | <=256 ID members, each <=128 bytes; ZEN's array branch uses a single `iter().any` with scalar equality. No substring, regex or dynamic value branch is generated. |
| graph / expression | 256 per graph + 8 per generated expression byte + 8 per serialized constant byte | Exactly 3 nodes and 2 edges per condition, <=4096 graphs. Each expression <=65536 bytes, combined <=8388608. Constants and bytecode are linear in bounded IR/source size. |
| Facts conversion / copies | per graph: 256 + 16 per missing value + 1024 per scalar + (16 + 1024 * members) per multi | <=256 fields. Every requested graph prepays full context conversion/copy, even for unread fields. No input object alias survives a call. Empty requests avoid conversion. |
| validation overhead | 256 per call | <=256 field shape checks; requested IDs <=4096 and validated before execution. |

The source paths audited in ZEN 2.1.3 are `src/decision.rs`,
`src/model/decision_content.rs`, `src/decision_graph/graph.rs`, and
`src/nodes/expression/mod.rs`; in zen-expression they are
`src/isolate.rs` and `src/functions/internal.rs` (array `contains`).
`GraphContent::compile` only visits the generated expression node for bytecode;
its function-node/type-stripper branch has no matching node. Graph execution
uses the default no-op loader and custom-node adapter. There is no function,
table, external schema, iteration node, callback or user expression in the
generated graph. Only the P0 test helper accepts an internal expression string.

The bound is on work and structural allocation counts, not allocator overhead or
process RSS. Cache/graph storage scales with bounded roots, nodes, constants and
expression bytes; temporary input storage scales with bounded facts/constants.
Concurrent calls each consume their own storage, and the host limits concurrency.
OOM/abort and arbitrary native pointer misuse are outside the catch-unwind guarantee.

Required evidence includes all operators/missing/exact decimals, a 2048-leaf
balanced tree, depth rejection, cumulative budget exhaustion, ordinary/Tokio and
C++ threads, mixed answers, and poisoning at the same boundary used by ZEN calls.
This backend cost evidence does not establish the full Core's flow/cleanup/sorting/output cost model.
