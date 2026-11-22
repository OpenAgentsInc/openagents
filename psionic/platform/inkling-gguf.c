/* GGUF v3 mmap loader and BPE tokenizer for Psionic Inkling 2 Mini. */
#include "inkling-gguf.h"
#include <ctype.h>
#include <errno.h>
#include <fcntl.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#ifdef _WIN32
#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#else
#include <sys/mman.h>
#include <sys/stat.h>
#include <unistd.h>
#endif

/* Map the whole checkpoint read-only. The file's pages are the weights: no
   heap copy is made on either system. Returns 0 and fills base/file_size/
   map_size, or writes why not into err. */
#ifdef _WIN32
static int ink_map_file(const char *path, uint8_t **base, size_t *file_size, size_t *map_size, char *err, size_t err_bytes) {
    /* Share like POSIX open(): a downloader, sync client or scanner holding
       the file must not make the open fail. */
    HANDLE file = CreateFileA(path, GENERIC_READ, FILE_SHARE_READ | FILE_SHARE_WRITE | FILE_SHARE_DELETE, NULL, OPEN_EXISTING, FILE_ATTRIBUTE_NORMAL, NULL);
    if (file == INVALID_HANDLE_VALUE) { snprintf(err, err_bytes, "open %s: Windows error %lu", path, (unsigned long)GetLastError()); return 1; }
    LARGE_INTEGER size;
    if (!GetFileSizeEx(file, &size) || size.QuadPart < 64) {
        snprintf(err, err_bytes, "stat %s failed", path);
        CloseHandle(file); return 1;
    }
    HANDLE mapping = CreateFileMappingA(file, NULL, PAGE_READONLY, 0, 0, NULL);
    if (!mapping) {
        snprintf(err, err_bytes, "mmap %s: Windows error %lu", path, (unsigned long)GetLastError());
        CloseHandle(file); return 1;
    }
    void *view = MapViewOfFile(mapping, FILE_MAP_READ, 0, 0, 0);
    unsigned long why = GetLastError();
    /* The view keeps the mapping and the file alive. */
    CloseHandle(mapping);
    CloseHandle(file);
    if (!view) { snprintf(err, err_bytes, "mmap %s: Windows error %lu", path, why); return 1; }
    *base = (uint8_t *)view;
    *file_size = (size_t)size.QuadPart;
    *map_size = (size_t)size.QuadPart;
    return 0;
}
static void ink_unmap_file(uint8_t *base, size_t map_size) { (void)map_size; UnmapViewOfFile(base); }
#else
static int ink_map_file(const char *path, uint8_t **base, size_t *file_size, size_t *map_size, char *err, size_t err_bytes) {
    int fd = open(path, O_RDONLY);
    if (fd < 0) { snprintf(err, err_bytes, "open %s: %s", path, strerror(errno)); return 1; }
    struct stat st;
    if (fstat(fd, &st) != 0 || st.st_size < 64) {
        snprintf(err, err_bytes, "stat %s failed", path);
        close(fd); return 1;
    }
    size_t page = (size_t)sysconf(_SC_PAGESIZE);
    size_t fsize = (size_t)st.st_size;
    size_t msize = (fsize + page - 1) & ~(page - 1);
    void *map = mmap(NULL, msize, PROT_READ, MAP_PRIVATE, fd, 0);
    /* The mapping outlives the descriptor. */
    int why = errno;
    close(fd);
    if (map == MAP_FAILED) { snprintf(err, err_bytes, "mmap %s: %s", path, strerror(why)); return 1; }
    *base = (uint8_t *)map;
    *file_size = fsize;
    *map_size = msize;
    return 0;
}
static void ink_unmap_file(uint8_t *base, size_t map_size) { munmap(base, map_size); }
#endif

#define GGUF_MAGIC 0x46554747u /* 'GGUF' */

enum {
    GT_UINT8   = 0, GT_INT8    = 1, GT_UINT16  = 2, GT_INT16   = 3,
    GT_UINT32  = 4, GT_INT32   = 5, GT_FLOAT32 = 6, GT_BOOL    = 7,
    GT_STRING  = 8, GT_ARRAY   = 9, GT_UINT64  = 10, GT_INT64  = 11,
    GT_FLOAT64 = 12
};

typedef struct {
    const uint8_t *p;
    const uint8_t *end;
} Reader;

static int rd_bytes(Reader *r, void *dst, size_t n) {
    if ((size_t)(r->end - r->p) < n) return 1;
    if (dst) memcpy(dst, r->p, n);
    r->p += n;
    return 0;
}
static int rd_u32(Reader *r, uint32_t *v) { return rd_bytes(r, v, 4); }
static int rd_u64(Reader *r, uint64_t *v) { return rd_bytes(r, v, 8); }

static int rd_str_view(Reader *r, const char **ptr, uint64_t *len) {
    if (rd_u64(r, len)) return 1;
    if ((uint64_t)(r->end - r->p) < *len) return 1;
    *ptr = (const char *)r->p;
    r->p += *len;
    return 0;
}

