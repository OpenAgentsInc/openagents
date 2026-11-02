/* Chapter 163's original-weight slices: the embedding rows of a short
 * prompt, Mamba-2 layer 0, MLP layer 1 on its output, attention layer 12 on
 * that, and readout rows 0..8191 for the last token. The checkpoint was
 * authenticated by nemotron_manifest.py first; nemotron_slices.py wrote the
 * tensor byte ranges from its header. All model arithmetic is Coax
 * (nemotron-slices-runtime.coax); this file maps the file, moves bytes and
 * writes outputs.
 *
 *   nemotron-slices MODEL RANGES TOKENS OUT_DIR
 */
#define _POSIX_C_SOURCE 200809L
#include "nemotron-slices-runtime.h"
#include <fcntl.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <time.h>
#include <unistd.h>

enum { H = 3136, V = 7680, C = 9728, HEADS = 96, STATES = 96 * 80 * 128, KV = 1024, MAX_TOKENS = 16, READOUT = 8192, RANGES = 300 };
static const unsigned char *map;
static int64_t size;
static char names[RANGES][96];
static int64_t begins[RANGES], ends[RANGES];
static int ranges;

static double now(void) { struct timespec t; clock_gettime(CLOCK_MONOTONIC, &t); return t.tv_sec + t.tv_nsec / 1e9; }
static coax_v_f32 span(float *p, int64_t n) { return (coax_v_f32){p, n}; }
/* The named tensor, decoded by Coax into a new f32 array of count values. */
static float *tensor(const char *name, int64_t count) {
    for (int i = 0; i < ranges; i++) {
        if (strcmp(names[i], name)) continue;
        if (ends[i] - begins[i] != 2 * count || ends[i] > size) { fprintf(stderr, "%s: wrong size\n", name); exit(1); }
        float *out = malloc((size_t)count * sizeof(float));
        if (!out || !nhs_decode((coax_v_u8){(uint8_t *)map + begins[i], 2 * count}, span(out, count))) {
            fprintf(stderr, "%s: decode failed\n", name); exit(1);
        }
        return out;
    }
    fprintf(stderr, "%s: not in the ranges\n", name); exit(1);
}
static int write_f32(const char *dir, const char *name, const float *v, size_t n) {
    char path[4096]; snprintf(path, sizeof path, "%s/%s", dir, name);
    FILE *f = fopen(path, "wb");
    if (!f || fwrite(v, sizeof(float), n, f) != n || fclose(f)) { fprintf(stderr, "cannot write %s\n", path); return 1; }
    return 0;
}
static int same(const float *a, const float *b, size_t n) { return memcmp(a, b, n * sizeof(float)) == 0; }

