/* Granite 4.2 8B on Metal, and the CPU repeat of the same launches.
     granite_run MODEL.gguf KERNELS.metal generate [--cpu] [--prompt TEXT | --ids A,B,..] [--steps N] [--capacity N] [--raw]
       renders TEXT as one user turn with thinking off (unless --raw), then
       decodes greedily; prints the ids, the text and the speed.
     granite_run MODEL.gguf KERNELS.metal compare [--prompt TEXT | --ids ..] [--steps N]
       runs Metal and the CPU on the same tokens and compares every layer's
       residual stream and every step's logits and choice.
     granite_run MODEL.gguf KERNELS.metal dump [--prompt TEXT | --ids ..] --out FILE
       writes the prompt's ids, each layer's residual stream and the last
       token's logits (Metal) for the llama.cpp comparison. */
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "host.h"
#include "granite-metal.h"

static char *slurp(const char *path, size_t *n) {
    FILE *f = fopen(path, "rb"); if (!f) return NULL; fseek(f, 0, SEEK_END); long size = ftell(f); fseek(f, 0, SEEK_SET);
    char *p = malloc((size_t)size + 1); if (fread(p, 1, (size_t)size, f) != (size_t)size) { fclose(f); free(p); return NULL; }
    fclose(f); p[size] = 0; *n = (size_t)size; return p;
}
static int64_t prompt_ids(const GrFile *f, const char *prompt, const char *ids, int raw, int64_t *out, int64_t max) {
    if (ids) { int64_t n = 0; const char *p = ids; while (*p && n < max) { out[n++] = strtoll(p, (char **)&p, 10); if (*p == ',') p++; } return n; }
    static uint8_t text[1 << 20]; coax_string input = {prompt, (int64_t)strlen(prompt)};
    if (!raw) {
        coax_t_GrMessage m = {{"user", 4}, input};
        coax_t_GrChat c = gr_chat((coax_v_t_GrMessage){&m, 1}, 1, 0, 0, (coax_v_u8){text, sizeof text});
        if (c.status) { fprintf(stderr, "chat render refused: %u\n", c.status); return -1; }
        input = (coax_string){(const char *)text, c.written};
    }
    static int64_t symbols[4096];
    coax_t_GrTokResult r = gr_encode(f->tables, input, (coax_v_s64){out, max}, (coax_v_s64){symbols, 4096});
    if (r.status) { fprintf(stderr, "tokenizer refused: %u\n", r.status); return -1; }
    return r.written;
}
/* A conversation with tools, rendered by gr_chat_tools and tokenized. */
static int64_t chat_ids(const GrFile *f, coax_t_GrMessage *m, int64_t count, coax_string *tools, int64_t ntools, int think, int64_t *out, int64_t max) {
    static uint8_t text[1 << 20];
    coax_t_GrChat c = gr_chat_tools((coax_v_t_GrMessage){m, count}, (coax_v_string){tools, ntools}, 1, think, 0, (coax_v_u8){text, sizeof text});
    if (c.status) { fprintf(stderr, "chat render refused: %u (message %lld)\n", c.status, (long long)c.cause); return -1; }
    static int64_t symbols[4096];
    coax_t_GrTokResult r = gr_encode(f->tables, (coax_string){(const char *)text, c.written}, (coax_v_s64){out, max}, (coax_v_s64){symbols, 4096});
    if (r.status) { fprintf(stderr, "tokenizer refused: %u\n", r.status); return -1; }
    return r.written;
}
/* Greedy tokens until the end token or steps; the reply's text into reply (NUL-ended). */
static int64_t reply(GrSession *s, const GrFile *f, int64_t next, int64_t steps, char *reply, int64_t cap, int64_t *ids, int64_t *made) {
    *made = 0;
    for (int64_t step = 0; step < steps && next >= 0; step++) {
        if (next == f->binding.shape.eos) break;
        ids[(*made)++] = next;
        if (gr_session_eval(s, &next, 1, &next, NULL, NULL, NULL)) { fprintf(stderr, "%s\n", gr_session_error(s)); return -1; }
    }
    coax_t_GrTokResult d = gr_decode(f->tables, (coax_v_s64){ids, *made}, (coax_v_u8){(uint8_t *)reply, cap - 1});
    if (d.status) { fprintf(stderr, "decode refused %u\n", d.status); return -1; }
    reply[d.written] = 0; return d.written;
}
static void print_text(const GrFile *f, const int64_t *ids, int64_t n) {
    static uint8_t out[1 << 20]; coax_t_GrTokResult d = gr_decode(f->tables, (coax_v_s64){(int64_t *)ids, n}, (coax_v_u8){out, sizeof out});
    if (d.status) { printf("(decode refused %u)\n", d.status); return; }
    printf("%.*s\n", (int)d.written, out);
}

