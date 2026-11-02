/* Chapter 163's whole-model run of Nemotron 3 Nano 4B on the CPU.
 * Firm reason: 5 (thin harness: maps the checkpoint, reads files and moves bytes; all arithmetic and the tokenizer are Coax)
 * All model arithmetic and the tokenizer are Coax (nemotron-run-runtime.coax);
 * this file maps the checkpoint, reads tokenizer.json, moves bytes, times
 * the steps and writes outputs for psionic/tests/nemotron_slices.py model.
 *
 *   nemotron-run MODEL_DIRECTORY RANGES TEXTS OUT_DIR STEPS
 */
#define _POSIX_C_SOURCE 200809L
#include "nemotron-run-runtime.h"
#include <fcntl.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/resource.h>
#include <sys/stat.h>
#include <time.h>
#include <unistd.h>

enum { H = 3136, VOCAB = 131072, RANGES = 300, MAX_TOKENS = 64 };
static char names[RANGES][96];
static int64_t begins[RANGES], ends_[RANGES];
static int ranges;

static double now(void) { struct timespec t; clock_gettime(CLOCK_MONOTONIC, &t); return t.tv_sec + t.tv_nsec / 1e9; }
static coax_v_f32 fv(float *p, int64_t n) { return (coax_v_f32){p, n}; }
static coax_v_s64 iv(int64_t *p, int64_t n) { return (coax_v_s64){p, n}; }
static void *must(size_t n) { void *p = calloc(1, n ? n : 1); if (!p) { fprintf(stderr, "out of memory\n"); exit(1); } return p; }
static unsigned char *slurp(const char *path, int64_t *n) {
    FILE *f = fopen(path, "rb"); if (!f) { perror(path); exit(2); }
    fseek(f, 0, SEEK_END); *n = ftell(f); fseek(f, 0, SEEK_SET);
    unsigned char *b = must((size_t)*n);
    if (fread(b, 1, (size_t)*n, f) != (size_t)*n) { fprintf(stderr, "%s: short read\n", path); exit(2); }
    fclose(f); return b;
}
static int write_bytes(const char *dir, const char *name, const void *v, size_t n) {
    char path[4096]; snprintf(path, sizeof path, "%s/%s", dir, name);
    FILE *f = fopen(path, "wb");
    if (!f || fwrite(v, 1, n, f) != n || fclose(f)) { fprintf(stderr, "cannot write %s\n", path); return 1; }
    return 0;
}
static double rss_gb(void) { struct rusage u; getrusage(RUSAGE_SELF, &u); return u.ru_maxrss / 1048576.0; }

/* The tokenizer's storage, sized for the pinned file. */
static unsigned char *json; static int64_t json_n;
static uint8_t *tb; static int64_t *tids, *tat, *tcount, *tslots, *tleft, *tright, *tjoined, *tpairs, *tspec; static coax_string *ttexts;
enum { ENTRIES = 131072, SLOTS = 262144, MERGES = 269443, PAIRS = 1048576, SPECIALS = 1000, BYTES = 4 << 20 };
#define STORE (coax_v_u8){tb, BYTES}, iv(tids, ENTRIES), iv(tat, ENTRIES), iv(tcount, ENTRIES), iv(tslots, SLOTS), iv(tleft, MERGES), \
    iv(tright, MERGES), iv(tjoined, MERGES), iv(tpairs, PAIRS), iv(tspec, SPECIALS), (coax_v_string){ttexts, SPECIALS}

