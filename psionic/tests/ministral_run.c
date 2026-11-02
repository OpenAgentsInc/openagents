/* Chapter 81's recorded whole-model run of Ministral 3 3B on the original
 * weights. All arithmetic is Coax (psionic/lib/ministral-decode.coax through
 * ministral-run-runtime.coax); this file maps the file, finds tensors with
 * the Coax inventory reader, and writes or prints results. The file is
 * authenticated by ministral_reference.py pin first.
 *
 *   ministral-run MODEL_DIR TOKENS STEPS OUT_DIR
 *       all 26 layers: prefill the prompt, then STEPS greedy tokens through
 *       chapter 53's sampler (stopping at </s>), then recompute every layer
 *       from position 0 over the prompt and every emitted token but the
 *       last, compare the last logits bit for bit with the cached run, and
 *       write each layer's output (layers.f32), the full recompute's last
 *       logits (logits.f32) and the emitted tokens (emitted.txt) to OUT_DIR
 */
#define _POSIX_C_SOURCE 200809L
#include "ministral-run-runtime.h"
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <time.h>
#include <unistd.h>

enum { D = 3072, VOCAB = 131072, LAYERS = 26, CAPACITY = 64, TENSORS = 458 };

static coax_string str(const char *s) { return (coax_string){s, (int64_t)strlen(s)}; }
static coax_v_f32 span(float *p, int64_t n) { return (coax_v_f32){p, n}; }
static double now(void) { struct timespec t; clock_gettime(CLOCK_MONOTONIC, &t); return t.tv_sec + t.tv_nsec / 1e9; }

static int write_f32(const char *dir, const char *name, const float *v, size_t n) {
    char path[4096]; snprintf(path, sizeof path, "%s/%s", dir, name);
    FILE *f = fopen(path, "wb");
    if (!f || fwrite(v, sizeof(float), n, f) != n || fclose(f)) { fprintf(stderr, "cannot write %s\n", path); return 1; }
    return 0;
}

static coax_t_P5Tensor inv[1024]; static int64_t inv_count; static coax_t_PaFile file;

static coax_v_u8 tensor(const char *name, int64_t rows, int64_t cols) {
    coax_t_P5Shape shape = cols ? (coax_t_P5Shape){2, {{rows, cols, 0, 0, 0}}} : (coax_t_P5Shape){1, {{rows, 0, 0, 0, 0}}};
    coax_t_PaView v = pa_view(str(name), shape, COAX_PwDtype_bf16, (coax_v_t_P5Tensor){inv, inv_count}, (coax_v_t_PaFile){&file, 1});
    if (v.status) { fprintf(stderr, "%s: mapping status %u\n", name, v.status); exit(1); }
    return v.data;
}

static coax_t_MsLayer layer(int l) {
    char n[9][128]; const char *roles[9] = {"attention_norm", "attention.wq", "attention.wk", "attention.wv",
        "attention.wo", "ffn_norm", "feed_forward.w1", "feed_forward.w2", "feed_forward.w3"};
    for (int k = 0; k < 9; k++) snprintf(n[k], sizeof n[k], "layers.%d.%s.weight", l, roles[k]);
    return (coax_t_MsLayer){tensor(n[0], D, 0), tensor(n[1], 4096, D), tensor(n[2], 1024, D), tensor(n[3], 1024, D),
        tensor(n[4], D, 4096), tensor(n[5], D, 0), tensor(n[6], 9216, D), tensor(n[7], D, 9216), tensor(n[8], 9216, D)};
}

static const coax_t_MsShape S = {D, LAYERS, 32, 8, 128, 9216, VOCAB};

