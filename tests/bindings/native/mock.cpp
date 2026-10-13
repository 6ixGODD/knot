// Test-only controlled ABI boundary, never a product library or rule engine.
#include "mock.h"
#include "mock_fixtures.hpp"
#include <cstdlib>
#include <cstring>
#include <string>
struct knot_model {
    int marker = 42;
};
namespace {
std::string payload = mock_transition, last_state, last_event;
int32_t status = 0;
int freed = 0, models_freed = 0, live = 0, buffers = 0;
bool create_response = false, no_json = false;
knot_result_t make(const std::string &s, int32_t code) {
    auto *p = static_cast<uint8_t *>(std::malloc(s.size()));
    if (!p)
        std::abort();
    std::memcpy(p, s.data(), s.size());
    ++buffers;
    return {code, {p, s.size()}};
}
knot_result_t reply() {
    if (no_json)
        return {status, {nullptr, 0}};
    return make(payload, status);
}
} // namespace
extern "C" {
void knot_test_reset() {
    if (live || buffers)
        std::abort();
    payload = mock_transition;
    status = freed = models_freed = 0;
    create_response = no_json = false;
    last_state.clear();
    last_event.clear();
}
void knot_test_response(const char *p, size_t n, int32_t s) {
    payload = p ? std::string(p, n) : std::string{};
    status = s;
    no_json = p == nullptr;
}
void knot_test_create_response(int v) { create_response = v != 0; }
int knot_test_results_freed() { return freed; }
int knot_test_models_freed() { return models_freed; }
int knot_test_live_models() { return live; }
int knot_test_live_buffers() { return buffers; }
const char *knot_test_last_state() { return last_state.c_str(); }
const char *knot_test_last_event() { return last_event.c_str(); }
size_t knot_test_layout(int i) {
    switch (i) {
    case 0:
        return sizeof(knot_buffer_t);
    case 1:
        return sizeof(knot_result_t);
    case 2:
        return offsetof(knot_result_t, json);
    default:
        return sizeof(void *);
    }
}
uint32_t knot_abi_version() { return 1; }
knot_result_t knot_model_create(const uint8_t *p, size_t n, knot_model_t **out) {
    *out = nullptr;
    // Sentinels exist only in this test source for the Go component tests.
    if (n == 7 && std::memcmp(p, "no-json", 7) == 0)
        return {1999, {nullptr, 0}};
    if (n == 8 && std::memcmp(p, "bad-json", 8) == 0)
        return make("{", 0);
    if (n == 5 && std::memcmp(p, "stats", 5) == 0) {
        auto json = std::string("{\"buffers\":") + std::to_string(buffers) +
                    ",\"models\":" + std::to_string(live) + ",\"freed\":" + std::to_string(freed) +
                    "}";
        *out = new knot_model;
        ++live;
        return make(json, 0);
    }
    if (!create_response || status == 0) {
        *out = new knot_model;
        ++live;
    }
    return create_response ? reply() : make(mock_create, 0);
}
knot_result_t knot_initial(const knot_model_t *) { return reply(); }
knot_result_t knot_apply(const knot_model_t *, const uint8_t *s, size_t ns, const uint8_t *e,
                         size_t ne) {
    last_state.assign(reinterpret_cast<const char *>(s), ns);
    last_event.assign(reinterpret_cast<const char *>(e), ne);
    return reply();
}
knot_result_t knot_evaluate(const knot_model_t *, const uint8_t *s, size_t ns) {
    last_state.assign(reinterpret_cast<const char *>(s), ns);
    return reply();
}
void knot_result_free(knot_result_t *r) {
    if (!r)
        return;
    ++freed;
    if (r->json.data)
        --buffers;
    std::free(r->json.data);
    r->json = {nullptr, 0};
    r->status = 0;
}
void knot_model_free(knot_model_t *m) {
    if (m) {
        if (m->marker != 42)
            std::abort();
        delete m;
        ++models_freed;
        --live;
    }
}
}