static int rd_scalar_i64(Reader *r, uint32_t t, int64_t *out) {
    uint8_t u8; int8_t i8; uint16_t u16; int16_t i16;
    uint32_t u32; int32_t i32; uint64_t u64; int64_t i64;
    switch (t) {
        case GT_UINT8:  if (rd_bytes(r, &u8, 1)) return 1; *out = u8; return 0;
        case GT_INT8:   if (rd_bytes(r, &i8, 1)) return 1; *out = i8; return 0;
        case GT_UINT16: if (rd_bytes(r, &u16, 2)) return 1; *out = u16; return 0;
        case GT_INT16:  if (rd_bytes(r, &i16, 2)) return 1; *out = i16; return 0;
        case GT_UINT32: if (rd_bytes(r, &u32, 4)) return 1; *out = u32; return 0;
        case GT_INT32:  if (rd_bytes(r, &i32, 4)) return 1; *out = i32; return 0;
        case GT_UINT64: if (rd_bytes(r, &u64, 8)) return 1; *out = (int64_t)u64; return 0;
        case GT_INT64:  if (rd_bytes(r, &i64, 8)) return 1; *out = i64; return 0;
        case GT_BOOL:   if (rd_bytes(r, &u8, 1)) return 1; *out = u8 != 0; return 0;
        default: return 1;
    }
}

static int rd_scalar_f32(Reader *r, uint32_t t, float *out) {
    float f; double d;
    if (t == GT_FLOAT32) { if (rd_bytes(r, &f, 4)) return 1; *out = f; return 0; }
    if (t == GT_FLOAT64) { if (rd_bytes(r, &d, 8)) return 1; *out = (float)d; return 0; }
    int64_t iv;
    if (rd_scalar_i64(r, t, &iv) == 0) { *out = (float)iv; return 0; }
    return 1;
}

static int skip_val(Reader *r, uint32_t t) {
    switch (t) {
        case GT_UINT8: case GT_INT8: case GT_BOOL: return rd_bytes(r, NULL, 1);
        case GT_UINT16: case GT_INT16: return rd_bytes(r, NULL, 2);
        case GT_UINT32: case GT_INT32: case GT_FLOAT32: return rd_bytes(r, NULL, 4);
        case GT_UINT64: case GT_INT64: case GT_FLOAT64: return rd_bytes(r, NULL, 8);
        case GT_STRING: { const char *s; uint64_t l; return rd_str_view(r, &s, &l); }
        case GT_ARRAY: {
            uint32_t at; uint64_t al;
            if (rd_u32(r, &at) || rd_u64(r, &al)) return 1;
            for (uint64_t i = 0; i < al; i++) if (skip_val(r, at)) return 1;
            return 0;
        }
        default: return 1;
    }
}

static uint64_t fnv1a(const char *s, size_t n) {
    uint64_t h = 14695981039346656037ull;
    for (size_t i = 0; i < n; i++) { h ^= (uint8_t)s[i]; h *= 1099511628211ull; }
    return h ? h : 1;
}

static uint64_t pair_key(int32_t a, int32_t b) {
    return (((uint64_t)(uint32_t)a) << 32) | (uint32_t)b;
}

static int key_eq(const char *k, uint64_t kl, const char *lit) {
    size_t ll = strlen(lit);
    return kl == ll && memcmp(k, lit, ll) == 0;
}

static void init_gpt2_byte_maps(InkTokenizer *tok) {
    int is_direct[256];
    memset(is_direct, 0, sizeof is_direct);
    for (int b = 33; b <= 126; b++) is_direct[b] = 1;
    for (int b = 161; b <= 172; b++) is_direct[b] = 1;
    for (int b = 174; b <= 255; b++) is_direct[b] = 1;
    int next_cp = 256;
    memset(tok->byte_decoder, 0, sizeof tok->byte_decoder);
    for (int b = 0; b < 256; b++) {
        int cp = is_direct[b] ? b : next_cp++;
        tok->byte_encoder[b] = cp;
        if (cp < 512) tok->byte_decoder[cp] = (uint8_t)b;
    }
}

static int tok_lookup(const InkTokenizer *tok, const char *s, size_t n) {
    if (!tok->tok_map || tok->tok_cap == 0) return -1;
    uint64_t h = fnv1a(s, n);
    size_t mask = tok->tok_cap - 1;
    size_t idx = (size_t)h & mask;
    for (;;) {
        InkTokHashEntry e = tok->tok_map[idx];
        if (e.hash == 0) return -1;
        if (e.hash == h) {
            const InkVocabTok *vt = &tok->tokens[e.id];
            if (vt->len == n && memcmp(vt->text, s, n) == 0) return e.id;
        }
        idx = (idx + 1) & mask;
    }
}

static int merge_lookup(const InkTokenizer *tok, int32_t a, int32_t b, int32_t *out_merged) {
    if (!tok->merge_map || tok->merge_cap == 0) return -1;
    uint64_t k = pair_key(a, b) + 1;
    uint64_t h = (k ^ (k >> 30)) * 0xbf58476d1ce4e5b9ull;
    h = (h ^ (h >> 27)) * 0x94d049bb133111ebull;
    size_t mask = tok->merge_cap - 1;
    size_t idx = (size_t)h & mask;
    for (;;) {
        InkMergeEntry e = tok->merge_map[idx];
        if (e.key == 0) return -1;
        if (e.key == k) {
            if (out_merged) *out_merged = e.merged_id;
            return e.rank;
        }
        idx = (idx + 1) & mask;
    }
}

static uint64_t tensor_nbytes(uint32_t type, uint32_t ndims, const uint64_t dims[4]) {
    uint64_t ne = 1;
    for (uint32_t i = 0; i < ndims; i++) ne *= dims[i];
    if (type == INK_GGML_F32)    return ne * 4;
    if (type == INK_GGML_Q8_0)   return (ne / 32) * 34;
    if (type == INK_GGML_IQ4_NL) return (ne / 32) * 18;
    return 0;
}