int main(int argc, char **argv) {
    if (argc != 5) { fprintf(stderr, "usage: ministral-run MODEL_DIR TOKENS STEPS OUT_DIR\n"); return 2; }
    const char *out_dir = argv[4];
    int steps = atoi(argv[3]);
    int64_t tokens[CAPACITY]; int count = 0; long long t;
    FILE *tf = fopen(argv[2], "r"); if (!tf) { perror(argv[2]); return 2; }
    while (count < CAPACITY && fscanf(tf, "%lld", &t) == 1) tokens[count++] = t;
    fclose(tf);
    if (count < 1 || steps < 1 || count + steps > CAPACITY) { fprintf(stderr, "tokens or steps out of range\n"); return 2; }
    char path[4096]; snprintf(path, sizeof path, "%s/consolidated.safetensors", argv[1]);
    int fd = open(path, O_RDONLY); if (fd < 0) { perror(path); return 2; }
    struct stat st; if (fstat(fd, &st) || st.st_size != 7698234952LL) { fprintf(stderr, "not the pinned file size\n"); return 2; }
    void *map = mmap(NULL, (size_t)st.st_size, PROT_READ, MAP_PRIVATE, fd, 0); close(fd);
    if (map == MAP_FAILED) { perror("mmap"); return 2; }
    static coax_t_P5Tensor draft[1024];
    file.data = (coax_v_u8){map, st.st_size};
    coax_t_PwResult scan = p5_inspect_checked(file.data, (coax_v_t_P5Tensor){inv, 1024}, (coax_v_t_P5Tensor){draft, 1024}, 0);
    if (scan.status || scan.written != TENSORS) { fprintf(stderr, "inventory status %u, %lld tensors\n", scan.status, (long long)scan.written); return 1; }
    inv_count = scan.written;

    coax_t_MsLayer layers[LAYERS];
    for (int l = 0; l < LAYERS; l++) layers[l] = layer(l);
    coax_t_LmModel m = md_model(tensor("tok_embeddings.weight", VOCAB, D), tensor("norm.weight", D, 0), (coax_v_t_MsLayer){layers, LAYERS});
    int64_t cache_n = lm_cache_size(S, CAPACITY), work_n = lm_work(S, CAPACITY);
    float *keys = calloc(cache_n, 4), *values = calloc(cache_n, 4), *hidden = calloc(2 * CAPACITY * D, 4);
    float *work = calloc(work_n, 4), *logits = calloc(VOCAB, 4), *again = calloc(VOCAB, 4);
    float *dump = calloc((size_t)LAYERS * CAPACITY * D, 4);
    double *stage = calloc(VOCAB, 8), *probs = calloc(VOCAB, 8);
    int64_t *order = calloc(VOCAB, 8), *temp = calloc(VOCAB, 8), out[CAPACITY], stops[1] = {2};
    if (!keys || !values || !hidden || !work || !logits || !again || !dump || !stage || !probs || !order || !temp) { fprintf(stderr, "allocation failed\n"); return 2; }
    coax_t_LmCache c = {span(keys, cache_n), span(values, cache_n), CAPACITY};
    coax_t_LmSampler k = {{0.0, 0, 1.0}, {0, 0}, {stops, 1}, {probs, VOCAB}, {{stage, VOCAB}, {order, VOCAB}, {temp, VOCAB}}};
    printf("prompt (%d tokens):", count);
    for (int i = 0; i < count; i++) printf(" %lld", (long long)tokens[i]);
    printf("\n");
    fflush(stdout);
    double t0 = now();
    coax_t_MsResult r = lm_continue(S, m, c, (coax_v_s64){tokens, count}, steps, k, (coax_v_s64){out, steps},
                                    span(hidden, 2 * CAPACITY * D), span(logits, VOCAB), span(work, work_n));
    if (r.status) { fprintf(stderr, "continue status %u written %lld cause %lld\n", r.status, (long long)r.written, (long long)r.cause); return 1; }
    printf("greedy, all 26 layers through the cache: %lld tokens, cache length %lld, %.1f s:", (long long)r.written, (long long)r.cause, now() - t0);
    for (int i = 0; i < r.written; i++) printf(" %lld", (long long)out[i]);
    printf("\n");
    fflush(stdout);

    /* The full recompute, layer by layer from position 0, keeping each
     * layer's output for the independent reference. */
    int64_t all[CAPACITY]; int n = 0;
    for (int i = 0; i < count; i++) all[n++] = tokens[i];
    for (int i = 0; i + 1 < r.written; i++) all[n++] = out[i];
    double t1 = now();
    float *x = hidden, *y = hidden + CAPACITY * D;
    coax_t_MsResult f = ms_embed(S, m.embed, (coax_v_s64){all, n}, span(x, (int64_t)n * D));
    for (int l = 0; l < LAYERS && !f.status; l++) {
        f = lm_layer_rope(S, layers[l], c, l, 0, span(x, (int64_t)n * D), n, span(y, (int64_t)n * D), span(work, work_n), COAX_LmRope_yarn);
        memcpy(dump + (size_t)l * n * D, y, (size_t)n * D * 4);
        float *swap = x; x = y; y = swap;
    }
    if (!f.status) f = lm_readout(S, m.head, m.norm, span(x + (size_t)(n - 1) * D, D), span(again, VOCAB), span(work, work_n), 32768);
    if (f.status) { fprintf(stderr, "full recompute status %u cause %lld\n", f.status, (long long)f.cause); return 1; }
    int same = memcmp(logits, again, VOCAB * 4) == 0;
    printf("full recompute over %d tokens, %.1f s: last logits %s\n", n, now() - t1, same ? "identical bits to the cached run" : "DIFFERENT");
    if (write_f32(out_dir, "layers.f32", dump, (size_t)LAYERS * n * D) || write_f32(out_dir, "logits.f32", again, VOCAB)) return 1;
    snprintf(path, sizeof path, "%s/emitted.txt", out_dir);
    FILE *ef = fopen(path, "w"); if (!ef) { perror(path); return 1; }
    for (int i = 0; i < r.written; i++) fprintf(ef, "%lld\n", (long long)out[i]);
    fclose(ef);
    return same ? 0 : 1;
}
