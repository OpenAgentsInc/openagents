/* psionic-whisper: process I/O for Psionic's Whisper. It maps files,
 * allocates the arenas psionic/lib/whisper/engine.coax lays out, calls its
 * exports and prints. Every decision about the model, the audio, the search
 * and the output is Coax.
 *   psionic/platform/whisper-build.sh OUT
 *   OUT/psionic-whisper info MODEL
 *   OUT/psionic-whisper filters MODEL
 *   OUT/psionic-whisper transcribe MODEL WAV [--text|--srt|--vtt|--json] [--language CODE] [--ctx N]
 * WAV is 16 kHz mono 16-bit PCM (`ffmpeg -i IN -ar 16000 -ac 1 OUT.wav`).
 * Design: psionic/docs/transcribe.md. */
#include "whisper.h"
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <time.h>
#include <unistd.h>

enum { TENSORS = 1024, SEGMENTS = 4096 };

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
          "       psionic-whisper transcribe MODEL WAV [--text|--srt|--vtt|--json] [--language CODE] [--ctx N]\n", stderr);
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
    int format = 0; const char *lang = "en"; long ctx = p.n_audio_ctx;
    for (int i = 4; i < argc; i++) {
        if (strcmp(argv[i], "--text") == 0) format = 0;
        else if (strcmp(argv[i], "--srt") == 0) format = 1;
        else if (strcmp(argv[i], "--vtt") == 0) format = 2;
        else if (strcmp(argv[i], "--json") == 0) format = 3;
        else if (strcmp(argv[i], "--language") == 0 && i + 1 < argc) lang = argv[++i];
        else if (strcmp(argv[i], "--ctx") == 0 && i + 1 < argc) ctx = strtol(argv[++i], 0, 10);
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
    int64_t language = psionic_whisper_language(*net, (coax_string){lang, (int64_t)strlen(lang)});
    static coax_t_WsSegment segs[SEGMENTS];
    size_t text_cap = 1 << 22;
    uint8_t *text = malloc(text_cap), *out = malloc(text_cap * 2);
    if (!text || !out) return 1;
    coax_t_WeRun run = psionic_whisper_transcribe(*net, (coax_v_f32){W, net->weights}, (coax_v_f32){work, net->work.total}, file, m,
                                                  (coax_v_f64){bank, p.n_mels * 201}, (coax_v_f32){audio, wh.samples}, language,
                                                  (coax_v_t_WsSegment){segs, SEGMENTS}, (coax_v_u8){text, (int64_t)text_cap});
    double t_end = now_ms();
    if (run.status != COAX_WeStatus_ok) { fprintf(stderr, "psionic-whisper: transcribe (status %d)\n", (int)run.status); return 1; }
    const char *name = strrchr(argv[2], '/'); name = name ? name + 1 : argv[2];
    int64_t n = psionic_whisper_format((coax_v_t_WsSegment){segs, run.segments}, format, (coax_string){name, (int64_t)strlen(name)},
                                       (coax_string){lang, (int64_t)strlen(lang)}, (coax_v_u8){out, (int64_t)text_cap * 2});
    if (n < 0) { fputs("psionic-whisper: output too long\n", stderr); return 1; }
    fwrite(out, 1, (size_t)n, stdout);
    fprintf(stderr, "psionic-whisper: %.0f ms load, %.0f ms transcribe (%lld windows, %lld tokens, %.1f s audio)\n",
            t_load - t0, t_end - t_load, (long long)run.windows, (long long)run.tokens, wh.samples / 16000.0);
    return 0;
}