static void bind_tensor(InkGGUF *g, const char *name, uint64_t nlen, const InkTensor *t) {
    char buf[128];
    if (nlen >= sizeof buf) return;
    memcpy(buf, name, nlen);
    buf[nlen] = '\0';

    if (!strcmp(buf, "token_embd.weight"))       { g->tok_embd = *t; return; }
    if (!strcmp(buf, "token_embd_norm.weight"))  { g->tok_norm = *t; return; }
    if (!strcmp(buf, "output_res_query.weight")) { g->out_res_query = *t; return; }
    if (!strcmp(buf, "output_norm.weight"))      { g->out_norm = *t; return; }
    if (!strcmp(buf, "output.weight"))           { g->output = *t; return; }

    if (strncmp(buf, "blk.", 4) != 0) return;
    char *dot = strchr(buf + 4, '.');
    if (!dot) return;
    int il = atoi(buf + 4);
    if (il < 0 || il >= 64) return;
    const char *s = dot + 1;
    InkLayerTensors *L = &g->layers[il];

    if (!strcmp(s, "attn_norm.weight"))         L->attn_norm = *t;
    else if (!strcmp(s, "attn_q_inp_norm.weight"))   L->attn_q_inp_norm = *t;
    else if (!strcmp(s, "attn_k_inp_norm.weight"))   L->attn_k_inp_norm = *t;
    else if (!strcmp(s, "attn_v_inp_norm.weight"))   L->attn_v_inp_norm = *t;
    else if (!strcmp(s, "attn_q.weight"))            L->attn_q = *t;
    else if (!strcmp(s, "attn_k.weight"))            L->attn_k = *t;
    else if (!strcmp(s, "attn_v.weight"))            L->attn_v = *t;
    else if (!strcmp(s, "attn_output.weight"))       L->attn_output = *t;
    else if (!strcmp(s, "attn_gate.weight"))         L->attn_gate = *t;
    else if (!strcmp(s, "attn_rel.weight"))          L->attn_rel = *t;
    else if (!strcmp(s, "attn_rel_bias.weight"))     L->attn_rel_bias = *t;
    else if (!strcmp(s, "attn_q_clip.weight"))       L->attn_q_clip = *t;
    else if (!strcmp(s, "attn_rel_clip.weight"))     L->attn_rel_clip = *t;
    else if (!strcmp(s, "attn_sink_proj.weight"))    L->attn_sink_proj = *t;
    else if (!strcmp(s, "attn_k_conv.weight"))       L->attn_k_conv = *t;
    else if (!strcmp(s, "attn_k_conv_proj.weight"))  L->attn_k_conv_proj = *t;
    else if (!strcmp(s, "attn_v_conv.weight"))       L->attn_v_conv = *t;
    else if (!strcmp(s, "attn_v_conv_proj.weight"))  L->attn_v_conv_proj = *t;
    else if (!strcmp(s, "attn_post_conv.weight"))    L->attn_post_conv = *t;
    else if (!strcmp(s, "attn_res_query.weight"))    L->attn_res_query = *t;
    else if (!strcmp(s, "ffn_res_query.weight"))     L->ffn_res_query = *t;
    else if (!strcmp(s, "ffn_norm.weight"))          L->ffn_norm = *t;
    else if (!strcmp(s, "ffn_gate_inp.weight"))      L->ffn_gate_inp = *t;
    else if (!strcmp(s, "exp_probs_b.bias"))         L->ffn_exp_probs_b = *t;
    else if (!strcmp(s, "ffn_gate_up_exps.weight"))  L->ffn_gate_up_exps = *t;
    else if (!strcmp(s, "ffn_down_exps.weight"))     L->ffn_down_exps = *t;
    else if (!strcmp(s, "ffn_post_conv.weight"))     L->ffn_post_conv = *t;
    else if (!strcmp(s, "loop_msg_query.weight"))    L->loop_msg_query = *t;
    else if (!strcmp(s, "loop_q_ctrl_down.weight"))    L->loop_ctrl_down[0] = *t;
    else if (!strcmp(s, "loop_q_ctrl_gain.weight"))    L->loop_ctrl_gain[0] = *t;
    else if (!strcmp(s, "loop_q_ctrl_forget.weight"))  L->loop_ctrl_forget[0] = *t;
    else if (!strcmp(s, "loop_k_ctrl_down.weight"))    L->loop_ctrl_down[1] = *t;
    else if (!strcmp(s, "loop_k_ctrl_gain.weight"))    L->loop_ctrl_gain[1] = *t;
    else if (!strcmp(s, "loop_k_ctrl_forget.weight"))  L->loop_ctrl_forget[1] = *t;
    else if (!strcmp(s, "loop_v_ctrl_down.weight"))    L->loop_ctrl_down[2] = *t;
    else if (!strcmp(s, "loop_v_ctrl_gain.weight"))    L->loop_ctrl_gain[2] = *t;
    else if (!strcmp(s, "loop_v_ctrl_forget.weight"))  L->loop_ctrl_forget[2] = *t;
    else if (!strcmp(s, "loop_ffn_ctrl_down.weight"))  L->loop_ctrl_down[3] = *t;
    else if (!strcmp(s, "loop_ffn_ctrl_gain.weight"))  L->loop_ctrl_gain[3] = *t;
    else if (!strcmp(s, "loop_ffn_ctrl_forget.weight")) L->loop_ctrl_forget[3] = *t;
}

