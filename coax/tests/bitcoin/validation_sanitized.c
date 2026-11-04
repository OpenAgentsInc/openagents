/* Firm reason: 2 (native ABI for the independent Python consensus oracle,
   enabling ASan/UBSan without loading their runtime into Python).
   Platform effects only: bounded binary stdin/stdout and owned allocation. */
#include "validation-generated.c"

int main(void) {
    uint64_t q[6];
    if (fread(q, sizeof(q), 1, stdin) != 1 || q[0] > 3 ||
        q[3] > 160000000 || q[4] > 8000000 || q[5] > 64000000) return 2;
    uint8_t *headers = calloc(1, q[3] + 1), *body = calloc(1, q[4] + 1);
    uint8_t *flat = calloc(1, q[5] + 1), *table = calloc(1, q[5] * 2 + 16000000);
    coax_t_ValidationNative *state = calloc(1, (size_t)validation_native_size());
    if (!headers || !body || !flat || !table || !state) return 2;
    if (fread(headers, 1, q[3], stdin) != q[3] || fread(body, 1, q[4], stdin) != q[4] ||
        fread(flat, 1, q[5], stdin) != q[5] || fgetc(stdin) != EOF) return 2;
    coax_v_t_ValidationNative d = {state, 1};
    coax_v_u8 h = {headers, (int64_t)q[3]}, b = {body, (int64_t)q[4]};
    coax_v_u8 v = {table, (int64_t)(q[5] * 2 + 16000000)}, f = {flat, (int64_t)q[5]};
    int64_t used = (int64_t)q[5];
    if (q[0] & 2) { used = validation_native_indexed(v, f); if (used < 0) return 2; }
    else { memcpy(table, flat, q[5]); }
    int64_t header = (q[0] & 1) ? validation_native_fixture(d, (int64_t)q[1], (int64_t)q[2], h)
                         : validation_native_init(d, (int64_t)q[1], h);
    coax_t_ValidationResult r = {.code = -1};
    if (header >= 0) r = validation_native_block(d, (int64_t)q[2], b, v, used);
    int64_t n = 0; uint8_t *out = table;
    if (r.code == 0) {
        if (r.used < 0 || r.used > v.count) return 2;
        n = r.used;
        if (q[0] & 2) {
            out = calloc(1, (size_t)v.count); if (!out) return 2;
            n = validation_native_canonical(v, r.used, (coax_v_u8){out, v.count});
            if (n < 0) return 2;
        }
    }
    if (fwrite(&header, sizeof(header), 1, stdout) != 1 || fwrite(&r, sizeof(r), 1, stdout) != 1 ||
        fwrite(&n, sizeof(n), 1, stdout) != 1 || fwrite(out, 1, (size_t)n, stdout) != (size_t)n) return 2;
    if (out != table) free(out);
    free(state); free(table); free(flat); free(body); free(headers);
    return 0;
}
