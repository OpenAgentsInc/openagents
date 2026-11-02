/* Chapter 58's original-weight runs of Llama-3.1-Minitron-4B-Width-Base.
 * All arithmetic is Coax (psionic/lib/minitron.coax through
 * minitron-slices-runtime.coax); this file maps the two shards, finds
 * tensors with the Coax inventory reader, and writes or prints results. The
 * files are authenticated by minitron_reference.py pin first.
 *
 *   minitron-run slices MODEL_DIR TOKENS OUT_DIR
 *       the embedding gather of the tokens, layer 0 over them in one call
 *       and again through the cache (all but two, then one, then one),
 *       compared bit for bit, and lm_head rows 0..4095 for the last token
 *   minitron-run continue MODEL_DIR TOKENS STEPS TEMPERATURE SEED [full]
 *       all 32 layers: prefill, then STEPS tokens through chapter 53's
 *       sampler (temperature 0 is greedy); with "full", recompute the last
 *       logits from position 0 and compare them bit for bit
 */
#define _POSIX_C_SOURCE 200809L
#include "minitron-slices-runtime.h"
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <time.h>
#include <unistd.h>

enum { D = 3072, VOCAB = 128256, LAYERS = 32, CAPACITY = 64, READOUT_ROWS = 4096, TENSORS = 291 };
static const char *SHARDS[2] = {"model-00001-of-00002.safetensors", "model-00002-of-00002.safetensors"};
static const int64_t SIZES[2] = {4978354640LL, 4047172128LL};

static coax_string str(const char *s) { return (coax_string){s, (int64_t)strlen(s)}; }
static coax_v_f32 span(float *p, int64_t n) { return (coax_v_f32){p, n}; }
static double now(void) { struct timespec t; clock_gettime(CLOCK_MONOTONIC, &t); return t.tv_sec + t.tv_nsec / 1e9; }

static int write_f32(const char *dir, const char *name, const float *v, size_t n) {
    char path[4096]; snprintf(path, sizeof path, "%s/%s", dir, name);
    FILE *f = fopen(path, "wb");
    if (!f || fwrite(v, sizeof(float), n, f) != n || fclose(f)) { fprintf(stderr, "cannot write %s\n", path); return 1; }
    return 0;
}

static coax_t_P5Tensor inv[1024]; static int64_t inv_count; static coax_t_PaFile files[2];

static coax_v_u8 tensor(const char *name, int64_t rows, int64_t cols) {
    coax_t_P5Shape shape = cols ? (coax_t_P5Shape){2, {{rows, cols, 0, 0, 0}}} : (coax_t_P5Shape){1, {{rows, 0, 0, 0, 0}}};
    coax_t_PaView v = pa_view(str(name), shape, COAX_PwDtype_bf16, (coax_v_t_P5Tensor){inv, inv_count}, (coax_v_t_PaFile){files, 2});
    if (v.status) { fprintf(stderr, "%s: mapping status %u\n", name, v.status); exit(1); }
    return v.data;
}

static coax_t_MsLayer layer(int l) {
    char n[9][128]; const char *roles[9] = {"input_layernorm", "self_attn.q_proj", "self_attn.k_proj", "self_attn.v_proj",
        "self_attn.o_proj", "post_attention_layernorm", "mlp.gate_proj", "mlp.down_proj", "mlp.up_proj"};
    for (int k = 0; k < 9; k++) snprintf(n[k], sizeof n[k], "model.layers.%d.%s.weight", l, roles[k]);
    return (coax_t_MsLayer){tensor(n[0], D, 0), tensor(n[1], 4096, D), tensor(n[2], 1024, D), tensor(n[3], 1024, D),
        tensor(n[4], D, 4096), tensor(n[5], D, 0), tensor(n[6], 9216, D), tensor(n[7], D, 9216), tensor(n[8], 9216, D)};
}

