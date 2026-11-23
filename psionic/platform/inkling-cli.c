#include "inkling-engine.h"
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#ifdef _WIN32
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
static double now_sec(void) {
    LARGE_INTEGER f, c;
    QueryPerformanceFrequency(&f);
    QueryPerformanceCounter(&c);
    return (double)c.QuadPart / (double)f.QuadPart;
}
#else
#include <time.h>
static double now_sec(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (double)ts.tv_sec + (double)ts.tv_nsec * 1e-9;
}
#endif

/* --tokens "1,2,3": token ids to run in place of an encoded prompt, so two
   runtimes can be compared on exactly the same input. */
static int parse_token_list(const char *text, int32_t *out, int max_out) {
    int n = 0;
    while (*text && n < max_out) {
        char *end = NULL;
        long v = strtol(text, &end, 10);
        if (end == text) break;
        out[n++] = (int32_t)v;
        text = end;
        while (*text == ',' || *text == ' ') text++;
    }
    return n;
}

int main(int argc, char **argv) {
    const char *gguf = NULL;
    const char *metal = NULL;
    const char *lora = getenv("INK_LORA");
    const char *prompt = "What is 17 * 23? Answer with just the number.";
    int max_tokens = 64;
    int max_ctx = 4096;
    int loops = 0;
    int verify_prefill_decode = 0;
    int test_quant_kernels = 0;
    int raw = 0;
    float temp = 0.0f;
    const char *token_list = NULL;
    const char *dump_logits = NULL;

    for (int i = 1; i < argc; i++) {
        if (!strcmp(argv[i], "--model") && i + 1 < argc) gguf = argv[++i];
        else if ((!strcmp(argv[i], "--kernels") || !strcmp(argv[i], "--metal")) && i + 1 < argc) metal = argv[++i];
        else if (!strcmp(argv[i], "--lora") && i + 1 < argc) lora = argv[++i];
        else if (!strcmp(argv[i], "--tokens") && i + 1 < argc) token_list = argv[++i];
        else if (!strcmp(argv[i], "--dump-logits") && i + 1 < argc) dump_logits = argv[++i];
        else if (!strcmp(argv[i], "--prompt") && i + 1 < argc) prompt = argv[++i];
        else if (!strcmp(argv[i], "--max-tokens") && i + 1 < argc) max_tokens = atoi(argv[++i]);
        else if (!strcmp(argv[i], "--ctx") && i + 1 < argc) max_ctx = atoi(argv[++i]);
        else if (!strcmp(argv[i], "--loops") && i + 1 < argc) loops = atoi(argv[++i]);
        else if (!strcmp(argv[i], "--verify-prefill-decode")) verify_prefill_decode = 1;
        else if (!strcmp(argv[i], "--test-quant-kernels")) test_quant_kernels = 1;
        else if (!strcmp(argv[i], "--temp") && i + 1 < argc) temp = (float)atof(argv[++i]);
        else if (!strcmp(argv[i], "--raw")) raw = 1;
    }
    if (test_quant_kernels) {
        if (!metal) { fprintf(stderr, "usage: %s --kernels PATH --test-quant-kernels\n", argv[0]); return 2; }
        char qerr[512];
        if (ink_test_quant_kernels(metal, qerr, sizeof qerr) != 0) {
            fprintf(stderr, "test-quant-kernels failed: %s\n", qerr);
            return 1;
        }
        return 0;
    }
    if (!gguf || !metal) {
        fprintf(stderr, "usage: %s --model PATH.gguf --kernels PATH [--lora PATH.gguf] [--prompt TEXT | --tokens 1,2,3] [--max-tokens N] [--ctx N] [--loops K] [--temp T] [--verify-prefill-decode] [--test-quant-kernels] [--dump-logits FILE] [--raw]\n"
                        "  --kernels  the engine's generated kernel source: inkling-kernels.metal or inkling-kernels-cuda.cu (--metal is the same option)\n", argv[0]);
        return 2;
    }

    double t0 = now_sec();
    InkSession *s = NULL;
    char err[512];
    if (ink_open_lora(gguf, metal, (lora && lora[0]) ? lora : NULL, max_ctx, loops, &s, err, sizeof err) != 0) {
        fprintf(stderr, "ink_open failed: %s\n", err);
        return 1;
    }
    const InkHParams *hp = ink_hparams(s);
    const InkMemoryBudget *mb = ink_memory_budget(s);
    fprintf(stderr, "engine %s\n", ink_backend_name(s));
    fprintf(stderr, "loaded in %.2fs (vocab=%d unpadded=%d bos=%d eos=%d phys=%lld loop=[%lld..%lld) K=%lld slots=%lld)\n",
            now_sec() - t0, ink_vocab_size(s), ink_vocab_unpadded(s), ink_bos_token(s), ink_eos_token(s),
            (long long)hp->n_phys, (long long)hp->loop_start, (long long)hp->loop_end,
            (long long)hp->loop_count, (long long)hp->n_slot);
    if (mb) {
        fprintf(stderr, "memory budget: ram=%lld MB weights=%lld MB kv=%lld MB (%d full + %d swa) scratch=%lld MB os=%lld MB total=%lld MB headroom=%lld MB max_ctx=%lld\n",
                (long long)mb->device_ram_mb, (long long)mb->weights_mb, (long long)mb->kv_cache_mb,
                mb->full_slots, mb->swa_slots, (long long)mb->scratchpad_mb, (long long)mb->os_reserve_mb,
                (long long)mb->total_mb, (long long)mb->headroom_mb, (long long)mb->max_context_len);
    }

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
    int n_tok = token_list ? parse_token_list(token_list, toks, 4096)
                           : ink_encode(s, formatted, strlen(formatted), toks, 4096, 1);
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

    if (dump_logits) {
        /* The next-token logits as raw little-endian f32, one per vocabulary entry. */
        FILE *lf = fopen(dump_logits, "wb");
        if (!lf || fwrite(ink_logits(s), sizeof(float), (size_t)ink_vocab_unpadded(s), lf) != (size_t)ink_vocab_unpadded(s)) {
            fprintf(stderr, "cannot write %s\n", dump_logits);
            if (lf) fclose(lf);
            ink_close(s); return 1;
        }
        fclose(lf);
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
