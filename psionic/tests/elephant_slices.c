/* Original-weight slices: the embedding gather, linear layer 0, full-attention
 * layer 3 and a bounded vocabulary-readout chunk over a handful of tokens.
 * The tensor bytes are authenticated by elephant_slices.py first. All model
 * arithmetic is Coax (elephant-slices-runtime.coax); this file maps the file,
 * reads inputs and writes outputs.
 *
 *   elephant-slices MODEL TOKENS OUT_DIR                        chain: each slice feeds the next
 *   elephant-slices MODEL TOKENS OUT_DIR FULL_INPUT READOUT_INPUT
 *                                       recorded inputs for layer 3 and the readout
 */
#define _POSIX_C_SOURCE 200809L
#include "elephant-slices-runtime.h"
#include <fcntl.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <time.h>
#include <unistd.h>

enum { D = 4096, VOCAB = 248320, MAX_TOKENS = 16, READOUT_ROWS = 4096, LINEAR_LAYER = 0, FULL_LAYER = 3 };

static coax_string str(const char *s) { return (coax_string){s, (int64_t)strlen(s)}; }
static coax_v_f32 span(float *p, int64_t n) { return (coax_v_f32){p, n}; }
static double now(void) { struct timespec t; clock_gettime(CLOCK_MONOTONIC, &t); return t.tv_sec + t.tv_nsec / 1e9; }

static int write_f32(const char *dir, const char *name, const float *v, size_t n) {
    char path[4096]; snprintf(path, sizeof path, "%s/%s", dir, name);
    FILE *f = fopen(path, "wb");
    if (!f || fwrite(v, sizeof(float), n, f) != n || fclose(f)) { fprintf(stderr, "cannot write %s\n", path); return 1; }
    return 0;
}
static int read_f32(const char *path, float *v, size_t n) {
    FILE *f = fopen(path, "rb");
    if (!f) { perror(path); return 1; }
    size_t got = fread(v, sizeof(float), n, f);
    int extra = fgetc(f) != EOF;
    fclose(f);
    if (got != n || extra) { fprintf(stderr, "%s: expected %zu floats\n", path, n); return 1; }
    return 0;
}

