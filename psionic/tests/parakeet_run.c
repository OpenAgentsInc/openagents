/* Chapter 153's original-weight run: nvidia/parakeet-tdt-0.6b-v3 transcribes
 * the clips in CLIPS/clips.txt on the CPU, and writes the slices the
 * independent oracle (parakeet_oracle.py) recomputes. The checkpoint was
 * authenticated by parakeet_manifest.py first; parakeet_oracle.py wrote the
 * tensor byte ranges from its header. All model arithmetic, the vocabulary,
 * the text and the error rates are Coax (parakeet-run-runtime.coax); this
 * file maps the file, moves bytes, forks one process per clip and times.
 *
 *   parakeet-run MODEL_DIR RANGES CLIPS_DIR OUT_DIR SLICE_CLIP_ID
 */
#define _POSIX_C_SOURCE 200809L
#include "parakeet-run-runtime.h"
#include <fcntl.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

enum { RANGES = 800, CLIPS = 16, LINE = 4096, TEXT = 8192, NODES = 65536 };
static char names[RANGES][96];
static int64_t begins[RANGES], ends[RANGES];
static int ranges;

static double now(void) { struct timespec t; clock_gettime(CLOCK_MONOTONIC, &t); return t.tv_sec + t.tv_nsec / 1e9; }
static void *read_file(const char *path, int64_t *size) {
    FILE *f = fopen(path, "rb"); if (!f) { perror(path); exit(2); }
    fseek(f, 0, SEEK_END); long n = ftell(f); fseek(f, 0, SEEK_SET);
    void *p = malloc((size_t)n + 1);
    if (!p || fread(p, 1, (size_t)n, f) != (size_t)n) { fprintf(stderr, "cannot read %s\n", path); exit(2); }
    fclose(f); *size = n; return p;
}
static int write_f32(const char *dir, const char *name, const float *v, int64_t n) {
    char path[4096]; snprintf(path, sizeof path, "%s/%s", dir, name);
    FILE *f = fopen(path, "wb");
    if (!f || fwrite(v, sizeof(float), (size_t)n, f) != (size_t)n || fclose(f)) { fprintf(stderr, "cannot write %s\n", path); return 1; }
    return 0;
}
static int16_t *read_pcm(const char *dir, const char *id, int64_t *samples) {
    char path[4096]; snprintf(path, sizeof path, "%s/%s.pcm", dir, id);
    int64_t n; int16_t *p = read_file(path, &n); *samples = n / 2; return p;
}