int ink_gguf_open(const char *path, InkGGUF *g, char *err, size_t err_bytes) {
    memset(g, 0, sizeof *g);
    g->fd = -1;
    if (ink_map_file(path, &g->map, &g->file_size, &g->map_size, err, err_bytes) != 0) return 1;

    Reader r = { g->map, g->map + g->file_size };
    uint32_t magic = 0, ver = 0;
    uint64_t n_tensors = 0, n_kv = 0;
    if (rd_u32(&r, &magic) || magic != GGUF_MAGIC || rd_u32(&r, &ver) || (ver != 2 && ver != 3) ||
        rd_u64(&r, &n_tensors) || rd_u64(&r, &n_kv)) {
        snprintf(err, err_bytes, "invalid GGUF header in %s", path);
        ink_gguf_close(g); return 1;
    }

    /* Defaults matching Inkling 2 Mini */
    InkHParams *hp = &g->hp;
    hp->rms_eps = 1e-6f;
    hp->expert_weights_scale = 8.0f;
    hp->situ_beta = 4.0f;
    hp->situ_linear_beta = 25.0f;
    hp->logit_softcap = 30.0f;
    hp->qk_clip = 22.6274166f;
    hp->rel_clip = 8.0f;
    hp->log_scale_alpha = 0.1f;
    hp->log_scale_floor = 8192;
    hp->conv_len = 4;
    hp->block_size = 6;
    hp->eos_token = 200006;
    uint64_t alignment = 32;

    const uint8_t *merges_start = NULL;
    uint64_t merges_count = 0;
    int32_t *tok_types = NULL;
    uint64_t tok_types_count = 0;

    for (uint64_t i = 0; i < n_kv; i++) {
        const char *k; uint64_t kl;
        uint32_t vt;
        if (rd_str_view(&r, &k, &kl) || rd_u32(&r, &vt)) {
            snprintf(err, err_bytes, "truncated GGUF KV"); ink_gguf_close(g); return 1;
        }
        if (key_eq(k, kl, "general.alignment")) {
            int64_t a = 32; if (rd_scalar_i64(&r, vt, &a)) goto fail_kv;
            if (a > 0) alignment = (uint64_t)a;
        } else if (key_eq(k, kl, "inkling2_loop.block_count")) {
            if (rd_scalar_i64(&r, vt, &hp->n_phys)) goto fail_kv;
        } else if (key_eq(k, kl, "inkling2_loop.embedding_length")) {
            if (rd_scalar_i64(&r, vt, &hp->n_embd)) goto fail_kv;
        } else if (key_eq(k, kl, "inkling2_loop.expert_feed_forward_length")) {
            if (rd_scalar_i64(&r, vt, &hp->n_ff_exp)) goto fail_kv;
        } else if (key_eq(k, kl, "inkling2_loop.expert_count")) {
            if (rd_scalar_i64(&r, vt, &hp->n_expert)) goto fail_kv;
        } else if (key_eq(k, kl, "inkling2_loop.expert_used_count")) {
            if (rd_scalar_i64(&r, vt, &hp->n_expert_used)) goto fail_kv;
        } else if (key_eq(k, kl, "inkling2_loop.attention.key_length")) {
            if (rd_scalar_i64(&r, vt, &hp->head_dim)) goto fail_kv;
        } else if (key_eq(k, kl, "inkling2_loop.vocab_size")) {
            if (rd_scalar_i64(&r, vt, &hp->n_vocab)) goto fail_kv;
        } else if (key_eq(k, kl, "inkling2_loop.vocab_size_unpadded")) {
            if (rd_scalar_i64(&r, vt, &hp->n_vocab_unpadded)) goto fail_kv;
        } else if (key_eq(k, kl, "inkling2_loop.attention.sliding_window")) {
            if (rd_scalar_i64(&r, vt, &hp->swa)) goto fail_kv;
        } else if (key_eq(k, kl, "inkling2_loop.shortconv.l_cache")) {
            if (rd_scalar_i64(&r, vt, &hp->conv_len)) goto fail_kv;
        } else if (key_eq(k, kl, "inkling2_loop.attn_res.block_size")) {
            if (rd_scalar_i64(&r, vt, &hp->block_size)) goto fail_kv;
        } else if (key_eq(k, kl, "inkling2_loop.attention.relative_length")) {
            if (rd_scalar_i64(&r, vt, &hp->rel_len)) goto fail_kv;
        } else if (key_eq(k, kl, "inkling2_loop.attention.relative_extent")) {
            if (rd_scalar_i64(&r, vt, &hp->rel_extent)) goto fail_kv;
        } else if (key_eq(k, kl, "inkling2_loop.attention.log_scale_floor")) {
            if (rd_scalar_i64(&r, vt, &hp->log_scale_floor)) goto fail_kv;
        } else if (key_eq(k, kl, "inkling2_loop.loop.start")) {
            if (rd_scalar_i64(&r, vt, &hp->loop_start)) goto fail_kv;
        } else if (key_eq(k, kl, "inkling2_loop.loop.end")) {
            if (rd_scalar_i64(&r, vt, &hp->loop_end)) goto fail_kv;
        } else if (key_eq(k, kl, "inkling2_loop.loop.count")) {
            if (rd_scalar_i64(&r, vt, &hp->loop_count)) goto fail_kv;
        } else if (key_eq(k, kl, "inkling2_loop.loop.gate_rank")) {
            if (rd_scalar_i64(&r, vt, &hp->gate_rank)) goto fail_kv;
        } else if (key_eq(k, kl, "inkling2_loop.attention.layer_norm_rms_epsilon")) {
            if (rd_scalar_f32(&r, vt, &hp->rms_eps)) goto fail_kv;
        } else if (key_eq(k, kl, "inkling2_loop.expert_weights_scale")) {
            if (rd_scalar_f32(&r, vt, &hp->expert_weights_scale)) goto fail_kv;
        } else if (key_eq(k, kl, "inkling2_loop.activation.situ_beta")) {
            if (rd_scalar_f32(&r, vt, &hp->situ_beta)) goto fail_kv;
        } else if (key_eq(k, kl, "inkling2_loop.activation.situ_linear_beta")) {
            if (rd_scalar_f32(&r, vt, &hp->situ_linear_beta)) goto fail_kv;
        } else if (key_eq(k, kl, "inkling2_loop.final_logit_softcapping")) {
            if (rd_scalar_f32(&r, vt, &hp->logit_softcap)) goto fail_kv;
        } else if (key_eq(k, kl, "inkling2_loop.attention.qk_clip")) {
            if (rd_scalar_f32(&r, vt, &hp->qk_clip)) goto fail_kv;
        } else if (key_eq(k, kl, "inkling2_loop.attention.relative_clip")) {
            if (rd_scalar_f32(&r, vt, &hp->rel_clip)) goto fail_kv;
        } else if (key_eq(k, kl, "inkling2_loop.attention.log_scale_alpha")) {
            if (rd_scalar_f32(&r, vt, &hp->log_scale_alpha)) goto fail_kv;
        } else if (key_eq(k, kl, "tokenizer.ggml.eos_token_id")) {
            if (rd_scalar_i64(&r, vt, &hp->eos_token)) goto fail_kv;
        } else if (key_eq(k, kl, "inkling2_loop.attention.head_count")) {
            if (vt == GT_ARRAY) {
                uint32_t at; uint64_t al; if (rd_u32(&r, &at) || rd_u64(&r, &al)) goto fail_kv;
                for (uint64_t j = 0; j < al; j++) {
                    int64_t v = 0; if (rd_scalar_i64(&r, at, &v)) goto fail_kv;
                    if (j == 0) hp->n_head = v;
                }
            } else if (rd_scalar_i64(&r, vt, &hp->n_head)) goto fail_kv;
        } else if (key_eq(k, kl, "inkling2_loop.attention.head_count_kv")) {
            if (vt == GT_ARRAY) {
                uint32_t at; uint64_t al; if (rd_u32(&r, &at) || rd_u64(&r, &al)) goto fail_kv;
                for (uint64_t j = 0; j < al; j++) {
                    int64_t v = 0; if (rd_scalar_i64(&r, at, &v)) goto fail_kv;
                    if (j == 0) hp->n_head_kv = v;
                }
            } else if (rd_scalar_i64(&r, vt, &hp->n_head_kv)) goto fail_kv;
        } else if (key_eq(k, kl, "inkling2_loop.attention.sliding_window_pattern")) {
            if (vt != GT_ARRAY) goto fail_kv;
            uint32_t at; uint64_t al; if (rd_u32(&r, &at) || rd_u64(&r, &al)) goto fail_kv;
            for (uint64_t j = 0; j < al; j++) {
                int64_t v = 0; if (rd_scalar_i64(&r, at, &v)) goto fail_kv;
                if (j < 64) hp->is_swa[j] = v ? 1 : 0;
            }
        } else if (key_eq(k, kl, "tokenizer.ggml.tokens")) {
            if (vt != GT_ARRAY) goto fail_kv;
            uint32_t at; uint64_t al; if (rd_u32(&r, &at) || rd_u64(&r, &al) || at != GT_STRING) goto fail_kv;
            g->tok.n_tokens = (int64_t)al;
            g->tok.tokens = calloc(al, sizeof(InkVocabTok));
            size_t cap = 1; while (cap < (size_t)al * 2) cap <<= 1;
            g->tok.tok_cap = cap;
            g->tok.tok_map = calloc(cap, sizeof(InkTokHashEntry));
            if (!g->tok.tokens || !g->tok.tok_map) { snprintf(err, err_bytes, "oom tokens"); goto fail_kv; }
            size_t mask = cap - 1;
            for (uint64_t j = 0; j < al; j++) {
                const char *sp; uint64_t sl; if (rd_str_view(&r, &sp, &sl)) goto fail_kv;
                char *copy = malloc((size_t)sl + 1);
                if (!copy) goto fail_kv;
                memcpy(copy, sp, (size_t)sl); copy[sl] = '\0';
                g->tok.tokens[j].text = copy;
                g->tok.tokens[j].len = (uint32_t)sl;
                g->tok.tokens[j].type = 1;
                uint64_t h = fnv1a(copy, (size_t)sl);
                size_t idx = (size_t)h & mask;
                while (g->tok.tok_map[idx].hash != 0) idx = (idx + 1) & mask;
                g->tok.tok_map[idx].hash = h;
                g->tok.tok_map[idx].id = (int32_t)j;
            }
        } else if (key_eq(k, kl, "tokenizer.ggml.token_type")) {
            if (vt != GT_ARRAY) goto fail_kv;
            uint32_t at; uint64_t al; if (rd_u32(&r, &at) || rd_u64(&r, &al)) goto fail_kv;
            tok_types = calloc(al, sizeof(int32_t));
            tok_types_count = al;
            for (uint64_t j = 0; j < al; j++) {
                int64_t v = 1; if (rd_scalar_i64(&r, at, &v)) { free(tok_types); goto fail_kv; }
                tok_types[j] = (int32_t)v;
            }
        } else if (key_eq(k, kl, "tokenizer.ggml.merges")) {
            if (vt != GT_ARRAY) goto fail_kv;
            uint32_t at; uint64_t al; if (rd_u32(&r, &at) || rd_u64(&r, &al) || at != GT_STRING) goto fail_kv;
            merges_start = r.p;
            merges_count = al;
            for (uint64_t j = 0; j < al; j++) {
                const char *sp; uint64_t sl; if (rd_str_view(&r, &sp, &sl)) goto fail_kv;
            }
        } else {
            if (skip_val(&r, vt)) goto fail_kv;
        }
    }

    hp->n_slot = hp->n_phys + (hp->loop_count > 1 ? (hp->loop_count - 1) * (hp->loop_end - hp->loop_start) : 0);
    if (hp->n_vocab_unpadded <= 0) hp->n_vocab_unpadded = g->tok.n_tokens;

    /* Apply token types & build special token list (longest first for greedy matching). */
    init_gpt2_byte_maps(&g->tok);
    if (tok_types) {
        int64_t nspec = 0;
        for (int64_t j = 0; j < g->tok.n_tokens && (uint64_t)j < tok_types_count; j++) {
            g->tok.tokens[j].type = tok_types[j];
            if (tok_types[j] == 3 && g->tok.tokens[j].len > 0) nspec++;
        }
        g->tok.special_ids = malloc((size_t)(nspec + 1) * sizeof(int32_t));
        g->tok.n_special = 0;
        for (int64_t j = 0; j < g->tok.n_tokens && (uint64_t)j < tok_types_count; j++) {
            if (tok_types[j] == 3 && g->tok.tokens[j].len > 0) {
                g->tok.special_ids[g->tok.n_special++] = (int32_t)j;
            }
        }
        /* Sort special tokens by descending length so longer tags match first. */
        for (int64_t a = 0; a < g->tok.n_special; a++) {
            for (int64_t b = a + 1; b < g->tok.n_special; b++) {
                if (g->tok.tokens[g->tok.special_ids[b]].len > g->tok.tokens[g->tok.special_ids[a]].len) {
                    int32_t tmp = g->tok.special_ids[a];
                    g->tok.special_ids[a] = g->tok.special_ids[b];
                    g->tok.special_ids[b] = tmp;
                }
            }
        }
        free(tok_types);
        tok_types = NULL;
    }

    /* Build BPE merge table. */
    if (merges_start && merges_count > 0 && g->tok.n_tokens > 0) {
        size_t cap = 1; while (cap < (size_t)merges_count * 2) cap <<= 1;
        g->tok.merge_cap = cap;
        g->tok.merge_map = calloc(cap, sizeof(InkMergeEntry));
        if (!g->tok.merge_map) { snprintf(err, err_bytes, "oom merges"); ink_gguf_close(g); return 1; }
        size_t mask = cap - 1;
        Reader mr = { merges_start, g->map + g->file_size };
        char joined[1024];
        for (uint64_t rank = 0; rank < merges_count; rank++) {
            const char *sp; uint64_t sl; if (rd_str_view(&mr, &sp, &sl)) break;
            const char *space = memchr(sp, ' ', (size_t)sl);
            if (!space) continue;
            size_t l1 = (size_t)(space - sp);
            size_t l2 = (size_t)sl - l1 - 1;
            if (l1 + l2 >= sizeof joined) continue;
            int id1 = tok_lookup(&g->tok, sp, l1);
            int id2 = tok_lookup(&g->tok, space + 1, l2);
            if (id1 < 0 || id2 < 0) continue;
            memcpy(joined, sp, l1);
            memcpy(joined + l1, space + 1, l2);
            int mid = tok_lookup(&g->tok, joined, l1 + l2);
            if (mid < 0) continue;
            uint64_t k = pair_key(id1, id2) + 1;
            uint64_t h = (k ^ (k >> 30)) * 0xbf58476d1ce4e5b9ull;
            h = (h ^ (h >> 27)) * 0x94d049bb133111ebull;
            size_t idx = (size_t)h & mask;
            while (g->tok.merge_map[idx].key != 0 && g->tok.merge_map[idx].key != k) idx = (idx + 1) & mask;
            if (g->tok.merge_map[idx].key == 0) {
                g->tok.merge_map[idx].key = k;
                g->tok.merge_map[idx].rank = (int32_t)rank;
                g->tok.merge_map[idx].merged_id = mid;
            }
        }
    }

    /* Read tensor headers. */
    typedef struct { const char *name; uint64_t nlen; InkTensor t; } RawT;
    RawT *raw = calloc((size_t)n_tensors, sizeof(RawT));
    if (!raw) { snprintf(err, err_bytes, "oom tensor list"); ink_gguf_close(g); return 1; }
    for (uint64_t i = 0; i < n_tensors; i++) {
        const char *name; uint64_t nlen; uint32_t nd = 0, tt = 0; uint64_t off = 0;
        if (rd_str_view(&r, &name, &nlen) || rd_u32(&r, &nd) || nd > 4) {
            free(raw); snprintf(err, err_bytes, "bad tensor header"); ink_gguf_close(g); return 1;
        }
        InkTensor t; memset(&t, 0, sizeof t);
        t.ndims = nd; t.present = 1;
        for (uint32_t d = 0; d < nd; d++) if (rd_u64(&r, &t.dims[d])) { free(raw); ink_gguf_close(g); return 1; }
        if (rd_u32(&r, &tt) || rd_u64(&r, &off)) { free(raw); ink_gguf_close(g); return 1; }
        t.type = tt; t.offset = off; t.nbytes = tensor_nbytes(tt, nd, t.dims);
        raw[i].name = name; raw[i].nlen = nlen; raw[i].t = t;
    }
    uint64_t pos = (uint64_t)(r.p - g->map);
    g->data_offset = (pos + alignment - 1) & ~(alignment - 1);
    for (uint64_t i = 0; i < n_tensors; i++) {
        raw[i].t.offset += g->data_offset;
        bind_tensor(g, raw[i].name, raw[i].nlen, &raw[i].t);
    }
    free(raw);

    if (!g->tok_embd.present || !g->output.present || hp->n_phys <= 0 || hp->n_embd <= 0) {
        snprintf(err, err_bytes, "missing required Inkling 2 Mini tensors in %s", path);
        ink_gguf_close(g); return 1;
    }
    return 0;

fail_kv:
    free(tok_types);
    if (!err[0]) snprintf(err, err_bytes, "failed parsing GGUF KV");
    ink_gguf_close(g);
    return 1;
}

