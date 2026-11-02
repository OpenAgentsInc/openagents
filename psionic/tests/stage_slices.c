/* Chapter 186's original-weight slice of Ministral 3 3B: the first two
 * layers and the full readout over the prompt's tokens, once whole and once
 * split into two stages of one layer each, with the hidden rows crossing a
 * checked boundary message. The logits must agree bit for bit. Then stage 1
 * is given another artifact's digest and must refuse the message. The file
 * is authenticated by ministral_reference.py pin first. All arithmetic and
 * every check is Coax (psionic/lib/stage-run.coax through
 * stage-slices-runtime.coax); this file maps the file and finds tensors.
 *
 *   stage-slices CONSOLIDATED_SAFETENSORS TOKENS
 */
#define _POSIX_C_SOURCE 200809L
#include "stage-slices-runtime.h"
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <time.h>
#include <unistd.h>

enum { D = 3072, VOCAB = 131072, MAX_TOKENS = 64, TENSORS = 458 };

static coax_string str(const char *s) { return (coax_string){s, (int64_t)strlen(s)}; }
static coax_v_f32 span(float *p, int64_t n) { return (coax_v_f32){p, n}; }
static double now(void) { struct timespec t; clock_gettime(CLOCK_MONOTONIC, &t); return t.tv_sec + t.tv_nsec / 1e9; }

static coax_t_P5Tensor *inv; static int64_t inv_count; static coax_t_PaFile file;

static coax_v_u8 tensor(const char *name, int64_t rows, int64_t cols) {
    coax_t_P5Shape shape = cols ? (coax_t_P5Shape){2, {{rows, cols, 0, 0, 0}}} : (coax_t_P5Shape){1, {{rows, 0, 0, 0, 0}}};
    coax_t_PaView v = pa_view(str(name), shape, COAX_PwDtype_bf16, (coax_v_t_P5Tensor){inv, inv_count}, (coax_v_t_PaFile){&file, 1});
    if (v.status) { fprintf(stderr, "%s: mapping status %u\n", name, v.status); exit(1); }
    return v.data;
}

static coax_t_MsLayer layer(int l) {
    char n[9][96]; const char *suffix[9] = {"attention_norm.weight", "attention.wq.weight", "attention.wk.weight", "attention.wv.weight",
        "attention.wo.weight", "ffn_norm.weight", "feed_forward.w1.weight", "feed_forward.w2.weight", "feed_forward.w3.weight"};
    for (int k = 0; k < 9; k++) snprintf(n[k], sizeof n[k], "layers.%d.%s", l, suffix[k]);
    return (coax_t_MsLayer){tensor(n[0], D, 0), tensor(n[1], 4096, D), tensor(n[2], 1024, D), tensor(n[3], 1024, D),
        tensor(n[4], D, 4096), tensor(n[5], D, 0), tensor(n[6], 9216, D), tensor(n[7], D, 9216), tensor(n[8], 9216, D)};
}

