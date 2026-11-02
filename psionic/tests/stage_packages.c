/* Chapter 187's original-weight check of stage packages on Ministral 3 3B.
 * 1. The whole model split 14 + 12 (chapter 186's plan at 4,096 positions
 *    under two 6 GB budgets): build both stages' manifests from the pinned
 *    file, write them to OUTDIR, then verify and load each stage from its
 *    manifest, timing both.
 * 2. Chapter 186's slice (layers 0 and 1, one a stage): build, verify and
 *    load both packages, run the split from the loaded views and the whole
 *    from chapter 7's views by name, and compare all logits bit for bit.
 * 3. One byte of layer 1 changed in this process's private copy of the
 *    page: the slice's stage 1 is refused and does not report ready.
 * The file is authenticated by ministral_reference.py pin first. Every
 * choice, hash comparison and refusal is Coax (stage-package.coax through
 * stage-packages-runtime.coax); this file maps the file, keeps the clock
 * and writes the manifests. No weight byte is copied to disk.
 *
 *   stage-packages CONSOLIDATED_SAFETENSORS TOKENS OUTDIR
 */
#define _POSIX_C_SOURCE 200809L
#include "stage-packages-runtime.h"
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <time.h>
#include <unistd.h>

enum { D = 3072, VOCAB = 131072, MAX_TOKENS = 64, TENSORS = 458, ROOM = 1024, TEXT = 1 << 17 };
static const char *SHA = "722430146632596c6c41137b2e582095ed01a50342932b035cb8662f7855a340";

static coax_string str(const char *s) { return (coax_string){s, (int64_t)strlen(s)}; }
static coax_v_f32 span(float *p, int64_t n) { return (coax_v_f32){p, n}; }
static double now(void) { struct timespec t; clock_gettime(CLOCK_MONOTONIC, &t); return t.tv_sec + t.tv_nsec / 1e9; }
static void put(coax_string s) { fwrite(s.data, 1, (size_t)s.count, stdout); }

static coax_t_P5Tensor *inv, *draft; static int64_t *picked; static int64_t inv_count; static coax_t_PaFile file;
static coax_v_u8 bytes;
static coax_t_SpScratch scratch(void) { return (coax_t_SpScratch){{inv, ROOM}, {draft, ROOM}, {picked, ROOM}}; }

static coax_v_u8 tensor(const char *name, int64_t rows, int64_t cols) {
    coax_t_P5Tensor *all = calloc(ROOM, sizeof *all), *tmp = calloc(ROOM, sizeof *tmp);
    coax_t_PwResult scan = p5_inspect_checked(bytes, (coax_v_t_P5Tensor){all, ROOM}, (coax_v_t_P5Tensor){tmp, ROOM}, 0);
    coax_t_P5Shape shape = cols ? (coax_t_P5Shape){2, {{rows, cols, 0, 0, 0}}} : (coax_t_P5Shape){1, {{rows, 0, 0, 0, 0}}};
    coax_t_PaView v = pa_view(str(name), shape, COAX_PwDtype_bf16, (coax_v_t_P5Tensor){all, scan.written}, (coax_v_t_PaFile){&file, 1});
    if (scan.status || v.status) { fprintf(stderr, "%s: mapping status %u\n", name, v.status); exit(1); }
    free(all); free(tmp);
    return v.data;
}
static coax_t_MsLayer layer(int l) {
    char n[9][96]; const char *suffix[9] = {"attention_norm.weight", "attention.wq.weight", "attention.wk.weight", "attention.wv.weight",
        "attention.wo.weight", "ffn_norm.weight", "feed_forward.w1.weight", "feed_forward.w2.weight", "feed_forward.w3.weight"};
    for (int k = 0; k < 9; k++) snprintf(n[k], sizeof n[k], "layers.%d.%s", l, suffix[k]);
    return (coax_t_MsLayer){tensor(n[0], D, 0), tensor(n[1], 4096, D), tensor(n[2], 1024, D), tensor(n[3], 1024, D),
        tensor(n[4], D, 4096), tensor(n[5], D, 0), tensor(n[6], 9216, D), tensor(n[7], D, 9216), tensor(n[8], 9216, D)};
}

typedef struct { uint8_t *text; int64_t length; char digest[65]; } Package;

