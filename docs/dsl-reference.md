# Product DSL 1.0

[简体中文：模型规范与编写指南](dsl-reference.zh-CN.md)

The normative structural contract is [product.schema.json](../contracts/schemas/product.schema.json); the full [Speaker model](../contracts/examples/speaker.json) is the main example. References resolve within packaged schemas. Defaults are annotations; compilation applies only the declared defaults. Missing required fields, unknown fields, duplicate IDs/keys, cycles and unresolved references are rejected.

A small complete structural example:

<!-- schema: product expect: valid -->
```json
{
  "schemaVersion": "1.0",
  "productId": "example",
  "productVersion": "1",
  "questions": [
    {
      "id": "q",
      "type": "integer",
      "label": "Quantity",
      "min": 1,
      "max": 10,
      "step": 1
    }
  ],
  "flow": {
    "start": "q",
    "edges": [
      {
        "from": "q",
        "to": "END"
      }
    ]
  },
  "pricing": {
    "currency": "USD",
    "components": [
      {
        "id": "base",
        "category": "base",
        "amountMinor": 100
      }
    ]
  },
  "visual": {
    "targets": [],
    "slots": []
  }
}
```

Counterexample: an empty object omits required identity and sections.

<!-- schema: product expect: invalid -->
```json
{}
```

Event versions are mandatory, including in language clients:

<!-- schema: event expect: valid -->
```json
{"schemaVersion":"1.0","type":"set_answer","questionId":"q","value":2,"expectedRevision":0}
```

Null is not an answer:

<!-- schema: event expect: invalid -->
```json
{"schemaVersion":"1.0","type":"set_answer","questionId":"q","value":null,"expectedRevision":0}
```

IDs are case-sensitive, nonempty and at most 128 UTF-8 bytes without controls. END is reserved for flow termination. required defaults to true; target order defaults to zero. Other absent optional numeric bounds are unbounded within the global numeric domain, not zero. No required field receives a guessed default. Metadata is descriptive bounded JSON; it does not execute rules.

Question types: single uses one declared option ID; multi uses a unique option-ID array normalized to declaration order; integer is an exact safe integer; decimal is an exact string without exponent/plus/whitespace, normalized before scale checks; color is #RRGGBB normalized uppercase. Min/max/step must be compatible, step positive with anchor min or zero. Decimal scale is 0–9. `1.0` may normalize to integer 1; `9007199254740991.1` is never a safe integer. Optional empty multi is an answer; required empty multi is invalid.

Conditions are exactly one of nonempty all/any, not, or field/op/value. eq/ne use same-type scalars (not multi); gt/gte/lt/lte and closed between use numeric operands; in has a nonempty same-type constant array; contains tests a multi option ID; exists has no value. Missing answers make ordinary comparisons false, including ne. Thus not(eq(missing,x)) differs from ne(missing,x); skipped is missing. References must resolve to declared questions/options. There are no executable source strings.

Flow edges use declared question IDs or END and preserve declaration order. Price components, visual targets, slots and operands also meet compiler semantic references and types. See [pricing](pricing.md), [visual IR](visual-ir.md), [state transitions](state-machine.md) and [limits](limits.md). Bindings do not execute rules.

The table is generated from the frozen Schema. `none` means no Schema default. Union tags identify alternatives; `$defs` are included local definitions. Bounds are structural, supplemented by the semantic rules above.