int main(int argc, char **argv) {
    if (argc != 6) { fprintf(stderr, "usage: nemotron-run MODEL_DIRECTORY RANGES TEXTS OUT_DIR STEPS\n"); return 2; }
    const char *dir = argv[1], *out = argv[4];
    int64_t steps = atoll(argv[5]);
    char path[4096];
    FILE *rf = fopen(argv[2], "r"); if (!rf) { perror(argv[2]); return 2; }
    long long b, e;
    while (ranges < RANGES && fscanf(rf, "%95s %lld %lld", names[ranges], &b, &e) == 3) { begins[ranges] = b; ends_[ranges] = e; ranges++; }
    fclose(rf);

    /* The tokenizer: the fixture texts, then the prompt. */
    double t0 = now();
    snprintf(path, sizeof path, "%s/tokenizer.json", dir);
    json = slurp(path, &json_n);
    tb = must(BYTES); tids = must(8 * ENTRIES); tat = must(8 * ENTRIES); tcount = must(8 * ENTRIES); tslots = must(8 * SLOTS);
    tleft = must(8 * MERGES); tright = must(8 * MERGES); tjoined = must(8 * MERGES); tpairs = must(8 * PAIRS); tspec = must(8 * SPECIALS);
    ttexts = must(sizeof(coax_string) * SPECIALS);
    int64_t texts_n; unsigned char *texts = slurp(argv[3], &texts_n);
    int64_t *ids = must(8 * 8192), *ends = must(8 * 256);
    int64_t k = nhr_encode((coax_v_u8){json, json_n}, (coax_v_u8){texts, texts_n}, STORE, iv(ids, 8192), iv(ends, 256));
    if (k < 1) { fprintf(stderr, "tokenizer refused: %lld\n", (long long)k); return 1; }
    snprintf(path, sizeof path, "%s/coax-ids.txt", out);
    FILE *idf = fopen(path, "w"); if (!idf) return 1;
    for (int64_t i = 0, from = 0; i < k; from = ends[i], i++) {
        for (int64_t j = from; j < ends[i]; j++) fprintf(idf, j > from ? " %lld" : "%lld", (long long)ids[j]);
        fputc('\n', idf);
    }
    fclose(idf);
    /* The first text is the prompt; <s> (id 1) goes first. */
    int64_t prompt[MAX_TOKENS]; int64_t n = 1; prompt[0] = 1;
    for (int64_t j = 0; j < ends[0]; j++) prompt[n++] = ids[j];
    printf("tokenizer: %lld texts encoded over the full file, %.1f s\n", (long long)k, now() - t0);
    printf("prompt tokens");
    for (int64_t j = 0; j < n; j++) printf(" %lld", (long long)prompt[j]);
    printf("\n");

    /* The weights, decoded once into one f32 arena. */
    snprintf(path, sizeof path, "%s/model.safetensors", dir);
    int fd = open(path, O_RDONLY); if (fd < 0) { perror("open"); return 2; }
    struct stat st; if (fstat(fd, &st) || st.st_size != 7947142640LL) { fprintf(stderr, "not the pinned file size\n"); return 2; }
    const unsigned char *map = mmap(NULL, (size_t)st.st_size, PROT_READ, MAP_PRIVATE, fd, 0); close(fd);
    if (map == MAP_FAILED) { perror("mmap"); return 2; }
    double t1 = now();
    int64_t total = nhr_arena_size();
    float *arena = malloc((size_t)total * sizeof(float)); if (!arena) { fprintf(stderr, "out of memory\n"); return 1; }
    for (int64_t i = 0, at = 0; i < nhr_tensors(); i++) {
        char name[128]; int64_t len = nhr_tensor_name(i, (coax_v_u8){(uint8_t *)name, 127}); name[len] = 0;
        int64_t count = nhr_tensor_size(i); int r;
        for (r = 0; r < ranges && strcmp(names[r], name); r++) {}
        if (r == ranges || ends_[r] - begins[r] != 2 * count) { fprintf(stderr, "%s: missing or wrong size\n", name); return 1; }
        if (!nhr_decode((coax_v_u8){(uint8_t *)map + begins[r], 2 * count}, fv(arena + at, count))) { fprintf(stderr, "%s: decode failed\n", name); return 1; }
        at += count;
    }
    munmap((void *)map, (size_t)st.st_size);
    printf("weights: %lld values decoded from BF16, %.1f s\n", (long long)total, now() - t1);

    int64_t cap = n + steps, total_tokens = n + steps - 1;
    float *history = must(4 * (size_t)nhr_history_size()), *state = must(4 * (size_t)nhr_state_size());
    float *keys = must(4 * (size_t)nhr_keys_size(cap)), *values = must(4 * (size_t)nhr_keys_size(cap));
    float *work = must(4 * (size_t)nhr_work(cap)); int64_t fneed = nhr_fwork(cap, cap); double *fwork = must(8 * (size_t)fneed);
    float *hidden = must(4 * (size_t)cap * H), *logits = must(4 * VOCAB), *last = must(4 * VOCAB), *again = must(4 * VOCAB);
    float *trace = must(4 * (size_t)42 * n * H);
    #define CACHE cap, fv(history, nhr_history_size()), fv(state, nhr_state_size()), fv(keys, nhr_keys_size(cap)), fv(values, nhr_keys_size(cap))
    #define WORK(t) fv(work, nhr_work(t)), (coax_v_f64){fwork, fneed}

    /* The prompt layer by layer, kept for the oracle, then its logits. */
    double t2 = now();
    nhr_clear(CACHE);
    if (nhr_trace(fv(arena, total), iv(prompt, n), n, CACHE, fv(hidden, n * H), fv(trace, 42 * n * H), WORK(n))) { fprintf(stderr, "trace refused\n"); return 1; }
    printf("prompt through 42 layers, layer by layer: %.1f s\n", now() - t2);
    if (write_bytes(out, "trace.f32", trace, 4 * (size_t)42 * n * H)) return 1;

    /* Greedy decoding: the prompt at once, then one token at a time. */
    int64_t chosen[MAX_TOKENS];
    nhr_clear(CACHE);
    double t3 = now();
    int64_t best = nhr_forward(fv(arena, total), iv(prompt, n), 0, n, CACHE, fv(hidden, n * H), fv(logits, VOCAB), WORK(n));
    if (best < 0) { fprintf(stderr, "prompt refused: %lld\n", (long long)best); return 1; }
    double prefill = now() - t3;
    if (write_bytes(out, "logits-prompt.f32", logits, 4 * VOCAB)) return 1;
    nhr_clear(CACHE);
    double t4 = now();
    int64_t r = nhr_greedy(fv(arena, total), iv(prompt, n), steps, n, iv(chosen, steps), CACHE, fv(hidden, n * H), fv(last, VOCAB), WORK(n));
    if (r < 0) { fprintf(stderr, "greedy refused: %lld\n", (long long)r); return 1; }
    double greedy = now() - t4;
    if (write_bytes(out, "logits-last.f32", last, 4 * VOCAB)) return 1;
    printf("prefill of %lld tokens: %.1f s; greedy, prefill and %lld steps: %.1f s (%.1f s a step)\n",
        (long long)n, prefill, (long long)steps - 1, greedy, (greedy - prefill) / (double)(steps - 1));
    printf("greedy tokens");
    for (int64_t j = 0; j < steps; j++) printf(" %lld", (long long)chosen[j]);
    printf("\n");
    printf("first choice equals the prompt pass: %s\n", chosen[0] == best ? "true" : "false");

    /* The whole sequence recomputed from an empty cache in one pass. */
    int64_t all[MAX_TOKENS];
    for (int64_t j = 0; j < n; j++) all[j] = prompt[j];
    for (int64_t j = 0; j + 1 < steps; j++) all[n + j] = chosen[j];
    nhr_clear(CACHE);
    double t5 = now();
    int64_t whole = nhr_forward(fv(arena, total), iv(all, total_tokens), 0, total_tokens, CACHE, fv(hidden, total_tokens * H), fv(again, VOCAB), WORK(total_tokens));
    if (whole < 0) { fprintf(stderr, "recompute refused: %lld\n", (long long)whole); return 1; }
    double worst = 0, big = 0;
    for (int i = 0; i < VOCAB; i++) {
        double d = last[i] - again[i]; if (d < 0) d = -d; if (d > worst) worst = d;
        double a = again[i] < 0 ? -again[i] : again[i]; if (a > big) big = a;
    }
    printf("recomputed %lld tokens in one pass: %.1f s; last logits differ by at most %.3g (largest |logit| %.3g); choice %lld, cached %lld, %s\n",
        (long long)total_tokens, now() - t5, worst, big, (long long)whole, (long long)chosen[steps - 1], whole == chosen[steps - 1] ? "same" : "DIFFERENT");

    /* The continuation as text. */
    unsigned char text[4096];
    int64_t len = nhr_text((coax_v_u8){json, json_n}, iv(chosen, steps), STORE, (coax_v_u8){text, sizeof text - 1});
    if (len < 0) { fprintf(stderr, "decode refused\n"); return 1; }
    text[len] = 0;
    for (int64_t i = 0; i < len; i++) if (text[i] == '\n') text[i] = '|';
    printf("continuation: \"%s\" (line feeds shown as |)\n", text);
    snprintf(path, sizeof path, "%s/run-tokens.txt", out);
    FILE *tf = fopen(path, "w"); if (!tf) return 1;
    for (int64_t j = 0; j < n; j++) fprintf(tf, "%lld ", (long long)prompt[j]);
    fprintf(tf, "\n");
    for (int64_t j = 0; j < steps; j++) fprintf(tf, "%lld ", (long long)chosen[j]);
    fprintf(tf, "\n");
    fclose(tf);
    printf("whole run: %.1f s, peak resident memory %.1f GB\n", now() - t0, rss_gb());
    return 0;
}