int main(int argc, char **argv) {
    if (argc != 5) { fprintf(stderr, "usage: nemotron-slices MODEL RANGES TOKENS OUT_DIR\n"); return 2; }
    FILE *rf = fopen(argv[2], "r"); if (!rf) { perror(argv[2]); return 2; }
    long long b, e;
    while (ranges < RANGES && fscanf(rf, "%95s %lld %lld", names[ranges], &b, &e) == 3) { begins[ranges] = b; ends[ranges] = e; ranges++; }
    fclose(rf);
    int64_t tokens[MAX_TOKENS]; int64_t n = 0;
    FILE *tf = fopen(argv[3], "r"); if (!tf) { perror(argv[3]); return 2; }
    long long t;
    while (n < MAX_TOKENS && fscanf(tf, "%lld", &t) == 1) tokens[n++] = t;
    fclose(tf);
    if (n < 2) { fprintf(stderr, "need at least two tokens\n"); return 2; }
    int fd = open(argv[1], O_RDONLY); if (fd < 0) { perror("open"); return 2; }
    struct stat st; if (fstat(fd, &st) || st.st_size != 7947142640LL) { fprintf(stderr, "not the pinned file size\n"); return 2; }
    size = st.st_size;
    map = mmap(NULL, (size_t)size, PROT_READ, MAP_PRIVATE, fd, 0); close(fd);
    if (map == MAP_FAILED) { perror("mmap"); return 2; }
    const char *out = argv[4];
    double t0 = now();

    /* The embedding rows. */
    float *hidden = calloc((size_t)n * H, sizeof(float));
    for (int64_t i = 0; i < ranges; i++) if (!strcmp(names[i], "backbone.embeddings.weight")) {
        for (int64_t k = 0; k < n; k++) {
            if (tokens[k] < 0 || tokens[k] >= 131072) { fprintf(stderr, "token out of range\n"); return 1; }
            if (!nhs_decode((coax_v_u8){(uint8_t *)map + begins[i] + tokens[k] * H * 2, 2 * H}, span(hidden + k * H, H))) return 1;
        }
    }
    if (write_f32(out, "embed.f32", hidden, (size_t)n * H)) return 1;

    /* Mamba-2 layer 0: the prompt in one chunk; then chunks of 4 resumed
     * after 4 tokens; then one token at a time. */
    float *norm = tensor("backbone.layers.0.norm.weight", H), *in_proj = tensor("backbone.layers.0.mixer.in_proj.weight", (int64_t)(V + C + HEADS) * H);
    float *conv = tensor("backbone.layers.0.mixer.conv1d.weight", 4 * C), *conv_bias = tensor("backbone.layers.0.mixer.conv1d.bias", C);
    float *a_log = tensor("backbone.layers.0.mixer.A_log", HEADS), *dt_bias = tensor("backbone.layers.0.mixer.dt_bias", HEADS);
    float *d = tensor("backbone.layers.0.mixer.D", HEADS), *gate = tensor("backbone.layers.0.mixer.norm.weight", V);
    float *out_proj = tensor("backbone.layers.0.mixer.out_proj.weight", (int64_t)H * V);
    int64_t fneed = nhs_mamba_fwork(n);
    float *work = calloc((size_t)nhs_mamba_work(n), sizeof(float)); double *fwork = calloc((size_t)fneed, sizeof(double));
    float *runs[3], *states[3], *histories[3];
    int64_t chunks[3] = {n, 4, 1};
    for (int r = 0; r < 3; r++) {
        runs[r] = malloc((size_t)n * H * sizeof(float)); memcpy(runs[r], hidden, (size_t)n * H * sizeof(float));
        states[r] = calloc(STATES, sizeof(float)); histories[r] = calloc(3 * C, sizeof(float));
    }
    double t1 = now();
    #define MAMBA(r, first, count) nhs_mamba(span(norm, H), span(in_proj, (int64_t)(V + C + HEADS) * H), span(conv, 4 * C), span(conv_bias, C), \
        span(a_log, HEADS), span(dt_bias, HEADS), span(d, HEADS), span(gate, V), span(out_proj, (int64_t)H * V), (count), chunks[r], \
        span(runs[r] + (first) * H, (count) * H), span(histories[r], 3 * C), span(states[r], STATES), \
        span(work, nhs_mamba_work(n)), (coax_v_f64){fwork, fneed})
    if (MAMBA(0, 0, n)) { fprintf(stderr, "mamba refused\n"); return 1; }
    double t2 = now();
    if (MAMBA(1, 0, 4) || MAMBA(1, 4, n - 4)) { fprintf(stderr, "mamba resume refused\n"); return 1; }
    for (int64_t k = 0; k < n; k++) if (MAMBA(2, k, 1)) { fprintf(stderr, "mamba step refused\n"); return 1; }
    if (write_f32(out, "mamba.f32", runs[0], (size_t)n * H) || write_f32(out, "mamba-state.f32", states[0], STATES)) return 1;
    if (write_f32(out, "mamba-step.f32", runs[2], (size_t)n * H)) return 1;
    printf("slice mamba layer 0: %lld tokens in one chunk, %.1f s\n", (long long)n, t2 - t1);
    float *layer0 = runs[0]; int64_t again = 4; chunks[0] = 4;
    float *resumed = malloc((size_t)n * H * sizeof(float)); memcpy(resumed, hidden, (size_t)n * H * sizeof(float));
    float *rs = calloc(STATES, sizeof(float)), *rh = calloc(3 * C, sizeof(float));
    runs[0] = resumed; states[0] = rs; histories[0] = rh;
    if (MAMBA(0, 0, n)) return 1;
    printf("  chunks of %lld, the prompt at once or resumed after 4 tokens: outputs, state, history bit for bit %s\n",
        (long long)again, same(resumed, runs[1], (size_t)n * H) && same(rs, states[1], STATES) && same(rh, histories[1], 3 * C) ? "true" : "false");
    free(in_proj); free(out_proj);

    /* MLP layer 1 on layer 0's output. */
    float *mlp = malloc((size_t)n * H * sizeof(float));
    float *n1 = tensor("backbone.layers.1.norm.weight", H), *up = tensor("backbone.layers.1.mixer.up_proj.weight", (int64_t)12544 * H);
    float *down = tensor("backbone.layers.1.mixer.down_proj.weight", (int64_t)H * 12544);
    float *mw = calloc((size_t)nhs_mlp_work(), sizeof(float));
    memcpy(mlp, layer0, (size_t)n * H * sizeof(float));
    double t3 = now();
    if (nhs_mlp(span(n1, H), span(up, (int64_t)12544 * H), span(down, (int64_t)H * 12544), n, span(mlp, n * H), span(mw, nhs_mlp_work()))) { fprintf(stderr, "mlp refused\n"); return 1; }
    printf("slice mlp layer 1: %.1f s\n", now() - t3);
    if (write_f32(out, "mlp.f32", mlp, (size_t)n * H)) return 1;

    /* Attention layer 12 on the MLP's output: the prompt at once, then one
     * token at a time from the cache. */
    float *n12 = tensor("backbone.layers.12.norm.weight", H), *q = tensor("backbone.layers.12.mixer.q_proj.weight", (int64_t)5120 * H);
    float *k = tensor("backbone.layers.12.mixer.k_proj.weight", (int64_t)KV * H), *v = tensor("backbone.layers.12.mixer.v_proj.weight", (int64_t)KV * H);
    float *o = tensor("backbone.layers.12.mixer.o_proj.weight", (int64_t)H * 5120);
    float *aw = calloc((size_t)nhs_attention_work(), sizeof(float)); double *af = calloc(MAX_TOKENS, sizeof(double));
    float *keys = calloc((size_t)n * KV, sizeof(float)), *values = calloc((size_t)n * KV, sizeof(float));
    float *k2 = calloc((size_t)n * KV, sizeof(float)), *v2 = calloc((size_t)n * KV, sizeof(float));
    float *att = malloc((size_t)n * H * sizeof(float)), *each = malloc((size_t)n * H * sizeof(float));
    memcpy(att, mlp, (size_t)n * H * sizeof(float)); memcpy(each, mlp, (size_t)n * H * sizeof(float));
    double t4 = now();
    if (nhs_attention(span(n12, H), span(q, (int64_t)5120 * H), span(k, (int64_t)KV * H), span(v, (int64_t)KV * H), span(o, (int64_t)H * 5120),
            n, 0, span(att, n * H), span(keys, n * KV), span(values, n * KV), span(aw, nhs_attention_work()), (coax_v_f64){af, n})) { fprintf(stderr, "attention refused\n"); return 1; }
    printf("slice attention layer 12: %.1f s\n", now() - t4);
    for (int64_t p = 0; p < n; p++)
        if (nhs_attention(span(n12, H), span(q, (int64_t)5120 * H), span(k, (int64_t)KV * H), span(v, (int64_t)KV * H), span(o, (int64_t)H * 5120),
                1, p, span(each + p * H, H), span(k2, n * KV), span(v2, n * KV), span(aw, nhs_attention_work()), (coax_v_f64){af, n})) return 1;
    printf("  the prompt at once or one token at a time from the cache: bit for bit %s\n",
        same(att, each, (size_t)n * H) && same(keys, k2, (size_t)n * KV) && same(values, v2, (size_t)n * KV) ? "true" : "false");
    if (write_f32(out, "attention.f32", att, (size_t)n * H)) return 1;

    /* The final norm and readout rows 0..8191 for the last token. */
    float *nf = tensor("backbone.norm_f.weight", H), *logits = calloc(READOUT, sizeof(float)), *normed = calloc(H, sizeof(float));
    float *head = NULL;
    for (int i = 0; i < ranges; i++) if (!strcmp(names[i], "lm_head.weight")) {
        head = malloc((size_t)READOUT * H * sizeof(float));
        if (!nhs_decode((coax_v_u8){(uint8_t *)map + begins[i], 2 * READOUT * H}, span(head, READOUT * H))) return 1;
    }
    if (!head) return 1;
    int64_t best = nhs_readout(span(nf, H), span(head, READOUT * H), span(att + (n - 1) * H, H), 0, span(logits, READOUT), span(normed, H));
    if (best < 0) { fprintf(stderr, "readout refused\n"); return 1; }
    printf("slice readout rows 0..%d: largest at %lld\n", READOUT - 1, (long long)best);
    if (write_f32(out, "readout.f32", logits, READOUT)) return 1;
    printf("slices: %.1f s\n", now() - t0);
    return 0;
}
