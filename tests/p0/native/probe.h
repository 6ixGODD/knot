#pragma once
#include <stdint.h>
#include <stddef.h>
#if defined(_WIN32) && !defined(P0_STATIC)
#define P0_API __declspec(dllimport)
#else
#define P0_API
#endif
#ifdef __cplusplus
extern "C" {
#endif
typedef struct p0_probe p0_probe;
typedef struct { uint8_t* data; size_t len; } p0_buffer;
typedef struct { int32_t status; p0_buffer json; } p0_response;
P0_API p0_response p0_probe_create(p0_probe** out);
P0_API p0_response p0_probe_evaluate(const p0_probe*, const uint8_t*, size_t);
P0_API void p0_probe_result_free(p0_response*);
P0_API void p0_probe_destroy(p0_probe*);
#ifdef __cplusplus
}
#endif
