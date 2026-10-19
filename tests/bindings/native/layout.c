#include "mock.h"
#include <stddef.h>
int main(void) {
    return knot_test_layout(0) != sizeof(knot_buffer_t) ||
           knot_test_layout(1) != sizeof(knot_result_t) ||
           knot_test_layout(2) != offsetof(knot_result_t, json) || knot_abi_version() != 1;
}
