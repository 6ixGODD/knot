#include <knot.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static uint8_t *read_file(const char *path, size_t limit, size_t *length) {
    FILE *file = fopen(path, "rb");
    if (!file)
        return NULL;
    uint8_t *data = (uint8_t *)malloc(limit + 1);
    if (!data) {
        fclose(file);
        return NULL;
    }
    *length = fread(data, 1, limit + 1, file);
    int failed = ferror(file) || *length > limit;
    fclose(file);
    if (failed) {
        free(data);
        return NULL;
    }
    return data;
}
int main(int argc, char **argv) {
    if (argc != 5 || knot_abi_version() != 1)
        return 2;
    size_t plen = 0, slen = 0, elen = 0;
    uint8_t *product = read_file(argv[2], 4194304, &plen);
    uint8_t *state = read_file(argv[3], 524288, &slen);
    uint8_t *event = read_file(argv[4], 65536, &elen);
    if (!product || !state || !event) {
        free(product);
        free(state);
        free(event);
        return 2;
    }
    knot_model_t *model = NULL;
    knot_result_t result = knot_model_create(product, plen, &model);
    if (result.status == 0 && strcmp(argv[1], "model_create") != 0) {
        knot_result_free(&result);
        if (strcmp(argv[1], "initial") == 0)
            result = knot_initial(model);
        else if (strcmp(argv[1], "evaluate") == 0)
            result = knot_evaluate(model, state, slen);
        else if (strcmp(argv[1], "apply") == 0)
            result = knot_apply(model, state, slen, event, elen);
        else {
            knot_model_free(model);
            free(product);
            free(state);
            free(event);
            return 2;
        }
    }
    int failed = printf("{\"status\":%d,\"result\":", result.status) < 0;
    if (result.json.data)
        failed |= fwrite(result.json.data, 1, result.json.len, stdout) != result.json.len;
    else
        failed |= fputs("null", stdout) < 0;
    failed |= fputs("}\n", stdout) < 0;
    failed |= fflush(stdout) != 0;
    knot_result_free(&result);
    knot_result_free(&result);
    failed |= result.json.data != NULL || result.json.len != 0;
    knot_model_free(model);
    free(product);
    free(state);
    free(event);
    return failed ? 1 : 0;
}
