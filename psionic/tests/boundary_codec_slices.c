/* Chapter 193's original-weight slice of Ministral 3 3B: the residual rows
 * after layer 0 and after layer 1 over the prompt's tokens, packed as
 * chapter 186's boundary messages, framed by each codec of
 * psionic/lib/boundary-codec.coax, unframed, received, and compared bit for
 * bit. It reports bytes, encode and decode time, and what one decode-step
 * token (one row) costs on chapter 182's links. The file is authenticated
 * by ministral_reference.py pin first. All arithmetic and every check is
 * Coax (through boundary-codec-slices-runtime.coax); this file maps the
 * file, finds tensors and keeps the clock.
 *
 *   boundary-codec-slices CONSOLIDATED_SAFETENSORS TOKENS [FIXTURE.coax]
 *
 * With FIXTURE.coax it also writes the first 256 values of the first three
 * rows after layer 0 as the checks' real rows.
 */
#define _POSIX_C_SOURCE 200809L
#include "boundary-codec-slices-runtime.h"
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <time.h>
#include <unistd.h>

enum { D = 3072, VOCAB = 131072, MAX_TOKENS = 64, TENSORS = 458, REPS = 20 };

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

static const char *names[3] = {"raw", "planes", "delta"};
static uint8_t *frame, *work; static uint32_t *msg, *back; static float *rows_back;

/* Frame a message with a codec REPS times, unframe it REPS times, check the
 * words and the received rows bit for bit. Returns the frame's bytes and
 * the mean encode and decode seconds. */
typedef struct { int64_t bytes; coax_t_BcFrame f; double enc, dec; } Trial;
static Trial trial(coax_t_BcCodec codec, int64_t words, int64_t first, int64_t start, const float *rows) {
    int64_t n = words - 9; Trial t = {0};
    double t0 = now();
    for (int r = 0; r < REPS; r++) t.f = bc_encode(codec, (coax_v_u32){msg, words}, (coax_v_u8){frame, bc_bound(n)}, (coax_v_u8){work, bc_work(n)});
    double t1 = now();
    if (!t.f.bytes) { fprintf(stderr, "%s: the encoder refused\n", names[codec]); exit(1); }
    coax_t_BcDecoded d = {0};
    for (int r = 0; r < REPS; r++) d = bc_decode(7, (coax_v_u8){frame, t.f.bytes}, (coax_v_u32){back, words});
    double t2 = now();
    if (d.refusal != COAX_BcRefusal_none || d.words != words || memcmp(back, msg, (size_t)words * 4)) {
        fprintf(stderr, "%s: refusal %u, or the words differ\n", names[codec], d.refusal); exit(1);
    }
    coax_t_StgReceived got = bc_slice_receive(first, start, D, (coax_v_u32){back, words}, span(rows_back, n));
    if (got.refusal != COAX_StgRefusal_none || memcmp(rows_back, rows, (size_t)n * 4)) { fprintf(stderr, "%s: the rows differ\n", names[codec]); exit(1); }
    t.bytes = t.f.bytes; t.enc = (t1 - t0) / REPS; t.dec = (t2 - t1) / REPS;
    return t;
}

