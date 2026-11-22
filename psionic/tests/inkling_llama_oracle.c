/* Parity and speed oracle for the Psionic Inkling engines: run the same
   token ids through llama.cpp and write the next-token logits in the format
   of `psionic-inkling-cli --dump-logits`, then optionally time greedy decode.

   This is a comparator only. It is built by hand against an unpacked
   llama.cpp release and its headers of the same tag, and nothing here is
   linked into anything Psionic ships:

     cc -O2 -I$LLAMA_INCLUDE psionic/tests/inkling_llama_oracle.c \
        -L$LLAMA_DIR -lllama -lggml -lggml-base -Wl,-rpath,$LLAMA_DIR -o inkling-llama-oracle

   Run it with LLAMA_DIR set to the unpacked release.

   Usage:
     inkling-llama-oracle --model PATH.gguf (--tokens 1,2,3 | --prompt TEXT)
        [--loops K] [--ngl N] [--ctx N] [--chunk N] [--dump-logits FILE] [--generate N] [--print-tokens] */
#include "llama.h"
#include "ggml-backend.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

static double now_sec(void) {
    struct timespec ts;
    clock_gettime(CLOCK_MONOTONIC, &ts);
    return (double)ts.tv_sec + (double)ts.tv_nsec * 1e-9;
}

static int parse_token_list(const char *text, llama_token *out, int max_out) {
    int n = 0;
    while (*text && n < max_out) {
        char *end = NULL;
        long v = strtol(text, &end, 10);
        if (end == text) break;
        out[n++] = (llama_token)v;
        text = end;
        while (*text == ',' || *text == ' ') text++;
    }
    return n;
}

static void quiet(enum ggml_log_level level, const char *text, void *user) {
    (void)user;
    if (level >= GGML_LOG_LEVEL_ERROR) fputs(text, stderr);
}

