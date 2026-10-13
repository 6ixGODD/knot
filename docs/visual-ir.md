# Visual intermediate representation

KNOT returns descriptions, not pixels or render commands. The host owns asset resolution, resource loading, shaders and rendering. Schema: [visual.schema.json](../contracts/schemas/visual.schema.json). Result targets have id, safe-integer order and properties. Sort order is numeric order then target ID UTF-8 byte order; do not derive order from object-map enumeration.

Targets define defaults. Slots name an existing target and property and provide conditional cases with either a constant value or valueFrom answer. No matching case restores the target default. Multiple matching cases within one slot fail VISUAL_CONFLICT. More than one slot declaring the same target/property is rejected at compile time, even if their conditions appear exclusive; never use last-writer-wins. Missing valueFrom fails. A property with no declared default and no matching case is absent. Declared defaults make reset behavior explicit.

| Property | Value |
|---|---|
| visible | boolean |
| variant, assetRef, surface.textureRef | nonempty ID, at most 128 UTF-8 bytes |
| color, emission.color | #RRGGBB, canonical uppercase |
| opacity, surface.metallic, surface.roughness, emission.strength, light.intensity | finite number, 0 through 1 |
| transform.position, transform.scale | exactly three finite numbers, each absolute value ≤ 1e9 |

Unknown properties are rejected at compile/Schema boundaries. Hosts must report unsupported known properties instead of silently rendering the wrong result. Resource identifiers are host-owned names, not instructions for KNOT to access files or URLs. On success replace the full visual snapshot; Changes.visualChangedTargets only identifies changed targets. On failed apply keep the previous display and State.