void ink_gguf_close(InkGGUF *g) {
    if (!g) return;
    if (g->tok.tokens) {
        for (int64_t i = 0; i < g->tok.n_tokens; i++) free(g->tok.tokens[i].text);
        free(g->tok.tokens);
    }
    free(g->tok.tok_map);
    free(g->tok.merge_map);
    free(g->tok.special_ids);
    if (g->map && g->map_size) ink_unmap_file(g->map, g->map_size);
    memset(g, 0, sizeof *g);
    g->fd = -1;
}

/* ---- GPT-2 byte-level BPE encoding & decoding ---- */

static size_t encode_utf8_cp(int32_t cp, char *out) {
    if (cp < 0x80) { out[0] = (char)cp; return 1; }
    if (cp < 0x800) { out[0] = (char)(0xC0 | (cp >> 6)); out[1] = (char)(0x80 | (cp & 0x3F)); return 2; }
    out[0] = (char)(0xE0 | (cp >> 12));
    out[1] = (char)(0x80 | ((cp >> 6) & 0x3F));
    out[2] = (char)(0x80 | (cp & 0x3F));
    return 3;
}

/* Encode one pre-tokenized byte chunk using GPT-2 byte mapping + BPE merges. */
static int64_t bpe_encode_chunk(const InkTokenizer *tok, const uint8_t *bytes, size_t n,
                                int32_t *out, int64_t max_out) {
    if (n == 0) return 0;
    int32_t *syms = malloc(n * sizeof(int32_t));
    if (!syms) return 0;
    size_t count = 0;
    char u8[4];
    for (size_t i = 0; i < n; i++) {
        size_t ul = encode_utf8_cp(tok->byte_encoder[bytes[i]], u8);
        int id = tok_lookup(tok, u8, ul);
        if (id >= 0) syms[count++] = id;
    }
    while (count >= 2) {
        int32_t best_rank = 0x7fffffff;
        size_t best_pos = (size_t)-1;
        int32_t best_mid = -1;
        for (size_t i = 0; i + 1 < count; i++) {
            int32_t mid = -1;
            int r = merge_lookup(tok, syms[i], syms[i + 1], &mid);
            if (r >= 0 && r < best_rank) {
                best_rank = r;
                best_pos = i;
                best_mid = mid;
            }
        }
        if (best_pos == (size_t)-1) break;
        syms[best_pos] = best_mid;
        memmove(&syms[best_pos + 1], &syms[best_pos + 2], (count - best_pos - 2) * sizeof(int32_t));
        count--;
    }
    int64_t written = 0;
    for (size_t i = 0; i < count && written < max_out; i++) out[written++] = syms[i];
    free(syms);
    return written;
}

