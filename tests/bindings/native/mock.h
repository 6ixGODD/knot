#pragma once
#include <knot.h>
#if defined(_WIN32) && !defined(KNOT_STATIC)
#if defined(KNOT_BUILD)
#define TEST_API __declspec(dllexport)
#else
#define TEST_API __declspec(dllimport)
#endif
#else
#define TEST_API
#endif
#ifdef __cplusplus
extern "C" {
#endif
TEST_API void knot_test_reset(void);
TEST_API void knot_test_response(const char *, size_t, int32_t);
TEST_API void knot_test_create_response(int);
TEST_API int knot_test_results_freed(void);
TEST_API int knot_test_models_freed(void);
TEST_API int knot_test_live_models(void);
TEST_API int knot_test_live_buffers(void);
TEST_API size_t knot_test_layout(int);
TEST_API const char *knot_test_last_state(void);
TEST_API const char *knot_test_last_event(void);
#ifdef __cplusplus
}
#endif
