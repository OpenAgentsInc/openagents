/* Chapter 81's original-weight slices of Ministral 3 3B: the embedding
 * gather of the prompt's tokens, layer 0 over them, and readout rows
 * 0..4095 for the last token after layer 0. The file is authenticated by
 * ministral_reference.py pin first. All arithmetic is Coax
 * (psionic/lib/ministral.coax through ministral-slices-runtime.coax); this
 * file maps the file, finds tensors with the Coax inventory reader, and
 * writes the outputs.
 *
 *   ministral-slices CONSOLIDATED_SAFETENSORS TOKENS OUT_DIR
 */
#define _POSIX_C_SOURCE 200809L
#include "ministral-slices-runtime.h"
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <time.h>
#include <unistd.h>

enum { D = 3072, VOCAB = 131072, MAX_TOKENS = 64, READOUT_ROWS = 4096, TENSORS = 458 };

static coax_string str(const char *s) { return (coax_string){s, (int64_t)strlen(s)}; }
static coax_v_f32 span(float *p, int64_t n) { return (coax_v_f32){p, n}; }
static double now(void) { struct timespec t; clock_gettime(CLOCK_MONOTONIC, &t); return t.tv_sec + t.tv_nsec / 1e9; }

static int write_f32(const char *dir, const char *name, const float *v, size_t n) {
    char path[4096]; snprintf(path, sizeof path, "%s/%s", dir, name);
    FILE *f = fopen(path, "wb");
    if (!f || fwrite(v, sizeof(float), n, f) != n || fclose(f)) { fprintf(stderr, "cannot write %s\n", path); return 1; }
    return 0;
}

static coax_t_P5Tensor *inv; static int64_t inv_count; static coax_t_PaFile file;

static coax_v_u8 tensor(const char *name, int64_t rows, int64_t cols) {
    coax_t_P5Shape shape = cols ? (coax_t_P5Shape){2, {{rows, cols, 0, 0, 0}}} : (coax_t_P5Shape){1, {{rows, 0, 0, 0, 0}}};
    coax_t_PaView v = pa_view(str(name), shape, COAX_PwDtype_bf16, (coax_v_t_P5Tensor){inv, inv_count}, (coax_v_t_PaFile){&file, 1});
    if (v.status) { fprintf(stderr, "%s: mapping status %u\n", name, v.status); exit(1); }
    return v.data;
}

int main(int argc, char **argv) {
    if (argc != 4) { fprintf(stderr, "usage: ministral-slices CONSOLIDATED_SAFETENSORS TOKENS OUT_DIR\n"); return 2; }
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

    coax_t_MsShape s = {D, 26, 32, 8, 128, 9216, VOCAB};
    coax_v_u8 embed = tensor("tok_embeddings.weight", VOCAB, D), norm = tensor("norm.weight", D, 0);
    coax_t_MsLayer w = {
        tensor("layers.0.attention_norm.weight", D, 0), tensor("layers.0.attention.wq.weight", 4096, D),
        tensor("layers.0.attention.wk.weight", 1024, D), tensor("layers.0.attention.wv.weight", 1024, D),
        tensor("layers.0.attention.wo.weight", D, 4096), tensor("layers.0.ffn_norm.weight", D, 0),
        tensor("layers.0.feed_forward.w1.weight", 9216, D), tensor("layers.0.feed_forward.w2.weight", D, 9216),
        tensor("layers.0.feed_forward.w3.weight", 9216, D)};
    int64_t need = ms_layer_work(s, count);
    if (need < D + READOUT_ROWS) need = D + READOUT_ROWS;
    float *x = calloc((size_t)(count * D), sizeof(float)), *y = calloc((size_t)(count * D), sizeof(float));
    float *work = calloc((size_t)need, sizeof(float)), *logits = calloc(READOUT_ROWS, sizeof(float));
    if (!x || !y || !work || !logits || !inv || !draft) { fprintf(stderr, "allocation failed\n"); return 2; }
    const char *out = argv[3];

    double t0 = now();
    coax_t_MsResult r = ms_embed(s, embed, (coax_v_s64){tokens, count}, span(x, count * D));
    if (r.status) { fprintf(stderr, "embed status %u cause %lld\n", r.status, (long long)r.cause); return 1; }
    double t1 = now();
    printf("slice embed: %lld tokens, %.3f s\n", (long long)count, t1 - t0);
    r = ms_layer(s, w, span(x, count * D), count, span(y, count * D), span(work, need));
    if (r.status) { fprintf(stderr, "layer status %u cause %lld\n", r.status, (long long)r.cause); return 1; }
    double t2 = now();
    printf("slice layer0: %lld tokens, %.3f s\n", (long long)count, t2 - t1);
    coax_v_u8 chunk = {embed.data, (int64_t)READOUT_ROWS * D * 2};
    r = ms_readout(s, chunk, READOUT_ROWS, norm, span(y + (count - 1) * D, D), span(logits, READOUT_ROWS), span(work, need));
    if (r.status) { fprintf(stderr, "readout status %u cause %lld\n", r.status, (long long)r.cause); return 1; }
    printf("slice readout: rows 0..%d, %.3f s\n", READOUT_ROWS, now() - t2);
    if (write_f32(out, "embed.f32", x, (size_t)(count * D)) || write_f32(out, "layer0.f32", y, (size_t)(count * D)) ||
        write_f32(out, "readout.f32", logits, READOUT_ROWS))
        return 1;
    free(x); free(y); free(work); free(logits); free(inv); free(draft); munmap(map, (size_t)st.st_size);
    return 0;
}