typedef struct { float *layers; int64_t width, tokens, count; } Capture;
static void capture(void *context, int layer, const float *x, int64_t tokens, int64_t width) {
    Capture *c = context; if (layer >= c->count) return;
    c->width = width; c->tokens = tokens; memcpy(c->layers + (size_t)layer * 8 * width, x, (size_t)(tokens * width) * 4);
}
/* max |a - b| / (1 + max |b|) */
static double distance(const float *a, const float *b, int64_t n) {
    double worst = 0, scale = 0; for (int64_t i = 0; i < n; i++) { double d = fabs((double)a[i] - b[i]); if (!(d <= worst)) worst = d; if (fabs(b[i]) > scale) scale = fabs(b[i]); }
    return worst / (1 + scale);
}

int main(int argc, char **argv) {
    if (argc < 4) { fprintf(stderr, "usage: granite_run MODEL.gguf KERNELS.metal generate|compare|dump|tools [options]\n"); return 2; }
    const char *model = argv[1], *mode = argv[3], *prompt = "Hello", *ids_text = NULL, *out_path = NULL, *tools_path = NULL, *result = NULL; int cpu = 0, raw = 0, think = 0; int64_t steps = 16, capacity = 4096;
    for (int i = 4; i < argc; i++) {
        if (!strcmp(argv[i], "--cpu")) cpu = 1; else if (!strcmp(argv[i], "--raw")) raw = 1;
        else if (i + 1 < argc && !strcmp(argv[i], "--prompt")) prompt = argv[++i]; else if (i + 1 < argc && !strcmp(argv[i], "--ids")) ids_text = argv[++i];
        else if (i + 1 < argc && !strcmp(argv[i], "--steps")) steps = atoll(argv[++i]); else if (i + 1 < argc && !strcmp(argv[i], "--capacity")) capacity = atoll(argv[++i]);
        else if (i + 1 < argc && !strcmp(argv[i], "--out")) out_path = argv[++i];
        else if (i + 1 < argc && !strcmp(argv[i], "--tools")) tools_path = argv[++i]; else if (i + 1 < argc && !strcmp(argv[i], "--result")) result = argv[++i];
        else if (!strcmp(argv[i], "--think")) think = 1;
        else { fprintf(stderr, "unknown option %s\n", argv[i]); return 2; }
    }
    size_t source_bytes = 0; char *source = slurp(argv[2], &source_bytes); if (!source) { fprintf(stderr, "cannot read %s\n", argv[2]); return 1; }
    char error[512]; GrSession *s;
    if (gr_session_open(model, capacity, cpu && strcmp(mode, "compare") ? GR_CPU : GR_METAL, source, source_bytes, &s, error, sizeof error)) { fprintf(stderr, "%s\n", error); return 1; }
    const GrFile *f = gr_session_file(s); int64_t vocab = f->binding.shape.vocab, width = f->binding.shape.width, layers = f->binding.shape.layers;
    static int64_t ids[1 << 15];
    if (!strcmp(mode, "tools")) {
        /* tools --tools FILE (one definition a line, as the template's tool_to_json writes it) --prompt TEXT
           [--result TEXT] [--think]: the model's reply and its calls; with --result, each call answered with
           TEXT and the model's next reply. The second turn reuses the first's cache when it extends it. */
        size_t tb = 0; char *tt = tools_path ? slurp(tools_path, &tb) : NULL; if (!tt) { fprintf(stderr, "tools needs --tools FILE\n"); return 2; }
        coax_string tools[64]; int64_t nt = 0;
        for (char *line = strtok(tt, "\n"); line && nt < 64; line = strtok(NULL, "\n")) if (*line) tools[nt++] = (coax_string){line, (int64_t)strlen(line)};
        coax_t_GrMessage m[3] = {{{"user", 4}, {prompt, (int64_t)strlen(prompt)}}};
        int64_t n = chat_ids(f, m, 1, tools, nt, think, ids, 1 << 14); if (n < 1) return 1;
        printf("turn 1: %lld prompt tokens, %lld tools\n", (long long)n, (long long)nt);
        int64_t next, made; double t; static char text[1 << 16]; static int64_t got[4096];
        if (gr_session_eval(s, ids, n, &next, NULL, NULL, &t)) { fprintf(stderr, "%s\n", gr_session_error(s)); return 1; }
        printf("prefill %.1f s\n", t);
        if (reply(s, f, next, steps, text, sizeof text, got, &made) < 0) return 1;
        printf("reply:\n%s\n", text);
        static coax_t_GrParsedCall calls[16]; static coax_t_GrArg args[64];
        coax_t_GrCalls r = gr_tool_calls((coax_string){text, (int64_t)strlen(text)}, (coax_v_t_GrParsedCall){calls, 16}, (coax_v_t_GrArg){args, 64});
        if (r.status) { printf("calls: %s\n", r.status == 1 ? "none" : r.status == 2 ? "malformed" : "too many"); gr_session_close(s); return r.status == 1 ? 0 : 1; }
        for (int64_t c = 0; c < r.calls; c++) {
            printf("call %lld: %.*s(", (long long)c, (int)calls[c].name.count, calls[c].name.data);
            for (int64_t a = 0; a < calls[c].count; a++) { coax_t_GrArg *x = &args[calls[c].first + a]; printf("%s%.*s=%.*s", a ? ", " : "", (int)x->name.count, x->name.data, (int)x->value.count, x->value.data); }
            printf(")\n");
        }
        if (!result) { gr_session_close(s); return 0; }
        /* Turn 2: the reply as an assistant turn with its calls, one result a call. */
        static coax_t_GrToolCall tc[16]; for (int64_t c = 0; c < r.calls; c++) tc[c] = (coax_t_GrToolCall){calls[c].name, (coax_v_t_GrArg){args + calls[c].first, calls[c].count}};
        static coax_t_GrMessage m2[18]; m2[0] = m[0];
        m2[1] = (coax_t_GrMessage){{"assistant", 9}, {text, r.text}, (coax_v_t_GrToolCall){tc, r.calls}};
        for (int64_t c = 0; c < r.calls; c++) m2[2 + c] = (coax_t_GrMessage){{"tool", 4}, {result, (int64_t)strlen(result)}};
        static int64_t ids2[1 << 15]; int64_t n2 = chat_ids(f, m2, 2 + r.calls, tools, nt, think, ids2, 1 << 14); if (n2 < 1) return 1;
        int64_t had = n + made, same = 0; while (same < had && same < n2 && ids2[same] == (same < n ? ids[same] : got[same - n])) same++;
        if (same == had) printf("turn 2: %lld tokens, %lld of them already in the cache\n", (long long)n2, (long long)had);
        else { printf("turn 2: %lld tokens, the cache shares %lld of %lld: evaluated from the start\n", (long long)n2, (long long)same, (long long)had); gr_session_reset(s); had = 0; }
        if (gr_session_eval(s, ids2 + had, n2 - had, &next, NULL, NULL, &t)) { fprintf(stderr, "%s\n", gr_session_error(s)); return 1; }
        printf("prefill %.1f s\n", t);
        if (reply(s, f, next, steps, text, sizeof text, got, &made) < 0) return 1;
        printf("reply:\n%s\n", text); gr_session_close(s); return 0;
    }
    int64_t n = strcmp(mode, "bench-kernels") ? prompt_ids(f, prompt, ids_text, raw, ids, 1 << 14) : 1; if (n < 1) return 1;
    printf("prompt %lld tokens:", (long long)n); for (int64_t i = 0; i < n; i++) printf(" %lld", (long long)ids[i]); printf("\n");
    int code = 0;
    if (!strcmp(mode, "bench-kernels")) {
        /* --prompt "name:per:index,name:per:index,..." alternated, best of trials */
        char list[512]; snprintf(list, sizeof list, "%s", prompt); char *names[16]; int per[16], index[16], count = 0;
        for (char *t = strtok(list, ","); t && count < 16; t = strtok(NULL, ",")) { names[count] = t; char *c = strchr(t, ':'); *c = 0; per[count] = atoi(c + 1); index[count] = atoi(strchr(c + 1, ':') + 1); count++; }
        double best[16]; for (int i = 0; i < count; i++) best[i] = 1e9;
        /* A negative index sweeps that slot over every layer, so nothing is cached. */
        for (int round = 0; round < 10; round++) for (int i = 0; i < count; i++) {
            double g = 0;
            if (index[i] < 0) { for (int l = 0; l < layers; l++) g += gr_session_bench_kernel(s, names[i], per[i], 3 + l * 9 - index[i], 1, 1); g /= layers; }
            else g = gr_session_bench_kernel(s, names[i], per[i], index[i], 20, 1);
            if (g < best[i]) best[i] = g;
        }
        for (int i = 0; i < count; i++) if (index[i] < 0) index[i] = 3 - index[i];
        for (int i = 0; i < count; i++) printf("%-12s rows/group %2d tensor %3d: %7.1f us %6.1f GB/s\n", names[i], per[i], index[i], best[i] * 1e6, f->table[index[i]].bytes / best[i] / 1e9);
        gr_session_close(s); return 0;
    }
    if (!strcmp(mode, "bench-products")) {
        const char *names[9] = {"attn_norm", "q", "k", "v", "o", "ffn_norm", "gate", "up", "down"};
        for (int slot = 1; slot < 9; slot++) {
            if (slot == 5) continue;
            for (int layer = 0; layer < 2; layer++) {
                const coax_t_GrTensor *w = &f->table[3 + layer * 9 + slot];
                for (int64_t t = 1; t <= 8; t *= 8) {
                    double g = gr_session_bench_product(s, 3 + layer * 9 + slot, t, 50) / 50;
                    printf("layer %d %-5s kind %2lld %6lldx%-6lld tokens %lld: %7.1f us %6.1f GB/s\n", layer, names[slot], (long long)w->kind, (long long)w->rows, (long long)w->cols, (long long)t, g * 1e6, w->bytes / g / 1e9);
                }
            }
        }
        double g = gr_session_bench_product(s, 2, 1, 10) / 10; printf("output %.1f us %.1f GB/s\n", g * 1e6, f->table[2].bytes / g / 1e9);
    } else if (!strcmp(mode, "generate")) {
        int64_t next; double prefill = 0, decode = 0, t;
        if (gr_session_eval(s, ids, n, &next, NULL, NULL, &prefill)) { fprintf(stderr, "%s\n", gr_session_error(s)); return 1; }
        int64_t made = 0, out[4096]; double gpu0 = gr_session_gpu_seconds(s);
        for (int64_t step = 0; step < steps && next >= 0; step++) {
            out[made++] = next; if (next == f->binding.shape.eos) break;
            if (step + 1 == steps) break;
            if (gr_session_eval(s, &next, 1, &next, NULL, NULL, &t)) { fprintf(stderr, "%s\n", gr_session_error(s)); return 1; }
            decode += t;
        }
        double gpu = gr_session_gpu_seconds(s) - gpu0;
        printf("generated %lld:", (long long)made); for (int64_t i = 0; i < made; i++) printf(" %lld", (long long)out[i]); printf("\n");
        print_text(f, out, made);
        printf("%s: prefill %lld tokens in %.3f s (%.1f tok/s), decode %lld tokens in %.3f s (%.2f tok/s)\n", cpu ? "cpu" : "metal",
               (long long)n, prefill, n / prefill, (long long)(made - 1), decode, made > 1 ? (made - 1) / decode : 0.0);
        if (!cpu && made > 1) printf("metal: decode GPU time %.2f ms a token (%.2f tok/s if encoding were free)\n", 1e3 * gpu / (made - 1), (made - 1) / gpu);
        if (next == -2) { printf("a logit was not finite\n"); code = 1; }
    } else if (!strcmp(mode, "compare")) {
        GrSession *c; if (gr_session_open(model, capacity, GR_CPU, source, source_bytes, &c, error, sizeof error)) { fprintf(stderr, "%s\n", error); return 1; }
        if (n > 8) n = 8; /* one launch of tokens, so every layer is captured */
        Capture a = {calloc((size_t)layers * 8 * width, 4), 0, 0, layers}, b = {calloc((size_t)layers * 8 * width, 4), 0, 0, layers};
        int64_t na, nb; double worst_layer = 0, worst_logit = 0;
        if (gr_session_eval(s, ids, n, &na, capture, &a, NULL) || gr_session_eval(c, ids, n, &nb, capture, &b, NULL)) { fprintf(stderr, "%s %s\n", gr_session_error(s), gr_session_error(c)); return 1; }
        for (int64_t l = 0; l < layers; l++) { double d = distance(a.layers + l * 8 * width, b.layers + l * 8 * width, n * width); if (d > worst_layer) worst_layer = d; }
        int same = na == nb; double d = distance(gr_session_logits(s), gr_session_logits(c), vocab); worst_logit = d;
        int64_t step = 0;
        for (; step < steps && same && na >= 0; step++) {
            int64_t x = na;
            if (gr_session_eval(s, &x, 1, &na, NULL, NULL, NULL) || gr_session_eval(c, &x, 1, &nb, NULL, NULL, NULL)) { fprintf(stderr, "%s\n", gr_session_error(s)); return 1; }
            d = distance(gr_session_logits(s), gr_session_logits(c), vocab); if (d > worst_logit) worst_logit = d; same = na == nb;
        }
        printf("metal against cpu: %lld prompt tokens, %lld layers: largest residual distance %.3g; %lld greedy steps %s; largest logit distance %.3g\n",
               (long long)n, (long long)layers, worst_layer, (long long)step, same ? "choose the same tokens" : "DIFFER", worst_logit);
        code = !same || worst_layer > 1e-3 || worst_logit > 1e-3;
        gr_session_close(c);
    } else if (!strcmp(mode, "dump")) {
        /* The format of granite_llama.c: header {n, layers, width, vocab,
           steps}, ids, each layer's residual stream, the prompt's logits,
           then each greedy token and the logits after it. */
        if (!out_path) { fprintf(stderr, "dump needs --out\n"); return 2; }
        FILE *o = fopen(out_path, "wb"); int64_t next;
        int64_t header[5] = {n, layers, width, vocab, steps}; fwrite(header, 8, 5, o); fwrite(ids, 8, (size_t)n, o);
        float *all = calloc((size_t)(layers * n * width), 4); Capture a = {calloc((size_t)layers * 8 * width, 4), 0, 0, layers};
        for (int64_t at = 0; at < n; at += 8) {
            int64_t m = n - at < 8 ? n - at : 8;
            if (gr_session_eval(s, ids + at, m, &next, capture, &a, NULL)) { fprintf(stderr, "%s\n", gr_session_error(s)); return 1; }
            for (int64_t l = 0; l < layers; l++) memcpy(all + (size_t)(l * n + at) * width, a.layers + l * 8 * width, (size_t)(m * width) * 4);
        }
        fwrite(all, 4, (size_t)(layers * n * width), o); fwrite(gr_session_logits(s), 4, (size_t)vocab, o);
        for (int64_t step = 0; step < steps; step++) {
            fwrite(&next, 8, 1, o); int64_t x = next;
            if (gr_session_eval(s, &x, 1, &next, NULL, NULL, NULL)) { fprintf(stderr, "%s\n", gr_session_error(s)); return 1; }
            fwrite(gr_session_logits(s), 4, (size_t)vocab, o);
        }
        fclose(o); printf("wrote %s\n", out_path);
    }
    gr_session_close(s); free(source); return code;
}