int main(int argc, char **argv) {
    if (argc != 3) { fprintf(stderr, "usage: stage-slices CONSOLIDATED_SAFETENSORS TOKENS\n"); return 2; }
    int64_t tokens[MAX_TOKENS]; int64_t count = 0; long long t;
    FILE *tf = fopen(argv[2], "r"); if (!tf) { perror(argv[2]); return 2; }
    while (count < MAX_TOKENS && fscanf(tf, "%lld", &t) == 1) tokens[count++] = t;
    fclose(tf);
    if (count < 1) { fprintf(stderr, "no tokens\n"); return 2; }
    int fd = open(argv[1], O_RDONLY); if (fd < 0) { perror("open"); return 2; }
    struct stat st; if (fstat(fd, &st) || st.st_size != 7698234952LL) { fprintf(stderr, "not the pinned file size\n"); return 2; }
    void *map = mmap(NULL, (size_t)st.st_size, PROT_READ, MAP_PRIVATE, fd, 0); close(fd);
    if (map == MAP_FAILED) { perror("mmap"); return 2; }
    inv = calloc(1024, sizeof *inv); coax_t_P5Tensor *draft = calloc(1024, sizeof *draft);
    coax_v_u8 bytes = {map, st.st_size};
    coax_t_PwResult scan = p5_inspect_checked(bytes, (coax_v_t_P5Tensor){inv, 1024}, (coax_v_t_P5Tensor){draft, 1024}, 0);
    if (scan.status || scan.written != TENSORS) { fprintf(stderr, "inventory status %u, %lld tensors\n", scan.status, (long long)scan.written); return 1; }
    inv_count = scan.written; file.data = bytes;

    /* The first two layers as a model of their own. */
    coax_t_MsShape s = {D, 2, 32, 8, 128, 9216, VOCAB};
    coax_v_u8 embed = tensor("tok_embeddings.weight", VOCAB, D), norm = tensor("norm.weight", D, 0);
    coax_t_MsLayer layers[2] = {layer(0), layer(1)};
    coax_v_t_MsLayer view = {layers, 2};
    int64_t floats = stg_slice_floats(s, count), words = stg_slice_words(s, count);
    float *hidden = calloc((size_t)(2 * count * D), sizeof(float)), *work = calloc((size_t)floats, sizeof(float));
    float *whole = calloc(VOCAB, sizeof(float)), *split = calloc(VOCAB, sizeof(float)), *mem = calloc((size_t)(2 * floats), sizeof(float));
    uint32_t *wire = calloc((size_t)words, sizeof(uint32_t));
    if (!hidden || !work || !whole || !split || !mem || !wire || !inv || !draft) { fprintf(stderr, "allocation failed\n"); return 2; }
    coax_v_s64 toks = {tokens, count};

    double t0 = now();
    coax_t_MsResult r = stg_slice_whole(s, embed, norm, view, toks, span(hidden, 2 * count * D), span(whole, VOCAB), span(work, floats));
    if (r.status) { fprintf(stderr, "whole status %u cause %lld\n", r.status, (long long)r.cause); return 1; }
    double t1 = now();
    printf("whole: layers 0..<2 and the readout over %lld tokens, %.3f s\n", (long long)count, t1 - t0);
    coax_t_StgRun run = stg_slice_split(s, 0x722430146632596cULL, embed, norm, view, toks, span(split, VOCAB), span(mem, 2 * floats),
        (coax_v_u32){wire, words});
    if (run.status) { fprintf(stderr, "split status %u refusal %u stage %lld\n", run.status, run.refusal, (long long)run.stage); return 1; }
    printf("split: stage 0 runs layer 0, stage 1 runs layer 1 and the readout, %.3f s\n", now() - t1);
    printf("boundary: %lld message of %lld bytes (%lld rows of %d f32 and a 36-byte header)\n",
        (long long)run.messages, (long long)run.bytes, (long long)count, D);
    int64_t differ = 0, best = 0;
    for (int64_t i = 0; i < VOCAB; i++) {
        uint32_t a, b; memcpy(&a, &whole[i], 4); memcpy(&b, &split[i], 4);
        if (a != b) differ++;
        if (whole[i] > whole[best]) best = i;
    }
    printf("logits differing in any bit: %lld of %d; greedy token %lld both ways\n", (long long)differ, VOCAB, (long long)best);
    if (differ || run.messages != 1 || run.bytes != 4 * (9 + count * D)) return 1;
    run = stg_slice_split(s, 0x722430146632596dULL, embed, norm, view, toks, span(split, VOCAB), span(mem, 2 * floats),
        (coax_v_u32){wire, words});
    printf("stage 1 holding another artifact: status %u, refusal %u (model), stage %lld\n", run.status, run.refusal, (long long)run.stage);
    if (run.status != COAX_StgRunStatus_refused || run.refusal != COAX_StgRefusal_model || run.stage != 1) return 1;
    free(hidden); free(work); free(whole); free(split); free(mem); free(wire); free(inv); free(draft); munmap(map, (size_t)st.st_size);
    return 0;
}