| Field / variant | Type / values | Required | Default | Bounds / reference |
|---|---|---|---|---|
| `Product.schemaVersion` | "1.0" | yes | none | {} |
| `Product.productId` | "string" | yes | none | {"minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$","format":"knot-id"} |
| `Product.productVersion` | "string" | yes | none | {"minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$","format":"knot-id"} |
| `Product.questions` | "array" | yes | none | {"minItems":1,"maxItems":256} |
| `Product.flow` | "object" | yes | none | {} |
| `Product.flow.start` | "string" | yes | none | {"minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$","format":"knot-id"} |
| `Product.flow.edges` | "array" | yes | none | {"minItems":1,"maxItems":2048} |
| `Product.flow.edges[].from` | "string" | yes | none | {"minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$","format":"knot-id"} |
| `Product.flow.edges[].to` | "string" | yes | none | {"minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$","format":"knot-id"} |
| `Product.flow.edges[].when` | "reference" | no | none | {"$ref":"#/$defs/condition"} |
| `Product.pricing` | "object" | yes | none | {} |
| `Product.pricing.currency` | "string" | yes | none | {"pattern":"^[A-Z]{3}$"} |
| `Product.pricing.components` | "array" | yes | none | {"minItems":0,"maxItems":1024} |
| `Product.pricing.components[][1].id` | "string" | yes | none | {"minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$","format":"knot-id"} |
| `Product.pricing.components[][1].category` | ["base","adjustment"] | yes | none | {} |
| `Product.pricing.components[][1].amountMinor` | "integer" | yes | none | {"minimum":-9007199254740991,"maximum":9007199254740991} |
| `Product.pricing.components[][1].when` | "reference" | no | none | {"$ref":"#/$defs/condition"} |
| `Product.pricing.components[][2].id` | "string" | yes | none | {"minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$","format":"knot-id"} |
| `Product.pricing.components[][2].category` | ["base","adjustment"] | yes | none | {} |
| `Product.pricing.components[][2].unitMinor` | "integer" | yes | none | {"minimum":-9007199254740991,"maximum":9007199254740991} |
| `Product.pricing.components[][2].quantityFrom` | "string" | yes | none | {"minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$","format":"knot-id"} |
| `Product.pricing.components[][2].when` | "reference" | no | none | {"$ref":"#/$defs/condition"} |
| `Product.visual` | "object" | yes | none | {} |
| `Product.visual.targets` | "array" | yes | none | {"minItems":0,"maxItems":256} |
| `Product.visual.targets[].id` | "string" | yes | none | {"minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$","format":"knot-id"} |
| `Product.visual.targets[].order` | "integer" | no | 0 | {"minimum":-9007199254740991,"maximum":9007199254740991} |
| `Product.visual.targets[].defaults` | "reference" | yes | none | {"$ref":"#/$defs/properties"} |
| `Product.visual.slots` | "array" | yes | none | {"minItems":0,"maxItems":2048} |
| `Product.visual.slots[][1].id` | "string" | yes | none | {"minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$","format":"knot-id"} |
| `Product.visual.slots[][1].target` | "string" | yes | none | {"minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$","format":"knot-id"} |
| `Product.visual.slots[][1].property` | "visible" | yes | none | {} |
| `Product.visual.slots[][1].cases` | "array" | yes | none | {"minItems":0,"maxItems":4096} |
| `Product.visual.slots[][1].cases[][1].when` | "reference" | yes | none | {"$ref":"#/$defs/condition"} |
| `Product.visual.slots[][1].cases[][1].value` | "boolean" | yes | none | {} |
| `Product.visual.slots[][1].cases[][2].when` | "reference" | yes | none | {"$ref":"#/$defs/condition"} |
| `Product.visual.slots[][1].cases[][2].valueFrom` | "string" | yes | none | {"minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$","format":"knot-id"} |
| `Product.visual.slots[][2].id` | "string" | yes | none | {"minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$","format":"knot-id"} |
| `Product.visual.slots[][2].target` | "string" | yes | none | {"minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$","format":"knot-id"} |
| `Product.visual.slots[][2].property` | "variant" | yes | none | {} |
| `Product.visual.slots[][2].cases` | "array" | yes | none | {"minItems":0,"maxItems":4096} |
| `Product.visual.slots[][2].cases[][1].when` | "reference" | yes | none | {"$ref":"#/$defs/condition"} |
| `Product.visual.slots[][2].cases[][1].value` | "string" | yes | none | {"minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$","format":"knot-id"} |
| `Product.visual.slots[][2].cases[][2].when` | "reference" | yes | none | {"$ref":"#/$defs/condition"} |
| `Product.visual.slots[][2].cases[][2].valueFrom` | "string" | yes | none | {"minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$","format":"knot-id"} |
| `Product.visual.slots[][3].id` | "string" | yes | none | {"minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$","format":"knot-id"} |
| `Product.visual.slots[][3].target` | "string" | yes | none | {"minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$","format":"knot-id"} |
| `Product.visual.slots[][3].property` | "assetRef" | yes | none | {} |
| `Product.visual.slots[][3].cases` | "array" | yes | none | {"minItems":0,"maxItems":4096} |
| `Product.visual.slots[][3].cases[][1].when` | "reference" | yes | none | {"$ref":"#/$defs/condition"} |
| `Product.visual.slots[][3].cases[][1].value` | "string" | yes | none | {"minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$","format":"knot-id"} |
| `Product.visual.slots[][3].cases[][2].when` | "reference" | yes | none | {"$ref":"#/$defs/condition"} |
| `Product.visual.slots[][3].cases[][2].valueFrom` | "string" | yes | none | {"minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$","format":"knot-id"} |
| `Product.visual.slots[][4].id` | "string" | yes | none | {"minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$","format":"knot-id"} |
| `Product.visual.slots[][4].target` | "string" | yes | none | {"minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$","format":"knot-id"} |
| `Product.visual.slots[][4].property` | "color" | yes | none | {} |
| `Product.visual.slots[][4].cases` | "array" | yes | none | {"minItems":0,"maxItems":4096} |
| `Product.visual.slots[][4].cases[][1].when` | "reference" | yes | none | {"$ref":"#/$defs/condition"} |
| `Product.visual.slots[][4].cases[][1].value` | "string" | yes | none | {"pattern":"^#[0-9a-fA-F]{6}$"} |
| `Product.visual.slots[][4].cases[][2].when` | "reference" | yes | none | {"$ref":"#/$defs/condition"} |
| `Product.visual.slots[][4].cases[][2].valueFrom` | "string" | yes | none | {"minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$","format":"knot-id"} |
| `Product.visual.slots[][5].id` | "string" | yes | none | {"minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$","format":"knot-id"} |
| `Product.visual.slots[][5].target` | "string" | yes | none | {"minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$","format":"knot-id"} |
| `Product.visual.slots[][5].property` | "opacity" | yes | none | {} |
| `Product.visual.slots[][5].cases` | "array" | yes | none | {"minItems":0,"maxItems":4096} |
| `Product.visual.slots[][5].cases[][1].when` | "reference" | yes | none | {"$ref":"#/$defs/condition"} |
| `Product.visual.slots[][5].cases[][1].value` | "number" | yes | none | {"minimum":0,"maximum":1} |
| `Product.visual.slots[][5].cases[][2].when` | "reference" | yes | none | {"$ref":"#/$defs/condition"} |
| `Product.visual.slots[][5].cases[][2].valueFrom` | "string" | yes | none | {"minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$","format":"knot-id"} |
| `Product.visual.slots[][6].id` | "string" | yes | none | {"minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$","format":"knot-id"} |
| `Product.visual.slots[][6].target` | "string" | yes | none | {"minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$","format":"knot-id"} |
| `Product.visual.slots[][6].property` | "surface.metallic" | yes | none | {} |
| `Product.visual.slots[][6].cases` | "array" | yes | none | {"minItems":0,"maxItems":4096} |
| `Product.visual.slots[][6].cases[][1].when` | "reference" | yes | none | {"$ref":"#/$defs/condition"} |
| `Product.visual.slots[][6].cases[][1].value` | "number" | yes | none | {"minimum":0,"maximum":1} |
| `Product.visual.slots[][6].cases[][2].when` | "reference" | yes | none | {"$ref":"#/$defs/condition"} |
| `Product.visual.slots[][6].cases[][2].valueFrom` | "string" | yes | none | {"minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$","format":"knot-id"} |
| `Product.visual.slots[][7].id` | "string" | yes | none | {"minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$","format":"knot-id"} |
| `Product.visual.slots[][7].target` | "string" | yes | none | {"minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$","format":"knot-id"} |
| `Product.visual.slots[][7].property` | "surface.roughness" | yes | none | {} |
| `Product.visual.slots[][7].cases` | "array" | yes | none | {"minItems":0,"maxItems":4096} |
| `Product.visual.slots[][7].cases[][1].when` | "reference" | yes | none | {"$ref":"#/$defs/condition"} |
| `Product.visual.slots[][7].cases[][1].value` | "number" | yes | none | {"minimum":0,"maximum":1} |
| `Product.visual.slots[][7].cases[][2].when` | "reference" | yes | none | {"$ref":"#/$defs/condition"} |
| `Product.visual.slots[][7].cases[][2].valueFrom` | "string" | yes | none | {"minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$","format":"knot-id"} |
| `Product.visual.slots[][8].id` | "string" | yes | none | {"minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$","format":"knot-id"} |
| `Product.visual.slots[][8].target` | "string" | yes | none | {"minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$","format":"knot-id"} |
| `Product.visual.slots[][8].property` | "surface.textureRef" | yes | none | {} |
| `Product.visual.slots[][8].cases` | "array" | yes | none | {"minItems":0,"maxItems":4096} |
| `Product.visual.slots[][8].cases[][1].when` | "reference" | yes | none | {"$ref":"#/$defs/condition"} |
| `Product.visual.slots[][8].cases[][1].value` | "string" | yes | none | {"minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$","format":"knot-id"} |
| `Product.visual.slots[][8].cases[][2].when` | "reference" | yes | none | {"$ref":"#/$defs/condition"} |
| `Product.visual.slots[][8].cases[][2].valueFrom` | "string" | yes | none | {"minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$","format":"knot-id"} |
| `Product.visual.slots[][9].id` | "string" | yes | none | {"minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$","format":"knot-id"} |
| `Product.visual.slots[][9].target` | "string" | yes | none | {"minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$","format":"knot-id"} |
| `Product.visual.slots[][9].property` | "emission.color" | yes | none | {} |
| `Product.visual.slots[][9].cases` | "array" | yes | none | {"minItems":0,"maxItems":4096} |
| `Product.visual.slots[][9].cases[][1].when` | "reference" | yes | none | {"$ref":"#/$defs/condition"} |
| `Product.visual.slots[][9].cases[][1].value` | "string" | yes | none | {"pattern":"^#[0-9a-fA-F]{6}$"} |
| `Product.visual.slots[][9].cases[][2].when` | "reference" | yes | none | {"$ref":"#/$defs/condition"} |
| `Product.visual.slots[][9].cases[][2].valueFrom` | "string" | yes | none | {"minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$","format":"knot-id"} |
| `Product.visual.slots[][10].id` | "string" | yes | none | {"minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$","format":"knot-id"} |
| `Product.visual.slots[][10].target` | "string" | yes | none | {"minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$","format":"knot-id"} |
| `Product.visual.slots[][10].property` | "emission.strength" | yes | none | {} |
| `Product.visual.slots[][10].cases` | "array" | yes | none | {"minItems":0,"maxItems":4096} |
| `Product.visual.slots[][10].cases[][1].when` | "reference" | yes | none | {"$ref":"#/$defs/condition"} |
| `Product.visual.slots[][10].cases[][1].value` | "number" | yes | none | {"minimum":0,"maximum":1} |
| `Product.visual.slots[][10].cases[][2].when` | "reference" | yes | none | {"$ref":"#/$defs/condition"} |
| `Product.visual.slots[][10].cases[][2].valueFrom` | "string" | yes | none | {"minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$","format":"knot-id"} |
| `Product.visual.slots[][11].id` | "string" | yes | none | {"minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$","format":"knot-id"} |
| `Product.visual.slots[][11].target` | "string" | yes | none | {"minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$","format":"knot-id"} |
| `Product.visual.slots[][11].property` | "transform.position" | yes | none | {} |
| `Product.visual.slots[][11].cases` | "array" | yes | none | {"minItems":0,"maxItems":4096} |
| `Product.visual.slots[][11].cases[][1].when` | "reference" | yes | none | {"$ref":"#/$defs/condition"} |
| `Product.visual.slots[][11].cases[][1].value` | "array" | yes | none | {"minItems":3,"maxItems":3} |
| `Product.visual.slots[][11].cases[][2].when` | "reference" | yes | none | {"$ref":"#/$defs/condition"} |
| `Product.visual.slots[][11].cases[][2].valueFrom` | "string" | yes | none | {"minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$","format":"knot-id"} |
| `Product.visual.slots[][12].id` | "string" | yes | none | {"minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$","format":"knot-id"} |
| `Product.visual.slots[][12].target` | "string" | yes | none | {"minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$","format":"knot-id"} |
| `Product.visual.slots[][12].property` | "transform.scale" | yes | none | {} |
| `Product.visual.slots[][12].cases` | "array" | yes | none | {"minItems":0,"maxItems":4096} |
| `Product.visual.slots[][12].cases[][1].when` | "reference" | yes | none | {"$ref":"#/$defs/condition"} |
| `Product.visual.slots[][12].cases[][1].value` | "array" | yes | none | {"minItems":3,"maxItems":3} |
| `Product.visual.slots[][12].cases[][2].when` | "reference" | yes | none | {"$ref":"#/$defs/condition"} |
| `Product.visual.slots[][12].cases[][2].valueFrom` | "string" | yes | none | {"minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$","format":"knot-id"} |
| `Product.visual.slots[][13].id` | "string" | yes | none | {"minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$","format":"knot-id"} |
| `Product.visual.slots[][13].target` | "string" | yes | none | {"minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$","format":"knot-id"} |
| `Product.visual.slots[][13].property` | "light.intensity" | yes | none | {} |
| `Product.visual.slots[][13].cases` | "array" | yes | none | {"minItems":0,"maxItems":4096} |
| `Product.visual.slots[][13].cases[][1].when` | "reference" | yes | none | {"$ref":"#/$defs/condition"} |
| `Product.visual.slots[][13].cases[][1].value` | "number" | yes | none | {"minimum":0,"maximum":1} |
| `Product.visual.slots[][13].cases[][2].when` | "reference" | yes | none | {"$ref":"#/$defs/condition"} |
| `Product.visual.slots[][13].cases[][2].valueFrom` | "string" | yes | none | {"minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$","format":"knot-id"} |
| `Product.metadata` | "object" | no | none | {} |
| `$defs.condition[1].all` | "array" | yes | none | {"minItems":1,"maxItems":32768} |
| `$defs.condition[2].any` | "array" | yes | none | {"minItems":1,"maxItems":32768} |
| `$defs.condition[3].not` | "reference" | yes | none | {"$ref":"#/$defs/condition"} |
| `$defs.condition[4].field` | "string" | yes | none | {"minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$","format":"knot-id"} |
| `$defs.condition[4].op` | "eq" | yes | none | {} |
| `$defs.condition[4].value` | "union" | yes | none | {} |
| `$defs.condition[5].field` | "string" | yes | none | {"minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$","format":"knot-id"} |
| `$defs.condition[5].op` | "ne" | yes | none | {} |
| `$defs.condition[5].value` | "union" | yes | none | {} |
| `$defs.condition[6].field` | "string" | yes | none | {"minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$","format":"knot-id"} |
| `$defs.condition[6].op` | "gt" | yes | none | {} |
| `$defs.condition[6].value` | "union" | yes | none | {} |
| `$defs.condition[7].field` | "string" | yes | none | {"minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$","format":"knot-id"} |
| `$defs.condition[7].op` | "gte" | yes | none | {} |
| `$defs.condition[7].value` | "union" | yes | none | {} |
| `$defs.condition[8].field` | "string" | yes | none | {"minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$","format":"knot-id"} |
| `$defs.condition[8].op` | "lt" | yes | none | {} |
| `$defs.condition[8].value` | "union" | yes | none | {} |
| `$defs.condition[9].field` | "string" | yes | none | {"minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$","format":"knot-id"} |
| `$defs.condition[9].op` | "lte" | yes | none | {} |
| `$defs.condition[9].value` | "union" | yes | none | {} |
| `$defs.condition[10].field` | "string" | yes | none | {"minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$","format":"knot-id"} |
| `$defs.condition[10].op` | "between" | yes | none | {} |
| `$defs.condition[10].value` | "array" | yes | none | {"minItems":2,"maxItems":2} |
| `$defs.condition[11].field` | "string" | yes | none | {"minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$","format":"knot-id"} |
| `$defs.condition[11].op` | "in" | yes | none | {} |
| `$defs.condition[11].value` | "array" | yes | none | {"minItems":1,"maxItems":256} |
| `$defs.condition[12].field` | "string" | yes | none | {"minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$","format":"knot-id"} |
| `$defs.condition[12].op` | "contains" | yes | none | {} |
| `$defs.condition[12].value` | "string" | yes | none | {"minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$","format":"knot-id"} |
| `$defs.condition[13].field` | "string" | yes | none | {"minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$","format":"knot-id"} |
| `$defs.condition[13].op` | "exists" | yes | none | {} |
| `$defs.question[single].id` | "string" | yes | none | {"minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$","format":"knot-id"} |
| `$defs.question[single].type` | "single" | yes | none | {} |
| `$defs.question[single].label` | "string" | yes | none | {"maxLength":16384} |
| `$defs.question[single].required` | "boolean" | no | true | {} |
| `$defs.question[single].options` | "array" | yes | none | {"minItems":1,"maxItems":256} |
| `$defs.question[single].options[].id` | "string" | yes | none | {"minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$","format":"knot-id"} |
| `$defs.question[single].options[].label` | "string" | yes | none | {"maxLength":16384} |
| `$defs.question[single].options[].enabledWhen` | "reference" | no | none | {"$ref":"#/$defs/condition"} |
| `$defs.question[multi].id` | "string" | yes | none | {"minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$","format":"knot-id"} |
| `$defs.question[multi].type` | "multi" | yes | none | {} |
| `$defs.question[multi].label` | "string" | yes | none | {"maxLength":16384} |
| `$defs.question[multi].required` | "boolean" | no | true | {} |
| `$defs.question[multi].options` | "array" | yes | none | {"minItems":1,"maxItems":256} |
| `$defs.question[multi].options[].id` | "string" | yes | none | {"minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$","format":"knot-id"} |
| `$defs.question[multi].options[].label` | "string" | yes | none | {"maxLength":16384} |
| `$defs.question[multi].options[].enabledWhen` | "reference" | no | none | {"$ref":"#/$defs/condition"} |
| `$defs.question[integer].id` | "string" | yes | none | {"minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$","format":"knot-id"} |
| `$defs.question[integer].type` | "integer" | yes | none | {} |
| `$defs.question[integer].label` | "string" | yes | none | {"maxLength":16384} |
| `$defs.question[integer].required` | "boolean" | no | true | {} |
| `$defs.question[integer].min` | "integer" | no | none | {"minimum":-9007199254740991,"maximum":9007199254740991} |
| `$defs.question[integer].max` | "integer" | no | none | {"minimum":-9007199254740991,"maximum":9007199254740991} |
| `$defs.question[integer].step` | "integer" | no | none | {"minimum":-9007199254740991,"maximum":9007199254740991} |
| `$defs.question[decimal].id` | "string" | yes | none | {"minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$","format":"knot-id"} |
| `$defs.question[decimal].type` | "decimal" | yes | none | {} |
| `$defs.question[decimal].label` | "string" | yes | none | {"maxLength":16384} |
| `$defs.question[decimal].required` | "boolean" | no | true | {} |
| `$defs.question[decimal].min` | "string" | no | none | {"maxLength":128,"pattern":"^-?[0-9]+(?:\\.[0-9]+)?$","format":"knot-decimal"} |
| `$defs.question[decimal].max` | "string" | no | none | {"maxLength":128,"pattern":"^-?[0-9]+(?:\\.[0-9]+)?$","format":"knot-decimal"} |
| `$defs.question[decimal].step` | "string" | no | none | {"maxLength":128,"pattern":"^-?[0-9]+(?:\\.[0-9]+)?$","format":"knot-decimal"} |
| `$defs.question[decimal].scale` | "integer" | yes | none | {"minimum":0,"maximum":9} |
| `$defs.question[color].id` | "string" | yes | none | {"minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$","format":"knot-id"} |
| `$defs.question[color].type` | "color" | yes | none | {} |
| `$defs.question[color].label` | "string" | yes | none | {"maxLength":16384} |
| `$defs.question[color].required` | "boolean" | no | true | {} |
| `$defs.properties.visible` | "boolean" | no | none | {} |
| `$defs.properties.variant` | "string" | no | none | {"minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$","format":"knot-id"} |
| `$defs.properties.assetRef` | "string" | no | none | {"minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$","format":"knot-id"} |
| `$defs.properties.color` | "string" | no | none | {"pattern":"^#[0-9a-fA-F]{6}$"} |
| `$defs.properties.opacity` | "number" | no | none | {"minimum":0,"maximum":1} |
| `$defs.properties.surface.metallic` | "number" | no | none | {"minimum":0,"maximum":1} |
| `$defs.properties.surface.roughness` | "number" | no | none | {"minimum":0,"maximum":1} |
| `$defs.properties.surface.textureRef` | "string" | no | none | {"minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$","format":"knot-id"} |
| `$defs.properties.emission.color` | "string" | no | none | {"pattern":"^#[0-9a-fA-F]{6}$"} |
| `$defs.properties.emission.strength` | "number" | no | none | {"minimum":0,"maximum":1} |
| `$defs.properties.transform.position` | "array" | no | none | {"minItems":3,"maxItems":3} |
| `$defs.properties.transform.scale` | "array" | no | none | {"minItems":3,"maxItems":3} |
| `$defs.properties.light.intensity` | "number" | no | none | {"minimum":0,"maximum":1} |
| `$defs.visual.targets` | "array" | yes | none | {"minItems":0,"maxItems":256} |
| `$defs.visual.targets[].id` | "string" | yes | none | {"minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$","format":"knot-id"} |
| `$defs.visual.targets[].order` | "integer" | yes | none | {"minimum":-9007199254740991,"maximum":9007199254740991} |
| `$defs.visual.targets[].properties` | "reference" | yes | none | {"$ref":"#/$defs/properties"} |
| `$defs.state.schemaVersion` | "1.0" | yes | none | {} |
| `$defs.state.productId` | "string" | yes | none | {"minLength":1,"maxLength":128,"pattern":"^[^\\u0000-\\u001f\\u007f-\\u009f]+$","format":"knot-id"} |
| `$defs.state.modelDigest` | "string" | yes | none | {"pattern":"^sha256:[0-9a-f]{64}$"} |
| `$defs.state.revision` | "integer" | yes | none | {"minimum":0,"maximum":9007199254740991} |
| `$defs.state.answers` | "object" | yes | none | {"maxProperties":256} |
| `$defs.state.skipped` | "array" | yes | none | {"minItems":0,"maxItems":256,"uniqueItems":true} |