int main(int argc, char **argv) {
    if (argc != 4 && argc != 6) {
        fprintf(stderr, "usage: elephant-slices MODEL_SAFETENSORS TOKENS OUT_DIR [FULL_INPUT_F32 READOUT_INPUT_F32]\n");
        return 2;
    }
    int64_t tokens[MAX_TOKENS]; int64_t count = 0;
    FILE *tf = fopen(argv[2], "r"); if (!tf) { perror(argv[2]); return 2; }
    long long t;
    while (count < MAX_TOKENS && fscanf(tf, "%lld", &t) == 1) tokens[count++] = t;
    fclose(tf);
    if (count < 1) { fprintf(stderr, "no tokens\n"); return 2; }

    int fd = open(argv[1], O_RDONLY); if (fd < 0) { perror("open"); return 2; }
    struct stat st; if (fstat(fd, &st) || st.st_size != 18819722392LL) { fprintf(stderr, "not the pinned file size\n"); return 2; }
    void *map = mmap(NULL, (size_t)st.st_size, PROT_READ, MAP_PRIVATE, fd, 0); close(fd);
    if (map == MAP_FAILED) { perror("mmap"); return 2; }

    coax_t_P5Tensor *inv = calloc(4096, sizeof *inv), *draft = calloc(4096, sizeof *draft);
    float *params = calloc(2 * 50000, sizeof(float)), *temporary = calloc(50000, sizeof(float));
    float *embedded = calloc((size_t)count * D, sizeof(float)), *linear = calloc((size_t)count * D, sizeof(float));
    float *full_input = calloc((size_t)count * D, sizeof(float)), *full = calloc((size_t)count * D, sizeof(float));
    float *hidden = calloc(D, sizeof(float)), *scale = calloc(D, sizeof(float)), *logits = calloc(READOUT_ROWS, sizeof(float));
    if (!inv || !draft || !params || !temporary || !embedded || !linear || !full_input || !full || !hidden || !scale || !logits) {
        fprintf(stderr, "allocation failed\n"); return 2;
    }
    coax_v_u8 bytes = {map, st.st_size};
    coax_t_PwResult scan = p5_inspect_checked(bytes, (coax_v_t_P5Tensor){inv, 4096}, (coax_v_t_P5Tensor){draft, 4096}, 0);
    if (scan.status || scan.written != 760) { fprintf(stderr, "inventory %u\n", scan.status); return 1; }
    coax_t_PaFile file = {bytes}; coax_v_t_PaFile files = {&file, 1}; coax_v_t_P5Tensor inventory = {inv, scan.written};
    coax_t_PdShape delta = {16, 32, 128, 128, 0.000001f}; coax_t_QfShape attention = {16, 4, 256, 64, 0.000001f};
    coax_t_QeShape shape = {.linear = {D, 12288, delta, COAX_QmDtype_bf16}, .full = {D, 12288, 1, attention, COAX_QmDtype_bf16},
                            .state = {24, 8, 1, delta, attention}};
    coax_t_P5Shape matrix = {2, {{VOCAB, D, 0, 0, 0}}}, vector = {1, {{D, 0, 0, 0, 0}}};
    coax_t_PaView embedding = pa_view(str("model.language_model.embed_tokens.weight"), matrix, COAX_PwDtype_bf16, inventory, files);
    coax_t_PaView norm = pa_view(str("model.language_model.norm.weight"), vector, COAX_PwDtype_bf16, inventory, files);
    coax_t_PaView head = pa_view(str("lm_head.weight"), matrix, COAX_PwDtype_bf16, inventory, files);
    if (embedding.status || norm.status || head.status) { fprintf(stderr, "embedding/norm/head mapping failed\n"); return 1; }
    const char *out = argv[3];
    double t0 = now();

    coax_t_EsResult e = es_embed(embedding.data, D, (coax_v_s64){tokens, count}, span(embedded, count * D), span(temporary, D));
    if (e.status) { fprintf(stderr, "embed status %u cause %lld\n", e.status, (long long)e.cause); return 1; }
    double t1 = now();
    printf("slice embed: %lld tokens, %.3f s\n", (long long)count, t1 - t0);

    int64_t layers[2] = {LINEAR_LAYER, FULL_LAYER};
    coax_t_QeLayer bound[2];
    for (int i = 0; i < 2; i++) {
        coax_t_PmResult r = pm_layer(shape, layers[i], inventory, files, span(params + i * 50000, 50000), span(temporary, 50000), 0);
        if (r.status) { fprintf(stderr, "layer %lld mapping %u\n", (long long)layers[i], r.status); return 1; }
        bound[i] = r.layer;
    }
    if (bound[0].kind != 0 || bound[1].kind != 1) { fprintf(stderr, "unexpected layer kinds\n"); return 1; }
    int64_t need = es_layer_work(shape, bound[0].kind, count), need_full = es_layer_work(shape, bound[1].kind, count);
    if (need_full > need) need = need_full;
    float *work = calloc((size_t)need, sizeof(float)); if (!work) { fprintf(stderr, "allocation failed\n"); return 2; }
    double t2 = now();
    coax_t_EsResult l = es_layer(shape, bound[0], span(embedded, count * D), count, span(linear, count * D), span(work, need));
    if (l.status) { fprintf(stderr, "linear layer status %u cause %lld\n", l.status, (long long)l.cause); return 1; }
    double t3 = now();
    printf("slice linear0: %lld tokens, %.3f s\n", (long long)count, t3 - t2);

    if (argc == 6) { if (read_f32(argv[4], full_input, (size_t)(count * D))) return 1; }
    else memcpy(full_input, linear, sizeof(float) * (size_t)(count * D));
    coax_t_EsResult f = es_layer(shape, bound[1], span(full_input, count * D), count, span(full, count * D), span(work, need));
    if (f.status) { fprintf(stderr, "full layer status %u cause %lld\n", f.status, (long long)f.cause); return 1; }
    double t4 = now();
    printf("slice full3: %lld tokens, %.3f s\n", (long long)count, t4 - t3);

    if (argc == 6) { if (read_f32(argv[5], hidden, D)) return 1; }
    else memcpy(hidden, full + (count - 1) * D, sizeof(float) * D);
    coax_t_PwResult s = pa_decode(norm, span(scale, D), span(temporary, D), 0);
    if (s.status) { fprintf(stderr, "norm decode %u\n", s.status); return 1; }
    coax_v_u8 chunk = {head.data.data, (int64_t)READOUT_ROWS * D * 2};
    coax_t_EsResult o = es_readout(D, attention.epsilon, span(hidden, D), span(scale, D), chunk, READOUT_ROWS, span(logits, READOUT_ROWS), span(work, need));
    if (o.status) { fprintf(stderr, "readout status %u cause %lld\n", o.status, (long long)o.cause); return 1; }
    double t5 = now();
    printf("slice readout: rows 0..%d, %.3f s\n", READOUT_ROWS, t5 - t4);

    if (write_f32(out, "embed.f32", embedded, (size_t)(count * D)) || write_f32(out, "linear0.f32", linear, (size_t)(count * D)) ||
        write_f32(out, "full3.f32", full, (size_t)(count * D)) || write_f32(out, "readout-input.f32", hidden, D) ||
        write_f32(out, "readout.f32", logits, READOUT_ROWS))
        return 1;
    free(work); free(inv); free(draft); free(params); free(temporary); free(embedded); free(linear); free(full_input); free(full);
    free(hidden); free(scale); free(logits); munmap(map, (size_t)st.st_size);
    return 0;
}