int main(int argc, char **argv) {
    if (argc != 3 && argc != 4) { fprintf(stderr, "usage: boundary-codec-slices CONSOLIDATED_SAFETENSORS TOKENS [FIXTURE.coax]\n"); return 2; }
    int64_t tokens[MAX_TOKENS]; int64_t count = 0; long long tk;
    FILE *tf = fopen(argv[2], "r"); if (!tf) { perror(argv[2]); return 2; }
    while (count < MAX_TOKENS && fscanf(tf, "%lld", &tk) == 1) tokens[count++] = tk;
    fclose(tf);
    if (count < 3) { fprintf(stderr, "fewer than 3 tokens\n"); return 2; }
    int fd = open(argv[1], O_RDONLY); if (fd < 0) { perror("open"); return 2; }
    struct stat st; if (fstat(fd, &st) || st.st_size != 7698234952LL) { fprintf(stderr, "not the pinned file size\n"); return 2; }
    void *map = mmap(NULL, (size_t)st.st_size, PROT_READ, MAP_PRIVATE, fd, 0); close(fd);
    if (map == MAP_FAILED) { perror("mmap"); return 2; }
    inv = calloc(1024, sizeof *inv); coax_t_P5Tensor *draft = calloc(1024, sizeof *draft);
    coax_v_u8 bytes = {map, st.st_size};
    coax_t_PwResult scan = p5_inspect_checked(bytes, (coax_v_t_P5Tensor){inv, 1024}, (coax_v_t_P5Tensor){draft, 1024}, 0);
    if (scan.status || scan.written != TENSORS) { fprintf(stderr, "inventory status %u, %lld tensors\n", scan.status, (long long)scan.written); return 1; }
    inv_count = scan.written; file.data = bytes;

    coax_t_MsShape s = {D, 2, 32, 8, 128, 9216, VOCAB};
    coax_v_u8 embed = tensor("tok_embeddings.weight", VOCAB, D);
    coax_t_MsLayer layers[2] = {layer(0), layer(1)};
    int64_t n = count * D, floats = bc_slice_floats(s, count);
    float *hidden = calloc((size_t)(3 * n), sizeof(float)), *wk = calloc((size_t)floats, sizeof(float));
    rows_back = calloc((size_t)n, sizeof(float));
    msg = calloc((size_t)(9 + n), 4); back = calloc((size_t)(9 + n), 4);
    frame = calloc((size_t)bc_bound(n), 1); work = calloc((size_t)bc_work(n), 1);
    if (!hidden || !wk || !rows_back || !msg || !back || !frame || !work || !inv || !draft) { fprintf(stderr, "allocation failed\n"); return 2; }
    coax_t_MsResult r = bc_slice_rows(s, embed, (coax_v_t_MsLayer){layers, 2}, (coax_v_s64){tokens, count}, span(hidden, 3 * n), span(wk, floats));
    if (r.status) { fprintf(stderr, "layers: status %u cause %lld\n", r.status, (long long)r.cause); return 1; }
    printf("prompt: %lld tokens; rows of %d f32 after layer 0 and after layer 1\n", (long long)count, D);

    for (int b = 1; b <= 2; b++) {
        const float *rows = hidden + b * n;
        /* The whole prompt in one message, as chapter 186's prefill sends it. */
        int64_t words = bc_slice_pack(b, 0, D, span((float *)rows, n), (coax_v_u32){msg, 9 + n});
        if (words != 9 + n) { fprintf(stderr, "pack refused\n"); return 1; }
        printf("boundary after layer %d, %lld rows in one message of %lld bytes:\n", b - 1, (long long)count, (long long)(4 * words));
        for (int c = 0; c < 3; c++) {
            Trial t = trial((coax_t_BcCodec)c, words, b, 0, rows);
            printf("  %-6s %8lld bytes, ratio %.3f, planes %lld/%lld/%lld/%lld bytes, encode %7.1f us (%5.0f MB/s), decode %7.1f us (%5.0f MB/s), exact\n",
                names[c], (long long)t.bytes, (double)(4 * words) / (double)t.bytes,
                (long long)t.f.plane.e[0], (long long)t.f.plane.e[1], (long long)t.f.plane.e[2], (long long)t.f.plane.e[3],
                t.enc * 1e6, 4.0 * n / t.enc / 1e6, t.dec * 1e6, 4.0 * n / t.dec / 1e6);
        }
        /* One row a message, as each decode step sends it. The cost of
         * planes is its CPU time beyond the raw frame's (both checksum). */
        int64_t raw = 0, coded = 0; double cpu_raw = 0, cpu_planes = 0;
        for (int64_t p = 0; p < count; p++) {
            int64_t w = bc_slice_pack(b, p, D, span((float *)rows + p * D, D), (coax_v_u32){msg, 9 + D});
            Trial a = trial(COAX_BcCodec_raw, w, b, p, rows + p * D), t = trial(COAX_BcCodec_planes, w, b, p, rows + p * D);
            raw += a.bytes; coded += t.bytes; cpu_raw += a.enc + a.dec; cpu_planes += t.enc + t.dec;
        }
        double per_raw = (double)raw / count, per = (double)coded / count;
        double cr = cpu_raw / count * 1e9, cp = cpu_planes / count * 1e9;
        printf("  one row a message: planes %.1f bytes a token against raw %.1f (ratio %.3f); encode + decode %.1f us against %.1f us, exact\n",
            per, per_raw, per_raw / per, cp / 1e3, cr / 1e3);
        int64_t gbits[3] = {1, 10, 100};
        for (int g = 0; g < 3; g++) {
            int64_t lr = bc_slice_link_ns(gbits[g], 50000, (int64_t)(per_raw + 0.5)), lc = bc_slice_link_ns(gbits[g], 50000, (int64_t)(per + 0.5));
            printf("    %3lld Gbit/s, 50 us latency: raw %lld + %.0f = %.0f ns, planes %lld + %.0f = %.0f ns a token\n",
                (long long)gbits[g], (long long)lr, cr, lr + cr, (long long)lc, cp, lc + cp);
        }
        printf("    planes pays for its extra CPU time only below %.3f Gbit/s\n", (per_raw - per) * 8.0 / (cp - cr));
    }

    if (argc == 4) {
        FILE *fx = fopen(argv[3], "w"); if (!fx) { perror(argv[3]); return 2; }
        fprintf(fx, "// Chapter 193's real rows: the first 256 values of positions 0, 1 and 2 of\n"
                    "// Ministral 3 3B's residual stream after layer 0 over chapter 186's prompt,\n"
                    "// as f32 bits, written by psionic/tests/check_boundary_codec_slices.sh.\n"
                    "BCX_REAL_WIDTH :: 256;\nBCX_REAL :: [768]u32.[");
        for (int i = 0; i < 768; i++) {
            uint32_t v; memcpy(&v, &hidden[n + (i / 256) * D + i % 256], 4);
            fprintf(fx, "%s0x%08x", i ? (i % 8 ? "," : ",\n    ") : "\n    ", v);
        }
        fprintf(fx, "];\n"); fclose(fx);
    }
    free(hidden); free(wk); free(rows_back); free(msg); free(back); free(frame); free(work); free(inv); free(draft);
    munmap(map, (size_t)st.st_size);
    return 0;
}
