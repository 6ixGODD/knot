# C ABI v1

Declarations: [knot.h](../contracts/abi/knot.h). knot_model_t is opaque; knot_status_t is int32; knot_buffer_t is uint8 pointer plus size_t length; knot_result_t is status plus buffer with natural platform alignment. Use the header, not guessed packing. Call convention is the platform C ABI.

## knot_abi_version

| Item | Contract |
|---|---|
| Parameters | none |
| Preconditions | compatible library loaded |
| Return | uint32 version 1 |
| Release | none |

## knot_model_create

| Item | Contract |
|---|---|
| Parameters | definition_json, definition_len, out_model |
| Preconditions | valid borrowed Product bytes; writable out_model not overlapping input |
| Return | status + identity/diagnostics on success; out_model NULL until success |
| Release | result_free; model_free for successful handle |

## knot_initial

| Item | Contract |
|---|---|
| Parameters | model |
| Preconditions | live handle |
| Return | complete TransitionResult or error only |
| Release | result_free |

## knot_apply

| Item | Contract |
|---|---|
| Parameters | model, state_json/state_len, event_json/event_len |
| Preconditions | live handle; valid borrowed buffers of stated lengths |
| Return | complete TransitionResult or error only |
| Release | result_free |

## knot_evaluate

| Item | Contract |
|---|---|
| Parameters | model, state_json/state_len |
| Preconditions | matching model and canonical State |
| Return | complete TransitionResult, no repair |
| Release | result_free |

## knot_result_free

| Item | Contract |
|---|---|
| Parameters | pointer to original result |
| Preconditions | single owner; valid pointer or NULL |
| Return | void; clears buffer descriptor |
| Release | NULL/repeat on cleared descriptor is a no-op |

## knot_model_free

| Item | Contract |
|---|---|
| Parameters | model |
| Preconditions | no concurrent calls; live handle or NULL |
| Return | void |
| Release | non-NULL handle exactly once |

Inputs are borrowed only until return; do not modify bytes during a call. Required NULL is NULL_ARGUMENT even with length zero; non-NULL zero-length is BAD_JSON. Non-NULL pointers must cover valid allocated memory. Arbitrary dangling pointers are not recoverable errors. Independent-state calls may share a live model; free/unload must wait for calls.

Outputs are data+len, not guaranteed NUL-terminated. Never use strlen or the host allocator to release them. Do not copy a result descriptor and free both copies. Release the original result on every path. Copy bytes first if retaining them. State/Result schemas are [packaged here](../contracts/schemas/result.schema.json).

On failure out_model is NULL. Error JSON follows [error.schema.json](../contracts/schemas/error.schema.json): schemaVersion plus error.code/message/path/ruleIds/details. Status 1999 may have data=NULL,len=0; handle it without parsing. Branch on code, never message. Successful create returns productId/productVersion/modelDigest/diagnostics, not a State. Transitions return state/snapshot/changes/diagnostics.

The [compiled C smoke](../examples/c/smoke.c) shows complete file/create/initial/error/free handling. Every result-producing function follows this ownership pattern:

```c
knot_result_t r = knot_initial(model);
if (r.status == KNOT_OK) {
    /* Copy or consume r.json.data[0..r.json.len) before releasing. */
} else {
    /* Inspect error JSON if nonempty; INTERNAL may have no JSON. */
}
knot_result_free(&r);
```

Status families: 1001 NULL_ARGUMENT; 1002 BAD_JSON/INVALID_UTF8; 1003 BAD_SCHEMA/INVALID_MODEL; 1004 RULE_COMPILE; 1005 INVALID_STATE/MODEL_MISMATCH; 1006 REVISION_CONFLICT; 1007 INVALID_ANSWER/INVALID_OPTION; 1008 INACTIVE_QUESTION; 1009 RULE_EVAL/MISSING_VALUE; 1010 business conflicts; 1011 limits/overflow; 1999 INTERNAL. KNOT_OK=0. See the [full catalog](../contracts/errors.json) for bounded details. A failed call never replaces caller State.

Only the native library owns its allocations. Catchable panics map to INTERNAL and may invalidate a model; release/rebuild it. Process OOM, abort and invalid memory access are not promised recoverable. On Windows KNOT_STATIC suppresses DLL imports; use the matching [CMake target](integration.md).