/* Build stage k's manifest, write it to OUTDIR/NAME-k.manifest, report it. */
static Package build(const char *name, coax_t_StgPlan p, int k, const char *outdir) {
    Package pk; pk.text = malloc(TEXT);
    double t0 = now();
    coax_t_SpBuilt b = sp_rt_build(bytes, str(SHA), p, k, scratch(), (coax_v_u8){pk.text, TEXT});
    double t1 = now();
    if (b.status) { printf("%s stage %d: build refused, ", name, k); put(sp_rt_status(b.status)); printf("\n"); exit(1); }
    pk.length = b.length;
    sp_rt_hex((coax_v_u8){pk.text, pk.length}, (coax_v_u8){(uint8_t *)pk.digest, 64}); pk.digest[64] = 0;
    char path[4096]; snprintf(path, sizeof path, "%s/%s-%d.manifest", outdir, name, k);
    FILE *f = fopen(path, "wb"); if (!f || fwrite(pk.text, 1, (size_t)pk.length, f) != (size_t)pk.length) { perror(path); exit(2); } fclose(f);
    printf("%s stage %d: layers %lld..<%lld, %lld tensors, %lld bytes (%.1f%% of the file), manifest %lld bytes, built in %.1f s\n",
        name, k, (long long)p.cut.e[k], (long long)p.cut.e[k + 1], (long long)b.count, (long long)b.bytes,
        100.0 * (double)b.bytes / (double)bytes.count, (long long)b.length, t1 - t0);
    printf("  manifest sha256 %s\n", pk.digest);
    return pk;
}
/* Verify and load stage k from its package; report the phases. */
static coax_t_SpStage stage(const char *name, Package pk, coax_t_StgPlan p, int k, coax_t_MsShape s, coax_t_MsLayer *layers) {
    double t0 = now();
    coax_t_SpStage c = sp_rt_stage((coax_v_u8){pk.text, pk.length}, str(pk.digest), bytes, str(SHA), p, k, s, scratch(),
        (coax_v_t_MsLayer){layers, s.layers});
    double t1 = now();
    printf("%s stage %d: ", name, k); put(sp_rt_status(c.status));
    printf(", %lld bytes hashed, %lld tensors and %lld bytes handed out, phase ", (long long)c.hashed, (long long)c.tensors, (long long)c.read);
    put(sp_rt_state(c.life.state)); printf(", %.1f s\n", t1 - t0);
    return c;
}

