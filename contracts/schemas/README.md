# Protocol schemas

The six documents use JSON Schema Draft 2020-12 and local fragment references.
`knot-schema` performs bounded lexical parsing before validation: duplicate keys,
invalid Unicode, excessive decoded UTF-8 length, depth and number-token limits are
not delegated to an ordinary JSON decoder.

The validator enables format assertions and registers `knot-id` (nonempty, at most
128 UTF-8 bytes, no control characters) and `knot-decimal` (the exact bounded
decimal domain). JSON Schema's `maxLength` counts characters, not UTF-8 bytes;
external validators must register these formats or separately enforce those
protocol constraints. Schema validation alone does not validate model references,
flow topology, state identity or question-specific answer semantics.

Use `cargo run -p knot-schema --example validate_json --locked -- --schema PATH
--input PATH` for the repository's offline validator. It never retrieves external
schemas. Defaults in schemas are annotations; normalization belongs to compilation.

Validate these resources with the contract and Golden checks before changing them.
