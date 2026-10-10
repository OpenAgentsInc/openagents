// Dumps llama.cpp per-layer residual rows (l_out-N) and result_norm for a
// token sequence, through the cb_eval hook of the prebuilt libllama.
// usage: lldump MODEL.gguf TOKENS.txt OUTDIR [ngl]
#include "llama.h"
#include "ggml.h"
#include "ggml-backend.h"
#include <cstdio>
#include <cstring>
#include <string>
#include <vector>
#include <fstream>

static std::string g_out;

static bool cb(struct ggml_tensor * t, bool ask, void *) {
    const char * name = ggml_get_name(t);
    bool want = strncmp(name, "l_out-", 6) == 0 || strcmp(name, "result_norm") == 0;
    if (ask) return want;
    if (!want) return true;
    if (t->type != GGML_TYPE_F32 || !ggml_is_contiguous(t)) {
        fprintf(stderr, "skip %s (type %d)\n", name, (int) t->type);
        return true;
    }
    size_t n = ggml_nbytes(t);
    std::vector<char> buf(n);
    ggml_backend_tensor_get(t, buf.data(), 0, n);
    std::string path = g_out + "/" + name + ".f32";
    FILE * f = fopen(path.c_str(), "wb");
    fwrite(buf.data(), 1, n, f);
    fclose(f);
    fprintf(stderr, "%s ne=[%lld,%lld]\n", name, (long long) t->ne[0], (long long) t->ne[1]);
    return true;
}

int main(int argc, char ** argv) {
    if (argc < 4) { fprintf(stderr, "usage\n"); return 1; }
    g_out = argv[3];
    int ngl = argc > 4 ? atoi(argv[4]) : 99;
    std::vector<llama_token> toks;
    { std::ifstream in(argv[2]); long v; while (in >> v) toks.push_back((llama_token) v); }
    llama_backend_init();
    ggml_backend_load_all();
    auto mp = llama_model_default_params();
    mp.n_gpu_layers = ngl;
    llama_model * model = llama_model_load_from_file(argv[1], mp);
    if (!model) { fprintf(stderr, "load failed\n"); return 1; }
    auto cp = llama_context_default_params();
    uint32_t n = (uint32_t) toks.size();
    cp.n_ctx = n + 256; cp.n_batch = n + 256; cp.n_ubatch = n + 256;
    cp.embeddings = true;
    cp.pooling_type = LLAMA_POOLING_TYPE_NONE;
    cp.cb_eval = cb; cp.cb_eval_user_data = nullptr;
    llama_context * ctx = llama_init_from_model(model, cp);
    if (!ctx) { fprintf(stderr, "ctx failed\n"); return 1; }
    llama_batch b = llama_batch_get_one(toks.data(), (int32_t) n);
    int rc = llama_decode(ctx, b);
    fprintf(stderr, "decode rc=%d tokens=%u\n", rc, n);
    llama_free(ctx); llama_model_free(model);
    return rc;
}
