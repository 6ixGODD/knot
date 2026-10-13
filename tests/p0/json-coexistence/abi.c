#include "knot.h"
_Static_assert(KNOT_ABI_VERSION == 1, "ABI version");
_Static_assert(sizeof(knot_status_t) == 4, "status width");
_Static_assert(offsetof(knot_buffer_t, data) == 0, "buffer pointer offset");
_Static_assert(offsetof(knot_buffer_t, len) == sizeof(void *), "buffer length offset");
_Static_assert(sizeof(knot_buffer_t) == sizeof(void *) + sizeof(size_t), "buffer size");
_Static_assert(offsetof(knot_result_t, status) == 0, "result status offset");
_Static_assert(KNOT_E_RESOURCE_LIMIT == 1011 && KNOT_E_INTERNAL == 1999, "status codes");
int main(void) { return 0; }
