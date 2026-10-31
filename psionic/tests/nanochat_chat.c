/* nanochat conversation tokens natively: the learned chapter 123 table read
   from disk, the tool-call conversation from the independent goldens, alias
   decided by the host from real addresses, and short or overlapping views
   that must refuse without writing. Usage: nanochat-chat LEARNED.pst */
#include "nanochat-chat.h"
#include "spans.h"
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
static coax_string S(const char *s) { return (coax_string){s, (int64_t)strlen(s)}; }
static coax_t_NcResult render(coax_v_u8 table, coax_v_s64 sp, coax_v_t_NcMessage m, coax_v_t_NcPart p, int64_t limit, coax_v_s64 ids, coax_v_u8 mask) {
    bool overlap = psionic_spans_overlap(ids.data, ids.count, 8, mask.data, mask.count, 1) || psionic_spans_overlap(ids.data, ids.count, 8, table.data, table.count, 1) ||
                   psionic_spans_overlap(mask.data, mask.count, 1, table.data, table.count, 1) || psionic_spans_overlap(ids.data, ids.count, 8, sp.data, sp.count, 8);
    return nc_render(table, sp, 32768, m, p, limit, ids, mask, overlap);
}
int main(int argc, char **argv) {
    if (argc != 2) return 2;
    FILE *f = fopen(argv[1], "rb"); if (!f) return 2;
    fseek(f, 0, SEEK_END); long n = ftell(f); fseek(f, 0, SEEK_SET);
    uint8_t *bytes = malloc((size_t)n); if (!bytes || fread(bytes, 1, (size_t)n, f) != (size_t)n) return 2; fclose(f);
    coax_v_u8 table = {bytes, n};
    int64_t base = pt_layout(table).counts.e[0], specials[9];
    for (int i = 0; i < 9; i++) specials[i] = base + i;
    coax_v_s64 sp = {specials, 9};
    assert(base == 320 && nc_specials(table, sp, 32768) == COAX_NcStatus_ok);
    /* The python-tool conversation of goldens case 2. */
    coax_t_NcPart parts[4] = {{COAX_NcPartKind_text, S("Let me compute.")}, {COAX_NcPartKind_python, S("6789*2")},
                              {COAX_NcPartKind_output, S("13578")}, {COAX_NcPartKind_text, S(" It is 13578.")}};
    coax_t_NcMessage msgs[2] = {{COAX_NcRole_user, S("What is 6789 * 2?"), 0, 0, 0}, {COAX_NcRole_assistant, S(""), 1, 0, 4}};
    coax_v_t_NcMessage m = {msgs, 2}; coax_v_t_NcPart p = {parts, 4};
    int64_t *ids = malloc(2048 * sizeof *ids); uint8_t *mask = malloc(2048);
    coax_t_NcResult r = render(table, sp, m, p, 2048, (coax_v_s64){ids, 2048}, (coax_v_u8){mask, 2048});
    assert(r.status == COAX_NcStatus_ok && r.written == r.total && ids[0] == base && ids[r.written - 1] == base + 4 && mask[r.written - 1] == 1);
    int python = -1, output = -1;
    for (int64_t i = 0; i < r.written; i++) { if (ids[i] == base + 5) python = (int)i; if (ids[i] == base + 7) output = (int)i; }
    assert(python > 0 && output > python && mask[python] == 1 && mask[output] == 0 && mask[output + 1] == 0);
    /* Exact-size buffers succeed; one short refuses; ids over mask is an alias. */
    int64_t *exact = malloc((size_t)r.written * sizeof *exact); uint8_t *exact_mask = malloc((size_t)r.written);
    coax_t_NcResult e = render(table, sp, m, p, r.written, (coax_v_s64){exact, r.written}, (coax_v_u8){exact_mask, r.written});
    assert(e.status == COAX_NcStatus_ok && e.written == r.written && !memcmp(exact, ids, (size_t)r.written * sizeof *ids) && !memcmp(exact_mask, mask, (size_t)r.written));
    memset(exact, 0x5a, (size_t)r.written * sizeof *exact);
    assert(render(table, sp, m, p, r.written, (coax_v_s64){exact, r.written - 1}, (coax_v_u8){exact_mask, r.written}).status == COAX_NcStatus_capacity);
    assert(render(table, sp, m, p, 16, (coax_v_s64){ids, 16}, (coax_v_u8){(uint8_t *)ids, 16}).status == COAX_NcStatus_alias);
    assert(render(table, sp, m, p, 16, (coax_v_s64){(int64_t *)bytes, 16}, (coax_v_u8){mask, 16}).status == COAX_NcStatus_alias);
    for (int64_t i = 0; i < r.written; i++) assert(exact[i] == 0x5a5a5a5a5a5a5a5aLL);
    /* A truncated table is not a profile; bits per byte over the rendered targets. */
    assert(nc_specials((coax_v_u8){bytes, n - 1}, sp, 32768) == COAX_NcStatus_artifact);
    double *loss = malloc((size_t)r.written * sizeof *loss); for (int64_t i = 0; i < r.written; i++) loss[i] = 1.0;
    coax_t_NcBpb b = nc_bpb(table, (coax_v_s64){ids, r.written}, (coax_v_f64){loss, r.written});
    int64_t want = 0; for (int64_t i = 0; i < r.written; i++) want += ids[i] < base ? nc_token_bytes(table, ids[i]) : 0;
    assert(b.status == COAX_NcStatus_ok && b.bytes == want && want == (int64_t)strlen("What is 6789 * 2?Let me compute.6789*213578 It is 13578."));
    printf("nanochat chat native: %lld tokens, python call supervised, output unsupervised, %lld bytes; short, aliased and truncated inputs refuse\n", (long long)r.written, (long long)b.bytes);
    free(bytes); free(ids); free(mask); free(exact); free(exact_mask); free(loss);
    return 0;
}
