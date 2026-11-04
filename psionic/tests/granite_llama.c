/* The llama.cpp side of the Granite parity check (opt-in; needs a packaged
   libllama, for example Homebrew's). Runs the same GGUF on the same token
   ids with the Metal backend and writes, in granite_run's dump format, each
   layer's residual stream (the graph's l_out-N tensors), the prompt's last
   logits and a greedy continuation with each step's logits.
     granite_llama MODEL.gguf IDS STEPS OUT
   IDS is comma-separated. GRANITE_LLAMA_ONE=1 feeds the prompt a token
   at a time. llama.cpp is a reference here, never a runtime
   dependency of Psionic. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "ggml-backend.h"
#include "ggml.h"
#include "llama.h"

typedef struct { float *layers; int64_t n, width, layers_count, row; int seen; } Capture;
static bool observe(struct ggml_tensor *t, bool ask, void *user) {
    Capture *c = user; int layer = -1;
    if (strncmp(t->name, "l_out-", 6) != 0) return ask ? false : true;
    layer = atoi(t->name + 6);
    if (ask) return true;
    if (layer < 0 || layer >= c->layers_count || t->type != GGML_TYPE_F32 || t->ne[0] != c->width) return true;
    /* The last layer keeps only the rows that produce logits: the last token. */
    if (c->row >= 0) { if (t->ne[1] != 1) return true; ggml_backend_tensor_get(t, c->layers + ((size_t)layer * c->n + (size_t)c->row) * c->width, 0, (size_t)c->width * 4); }
    else if (t->ne[1] == c->n) ggml_backend_tensor_get(t, c->layers + (size_t)layer * c->n * c->width, 0, (size_t)(c->n * c->width) * 4);
    else if (t->ne[1] == 1) ggml_backend_tensor_get(t, c->layers + ((size_t)layer * c->n + (size_t)(c->n - 1)) * c->width, 0, (size_t)c->width * 4);
    else return true;
    c->seen++;
    return true;
}
static void quiet(enum ggml_log_level level, const char *text, void *user) { (void)level; (void)text; (void)user; }

int main(int argc, char **argv) {
    if (argc != 5) { fprintf(stderr, "usage: granite_llama MODEL.gguf IDS STEPS OUT\n"); return 2; }
    llama_log_set(quiet, NULL); llama_backend_init();
    static llama_token ids[8192]; int64_t n = 0; for (const char *p = argv[2]; *p && n < 8192;) { ids[n++] = (llama_token)strtol(p, (char **)&p, 10); if (*p == ',') p++; }
    int steps = atoi(argv[3]);
    struct llama_model_params mp = llama_model_default_params(); mp.n_gpu_layers = 999;
    struct llama_model *model = llama_model_load_from_file(argv[1], mp); if (!model) { fprintf(stderr, "cannot load %s\n", argv[1]); return 1; }
    const struct llama_vocab *vocab = llama_model_get_vocab(model);
    int64_t width = llama_model_n_embd(model), layers = llama_model_n_layer(model), nv = llama_vocab_n_tokens(vocab);
    /* --one evaluates the prompt a token at a time (llama.cpp's matrix-vector
       kernels, as in decode); otherwise in one batch (its matrix-matrix
       kernels, which round activations and weights to f16). */
    int one = getenv("GRANITE_LLAMA_ONE") != NULL;
    Capture c = {calloc((size_t)(layers * n * width), 4), n, width, layers, one ? 0 : -1, 0};
    struct llama_context_params cp = llama_context_default_params();
    cp.n_ctx = 4096; cp.n_batch = 4096; cp.n_ubatch = 4096; cp.cb_eval = observe; cp.cb_eval_user_data = &c; cp.no_perf = true;
    struct llama_context *ctx = llama_init_from_model(model, cp); if (!ctx) { fprintf(stderr, "cannot create a context\n"); return 1; }
    if (one) {
        for (int64_t i = 0; i < n; i++) { c.row = i; if (llama_decode(ctx, llama_batch_get_one(ids + i, 1))) { fprintf(stderr, "decode failed\n"); return 1; } }
        c.seen /= (int)n;
    } else if (llama_decode(ctx, llama_batch_get_one(ids, (int32_t)n))) { fprintf(stderr, "decode failed\n"); return 1; }
    if (c.seen != layers) { fprintf(stderr, "captured %d of %lld layers\n", c.seen, (long long)layers); return 1; }
    FILE *o = fopen(argv[4], "wb");
    int64_t header[5] = {n, layers, width, nv, steps}; fwrite(header, 8, 5, o);
    for (int64_t i = 0; i < n; i++) { int64_t v = ids[i]; fwrite(&v, 8, 1, o); }
    fwrite(c.layers, 4, (size_t)(layers * n * width), o);
    c.layers_count = 0; /* no more captures */
    const float *logits = llama_get_logits_ith(ctx, -1); fwrite(logits, 4, (size_t)nv, o);
    /* Greedy: the first largest logit. */
    for (int s = 0; s < steps; s++) {
        llama_token best = 0; for (int64_t i = 1; i < nv; i++) if (logits[i] > logits[best]) best = (llama_token)i;
        int64_t v = best; fwrite(&v, 8, 1, o);
        if (llama_decode(ctx, llama_batch_get_one(&best, 1))) { fprintf(stderr, "decode failed\n"); return 1; }
        logits = llama_get_logits_ith(ctx, -1); fwrite(logits, 4, (size_t)nv, o);
    }
    fclose(o); llama_free(ctx); llama_model_free(model); llama_backend_free();
    printf("wrote %s: %lld tokens, %lld layers, %d greedy steps\n", argv[4], (long long)n, (long long)layers, steps);
    return 0;
}