int main(int argc, char **argv) {
    if (argc != 6) { fprintf(stderr, "usage: parakeet-run MODEL_DIR RANGES CLIPS_DIR OUT_DIR SLICE_CLIP_ID\n"); return 2; }
    const char *model = argv[1], *clips = argv[3], *out = argv[4], *slice = argv[5];
    FILE *rf = fopen(argv[2], "r"); if (!rf) { perror(argv[2]); return 2; }
    long long b, e;
    while (ranges < RANGES && fscanf(rf, "%95s %lld %lld", names[ranges], &b, &e) == 3) { begins[ranges] = b; ends[ranges] = e; ranges++; }
    fclose(rf);
    char path[4096]; snprintf(path, sizeof path, "%s/model.safetensors", model);
    int fd = open(path, O_RDONLY); if (fd < 0) { perror(path); return 2; }
    struct stat st; if (fstat(fd, &st) || st.st_size != 2508311120LL) { fprintf(stderr, "not the pinned file size\n"); return 2; }
    const uint8_t *map = mmap(NULL, (size_t)st.st_size, PROT_READ, MAP_PRIVATE, fd, 0); close(fd);
    if (map == MAP_FAILED) { perror("mmap"); return 2; }
    double t0 = now();

    /* Every tensor the architecture names, into its place; the layer
     * biases and the unused head stay zero. */
    int64_t ne = pkr_encoder_weights(), nd = pkr_decoder_weights(), loaded = 0;
    float *ew = calloc((size_t)ne, sizeof(float)), *dw = calloc((size_t)nd, sizeof(float));
    if (!ew || !dw) { fprintf(stderr, "out of memory\n"); return 2; }
    for (int64_t i = 0; i < pkr_tensors(); i++) {
        uint8_t name[160]; int64_t n = pkr_name(i, (coax_v_u8){name, sizeof name});
        int r = 0; while (r < ranges && (strlen(names[r]) != (size_t)n || memcmp(names[r], name, (size_t)n))) r++;
        if (n < 0 || r == ranges) { fprintf(stderr, "tensor %lld: not in the ranges\n", (long long)i); return 1; }
        int64_t part = pkr_part(i), at = pkr_at(i), count = pkr_count(i);
        if (part < 0) continue;
        if (ends[r] - begins[r] != 4 * count || ends[r] > st.st_size || at + count > (part ? nd : ne)) { fprintf(stderr, "%s: wrong size\n", names[r]); return 1; }
        memcpy((part ? dw : ew) + at, map + begins[r], (size_t)(4 * count)); loaded += count;
    }
    int64_t json_size; snprintf(path, sizeof path, "%s/tokenizer.json", model);
    uint8_t *json = read_file(path, &json_size);
    printf("loaded %lld weights in %.1f s: encoder array %lld, decoder array %lld\n", (long long)loaded, now() - t0, (long long)ne, (long long)nd);

    /* The clips: one process each, sharing the loaded weights. */
    snprintf(path, sizeof path, "%s/clips.txt", clips);
    FILE *cf = fopen(path, "r"); if (!cf) { perror(path); return 2; }
    static char ids[CLIPS][64], refs[CLIPS][LINE]; int count = 0; char line[LINE];
    while (count < CLIPS && fgets(line, sizeof line, cf)) {
        if (line[0] == '#' || line[0] == '\n') continue;
        line[strcspn(line, "\n")] = 0;
        char *space = strchr(line, ' '); if (!space || space - line >= 64) { fprintf(stderr, "bad clip line\n"); return 2; }
        memcpy(ids[count], line, (size_t)(space - line)); ids[count][space - line] = 0;
        snprintf(refs[count], LINE, "%s", space + 1); count++;
    }
    fclose(cf);
    fflush(stdout); double t1 = now();
    pid_t kids[CLIPS + 1];
    for (int c = 0; c <= count; c++) {
        kids[c] = fork();
        if (kids[c] < 0) { perror("fork"); return 2; }
        if (kids[c]) continue;
        const char *id = c < count ? ids[c] : slice;
        int64_t samples; int16_t *pcm = read_pcm(clips, id, &samples);
        int64_t n64 = pkr_work_f64(samples), n32 = pkr_work_f32(samples);
        double *wave = calloc((size_t)samples, sizeof(double)), *w64 = calloc((size_t)n64, sizeof(double));
        float *w32 = calloc((size_t)n32, sizeof(float));
        if (!wave || !w64 || !w32) _exit(3);
        double start = now();
        if (c == count) {
            /* The oracle's slices of one clip. */
            int64_t frames = samples / 160 + 1, enc = (((frames + 1) / 2 + 1) / 2 + 1) / 2;
            float *x = calloc((size_t)(128 * frames), sizeof(float)), *sub = calloc((size_t)(enc * 1024), sizeof(float));
            float *l0 = calloc((size_t)(enc * 1024), sizeof(float)), *l1 = calloc((size_t)(enc * 1024), sizeof(float));
            int64_t got = pkr_stages((coax_v_f32){ew, ne}, (coax_v_s16){pcm, samples}, (coax_v_f64){wave, samples}, (coax_v_f64){w64, n64},
                (coax_v_f32){w32, n32}, (coax_v_f32){x, 128 * frames}, (coax_v_f32){sub, enc * 1024}, (coax_v_f32){l0, enc * 1024}, (coax_v_f32){l1, enc * 1024});
            if (got != enc || write_f32(out, "features.f32", x, 128 * frames) || write_f32(out, "subsample.f32", sub, enc * 1024)
                || write_f32(out, "layer0.f32", l0, enc * 1024) || write_f32(out, "layer1.f32", l1, enc * 1024)) _exit(1);
            printf("slices of %s: %lld feature frames, %lld encoded frames, layers 0 and 1, %.1f s\n", id, (long long)frames, (long long)enc, now() - start);
            fflush(stdout); _exit(0);
        }
        uint8_t text[TEXT]; int64_t stats[8];
        coax_t_JvNode *nodes = calloc(NODES, sizeof(coax_t_JvNode));
        if (!nodes) _exit(3);
        int64_t status = pkr_transcribe((coax_v_f32){ew, ne}, (coax_v_f32){dw, nd}, (coax_v_u8){json, json_size}, (coax_v_t_JvNode){nodes, NODES}, (coax_v_s16){pcm, samples},
            (coax_v_u8){(uint8_t *)refs[c], (int64_t)strlen(refs[c])}, (coax_v_f64){wave, samples}, (coax_v_f64){w64, n64}, (coax_v_f32){w32, n32},
            (coax_v_u8){text, TEXT}, (coax_v_s64){stats, 8});
        snprintf(path, sizeof path, "%s/%s.res", out, id);
        FILE *f = fopen(path, "w"); if (!f) _exit(1);
        fprintf(f, "%lld %lld %lld %lld %lld %lld %lld %lld %lld %.1f %.3f\n", (long long)status, (long long)stats[0], (long long)stats[1], (long long)stats[2],
            (long long)stats[3], (long long)stats[4], (long long)stats[5], (long long)stats[6], (long long)stats[7], now() - start, samples / 16000.0);
        fwrite(text, 1, status ? 0 : (size_t)stats[3], f); fputc('\n', f);
        _exit(fclose(f) ? 1 : 0);
    }
    int failed = 0;
    for (int c = 0; c <= count; c++) { int ws; if (waitpid(kids[c], &ws, 0) < 0 || !WIFEXITED(ws) || WEXITSTATUS(ws)) failed = 1; }
    if (failed) { fprintf(stderr, "a clip's process failed\n"); return 1; }
    long long words = 0, word_errors = 0, chars = 0, char_errors = 0;
    for (int c = 0; c < count; c++) {
        snprintf(path, sizeof path, "%s/%s.res", out, ids[c]);
        FILE *f = fopen(path, "r"); if (!f) { perror(path); return 1; }
        long long s[9]; double secs, audio; char text[TEXT];
        if (fscanf(f, "%lld %lld %lld %lld %lld %lld %lld %lld %lld %lf %lf\n", &s[0], &s[1], &s[2], &s[3], &s[4], &s[5], &s[6], &s[7], &s[8], &secs, &audio) != 11) return 1;
        if (!fgets(text, sizeof text, f)) text[0] = 0;
        fclose(f); text[strcspn(text, "\n")] = 0;
        if (s[0]) { printf("%s: refused with status %lld\n", ids[c], s[0]); return 1; }
        printf("%s, %.2f s of audio, %lld feature frames, %lld encoded, %lld tokens, %.0f s:\n  \"%s\"\n  word errors %lld of %lld, character errors %lld of %lld\n",
            ids[c], audio, s[1], s[2], s[3], secs, text, s[6], s[5], s[8], s[7]);
        words += s[5]; word_errors += s[6]; chars += s[7]; char_errors += s[8];
    }
    printf("all %d clips: word errors %lld of %lld, character errors %lld of %lld: %s the frozen thresholds; %.0f s\n", count, word_errors, words, char_errors, chars,
        pkr_within(word_errors, words, char_errors, chars) ? "within" : "OUTSIDE", now() - t1);
    return pkr_within(word_errors, words, char_errors, chars) ? 0 : 1;
}