int main(int argc, char **argv) {
    const char *model_path = NULL, *token_list = NULL, *prompt = NULL, *dump = NULL;
    const char *backend_dir = getenv("LLAMA_DIR") ? getenv("LLAMA_DIR") : ".";
    int loops = 0, ngl = 0, ctx = 4096, generate = 0, print_tokens = 0, verbose = 0, chunk = 0;
    for (int i = 1; i < argc; i++) {
        if (!strcmp(argv[i], "--model") && i + 1 < argc) model_path = argv[++i];
        else if (!strcmp(argv[i], "--tokens") && i + 1 < argc) token_list = argv[++i];
        else if (!strcmp(argv[i], "--prompt") && i + 1 < argc) prompt = argv[++i];
        else if (!strcmp(argv[i], "--dump-logits") && i + 1 < argc) dump = argv[++i];
        else if (!strcmp(argv[i], "--loops") && i + 1 < argc) loops = atoi(argv[++i]);
        else if (!strcmp(argv[i], "--ngl") && i + 1 < argc) ngl = atoi(argv[++i]);
        else if (!strcmp(argv[i], "--ctx") && i + 1 < argc) ctx = atoi(argv[++i]);
        else if (!strcmp(argv[i], "--generate") && i + 1 < argc) generate = atoi(argv[++i]);
        else if (!strcmp(argv[i], "--chunk") && i + 1 < argc) chunk = atoi(argv[++i]);
        else if (!strcmp(argv[i], "--print-tokens")) print_tokens = 1;
        else if (!strcmp(argv[i], "--verbose")) verbose = 1;
    }
    if (!model_path || (!token_list && !prompt)) {
        fprintf(stderr, "usage: %s --model PATH.gguf (--tokens 1,2,3 | --prompt TEXT) [--loops K] [--ngl N] [--ctx N] [--chunk N] [--dump-logits FILE] [--generate N] [--print-tokens]\n", argv[0]);
        return 2;
    }
    if (!verbose) llama_log_set(quiet, NULL);
    ggml_backend_load_all_from_path(backend_dir); /* the release keeps its backends beside the library */
    llama_backend_init();

    struct llama_model_params mp = llama_model_default_params();
    mp.n_gpu_layers = ngl;
    struct llama_model_kv_override overrides[2];
    memset(overrides, 0, sizeof overrides);
    if (loops > 0) {
        overrides[0].tag = LLAMA_KV_OVERRIDE_TYPE_INT;
        snprintf(overrides[0].key, sizeof overrides[0].key, "inkling2_loop.loop.count");
        overrides[0].val_i64 = loops;
        mp.kv_overrides = overrides; /* the list ends at an empty key */
    }
    double t0 = now_sec();
    struct llama_model *model = llama_model_load_from_file(model_path, mp);
    if (!model) { fprintf(stderr, "cannot load %s\n", model_path); return 1; }
    const struct llama_vocab *vocab = llama_model_get_vocab(model);
    int n_vocab = llama_vocab_n_tokens(vocab);

    struct llama_context_params cp = llama_context_default_params();
    cp.n_ctx = (uint32_t)ctx;
    cp.n_batch = (uint32_t)ctx;
    cp.no_perf = true;
    struct llama_context *lc = llama_init_from_model(model, cp);
    if (!lc) { fprintf(stderr, "cannot create a context\n"); return 1; }
    fprintf(stderr, "llama.cpp loaded in %.2fs (vocab=%d ngl=%d loops=%d)\n", now_sec() - t0, n_vocab, ngl, loops);

    static llama_token toks[32768];
    int n_tok = token_list ? parse_token_list(token_list, toks, 32768)
                           : llama_tokenize(vocab, prompt, (int32_t)strlen(prompt), toks, 32768, false, true);
    if (n_tok <= 0) { fprintf(stderr, "no tokens\n"); return 1; }
    if (print_tokens) {
        printf("tokens (%d):", n_tok);
        for (int i = 0; i < n_tok; i++) printf("%s%d", i ? "," : " ", toks[i]);
        printf("\n");
    }

    double tp0 = now_sec();
    /* --chunk N feeds the prompt N tokens at a time; 0 feeds it whole. */
    if (chunk <= 0 || chunk > n_tok) chunk = n_tok;
    for (int at = 0; at < n_tok; at += chunk) {
        int n = n_tok - at < chunk ? n_tok - at : chunk;
        if (llama_decode(lc, llama_batch_get_one(toks + at, n)) != 0) { fprintf(stderr, "llama_decode failed\n"); return 1; }
    }
    const float *logits = llama_get_logits_ith(lc, -1);
    double tp1 = now_sec();
    fprintf(stderr, "prefill %d tok in %.3fs (%.1f tok/s)\n", n_tok, tp1 - tp0, n_tok / (tp1 - tp0 + 1e-9));
    int top = 0;
    for (int i = 1; i < n_vocab; i++) if (logits[i] > logits[top]) top = i;
    fprintf(stderr, "top-1 next: %d (%.4f)\n", top, logits[top]);
    if (dump) {
        FILE *f = fopen(dump, "wb");
        if (!f || fwrite(logits, sizeof(float), (size_t)n_vocab, f) != (size_t)n_vocab) { fprintf(stderr, "cannot write %s\n", dump); return 1; }
        fclose(f);
    }

    if (generate > 0) {
        double td0 = now_sec();
        printf("generated:");
        int made = 0;
        for (int i = 0; i < generate; i++) {
            llama_token next = 0;
            for (int j = 1; j < n_vocab; j++) if (logits[j] > logits[next]) next = j;
            printf("%s%d", i ? "," : " ", next);
            made++;
            if (llama_vocab_is_eog(vocab, next)) break;
            if (llama_decode(lc, llama_batch_get_one(&next, 1)) != 0) { fprintf(stderr, "llama_decode failed\n"); return 1; }
            logits = llama_get_logits_ith(lc, -1);
        }
        double td1 = now_sec();
        printf("\n");
        fprintf(stderr, "generated %d tok in %.3fs (%.2f tok/s)\n", made, td1 - td0, made / (td1 - td0 + 1e-9));
    }
    llama_free(lc);
    llama_model_free(model);
    llama_backend_free();
    return 0;
}
