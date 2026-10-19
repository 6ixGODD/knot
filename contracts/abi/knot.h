#ifndef KNOT_H
#define KNOT_H

#include <stddef.h>
#include <stdint.h>

#if defined(KNOT_STATIC)
  #define KNOT_API
#elif defined(_WIN32)
  #if defined(KNOT_BUILD)
    #define KNOT_API __declspec(dllexport)
  #else
    #define KNOT_API __declspec(dllimport)
  #endif
#else
  #define KNOT_API \
    __attribute__((visibility("default")))
#endif

#ifdef __cplusplus
extern "C" {
#endif

#define KNOT_ABI_VERSION 1u

typedef struct knot_model knot_model_t;

typedef int32_t knot_status_t;

enum {
    KNOT_OK = 0,

    KNOT_E_NULL_ARGUMENT = 1001,
    KNOT_E_BAD_JSON = 1002,
    KNOT_E_BAD_SCHEMA = 1003,
    KNOT_E_RULE_COMPILE = 1004,
    KNOT_E_INVALID_STATE = 1005,
    KNOT_E_REVISION_CONFLICT = 1006,
    KNOT_E_INVALID_ANSWER = 1007,
    KNOT_E_INACTIVE_QUESTION = 1008,
    KNOT_E_RULE_EVAL = 1009,
    KNOT_E_CONFLICT = 1010,
    KNOT_E_RESOURCE_LIMIT = 1011,

    KNOT_E_INTERNAL = 1999
};

typedef struct {
    uint8_t *data;
    size_t len;
} knot_buffer_t;

typedef struct {
    knot_status_t status;
    knot_buffer_t json;
} knot_result_t;

KNOT_API uint32_t knot_abi_version(void);

KNOT_API knot_result_t knot_model_create(
    const uint8_t *definition_json,
    size_t definition_len,
    knot_model_t **out_model
);

KNOT_API knot_result_t knot_initial(
    const knot_model_t *model
);

KNOT_API knot_result_t knot_apply(
    const knot_model_t *model,
    const uint8_t *state_json,
    size_t state_len,
    const uint8_t *event_json,
    size_t event_len
);

KNOT_API knot_result_t knot_evaluate(
    const knot_model_t *model,
    const uint8_t *state_json,
    size_t state_len
);

KNOT_API void knot_result_free(
    knot_result_t *result
);

KNOT_API void knot_model_free(
    knot_model_t *model
);

#ifdef __cplusplus
}
#endif

#endif