static void map_shards(const char *dir) {
    static coax_t_P5Tensor draft[1024];
    for (int k = 0; k < 2; k++) {
        char path[4096]; snprintf(path, sizeof path, "%s/%s", dir, SHARDS[k]);
        int fd = open(path, O_RDONLY); if (fd < 0) { perror(path); exit(2); }
        struct stat st; if (fstat(fd, &st) || st.st_size != SIZES[k]) { fprintf(stderr, "%s: not the pinned size\n", path); exit(2); }
        void *map = mmap(NULL, (size_t)st.st_size, PROT_READ, MAP_PRIVATE, fd, 0); close(fd);
        if (map == MAP_FAILED) { perror("mmap"); exit(2); }
        files[k].data = (coax_v_u8){map, st.st_size};
        coax_t_PwResult scan = p5_inspect_checked(files[k].data, (coax_v_t_P5Tensor){inv + inv_count, 1024 - inv_count},
                                                  (coax_v_t_P5Tensor){draft, 1024}, 0);
        if (scan.status) { fprintf(stderr, "%s: inventory status %u\n", path, scan.status); exit(1); }
        for (int64_t i = 0; i < scan.written; i++) inv[inv_count + i].shard = k;
        inv_count += scan.written;
    }
    if (inv_count != TENSORS) { fprintf(stderr, "%lld tensors, expected %d\n", (long long)inv_count, TENSORS); exit(1); }
}

static int read_tokens(const char *path, int64_t *tokens, int max) {
    FILE *tf = fopen(path, "r"); if (!tf) { perror(path); exit(2); }
    int count = 0; long long t;
    while (count < max && fscanf(tf, "%lld", &t) == 1) tokens[count++] = t;
    fclose(tf);
    if (count < 3) { fprintf(stderr, "need at least 3 tokens\n"); exit(2); }
    return count;
}

static const coax_t_MsShape S = {D, LAYERS, 32, 8, 128, 9216, VOCAB};

static int slices(const char *dir, const char *tokpath, const char *out) {
    int64_t tokens[CAPACITY]; int count = read_tokens(tokpath, tokens, CAPACITY);
    map_shards(dir);
    int64_t cache_n = lm_cache_size(S, CAPACITY), work_n = lm_work(S, count);
    float *keys = calloc(cache_n, 4), *values = calloc(cache_n, 4), *x = calloc(count * D, 4), *y = calloc(count * D, 4);
    float *z = calloc(count * D, 4), *work = calloc(work_n, 4), *logits = calloc(READOUT_ROWS, 4);
    if (!keys || !values || !x || !y || !z || !work || !logits) { fprintf(stderr, "allocation failed\n"); return 2; }
    coax_t_LmCache c = {span(keys, cache_n), span(values, cache_n), CAPACITY};
    coax_t_MsLayer w = layer(0);
    double t0 = now();
    coax_t_MsResult r = ms_embed(S, tensor("model.embed_tokens.weight", VOCAB, D), (coax_v_s64){tokens, count}, span(x, count * D));
    if (r.status) { fprintf(stderr, "embed status %u cause %lld\n", r.status, (long long)r.cause); return 1; }
    double t1 = now();
    printf("slice embed: %d tokens, %.3f s\n", count, t1 - t0);
    r = lm_layer_step(S, w, c, 0, 0, span(x, count * D), count, span(y, count * D), span(work, work_n));
    if (r.status) { fprintf(stderr, "layer status %u cause %lld\n", r.status, (long long)r.cause); return 1; }
    double t2 = now();
    printf("slice layer0: %d tokens in one call, %.3f s\n", count, t2 - t1);
    memset(keys, 0, cache_n * 4); memset(values, 0, cache_n * 4);
    int first = count - 2;
    r = lm_layer_step(S, w, c, 0, 0, span(x, first * D), first, span(z, first * D), span(work, work_n));
    for (int t = first; t < count && !r.status; t++)
        r = lm_layer_step(S, w, c, 0, t, span(x + t * D, D), 1, span(z + t * D, D), span(work, work_n));
    if (r.status) { fprintf(stderr, "cached layer status %u\n", r.status); return 1; }
    int same = memcmp(y, z, (size_t)count * D * 4) == 0;
    printf("slice layer0 through the cache (%d, then 1, then 1): %s\n", first, same ? "identical bits" : "DIFFERENT");
    if (!same) return 1;
    double t3 = now();
    coax_v_u8 head = tensor("lm_head.weight", VOCAB, D);
    coax_v_u8 chunk = {head.data, (int64_t)READOUT_ROWS * D * 2};
    r = ms_readout(S, chunk, READOUT_ROWS, tensor("model.norm.weight", D, 0), span(y + (count - 1) * D, D), span(logits, READOUT_ROWS), span(work, work_n));
    if (r.status) { fprintf(stderr, "readout status %u\n", r.status); return 1; }
    printf("slice readout: lm_head rows 0..%d, %.3f s\n", READOUT_ROWS, now() - t3);
    return write_f32(out, "embed.f32", x, count * D) || write_f32(out, "layer0.f32", y, count * D) || write_f32(out, "readout.f32", logits, READOUT_ROWS);
}