int main(int argc, char **argv) {
    if (argc != 4) { fprintf(stderr, "usage: stage-packages CONSOLIDATED_SAFETENSORS TOKENS OUTDIR\n"); return 2; }
    int64_t tokens[MAX_TOKENS]; int64_t count = 0; long long t;
    FILE *tf = fopen(argv[2], "r"); if (!tf) { perror(argv[2]); return 2; }
    while (count < MAX_TOKENS && fscanf(tf, "%lld", &t) == 1) tokens[count++] = t;
    fclose(tf);
    if (count < 1) { fprintf(stderr, "no tokens\n"); return 2; }
    int fd = open(argv[1], O_RDONLY); if (fd < 0) { perror("open"); return 2; }
    struct stat st; if (fstat(fd, &st) || st.st_size != 7698234952LL) { fprintf(stderr, "not the pinned file size\n"); return 2; }
    /* Private and writable: step 3 changes one byte in this process's copy of a page, never the file. */
    uint8_t *map = mmap(NULL, (size_t)st.st_size, PROT_READ | PROT_WRITE, MAP_PRIVATE, fd, 0); close(fd);
    if (map == MAP_FAILED) { perror("mmap"); return 2; }
    inv = calloc(ROOM, sizeof *inv); draft = calloc(ROOM, sizeof *draft); picked = calloc(ROOM, sizeof *picked);
    bytes = (coax_v_u8){map, st.st_size}; file.data = bytes;
    coax_t_PwResult scan = p5_inspect_checked(bytes, (coax_v_t_P5Tensor){inv, ROOM}, (coax_v_t_P5Tensor){draft, ROOM}, 0);
    if (scan.status || scan.written != TENSORS) { fprintf(stderr, "inventory status %u\n", scan.status); return 1; }
    inv_count = scan.written;

    /* 1. The whole model, 14 + 12. */
    coax_t_MsShape full = {D, 26, 32, 8, 128, 9216, VOCAB};
    coax_t_StgPlan p = sp_rt_full();
    if (p.status || p.cut.e[1] != 14) { fprintf(stderr, "plan status %u\n", p.status); return 1; }
    Package f0 = build("full", p, 0, argv[3]), f1 = build("full", p, 1, argv[3]);
    coax_t_MsLayer all[26]; memset(all, 0, sizeof all);
    coax_t_SpStage a = stage("full", f0, p, 0, full, all), b = stage("full", f1, p, 1, full, all);
    if (a.status || b.status || a.life.state != COAX_CsState_arenas || b.life.state != COAX_CsState_arenas) return 1;
    int64_t views = 0; for (int l = 0; l < 26; l++) views += all[l].wq.count > 0;
    printf("full: %lld of 26 layers have views; both stages hold the embedding (%lld bytes each)\n", (long long)views, (long long)a.embed.count);
    if (views != 26 || a.embed.data != b.embed.data || a.read + b.read != 7663319040LL) return 1;

    /* 2. Chapter 186's slice from packages. */
    coax_t_MsShape s = {D, 2, 32, 8, 128, 9216, VOCAB};
    coax_t_StgPlan q = sp_rt_slice();
    if (q.status || q.cut.e[1] != 1) { fprintf(stderr, "slice plan status %u\n", q.status); return 1; }
    Package s0 = build("slice", q, 0, argv[3]), s1 = build("slice", q, 1, argv[3]);
    fwrite(s1.text, 1, 300, stdout); printf("...\n");
    coax_t_MsLayer two[2]; memset(two, 0, sizeof two);
    coax_t_SpStage c0 = stage("slice", s0, q, 0, s, two), c1 = stage("slice", s1, q, 1, s, two);
    if (c0.status || c1.status || c0.tag != 0x722430146632596cULL || c1.tag != c0.tag || c0.embed.data != c1.embed.data) return 1;
    coax_t_MsLayer named[2] = {layer(0), layer(1)};
    coax_v_u8 embed = tensor("tok_embeddings.weight", VOCAB, D), norm = tensor("norm.weight", D, 0);
    int64_t floats = stg_slice_floats(s, count), words = stg_slice_words(s, count);
    float *hidden = calloc((size_t)(2 * count * D), sizeof(float)), *work = calloc((size_t)floats, sizeof(float));
    float *whole = calloc(VOCAB, sizeof(float)), *split = calloc(VOCAB, sizeof(float)), *mem = calloc((size_t)(2 * floats), sizeof(float));
    uint32_t *wire = calloc((size_t)words, sizeof(uint32_t));
    if (!hidden || !work || !whole || !split || !mem || !wire) { fprintf(stderr, "allocation failed\n"); return 2; }
    coax_v_s64 toks = {tokens, count};
    coax_t_MsResult r = stg_slice_whole(s, embed, norm, (coax_v_t_MsLayer){named, 2}, toks, span(hidden, 2 * count * D), span(whole, VOCAB), span(work, floats));
    if (r.status) { fprintf(stderr, "whole status %u\n", r.status); return 1; }
    coax_t_StgRun run = stg_slice_split(s, c1.tag, c1.embed, c1.norm, (coax_v_t_MsLayer){two, 2}, toks, span(split, VOCAB), span(mem, 2 * floats),
        (coax_v_u32){wire, words});
    if (run.status) { fprintf(stderr, "split status %u refusal %u\n", run.status, run.refusal); return 1; }
    int64_t differ = 0, best = 0;
    for (int64_t i = 0; i < VOCAB; i++) {
        uint32_t x, y; memcpy(&x, &whole[i], 4); memcpy(&y, &split[i], 4);
        if (x != y) differ++;
        if (whole[i] > whole[best]) best = i;
    }
    printf("slice over %lld tokens: logits differing in any bit between packaged stages and the named whole: %lld of %d; greedy token %lld\n",
        (long long)count, (long long)differ, VOCAB, (long long)best);
    if (differ || run.messages != 1) return 1;

    /* 3. One changed byte in layer 1's wq, in this process's private page only. */
    uint8_t *at = (uint8_t *)named[1].wq.data + 12345;
    *at ^= 1;
    coax_t_SpStage x = stage("slice, one byte of layer 1 changed,", s1, q, 1, s, two);
    printf("  refused at manifest tensor %lld (", (long long)x.tensor); put(inv[picked[x.tensor]].name); printf("); ready: %s\n", x.life.state == COAX_CsState_ready ? "yes" : "no");
    *at ^= 1;
    if (x.status != COAX_SpStatus_hash || x.life.state != COAX_CsState_failed) return 1;
    free(hidden); free(work); free(whole); free(split); free(mem); free(wire); free(inv); free(draft); free(picked);
    free(f0.text); free(f1.text); free(s0.text); free(s1.text);
    munmap(map, (size_t)st.st_size);
    return 0;
}
