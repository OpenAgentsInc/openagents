#include "inkling-metal.h"
#include <math.h>
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
    int loops = 0;
    int verify_prefill_decode = 0;
    int raw = 0;
    float temp = 0.0f;

    for (int i = 1; i < argc; i++) {
        if (!strcmp(argv[i], "--model") && i + 1 < argc) gguf = argv[++i];
        else if (!strcmp(argv[i], "--metal") && i + 1 < argc) metal = argv[++i];
        else if (!strcmp(argv[i], "--prompt") && i + 1 < argc) prompt = argv[++i];
        else if (!strcmp(argv[i], "--max-tokens") && i + 1 < argc) max_tokens = atoi(argv[++i]);
        else if (!strcmp(argv[i], "--ctx") && i + 1 < argc) max_ctx = atoi(argv[++i]);
        else if (!strcmp(argv[i], "--loops") && i + 1 < argc) loops = atoi(argv[++i]);
        else if (!strcmp(argv[i], "--verify-prefill-decode")) verify_prefill_decode = 1;
        else if (!strcmp(argv[i], "--temp") && i + 1 < argc) temp = (float)atof(argv[++i]);
        else if (!strcmp(argv[i], "--raw")) raw = 1;
    }
    if (!gguf || !metal) {
        fprintf(stderr, "usage: %s --model PATH.gguf --metal PATH.metal [--prompt TEXT] [--max-tokens N] [--ctx N] [--loops K] [--verify-prefill-decode] [--raw]\n", argv[0]);
        return 2;
    }

    double t0 = now_sec();
    InkSession *s = NULL;
    char err[512];
    if (ink_open_ex(gguf, metal, max_ctx, loops, &s, err, sizeof err) != 0) {
        fprintf(stderr, "ink_open failed: %s\n", err);
        return 1;
    }
    const InkHParams *hp = ink_hparams(s);
    fprintf(stderr, "loaded in %.2fs (vocab=%d unpadded=%d bos=%d eos=%d phys=%lld loop=[%lld..%lld) K=%lld slots=%lld)\n",
            now_sec() - t0, ink_vocab_size(s), ink_vocab_unpadded(s), ink_bos_token(s), ink_eos_token(s),
            (long long)hp->n_phys, (long long)hp->loop_start, (long long)hp->loop_end,
            (long long)hp->loop_count, (long long)hp->n_slot);

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
    if (ink_prefill(s, toks, n_tok, 1) != 0) {
        fprintf(stderr, "prefill failed: %s\n", ink_error(s));
        ink_close(s); return 1;
    }
    double tp1 = now_sec();
    fprintf(stderr, "prefill %d tok in %.3fs (%.1f tok/s)\n", n_tok, tp1 - tp0, n_tok / (tp1 - tp0 + 1e-9));

    if (verify_prefill_decode) {
        int nv = ink_vocab_unpadded(s);
        float *pre_lg = (float *)malloc((size_t)nv * sizeof(float));
        if (!pre_lg) { ink_close(s); return 1; }
        memcpy(pre_lg, ink_logits(s), (size_t)nv * sizeof(float));
        int top_pre = 0;
        for (int i = 1; i < nv; i++) if (pre_lg[i] > pre_lg[top_pre]) top_pre = i;

        ink_reset(s);
        double td_v0 = now_sec();
        for (int t = 0; t < n_tok; t++) {
            if (ink_step(s, toks[t], t == n_tok - 1 ? 1 : 0) != 0) {
                fprintf(stderr, "step %d failed: %s\n", t, ink_error(s));
                free(pre_lg); ink_close(s); return 1;
            }
        }
        double td_v1 = now_sec();
        const float *dec_lg = ink_logits(s);
        int top_dec = 0;
        float max_diff = 0.0f;
        double sum_sq = 0.0, dot = 0.0, na = 0.0, nb_sq = 0.0;
        for (int i = 0; i < nv; i++) {
            if (dec_lg[i] > dec_lg[top_dec]) top_dec = i;
            float d = fabsf(pre_lg[i] - dec_lg[i]);
            if (d > max_diff) max_diff = d;
            sum_sq += (double)d * (double)d;
            dot += (double)pre_lg[i] * (double)dec_lg[i];
            na += (double)pre_lg[i] * (double)pre_lg[i];
            nb_sq += (double)dec_lg[i] * (double)dec_lg[i];
        }
        double rmse = sqrt(sum_sq / (double)nv);
        double cos_sim = (na > 0.0 && nb_sq > 0.0) ? (dot / sqrt(na * nb_sq)) : 0.0;
        float top_diff = fabsf(pre_lg[top_pre] - dec_lg[top_pre]);
        int ok = (top_pre == top_dec) && (cos_sim > 0.995);
        fprintf(stderr,
                "verify-prefill-decode K=%lld slots=%lld n_tok=%d step_s=%.3f "
                "top1_pre=%d('%s' %.4f) top1_dec=%d('%s' %.4f) top1_diff=%.5f max_abs=%.5f rmse=%.6f cos=%.6f ok=%s\n",
                (long long)hp->loop_count, (long long)hp->n_slot, n_tok, td_v1 - td_v0,
                top_pre, ink_token_str(s, top_pre), pre_lg[top_pre],
                top_dec, ink_token_str(s, top_dec), dec_lg[top_dec],
                top_diff, max_diff, rmse, cos_sim, ok ? "true" : "false");
        free(pre_lg);
        ink_close(s);
        return ok ? 0 : 1;
    }

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