static int is_ascii_letter(uint8_t c) { return (c >= 'A' && c <= 'Z') || (c >= 'a' && c <= 'z') || c >= 0x80; }
static int is_ascii_digit(uint8_t c)  { return (c >= '0' && c <= '9'); }
static int is_ws_non_nl(uint8_t c)    { return c == ' ' || c == '\t' || c == '\v' || c == '\f'; }
static int is_nl(uint8_t c)           { return c == '\n' || c == '\r'; }

/* GPT-4o / o200k-style chunking for regular text segments (matches inkling2 pre-tokenizer closely). */
static size_t next_chunk_len(const uint8_t *s, size_t n) {
    if (n == 0) return 0;
    /* Contractions: 's 't 're 've 'm 'll 'd */
    if (s[0] == '\'' && n >= 2) {
        uint8_t c1 = (uint8_t)tolower(s[1]);
        if (c1 == 's' || c1 == 't' || c1 == 'm' || c1 == 'd') return 2;
        if (n >= 3) {
            uint8_t c2 = (uint8_t)tolower(s[2]);
            if ((c1 == 'r' && c2 == 'e') || (c1 == 'v' && c2 == 'e') || (c1 == 'l' && c2 == 'l')) return 3;
        }
    }
    /* Optional single space/punct + letters */
    size_t i = 0;
    if (is_ascii_letter(s[0])) {
        while (i < n && is_ascii_letter(s[i])) i++;
        return i;
    }
    if (!is_ascii_digit(s[0]) && !is_nl(s[0]) && !is_ascii_letter(s[0]) && n >= 2 && is_ascii_letter(s[1])) {
        i = 1;
        while (i < n && is_ascii_letter(s[i])) i++;
        return i;
    }
    /* Digits up to 3 (or 1-3) */
    if (is_ascii_digit(s[0])) {
        while (i < n && i < 3 && is_ascii_digit(s[i])) i++;
        return i;
    }
    /* Newlines (with optional preceding spaces handled below) */
    if (is_nl(s[0])) {
        while (i < n && is_nl(s[i])) i++;
        return i;
    }
    /* Single space + punctuation/symbols (must precede generic whitespace!) */
    if (s[0] == ' ' && n >= 2 && !is_ws_non_nl(s[1]) && !is_nl(s[1]) && !is_ascii_letter(s[1]) && !is_ascii_digit(s[1])) {
        i = 1;
        while (i < n && !is_ws_non_nl(s[i]) && !is_nl(s[i]) && !is_ascii_letter(s[i]) && !is_ascii_digit(s[i])) i++;
        while (i < n && is_nl(s[i])) i++;
        return i;
    }
    /* Whitespace */
    if (is_ws_non_nl(s[0])) {
        while (i < n && is_ws_non_nl(s[i])) i++;
        if (i < n && is_nl(s[i])) {
            while (i < n && is_nl(s[i])) i++;
            return i;
        }
        if (i > 1 && i < n) return i - 1; /* keep last space attached to next word */
        return i;
    }
    /* Punctuation / symbols */
    while (i < n && !is_ws_non_nl(s[i]) && !is_nl(s[i]) && !is_ascii_letter(s[i]) && !is_ascii_digit(s[i])) i++;
    while (i < n && is_nl(s[i])) i++;
    return i ? i : 1;
}