static int run(const char *dir, const char *tokpath, int steps, double temperature, unsigned long long seed, int full) {
    int64_t tokens[CAPACITY]; int count = read_tokens(tokpath, tokens, CAPACITY);
    if (steps < 1 || count + steps > CAPACITY) { fprintf(stderr, "steps out of range\n"); return 2; }
    map_shards(dir);
    coax_t_MsLayer layers[LAYERS];
    for (int l = 0; l < LAYERS; l++) layers[l] = layer(l);
    coax_t_LmModel m = {tensor("model.embed_tokens.weight", VOCAB, D), tensor("model.norm.weight", D, 0),
                        tensor("lm_head.weight", VOCAB, D), {layers, LAYERS}};
    int64_t cache_n = lm_cache_size(S, CAPACITY), work_n = lm_work(S, CAPACITY);
    float *keys = calloc(cache_n, 4), *values = calloc(cache_n, 4), *hidden = calloc(2 * CAPACITY * D, 4);
    float *work = calloc(work_n, 4), *logits = calloc(VOCAB, 4), *again = calloc(VOCAB, 4);
    double *stage = calloc(VOCAB, 8), *probs = calloc(VOCAB, 8);
    int64_t *order = calloc(VOCAB, 8), *temp = calloc(VOCAB, 8), out[CAPACITY], stops[1] = {128001};
    if (!keys || !values || !hidden || !work || !logits || !again || !stage || !probs || !order || !temp) { fprintf(stderr, "allocation failed\n"); return 2; }
    coax_t_LmCache c = {span(keys, cache_n), span(values, cache_n), CAPACITY};
    coax_t_LmSampler k = {{temperature, 0, 1.0}, {seed, 0}, {stops, 1}, {probs, VOCAB}, {{stage, VOCAB}, {order, VOCAB}, {temp, VOCAB}}};
    printf("prompt:");
    for (int i = 0; i < count; i++) printf(" %lld", (long long)tokens[i]);
    printf("\n");
    double t0 = now();
    coax_t_MsResult r = lm_continue(S, m, c, (coax_v_s64){tokens, count}, steps, k, (coax_v_s64){out, steps},
                                    span(hidden, 2 * CAPACITY * D), span(logits, VOCAB), span(work, work_n));
    if (r.status) { fprintf(stderr, "continue status %u written %lld cause %lld\n", r.status, (long long)r.written, (long long)r.cause); return 1; }
    printf("temperature %g seed %llu: %lld tokens, cache length %lld, %.1f s:", temperature, seed, (long long)r.written, (long long)r.cause, now() - t0);
    for (int i = 0; i < r.written; i++) printf(" %lld", (long long)out[i]);
    printf("\n");
    if (full) {
        int64_t all[CAPACITY]; int n = 0;
        for (int i = 0; i < count; i++) all[n++] = tokens[i];
        for (int i = 0; i + 1 < r.written; i++) all[n++] = out[i];
        double t1 = now();
        coax_t_MsResult f = lm_decode(S, m, c, 0, (coax_v_s64){all, n}, span(hidden, 2 * CAPACITY * D), span(again, VOCAB), span(work, work_n));
        if (f.status) { fprintf(stderr, "full decode status %u\n", f.status); return 1; }
        int same = memcmp(logits, again, VOCAB * 4) == 0;
        printf("full recompute over %d tokens, %.1f s: last logits %s\n", n, now() - t1, same ? "identical bits to the cached run" : "DIFFERENT");
        if (!same) return 1;
    }
    return 0;
}

int main(int argc, char **argv) {
    if (argc == 5 && !strcmp(argv[1], "slices")) return slices(argv[2], argv[3], argv[4]);
    if ((argc == 7 || (argc == 8 && !strcmp(argv[7], "full"))) && !strcmp(argv[1], "continue"))
        return run(argv[2], argv[3], atoi(argv[4]), atof(argv[5]), strtoull(argv[6], NULL, 10), argc == 8);
    fprintf(stderr, "usage: minitron-run slices MODEL_DIR TOKENS OUT_DIR\n"
                    "       minitron-run continue MODEL_DIR TOKENS STEPS TEMPERATURE SEED [full]\n");
    return 2;
}
