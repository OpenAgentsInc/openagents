/* psionic-whisper: process I/O for Psionic's Whisper. It maps files,
 * allocates the arenas psionic/lib/whisper/engine.coax lays out, calls its
 * exports and prints. Every decision about the model, the audio, the search
 * and the output is Coax.
 *   psionic/platform/whisper-build.sh OUT
 *   OUT/psionic-whisper info MODEL
 *   OUT/psionic-whisper filters MODEL
 *   OUT/psionic-whisper transcribe MODEL WAV [--text|--srt|--vtt|--json] [--language CODE] [--threads N] [--name NAME]
 * WAV is 16 kHz mono 16-bit PCM (`ffmpeg -i IN -ar 16000 -ac 1 OUT.wav`).
 * Design: psionic/docs/transcribe.md. */
#include "whisper.h"
#include <fcntl.h>
#include <pthread.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <time.h>
#include <unistd.h>

enum { TENSORS = 1024, SEGMENTS = 4096 };

/* ---- the foreign kernels: split a range across threads ------------------
 * engine.coax's cxwh_* calls land here. Each splits its range into one
 * contiguous run per thread and calls the matching *_part export; each
 * output element belongs to exactly one run, so the result is the same for
 * any thread count. Small work runs on the calling thread. */
static int g_threads = 8;
typedef struct { int kind; int64_t lo, hi; coax_v_f32 a, b, c, d, e; coax_v_f64 f, g; int64_t i0, i1, i2, i3, i4, i5; _Bool flag; } job;
static void run_job(job *j) {
    if (j->kind == 0) psionic_whisper_linear_part(j->a, j->i0, j->i1, j->b, j->c, j->i2, j->d, j->lo, j->hi);
    else if (j->kind == 1) psionic_whisper_attend_part(j->a, j->i0, j->b, j->c, j->i1, j->i2, j->i3, j->i4, j->flag, j->d, j->lo, j->hi);
    else if (j->kind == 3) psionic_whisper_mel_part(j->a, j->f, j->g, j->i0, j->b, j->lo, j->hi);
    else psionic_whisper_conv_part(j->a, j->i0, j->i1, j->b, j->c, j->i2, j->i3, j->i4, j->d, j->e, j->lo, j->hi);
}
static void *thread_main(void *p) { run_job(p); return 0; }
static void split(job base, int64_t n, double work) {
    int t = g_threads;
    if (work < 2e6 || t <= 1 || n < 2) { base.lo = 0; base.hi = n; run_job(&base); return; }
    if (t > n) t = (int)n;
    if (t > 64) t = 64;
    job jobs[64]; pthread_t ids[64];
    for (int i = 0; i < t; i++) {
        jobs[i] = base; jobs[i].lo = n * i / t; jobs[i].hi = n * (i + 1) / t;
        if (i > 0 && pthread_create(&ids[i], 0, thread_main, &jobs[i]) != 0) { run_job(&jobs[i]); ids[i] = 0; }
    }
    run_job(&jobs[0]);
    for (int i = 1; i < t; i++) if (ids[i]) pthread_join(ids[i], 0);
}
int64_t cxwh_linear(coax_v_f32 x, int64_t rows, int64_t n_in, coax_v_f32 w, coax_v_f32 b, int64_t n_out, coax_v_f32 y) {
    job j = {0}; j.kind = 0; j.a = x; j.i0 = rows; j.i1 = n_in; j.b = w; j.c = b; j.i2 = n_out; j.d = y;
    split(j, n_out, (double)rows * n_in * n_out);
    return 0;
}
int64_t cxwh_attend(coax_v_f32 q, int64_t nq, coax_v_f32 k, coax_v_f32 v, int64_t nk, int64_t d, int64_t heads, int64_t first, _Bool causal, coax_v_f32 out) {
    job j = {0}; j.kind = 1; j.a = q; j.i0 = nq; j.b = k; j.c = v; j.i1 = nk; j.i2 = d; j.i3 = heads; j.i4 = first; j.flag = causal; j.d = out;
    split(j, heads * nq, (double)nq * nk * d * 2);
    return 0;
}
int64_t cxwh_mel(coax_v_f32 x, coax_v_f64 window, coax_v_f64 bank, int64_t mels, coax_v_f32 cache, int64_t frames) {
    job j = {0}; j.kind = 3; j.a = x; j.f = window; j.g = bank; j.i0 = mels; j.b = cache;
    split(j, frames, (double)frames * 1e5);
    return 0;
}
int64_t cxwh_conv(coax_v_f32 src, int64_t n_in, int64_t t_in, coax_v_f32 w, coax_v_f32 b, int64_t n_out, int64_t t_out, int64_t stride, coax_v_f32 pos, coax_v_f32 out) {
    job j = {0}; j.kind = 2; j.a = src; j.i0 = n_in; j.i1 = t_in; j.b = w; j.c = b; j.i2 = n_out; j.i3 = t_out; j.i4 = stride; j.d = pos; j.e = out;
    split(j, n_out, (double)n_in * t_out * n_out * 3);
    return 0;
}