int64_t ink_gguf_tokenize(const InkGGUF *g, const char *text, size_t len,
                          int32_t *out_ids, int64_t max_ids) {
    const InkTokenizer *tok = &g->tok;
    int64_t out_n = 0;
    size_t pos = 0;
    while (pos < len && out_n < max_ids) {
        /* Check longest matching special token at pos. */
        int32_t matched_spec = -1;
        size_t matched_len = 0;
        if (text[pos] == '<') {
            for (int64_t s = 0; s < tok->n_special; s++) {
                int32_t sid = tok->special_ids[s];
                const InkVocabTok *vt = &tok->tokens[sid];
                if (vt->len <= len - pos && memcmp(text + pos, vt->text, vt->len) == 0) {
                    matched_spec = sid;
                    matched_len = vt->len;
                    break;
                }
            }
        }
        if (matched_spec >= 0) {
            out_ids[out_n++] = matched_spec;
            pos += matched_len;
            continue;
        }
        /* Scan until the next potential special token. */
        size_t seg_end = pos + 1;
        while (seg_end < len) {
            if (text[seg_end] == '<') {
                int hit = 0;
                for (int64_t s = 0; s < tok->n_special; s++) {
                    const InkVocabTok *vt = &tok->tokens[tok->special_ids[s]];
                    if (vt->len <= len - seg_end && memcmp(text + seg_end, vt->text, vt->len) == 0) { hit = 1; break; }
                }
                if (hit) break;
            }
            seg_end++;
        }
        size_t p = pos;
        while (p < seg_end && out_n < max_ids) {
            size_t cl = next_chunk_len((const uint8_t *)(text + p), seg_end - p);
            out_n += bpe_encode_chunk(tok, (const uint8_t *)(text + p), cl, out_ids + out_n, max_ids - out_n);
            p += cl;
        }
        pos = seg_end;
    }
    return out_n;
}

