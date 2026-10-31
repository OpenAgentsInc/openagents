/* Chapter 125 native evidence: the generated C library of
   psionic/lib/nanochat/checkpoint.coax (with gpt.coax) against the
   independent Python oracle. The harness only reads files and observes
   addresses; Coax admits the export, hashes it and runs the model.
   argv: export.json model.safetensors historical.json */
#include "nanochat.h"
#include <assert.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "nanochat-oracle.h"
#include "spans.h"

static coax_v_f64 v(double *x, int64_t n) { return (coax_v_f64){x, n}; }
static int64_t slurp(const char *path, uint8_t *buf, int64_t cap) {
    FILE *f = fopen(path, "rb"); assert(f);
    int64_t n = (int64_t)fread(buf, 1, (size_t)cap, f); assert(feof(f)); fclose(f); return n;
}
static double worst = 0;
static void close_to(const double *a, const double *b, int64_t n) {
    for (int64_t i = 0; i < n; i++) { double d = fabs(a[i] - b[i]) / (1 + fabs(b[i])); if (d > worst) worst = d; assert(d <= 1e-12); }
}
static double params[60000], cache[40000], work[40000], logits[4000], step[400], blocks[12000];
/* Full pass against the oracle, then one token at a time: bit-identical. */
static void model(coax_t_NcConfig c, const int *tok, int64_t n, double loss, const double *z, const double *b) {
    int64_t t64[256]; for (int64_t i = 0; i < n; i++) t64[i] = tok[i];
    int64_t count = nc_parameter_count(c);
    coax_t_NcResult r = nc_forward(c, v(params, count), (coax_v_s64){t64, n}, v(cache, 40000), v(work, 40000), v(logits, 4000), v(blocks, 12000), 0);
    assert(r.status == COAX_NcStatus_ok && r.written == n * c.vocab);
    close_to(logits, z, n * c.vocab); close_to(blocks, b, c.layers * n * c.width); close_to(&r.loss, &loss, 1);
    static double fresh[40000]; memset(fresh, 0, sizeof fresh);
    for (int64_t t = 0; t < n; t++) {
        r = nc_decode(c, v(params, count), v(fresh, 40000), t, t64[t], v(work, 40000), v(step, 400), 0);
        assert(r.status == COAX_NcStatus_ok && r.written == t + 1);
        assert(!memcmp(step, logits + t * c.vocab, (size_t)c.vocab * sizeof(double)));
    }
}
int main(int argc, char **argv) {
    assert(argc == 4);
    static uint8_t file[200000], identity[4200], historical[4200];
    int64_t bytes = slurp(argv[2], file, sizeof file), text = slurp(argv[1], identity, sizeof identity), old = slurp(argv[3], historical, sizeof historical);
    static coax_t_PwTensor inv[40], draft[40];
    coax_t_NcLoad L = nc_load((coax_string){(const char *)identity, text}, (coax_v_u8){file, bytes}, v(params, 53290), (coax_v_t_PwTensor){inv, 40}, (coax_v_t_PwTensor){draft, 40}, 0);
    assert(L.status == COAX_NcLoadStatus_ok && L.written == 53290 && L.step == 0);
    assert(L.config.sequence_len == 64 && L.config.vocab == 329 && L.config.layers == 2 && L.config.heads == 2 && L.config.kv_heads == 1 && L.config.width == 32);
    assert(L.config.pattern == 7 && L.config.pattern_len == 4 && L.config.context == 64);
    model(L.config, NC_CHECKPOINT_TOKENS, 12, NC_CHECKPOINT_LOSS, NC_CHECKPOINT_LOGITS, NC_CHECKPOINT_BLOCKS);
    double kept = params[100];
    /* Refusals write nothing: a flipped payload byte, the historical record; then aliasing. */
    file[bytes - 1] ^= 1; for (int i = 0; i < 53290; i++) params[i] = -9;
    L = nc_load((coax_string){(const char *)identity, text}, (coax_v_u8){file, bytes}, v(params, 53290), (coax_v_t_PwTensor){inv, 40}, (coax_v_t_PwTensor){draft, 40}, 0);
    assert(L.status == COAX_NcLoadStatus_digest); file[bytes - 1] ^= 1;
    L = nc_load((coax_string){(const char *)historical, old}, (coax_v_u8){file, bytes}, v(params, 53290), (coax_v_t_PwTensor){inv, 40}, (coax_v_t_PwTensor){draft, 40}, 0);
    assert(L.status == COAX_NcLoadStatus_historical);
    for (int i = 0; i < 53290; i++) assert(params[i] == -9);
    L = nc_load((coax_string){(const char *)identity, text}, (coax_v_u8){file, bytes}, v(params, 53290), (coax_v_t_PwTensor){inv, 40}, (coax_v_t_PwTensor){draft, 40},
                psionic_spans_overlap(file, bytes, 1, params, 53290, 8));
    assert(L.status == COAX_NcLoadStatus_ok && params[100] == kept);
    L = nc_load((coax_string){(const char *)identity, text}, (coax_v_u8){file, bytes}, v(params, 53290), (coax_v_t_PwTensor){inv, 40}, (coax_v_t_PwTensor){draft, 40},
                psionic_spans_overlap(file, bytes, 1, file, 8, 1));
    assert(L.status == COAX_NcLoadStatus_alias);
    coax_t_NcConfig tiny = {8, 5, 1, 4, 2, 24, 0, 1, 8};
    nc_formula(tiny, 0.5, v(params, nc_parameter_count(tiny)));
    model(tiny, NC_TINY_TOKENS, 6, NC_TINY_LOSS, NC_TINY_LOGITS, NC_TINY_BLOCKS);
    coax_t_NcConfig window = {256, 7, 3, 2, 1, 24, 1, 2, 140};
    nc_formula(window, 0.5, v(params, nc_parameter_count(window)));
    model(window, NC_WINDOW_TOKENS, 140, NC_WINDOW_LOSS, NC_WINDOW_LOGITS, NC_WINDOW_BLOCKS);
    int64_t t64[6] = {1, 3, 0, 2, 4, 1};
    assert(nc_forward(tiny, v(params, nc_parameter_count(tiny)), (coax_v_s64){t64, 6}, v(cache, 20000), v(cache + 10000, 20000), v(logits, 4000),
                      v(blocks, 0), psionic_spans_overlap(cache, 20000, 8, cache + 10000, 20000, 8)).status == COAX_NcStatus_alias);
    nc_formula(tiny, 0.5, v(params, nc_parameter_count(tiny))); params[7] = NAN;
    assert(nc_forward(tiny, v(params, nc_parameter_count(tiny)), (coax_v_s64){t64, 6}, v(cache, 40000), v(work, 40000), v(logits, 4000), v(blocks, 0), 0).status == COAX_NcStatus_number);
    printf("nanochat native: checkpoint export admitted and refused as recorded; 3 models within %.3g of the oracle; decode equals full\n", worst);
    return 0;
}
