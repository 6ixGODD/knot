# Pricing

Use integer minor currency units from the bundled [fixed currency table](../contracts/currency/minor-units.json). Amounts, revisions and integer answers are within ±9007199254740991 (revision and totals are nonnegative). Do not convert them to floating point. Currency upgrades affect model identity; no currency data is fetched at runtime.

Each component is either fixed `amountMinor`, or `unitMinor` times an exact numeric answer selected by `quantityFrom`. The category is base or adjustment. Conditions determine active components. A matching component with a missing quantity remains unresolved: while configuration is incomplete the price is unknown, and a complete configuration with missing required quantity fails UNPRICED. Missing is never zero; negative quantities are rejected. Exactly one applicable base is needed for a known price; multiple bases conflict. The result lists effective components in declaration order and totalMinor equals their sum.

Quantity multiplication uses exact decimal arithmetic and rounds each component to minor units, ties away from zero: +0.5 → +1 and -0.5 → -1. Bounds are checked; overflow is NUMERIC_OVERFLOW, a negative final total is NEGATIVE_TOTAL. Decimal strings do not accept exponent notation or implicit rounding to question scale.

Price status is unknown, estimated or final. Unknown has totalMinor=null and no components; it is never represented as zero. Estimated may have a known subtotal while configuration is incomplete. Final requires complete, valid configuration and resolved price, including explicitly answered/skipped optional questions. Final is not a charge or payment authorization.

The Speaker contract expects 282800 after active/aluminum/#315A79/200w/[led], and 159900 after material changes to walnut and invalid downstream answers are removed. These are [contract expectations](../contracts/golden/T12/expected-result.json); full production SDK execution remains an integration gate.