size_t ink_gguf_decode_token(const InkGGUF *g, int32_t id, char *out, size_t max_out) {
    if (id < 0 || id >= g->tok.n_tokens) return 0;
    const InkVocabTok *vt = &g->tok.tokens[id];
    if (vt->type == 3) {
        size_t n = vt->len < max_out ? vt->len : max_out;
        memcpy(out, vt->text, n);
        return n;
    }
    /* Decode GPT-2 unicode codepoints back to raw UTF-8 bytes. */
    size_t i = 0, o = 0;
    const uint8_t *s = (const uint8_t *)vt->text;
    while (i < vt->len && o < max_out) {
        uint32_t cp = 0;
        if (s[i] < 0x80) { cp = s[i++]; }
        else if ((s[i] & 0xE0) == 0xC0 && i + 1 < vt->len) {
            cp = ((uint32_t)(s[i] & 0x1F) << 6) | (s[i + 1] & 0x3F); i += 2;
        } else if ((s[i] & 0xF0) == 0xE0 && i + 2 < vt->len) {
            cp = ((uint32_t)(s[i] & 0x0F) << 12) | ((uint32_t)(s[i + 1] & 0x3F) << 6) | (s[i + 2] & 0x3F); i += 3;
        } else {
            out[o++] = (char)s[i++]; continue;
        }
        if (cp < 512) out[o++] = (char)g->tok.byte_decoder[cp];
        else out[o++] = '?';
    }
    return o;
}

int32_t ink_gguf_find_token(const InkGGUF *g, const char *name) {
    return tok_lookup(&g->tok, name, strlen(name));
}