static double now_ms(void) {
    struct timespec t;
    clock_gettime(CLOCK_MONOTONIC, &t);
    return t.tv_sec * 1e3 + t.tv_nsec / 1e6;
}
static coax_v_u8 map(const char *path) {
    coax_v_u8 v = {0, 0};
    int fd = open(path, O_RDONLY);
    struct stat st;
    if (fd < 0 || fstat(fd, &st) != 0 || st.st_size == 0) { if (fd >= 0) close(fd); return v; }
    void *p = mmap(0, (size_t)st.st_size, PROT_READ, MAP_PRIVATE, fd, 0);
    close(fd);
    if (p == MAP_FAILED) return v;
    v.data = p; v.count = st.st_size;
    return v;
}
static int usage(void) {
    fputs("usage: psionic-whisper info|filters MODEL\n"
          "       psionic-whisper transcribe MODEL WAV [--text|--srt|--vtt|--json] [--language CODE] [--threads N] [--name NAME]\n", stderr);
    return 64;
}

int main(int argc, char **argv) {
    if (argc < 3) return usage();
    const char *cmd = argv[1];
    double t0 = now_ms();
    coax_v_u8 file = map(argv[2]);
    if (!file.data) { fprintf(stderr, "psionic-whisper: cannot map %s\n", argv[2]); return 1; }
    static coax_t_WgTensor tensors[TENSORS];
    coax_v_t_WgTensor tv = {tensors, TENSORS};
    coax_t_WgModel m = psionic_whisper_read(file, tv);
    if (m.status != COAX_WgStatus_ok) { fprintf(stderr, "psionic-whisper: not a Whisper ggml file (status %d)\n", (int)m.status); return 1; }
    coax_t_WgParams p = m.params;
    if (strcmp(cmd, "info") == 0) {
        printf("vocab %lld\naudio %lld x %lld, %lld heads, %lld layers\ntext %lld x %lld, %lld heads, %lld layers\nmels %lld\nftype %lld\ntensors %lld\ntokens %lld\n",
               (long long)p.n_vocab, (long long)p.n_audio_ctx, (long long)p.n_audio_state, (long long)p.n_audio_head, (long long)p.n_audio_layer,
               (long long)p.n_text_ctx, (long long)p.n_text_state, (long long)p.n_text_head, (long long)p.n_text_layer,
               (long long)p.n_mels, (long long)p.ftype, (long long)m.tensor_count, (long long)m.tokens);
        return 0;
    }
    double *bank = calloc((size_t)(p.n_mels * 201), sizeof *bank);
    if (!bank || psionic_whisper_bank(p.n_mels, (coax_v_f64){bank, p.n_mels * 201}) != 0) { fputs("psionic-whisper: mel bank\n", stderr); return 1; }
    if (strcmp(cmd, "filters") == 0) {
        printf("max_abs_diff %.3e\n", psionic_whisper_filter_diff(file, m, (coax_v_f64){bank, p.n_mels * 201}));
        return 0;
    }
    if (strcmp(cmd, "transcribe") != 0 || argc < 4) return usage();
    int format = 0; const char *lang = "en", *name_arg = 0; long ctx = p.n_audio_ctx;
    for (int i = 4; i < argc; i++) {
        if (strcmp(argv[i], "--text") == 0) format = 0;
        else if (strcmp(argv[i], "--srt") == 0) format = 1;
        else if (strcmp(argv[i], "--vtt") == 0) format = 2;
        else if (strcmp(argv[i], "--json") == 0) format = 3;
        else if (strcmp(argv[i], "--language") == 0 && i + 1 < argc) lang = argv[++i];
        else if (strcmp(argv[i], "--name") == 0 && i + 1 < argc) name_arg = argv[++i];
        else if (strcmp(argv[i], "--threads") == 0 && i + 1 < argc) g_threads = (int)strtol(argv[++i], 0, 10);
        else return usage();
    }
    static int64_t offsets[TENSORS];
    coax_t_WeNet *net = malloc(sizeof *net);
    if (!net) return 1;
    *net = psionic_whisper_net(file, m, tv, (coax_v_s64){offsets, TENSORS}, ctx);
    if (net->status != COAX_WeStatus_ok) { fprintf(stderr, "psionic-whisper: model tensors (status %d)\n", (int)net->status); return 1; }
    float *W = malloc((size_t)net->weights * sizeof *W), *work = calloc((size_t)net->work.total, sizeof *work);
    if (!W || !work) { fputs("psionic-whisper: out of memory\n", stderr); return 1; }
    coax_t_WeStatus ls = psionic_whisper_load(file, tv, m.tensor_count, (coax_v_s64){offsets, TENSORS}, (coax_v_f32){W, net->weights});
    if (ls != COAX_WeStatus_ok) { fprintf(stderr, "psionic-whisper: weights (status %d)\n", (int)ls); return 1; }
    double t_load = now_ms();
    coax_v_u8 wav = map(argv[3]);
    if (!wav.data) { fprintf(stderr, "psionic-whisper: cannot map %s\n", argv[3]); return 1; }
    coax_t_WeWav wh = psionic_whisper_wav(wav);
    if (!wh.ok || wh.samples < 1) { fputs("psionic-whisper: WAV must be 16 kHz mono 16-bit PCM\n", stderr); return 1; }
    float *audio = malloc((size_t)wh.samples * sizeof *audio);
    if (!audio || psionic_whisper_pcm16(wav, wh.at, wh.samples, (coax_v_f32){audio, wh.samples}) != wh.samples) return 1;
    int64_t cache_n = psionic_whisper_cache_size(wh.samples, p.n_mels);
    float *cache = malloc((size_t)cache_n * sizeof *cache);
    if (!cache) return 1;
    int64_t language = psionic_whisper_language(*net, (coax_string){lang, (int64_t)strlen(lang)});
    static coax_t_WsSegment segs[SEGMENTS];
    size_t text_cap = 1 << 22;
    uint8_t *text = malloc(text_cap), *out = malloc(text_cap * 2);
    if (!text || !out) return 1;
    coax_t_WeRun run = psionic_whisper_transcribe(*net, (coax_v_f32){W, net->weights}, (coax_v_f32){work, net->work.total}, file, m,
                                                  (coax_v_f64){bank, p.n_mels * 201}, (coax_v_f32){audio, wh.samples}, (coax_v_f32){cache, cache_n}, language,
                                                  (coax_v_t_WsSegment){segs, SEGMENTS}, (coax_v_u8){text, (int64_t)text_cap});
    double t_end = now_ms();
    if (run.status != COAX_WeStatus_ok) { fprintf(stderr, "psionic-whisper: transcribe (status %d)\n", (int)run.status); return 1; }
    const char *name = strrchr(argv[2], '/'); name = name_arg ? name_arg : name ? name + 1 : argv[2];
    int64_t n = psionic_whisper_format((coax_v_t_WsSegment){segs, run.segments}, format, (coax_string){name, (int64_t)strlen(name)},
                                       (coax_string){lang, (int64_t)strlen(lang)}, (coax_v_u8){out, (int64_t)text_cap * 2});
    if (n < 0) { fputs("psionic-whisper: output too long\n", stderr); return 1; }
    fwrite(out, 1, (size_t)n, stdout);
    fprintf(stderr, "psionic-whisper: %.0f ms load, %.0f ms transcribe (%d threads, %lld windows, %lld tokens, %.1f s audio)\n",
            t_load - t0, t_end - t_load, g_threads, (long long)run.windows, (long long)run.tokens, wh.samples / 16000.0);
    return 0;
}
