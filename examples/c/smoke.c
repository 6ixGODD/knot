#include <knot.h>
#include <stdio.h>
#include <stdlib.h>

int main(int argc, char **argv) {
    if (argc != 2 || knot_abi_version() != KNOT_ABI_VERSION)
        return 2;
    FILE *file = fopen(argv[1], "rb");
    if (!file)
        return 2;
    const size_t limit = 4194304;
    uint8_t *bytes = (uint8_t *)malloc(limit + 1);
    if (!bytes) {
        fclose(file);
        return 2;
    }
    size_t length = fread(bytes, 1, limit + 1, file);
    int failed = ferror(file) || length > limit;
    fclose(file);
    if (failed) {
        free(bytes);
        return 2;
    }
    knot_model_t *model = NULL;
    knot_result_t result = knot_model_create(bytes, length, &model);
    free(bytes);
    failed = result.status != KNOT_OK;
    knot_result_free(&result);
    if (failed) {
        knot_model_free(model);
        return 3;
    }
    result = knot_initial(model);
    failed = result.status != KNOT_OK;
    if (result.json.data && fwrite(result.json.data, 1, result.json.len, stdout) != result.json.len)
        failed = 1;
    if (fflush(stdout) != 0)
        failed = 1;
    knot_result_free(&result);
    knot_model_free(model);
    return failed ? 3 : 0;
}
