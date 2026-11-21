#include "inkling-metal.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

static double now_sec(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (double)ts.tv_sec + (double)ts.tv_nsec * 1e-9;
}

int main(int argc, char **argv) {
    const char *gguf = NULL;
    const char *metal = NULL;
    const char *prompt = "What is 17 * 23? Answer with just the number.";
    int max_tokens = 64;
    int max_ctx = 4096;
    int raw = 0;
    float temp = 0.0f;

    for (int i = 1; i < argc; i++) {
        if (!strcmp(argv[i], "--model") && i + 1 < argc) gguf = argv[++i];
        else if (!strcmp(argv[i], "--metal") && i + 1 < argc) metal = argv[++i];
        else if (!strcmp(argv[i], "--prompt") && i + 1 < argc) prompt = argv[++i];
        else if (!strcmp(argv[i], "--max-tokens") && i + 1 < argc) max_tokens = atoi(argv[++i]);
        else if (!strcmp(argv[i], "--ctx") && i + 1 < argc) max_ctx = atoi(argv[++i]);
        else if (!strcmp(argv[i], "--temp") && i + 1 < argc) temp = (float)atof(argv[++i]);
        else if (!strcmp(argv[i], "--raw")) raw = 1;
    }
    if (!gguf || !metal) {
        fprintf(stderr, "usage: %s --model PATH.gguf --metal PATH.metal [--prompt TEXT] [--max-tokens N] [--raw]\n", argv[0]);
        return 2;
    }

    double t0 = now_sec();
    InkSession *s = NULL;
    char err[512];
    if (ink_open(gguf, metal, max_ctx, &s, err, sizeof err) != 0) {
        fprintf(stderr, "ink_open failed: %s\n", err);
        return 1;
    }
    fprintf(stderr, "loaded in %.2fs (vocab=%d unpadded=%d bos=%d eos=%d)\n",
            now_sec() - t0, ink_vocab_size(s), ink_vocab_unpadded(s), ink_bos_token(s), ink_eos_token(s));

    char formatted[16384];
    if (raw) {
        snprintf(formatted, sizeof formatted, "%s", prompt);
    } else {
        snprintf(formatted, sizeof formatted,
                 "<|message_system|><|content_text|>Thinking effort level: 0.7<|end_message|>"
                 "<|message_user|><|content_text|>%s<|end_message|>",
                 prompt);
    }

    int32_t toks[4096];
    int n_tok = ink_encode(s, formatted, strlen(formatted), toks, 4096, 1);
    if (n_tok <= 0) {
        fprintf(stderr, "ink_encode failed (%d)\n", n_tok);
        ink_close(s); return 1;
    }
    fprintf(stderr, "prompt tokens (%d):", n_tok);
    for (int i = 0; i < n_tok; i++) fprintf(stderr, " %d", toks[i]);
    fprintf(stderr, "\n");

    double tp0 = now_sec();
    for (int i = 0; i < n_tok; i++) {
        int need_logits = (i == n_tok - 1);
        if (ink_step(s, toks[i], need_logits) != 0) {
            fprintf(stderr, "prefill step %d failed: %s\n", i, ink_error(s));
            ink_close(s); return 1;
        }
    }
    double tp1 = now_sec();
    fprintf(stderr, "prefill %d tok in %.3fs (%.1f tok/s)\n", n_tok, tp1 - tp0, n_tok / (tp1 - tp0 + 1e-9));

    /* Print top-5 logits after prefill for verification */
    const float *lg = ink_logits(s);
    int nv = ink_vocab_unpadded(s);
    int top_i[5] = {0}; float top_v[5] = {-1e30f,-1e30f,-1e30f,-1e30f,-1e30f};
    for (int i = 0; i < nv; i++) {
        float v = lg[i];
        if (v > top_v[4]) {
            int j = 4;
            while (j > 0 && top_v[j - 1] < v) { top_v[j] = top_v[j - 1]; top_i[j] = top_i[j - 1]; j--; }
            top_v[j] = v; top_i[j] = i;
        }
    }
    fprintf(stderr, "top-5 next:");
    for (int k = 0; k < 5; k++) fprintf(stderr, " [%d %.3f '%s']", top_i[k], top_v[k], ink_token_str(s, top_i[k]));
    fprintf(stderr, "\n");

    InkSampleParams sp = { .temperature = temp, .top_p = 0.95f, .top_k = 40 };
    uint64_t rng = 123456789ULL;
    double td0 = now_sec();
    int gen = 0;
    for (int i = 0; i < max_tokens; i++) {
        int32_t next = ink_sample(s, &sp, &rng);
        if (next < 0) break;
        if (ink_is_eog(s, next)) {
            fprintf(stderr, "\n[EOG token %d '%s']\n", next, ink_token_str(s, next));
            break;
        }
        char piece[512];
        int plen = ink_decode_piece(s, next, piece, sizeof piece);
        if (plen > 0) {
            fwrite(piece, 1, (size_t)plen, stdout);
            fflush(stdout);
        }
        gen++;
        if (i + 1 < max_tokens) {
            if (ink_step(s, next, 1) != 0) {
                fprintf(stderr, "\ndecode step %d failed: %s\n", i, ink_error(s));
                break;
            }
        }
    }
    double td1 = now_sec();
    printf("\n");
    fprintf(stderr, "generated %d tok in %.3fs (%.2f tok/s, total gpu=%.3fs)\n",
            gen, td1 - td0, gen / (td1 - td0 + 1e-9), ink_last_gpu_seconds(s));
    ink_close(s);
    return 0;
}
