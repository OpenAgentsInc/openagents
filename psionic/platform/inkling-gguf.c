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

int ink_ggml_block_elems(uint32_t type) {
    switch (type) {
        case INK_GGML_F32:
        case INK_GGML_F16:
        case INK_GGML_BF16:    return 1;
        case INK_GGML_Q4_0:
        case INK_GGML_Q4_1:
        case INK_GGML_Q5_0:
        case INK_GGML_Q5_1:
        case INK_GGML_Q8_0:
        case INK_GGML_IQ4_NL:  return 32;
        case INK_GGML_Q2_0:    return 64;
        case INK_GGML_Q2_K:
        case INK_GGML_Q3_K:
        case INK_GGML_Q4_K:
        case INK_GGML_Q5_K:
        case INK_GGML_Q6_K:
        case INK_GGML_IQ2_XXS:
        case INK_GGML_IQ2_XS:
        case INK_GGML_IQ3_XXS:
        case INK_GGML_IQ3_S:
        case INK_GGML_IQ2_S:
        case INK_GGML_IQ4_XS:  return 256;
        default:               return 0;
    }
}

int ink_ggml_block_bytes(uint32_t type) {
    switch (type) {
        case INK_GGML_F32:     return 4;
        case INK_GGML_F16:     return 2;
        case INK_GGML_BF16:    return 2;
        case INK_GGML_Q4_0:    return 18;
        case INK_GGML_Q4_1:    return 20;
        case INK_GGML_Q5_0:    return 22;
        case INK_GGML_Q5_1:    return 24;
        case INK_GGML_Q8_0:    return 34;
        case INK_GGML_Q2_0:    return 18;
        case INK_GGML_Q2_K:    return 84;
        case INK_GGML_Q3_K:    return 110;
        case INK_GGML_Q4_K:    return 144;
        case INK_GGML_Q5_K:    return 176;
        case INK_GGML_Q6_K:    return 210;
        case INK_GGML_IQ2_XXS: return 66;
        case INK_GGML_IQ2_XS:  return 74;
        case INK_GGML_IQ3_XXS: return 98;
        case INK_GGML_IQ4_NL:  return 18;
        case INK_GGML_IQ3_S:   return 110;
        case INK_GGML_IQ2_S:   return 82;
        case INK_GGML_IQ4_XS:  return 136;
        default:               return 0;
    }
}

int ink_ggml_type_supported(uint32_t type) {
    return ink_ggml_block_elems(type) > 0;
}

const char *ink_ggml_type_name(uint32_t type) {
    switch (type) {
        case INK_GGML_F32:     return "F32";
        case INK_GGML_F16:     return "F16";
        case INK_GGML_Q4_0:    return "Q4_0";
        case INK_GGML_Q4_1:    return "Q4_1";
        case INK_GGML_Q5_0:    return "Q5_0";
        case INK_GGML_Q5_1:    return "Q5_1";
        case INK_GGML_Q8_0:    return "Q8_0";
        case INK_GGML_Q2_K:    return "Q2_K";
        case INK_GGML_Q3_K:    return "Q3_K";
        case INK_GGML_Q4_K:    return "Q4_K";
        case INK_GGML_Q5_K:    return "Q5_K";
        case INK_GGML_Q6_K:    return "Q6_K";
        case INK_GGML_IQ2_XXS: return "IQ2_XXS";
        case INK_GGML_IQ2_XS:  return "IQ2_XS";
        case INK_GGML_IQ3_XXS: return "IQ3_XXS";
        case INK_GGML_IQ4_NL:  return "IQ4_NL";
        case INK_GGML_IQ3_S:   return "IQ3_S";
        case INK_GGML_IQ2_S:   return "IQ2_S";
        case INK_GGML_IQ4_XS:  return "IQ4_XS";
        case INK_GGML_BF16:    return "BF16";
        case INK_GGML_Q2_0:    return "Q2_0";
        default:               return "UNKNOWN";
    }
}

static uint64_t tensor_nbytes(uint32_t type, uint32_t ndims, const uint64_t dims[4]) {
    uint64_t ne = 1;
    for (uint32_t i = 0; i < ndims; i++) ne *= dims[i];
    int be = ink_ggml_block_elems(type);
    int bb = ink_ggml_block_bytes(type);
    if (be <= 0 || bb <= 0) return 0;
    return (ne / (uint64_t)be) * (uint64_t)bb;
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

/* ---- Universal GGML quantization grids & CPU reference dequantizers (llama.cpp compatible) ---- */
static const uint32_t g_ggml_grids[INK_GRID_TOTAL_U32] = {
    0x08080808u, 0x08080808u, 0x0808082bu, 0x08080808u, 0x08081919u, 0x08080808u, 0x08082b08u, 0x08080808u,
    0x08082b2bu, 0x08080808u, 0x08190819u, 0x08080808u, 0x08191908u, 0x08080808u, 0x082b0808u, 0x08080808u,
    0x082b082bu, 0x08080808u, 0x082b2b08u, 0x08080808u, 0x082b2b2bu, 0x08080808u, 0x19080819u, 0x08080808u,
    0x19081908u, 0x08080808u, 0x19190808u, 0x08080808u, 0x19192b08u, 0x08080808u, 0x192b0819u, 0x08080808u,
    0x192b1908u, 0x08080808u, 0x2b080808u, 0x08080808u, 0x2b08082bu, 0x08080808u, 0x2b082b2bu, 0x08080808u,
    0x2b2b082bu, 0x08080808u, 0x08080819u, 0x08080819u, 0x08081908u, 0x08080819u, 0x08190808u, 0x08080819u,
    0x08191919u, 0x08080819u, 0x19080808u, 0x08080819u, 0x2b081908u, 0x08080819u, 0x2b192b08u, 0x08080819u,
    0x08080808u, 0x0808082bu, 0x0808082bu, 0x0808082bu, 0x082b082bu, 0x0808082bu, 0x2b08082bu, 0x0808082bu,
    0x08080819u, 0x08081908u, 0x08081908u, 0x08081908u, 0x08190808u, 0x08081908u, 0x082b0819u, 0x08081908u,
    0x082b1908u, 0x08081908u, 0x19080808u, 0x08081908u, 0x1908082bu, 0x08081908u, 0x19082b08u, 0x08081908u,
    0x192b0808u, 0x08081908u, 0x2b080819u, 0x08081908u, 0x2b081908u, 0x08081908u, 0x2b190808u, 0x08081908u,
    0x2b2b1908u, 0x08081908u, 0x08080808u, 0x08081919u, 0x0808082bu, 0x08081919u, 0x08082b08u, 0x08081919u,
    0x082b0808u, 0x08081919u, 0x1908192bu, 0x08081919u, 0x192b2b19u, 0x08081919u, 0x2b080808u, 0x08081919u,
    0x2b190819u, 0x08081919u, 0x08082b19u, 0x0808192bu, 0x08190808u, 0x0808192bu, 0x19080808u, 0x0808192bu,
    0x2b081908u, 0x0808192bu, 0x2b2b1908u, 0x0808192bu, 0x08080808u, 0x08082b08u, 0x08081919u, 0x08082b08u,
    0x08082b08u, 0x08082b08u, 0x08191908u, 0x08082b08u, 0x082b2b08u, 0x08082b08u, 0x19080819u, 0x08082b08u,
    0x19081908u, 0x08082b08u, 0x19190808u, 0x08082b08u, 0x1919082bu, 0x08082b08u, 0x2b082b08u, 0x08082b08u,
    0x08081908u, 0x08082b19u, 0x19080808u, 0x08082b19u, 0x0808082bu, 0x08082b2bu, 0x08191908u, 0x08082b2bu,
    0x08080819u, 0x08190808u, 0x08081908u, 0x08190808u, 0x08190808u, 0x08190808u, 0x082b0819u, 0x08190808u,
    0x19080808u, 0x08190808u, 0x192b0808u, 0x08190808u, 0x2b081908u, 0x08190808u, 0x2b190808u, 0x08190808u,
    0x2b191919u, 0x08190808u, 0x08080808u, 0x08190819u, 0x08082b08u, 0x08190819u, 0x082b0808u, 0x08190819u,
    0x19190808u, 0x08190819u, 0x19192b2bu, 0x08190819u, 0x2b080808u, 0x08190819u, 0x082b1908u, 0x0819082bu,
    0x19081919u, 0x0819082bu, 0x08080808u, 0x08191908u, 0x08082b08u, 0x08191908u, 0x082b0808u, 0x08191908u,
    0x082b1919u, 0x08191908u, 0x19082b19u, 0x08191908u, 0x2b080808u, 0x08191908u, 0x08192b08u, 0x08191919u,
    0x192b082bu, 0x08191919u, 0x08080808u, 0x0819192bu, 0x0819192bu, 0x0819192bu, 0x08080819u, 0x08192b08u,
    0x08081908u, 0x08192b08u, 0x08190808u, 0x08192b08u, 0x19080808u, 0x08192b08u, 0x2b080819u, 0x08192b08u,
    0x08080808u, 0x08192b19u, 0x08081919u, 0x08192b19u, 0x2b2b0808u, 0x08192b19u, 0x19190819u, 0x08192b2bu,
    0x08080808u, 0x082b0808u, 0x0808082bu, 0x082b0808u, 0x08082b2bu, 0x082b0808u, 0x19081908u, 0x082b0808u,
    0x192b0819u, 0x082b0808u, 0x2b080808u, 0x082b0808u, 0x2b08082bu, 0x082b0808u, 0x082b2b19u, 0x082b0819u,
    0x19082b08u, 0x082b0819u, 0x08080808u, 0x082b082bu, 0x0808082bu, 0x082b082bu, 0x08080819u, 0x082b1908u,
    0x08081908u, 0x082b1908u, 0x08190808u, 0x082b1908u, 0x19080808u, 0x082b1908u, 0x1919192bu, 0x082b1908u,
    0x08080808u, 0x082b1919u, 0x19080819u, 0x082b1919u, 0x192b1908u, 0x082b1919u, 0x2b190808u, 0x082b192bu,
    0x08082b08u, 0x082b2b08u, 0x082b0808u, 0x082b2b08u, 0x2b191908u, 0x082b2b08u, 0x19081908u, 0x082b2b2bu,
    0x08080819u, 0x19080808u, 0x08081908u, 0x19080808u, 0x08190808u, 0x19080808u, 0x08192b08u, 0x19080808u,
    0x082b0819u, 0x19080808u, 0x082b1908u, 0x19080808u, 0x19080808u, 0x19080808u, 0x19082b08u, 0x19080808u,
    0x1919192bu, 0x19080808u, 0x192b0808u, 0x19080808u, 0x2b080819u, 0x19080808u, 0x2b081908u, 0x19080808u,
    0x2b190808u, 0x19080808u, 0x08080808u, 0x19080819u, 0x082b0808u, 0x19080819u, 0x192b0819u, 0x19080819u,
    0x2b080808u, 0x19080819u, 0x2b081919u, 0x19080819u, 0x08080819u, 0x1908082bu, 0x08190808u, 0x1908082bu,
    0x19082b08u, 0x1908082bu, 0x1919192bu, 0x1908082bu, 0x192b2b08u, 0x1908082bu, 0x08080808u, 0x19081908u,
    0x08082b08u, 0x19081908u, 0x082b0808u, 0x19081908u, 0x2b080808u, 0x19081908u, 0x2b192b19u, 0x19081908u,
    0x0819082bu, 0x19081919u, 0x082b1908u, 0x19081919u, 0x08080808u, 0x1908192bu, 0x08080819u, 0x19082b08u,
    0x08081908u, 0x19082b08u, 0x08190808u, 0x19082b08u, 0x19080808u, 0x19082b08u, 0x19081919u, 0x19082b08u,
    0x08080808u, 0x19082b19u, 0x19192b08u, 0x19082b19u, 0x192b0819u, 0x19082b19u, 0x2b08082bu, 0x19082b19u,
    0x19081919u, 0x19082b2bu, 0x2b190808u, 0x19082b2bu, 0x08080808u, 0x19190808u, 0x08082b08u, 0x19190808u,
    0x08190819u, 0x19190808u, 0x08192b19u, 0x19190808u, 0x082b0808u, 0x19190808u, 0x2b080808u, 0x19190808u,
    0x2b082b08u, 0x19190808u, 0x08081908u, 0x19190819u, 0x1908082bu, 0x19190819u, 0x2b2b1908u, 0x19190819u,
    0x2b190819u, 0x1919082bu, 0x2b190808u, 0x19191908u, 0x2b19082bu, 0x19191908u, 0x08082b2bu, 0x19191919u,
    0x08080819u, 0x1919192bu, 0x19191908u, 0x1919192bu, 0x08080808u, 0x19192b08u, 0x08190819u, 0x19192b08u,
    0x08192b19u, 0x19192b08u, 0x192b1908u, 0x19192b08u, 0x19080808u, 0x19192b19u, 0x08082b08u, 0x19192b2bu,
    0x08081908u, 0x192b0808u, 0x08190808u, 0x192b0808u, 0x19080808u, 0x192b0808u, 0x192b2b08u, 0x192b0808u,
    0x08080808u, 0x192b0819u, 0x19191919u, 0x192b0819u, 0x08192b08u, 0x192b082bu, 0x192b0808u, 0x192b082bu,
    0x08080808u, 0x192b1908u, 0x08081919u, 0x192b1908u, 0x08190808u, 0x192b1919u, 0x0819082bu, 0x192b1919u,
    0x2b081908u, 0x192b1919u, 0x1908082bu, 0x192b2b08u, 0x08080808u, 0x2b080808u, 0x0808082bu, 0x2b080808u,
    0x08082b2bu, 0x2b080808u, 0x19080819u, 0x2b080808u, 0x2b08082bu, 0x2b080808u, 0x08081908u, 0x2b080819u,
    0x08192b08u, 0x2b080819u, 0x19080808u, 0x2b080819u, 0x08190819u, 0x2b08082bu, 0x08080819u, 0x2b081908u,
    0x08081908u, 0x2b081908u, 0x08190808u, 0x2b081908u, 0x08191919u, 0x2b081908u, 0x19080808u, 0x2b081908u,
    0x192b0808u, 0x2b081908u, 0x08080808u, 0x2b081919u, 0x1908192bu, 0x2b081919u, 0x2b191908u, 0x2b081919u,
    0x08082b19u, 0x2b08192bu, 0x19080808u, 0x2b08192bu, 0x192b0808u, 0x2b08192bu, 0x0808082bu, 0x2b082b08u,
    0x08081908u, 0x2b082b19u, 0x08190819u, 0x2b082b2bu, 0x08081908u, 0x2b190808u, 0x08190808u, 0x2b190808u,
    0x082b1908u, 0x2b190808u, 0x19080808u, 0x2b190808u, 0x2b2b0819u, 0x2b190808u, 0x0819192bu, 0x2b190819u,
    0x2b080808u, 0x2b190819u, 0x19081919u, 0x2b19082bu, 0x08080808u, 0x2b191908u, 0x082b082bu, 0x2b191908u,
    0x19081908u, 0x2b191908u, 0x19190819u, 0x2b191919u, 0x2b080819u, 0x2b192b08u, 0x082b0808u, 0x2b192b19u,
    0x0808082bu, 0x2b2b0808u, 0x19190808u, 0x2b2b0808u, 0x2b081919u, 0x2b2b0808u, 0x08082b19u, 0x2b2b0819u,
    0x08080808u, 0x2b2b082bu, 0x08192b08u, 0x2b2b1908u, 0x19190808u, 0x2b2b2b08u, 0x08081908u, 0x2b2b2b19u,
    0x08080808u, 0x08080808u, 0x0808082bu, 0x08080808u, 0x08081919u, 0x08080808u, 0x08082b08u, 0x08080808u,
    0x08082b2bu, 0x08080808u, 0x08190819u, 0x08080808u, 0x08191908u, 0x08080808u, 0x0819192bu, 0x08080808u,
    0x08192b19u, 0x08080808u, 0x082b0808u, 0x08080808u, 0x082b082bu, 0x08080808u, 0x082b1919u, 0x08080808u,
    0x082b2b08u, 0x08080808u, 0x19080819u, 0x08080808u, 0x19081908u, 0x08080808u, 0x1908192bu, 0x08080808u,
    0x19082b19u, 0x08080808u, 0x19190808u, 0x08080808u, 0x1919082bu, 0x08080808u, 0x19191919u, 0x08080808u,
    0x19192b08u, 0x08080808u, 0x192b0819u, 0x08080808u, 0x192b1908u, 0x08080808u, 0x2b080808u, 0x08080808u,
    0x2b08082bu, 0x08080808u, 0x2b081919u, 0x08080808u, 0x2b082b08u, 0x08080808u, 0x2b190819u, 0x08080808u,
    0x2b191908u, 0x08080808u, 0x2b192b19u, 0x08080808u, 0x2b2b0808u, 0x08080808u, 0x08080819u, 0x08080819u,
    0x08081908u, 0x08080819u, 0x0808192bu, 0x08080819u, 0x08082b19u, 0x08080819u, 0x08190808u, 0x08080819u,
    0x0819082bu, 0x08080819u, 0x08191919u, 0x08080819u, 0x08192b08u, 0x08080819u, 0x08192b2bu, 0x08080819u,
    0x082b0819u, 0x08080819u, 0x082b1908u, 0x08080819u, 0x19080808u, 0x08080819u, 0x1908082bu, 0x08080819u,
    0x19081919u, 0x08080819u, 0x19082b08u, 0x08080819u, 0x19190819u, 0x08080819u, 0x19191908u, 0x08080819u,
    0x192b0808u, 0x08080819u, 0x192b2b08u, 0x08080819u, 0x2b080819u, 0x08080819u, 0x2b081908u, 0x08080819u,
    0x2b190808u, 0x08080819u, 0x08080808u, 0x0808082bu, 0x0808082bu, 0x0808082bu, 0x08081919u, 0x0808082bu,
    0x08082b08u, 0x0808082bu, 0x08190819u, 0x0808082bu, 0x08191908u, 0x0808082bu, 0x082b0808u, 0x0808082bu,
    0x19080819u, 0x0808082bu, 0x19081908u, 0x0808082bu, 0x19190808u, 0x0808082bu, 0x19191919u, 0x0808082bu,
    0x2b080808u, 0x0808082bu, 0x2b082b2bu, 0x0808082bu, 0x08080819u, 0x08081908u, 0x08081908u, 0x08081908u,
    0x0808192bu, 0x08081908u, 0x08082b19u, 0x08081908u, 0x08190808u, 0x08081908u, 0x0819082bu, 0x08081908u,
    0x08191919u, 0x08081908u, 0x08192b08u, 0x08081908u, 0x082b0819u, 0x08081908u, 0x082b1908u, 0x08081908u,
    0x19080808u, 0x08081908u, 0x1908082bu, 0x08081908u, 0x19081919u, 0x08081908u, 0x19082b08u, 0x08081908u,
    0x19190819u, 0x08081908u, 0x19191908u, 0x08081908u, 0x1919192bu, 0x08081908u, 0x192b0808u, 0x08081908u,
    0x2b080819u, 0x08081908u, 0x2b081908u, 0x08081908u, 0x2b190808u, 0x08081908u, 0x08080808u, 0x08081919u,
    0x0808082bu, 0x08081919u, 0x08081919u, 0x08081919u, 0x08082b08u, 0x08081919u, 0x08190819u, 0x08081919u,
    0x08191908u, 0x08081919u, 0x082b0808u, 0x08081919u, 0x19080819u, 0x08081919u, 0x19081908u, 0x08081919u,
    0x19190808u, 0x08081919u, 0x192b0819u, 0x08081919u, 0x2b080808u, 0x08081919u, 0x08080819u, 0x0808192bu,
    0x08081908u, 0x0808192bu, 0x08190808u, 0x0808192bu, 0x082b192bu, 0x0808192bu, 0x19080808u, 0x0808192bu,
    0x1908082bu, 0x0808192bu, 0x2b081908u, 0x0808192bu, 0x08080808u, 0x08082b08u, 0x0808082bu, 0x08082b08u,
    0x08081919u, 0x08082b08u, 0x08082b08u, 0x08082b08u, 0x08082b2bu, 0x08082b08u, 0x08190819u, 0x08082b08u,
    0x08191908u, 0x08082b08u, 0x082b0808u, 0x08082b08u, 0x082b1919u, 0x08082b08u, 0x19080819u, 0x08082b08u,
    0x19081908u, 0x08082b08u, 0x19190808u, 0x08082b08u, 0x19192b08u, 0x08082b08u, 0x2b080808u, 0x08082b08u,
    0x2b2b0808u, 0x08082b08u, 0x2b2b2b2bu, 0x08082b08u, 0x08080819u, 0x08082b19u, 0x08081908u, 0x08082b19u,
    0x08190808u, 0x08082b19u, 0x19080808u, 0x08082b19u, 0x2b080819u, 0x08082b19u, 0x2b082b19u, 0x08082b19u,
    0x08080808u, 0x08082b2bu, 0x082b0808u, 0x08082b2bu, 0x082b2b08u, 0x08082b2bu, 0x2b19192bu, 0x08082b2bu,
    0x2b2b0808u, 0x08082b2bu, 0x08080819u, 0x08190808u, 0x08081908u, 0x08190808u, 0x0808192bu, 0x08190808u,
    0x08082b19u, 0x08190808u, 0x08190808u, 0x08190808u, 0x0819082bu, 0x08190808u, 0x08191919u, 0x08190808u,
    0x08192b08u, 0x08190808u, 0x082b0819u, 0x08190808u, 0x082b1908u, 0x08190808u, 0x19080808u, 0x08190808u,
    0x1908082bu, 0x08190808u, 0x19081919u, 0x08190808u, 0x19082b08u, 0x08190808u, 0x19190819u, 0x08190808u,
    0x19191908u, 0x08190808u, 0x192b0808u, 0x08190808u, 0x192b2b2bu, 0x08190808u, 0x2b080819u, 0x08190808u,
    0x2b081908u, 0x08190808u, 0x2b190808u, 0x08190808u, 0x08080808u, 0x08190819u, 0x0808082bu, 0x08190819u,
    0x08081919u, 0x08190819u, 0x08082b08u, 0x08190819u, 0x08190819u, 0x08190819u, 0x08191908u, 0x08190819u,
    0x082b0808u, 0x08190819u, 0x19080819u, 0x08190819u, 0x19081908u, 0x08190819u, 0x19190808u, 0x08190819u,
    0x2b080808u, 0x08190819u, 0x2b191908u, 0x08190819u, 0x2b19192bu, 0x08190819u, 0x08080819u, 0x0819082bu,
    0x08081908u, 0x0819082bu, 0x0808192bu, 0x0819082bu, 0x08190808u, 0x0819082bu, 0x19080808u, 0x0819082bu,
    0x192b0808u, 0x0819082bu, 0x08080808u, 0x08191908u, 0x0808082bu, 0x08191908u, 0x08081919u, 0x08191908u,
    0x08082b08u, 0x08191908u, 0x08190819u, 0x08191908u, 0x08191908u, 0x08191908u, 0x082b0808u, 0x08191908u,
    0x19080819u, 0x08191908u, 0x19081908u, 0x08191908u, 0x19082b19u, 0x08191908u, 0x19190808u, 0x08191908u,
    0x192b1908u, 0x08191908u, 0x2b080808u, 0x08191908u, 0x08080819u, 0x08191919u, 0x08081908u, 0x08191919u,
    0x08190808u, 0x08191919u, 0x19080808u, 0x08191919u, 0x08080808u, 0x0819192bu, 0x08191908u, 0x0819192bu,
    0x19082b19u, 0x0819192bu, 0x08080819u, 0x08192b08u, 0x08081908u, 0x08192b08u, 0x08190808u, 0x08192b08u,
    0x0819082bu, 0x08192b08u, 0x19080808u, 0x08192b08u, 0x19191908u, 0x08192b08u, 0x2b08192bu, 0x08192b08u,
    0x08080808u, 0x08192b19u, 0x08081919u, 0x08192b19u, 0x192b192bu, 0x08192b19u, 0x19190819u, 0x08192b2bu,
    0x2b2b2b19u, 0x08192b2bu, 0x08080808u, 0x082b0808u, 0x0808082bu, 0x082b0808u, 0x08081919u, 0x082b0808u,
    0x08082b08u, 0x082b0808u, 0x08082b2bu, 0x082b0808u, 0x08190819u, 0x082b0808u, 0x08191908u, 0x082b0808u,
    0x082b0808u, 0x082b0808u, 0x19080819u, 0x082b0808u, 0x19081908u, 0x082b0808u, 0x19190808u, 0x082b0808u,
    0x2b080808u, 0x082b0808u, 0x2b2b0808u, 0x082b0808u, 0x08080819u, 0x082b0819u, 0x08081908u, 0x082b0819u,
    0x08190808u, 0x082b0819u, 0x19080808u, 0x082b0819u, 0x19082b08u, 0x082b0819u, 0x192b1919u, 0x082b0819u,
    0x08080808u, 0x082b082bu, 0x082b082bu, 0x082b082bu, 0x2b080808u, 0x082b082bu, 0x2b2b2b08u, 0x082b082bu,
    0x08080819u, 0x082b1908u, 0x08081908u, 0x082b1908u, 0x08190808u, 0x082b1908u, 0x082b2b19u, 0x082b1908u,
    0x19080808u, 0x082b1908u, 0x08080808u, 0x082b1919u, 0x19080819u, 0x082b1919u, 0x1919082bu, 0x082b1919u,
    0x2b192b19u, 0x082b1919u, 0x08080819u, 0x082b192bu, 0x08192b2bu, 0x082b192bu, 0x2b2b192bu, 0x082b192bu,
    0x08080808u, 0x082b2b08u, 0x08082b08u, 0x082b2b08u, 0x08082b2bu, 0x082b2b08u, 0x082b0808u, 0x082b2b08u,
    0x19191919u, 0x082b2b08u, 0x2b082b08u, 0x082b2b08u, 0x2b2b082bu, 0x082b2b08u, 0x192b2b08u, 0x082b2b19u,
    0x2b190808u, 0x082b2b19u, 0x08082b08u, 0x082b2b2bu, 0x082b0808u, 0x082b2b2bu, 0x2b08082bu, 0x082b2b2bu,
    0x2b082b08u, 0x082b2b2bu, 0x2b082b2bu, 0x082b2b2bu, 0x08080819u, 0x19080808u, 0x08081908u, 0x19080808u,
    0x0808192bu, 0x19080808u, 0x08082b19u, 0x19080808u, 0x08190808u, 0x19080808u, 0x0819082bu, 0x19080808u,
    0x08191919u, 0x19080808u, 0x08192b08u, 0x19080808u, 0x082b0819u, 0x19080808u, 0x082b1908u, 0x19080808u,
    0x19080808u, 0x19080808u, 0x1908082bu, 0x19080808u, 0x19081919u, 0x19080808u, 0x19082b08u, 0x19080808u,
    0x19082b2bu, 0x19080808u, 0x19190819u, 0x19080808u, 0x19191908u, 0x19080808u, 0x192b0808u, 0x19080808u,
    0x192b1919u, 0x19080808u, 0x2b080819u, 0x19080808u, 0x2b081908u, 0x19080808u, 0x2b190808u, 0x19080808u,
    0x08080808u, 0x19080819u, 0x0808082bu, 0x19080819u, 0x08081919u, 0x19080819u, 0x08082b08u, 0x19080819u,
    0x08190819u, 0x19080819u, 0x08191908u, 0x19080819u, 0x082b0808u, 0x19080819u, 0x19080819u, 0x19080819u,
    0x19081908u, 0x19080819u, 0x19190808u, 0x19080819u, 0x2b080808u, 0x19080819u, 0x2b081919u, 0x19080819u,
    0x2b2b082bu, 0x19080819u, 0x08080819u, 0x1908082bu, 0x08081908u, 0x1908082bu, 0x08190808u, 0x1908082bu,
    0x0819082bu, 0x1908082bu, 0x082b2b19u, 0x1908082bu, 0x19080808u, 0x1908082bu, 0x08080808u, 0x19081908u,
    0x0808082bu, 0x19081908u, 0x08081919u, 0x19081908u, 0x08082b08u, 0x19081908u, 0x08190819u, 0x19081908u,
    0x08191908u, 0x19081908u, 0x08192b19u, 0x19081908u, 0x082b0808u, 0x19081908u, 0x19080819u, 0x19081908u,
    0x19081908u, 0x19081908u, 0x19190808u, 0x19081908u, 0x2b080808u, 0x19081908u, 0x2b191908u, 0x19081908u,
    0x08080819u, 0x19081919u, 0x08081908u, 0x19081919u, 0x08190808u, 0x19081919u, 0x082b1908u, 0x19081919u,
    0x19080808u, 0x19081919u, 0x2b192b2bu, 0x19081919u, 0x08080808u, 0x1908192bu, 0x08082b2bu, 0x1908192bu,
    0x19081908u, 0x1908192bu, 0x19190808u, 0x1908192bu, 0x08080819u, 0x19082b08u, 0x08081908u, 0x19082b08u,
    0x08190808u, 0x19082b08u, 0x19080808u, 0x19082b08u, 0x19081919u, 0x19082b08u, 0x19191908u, 0x19082b08u,
    0x192b082bu, 0x19082b08u, 0x08080808u, 0x19082b19u, 0x08190819u, 0x19082b19u, 0x19081908u, 0x19082b19u,
    0x19190808u, 0x19082b19u, 0x192b2b19u, 0x19082b19u, 0x08081908u, 0x19082b2bu, 0x08080808u, 0x19190808u,
    0x0808082bu, 0x19190808u, 0x08081919u, 0x19190808u, 0x08082b08u, 0x19190808u, 0x08190819u, 0x19190808u,
    0x08191908u, 0x19190808u, 0x082b0808u, 0x19190808u, 0x082b2b08u, 0x19190808u, 0x19080819u, 0x19190808u,
    0x19081908u, 0x19190808u, 0x19190808u, 0x19190808u, 0x2b080808u, 0x19190808u, 0x08080819u, 0x19190819u,
    0x08081908u, 0x19190819u, 0x08190808u, 0x19190819u, 0x08191919u, 0x19190819u, 0x19080808u, 0x19190819u,
    0x1908082bu, 0x19190819u, 0x08080808u, 0x1919082bu, 0x19081908u, 0x1919082bu, 0x2b2b2b2bu, 0x1919082bu,
    0x08080819u, 0x19191908u, 0x08081908u, 0x19191908u, 0x08190808u, 0x19191908u, 0x082b0819u, 0x19191908u,
    0x19080808u, 0x19191908u, 0x192b0808u, 0x19191908u, 0x2b080819u, 0x19191908u, 0x2b2b0819u, 0x19191908u,
    0x08080808u, 0x19191919u, 0x08082b08u, 0x19191919u, 0x2b080808u, 0x19191919u, 0x2b082b08u, 0x19191919u,
    0x082b0819u, 0x1919192bu, 0x192b2b08u, 0x1919192bu, 0x2b2b0819u, 0x1919192bu, 0x08080808u, 0x19192b08u,
    0x08191908u, 0x19192b08u, 0x19080819u, 0x19192b08u, 0x19190808u, 0x19192b08u, 0x2b192b19u, 0x19192b08u,
    0x08192b2bu, 0x19192b19u, 0x19080808u, 0x19192b19u, 0x1908082bu, 0x19192b19u, 0x2b081919u, 0x19192b2bu,
    0x08080819u, 0x192b0808u, 0x08081908u, 0x192b0808u, 0x08190808u, 0x192b0808u, 0x19080808u, 0x192b0808u,
    0x19191908u, 0x192b0808u, 0x192b082bu, 0x192b0808u, 0x2b08192bu, 0x192b0808u, 0x2b2b2b19u, 0x192b0808u,
    0x08080808u, 0x192b0819u, 0x082b1908u, 0x192b082bu, 0x19082b2bu, 0x192b082bu, 0x2b19082bu, 0x192b082bu,
    0x08080808u, 0x192b1908u, 0x0819192bu, 0x192b1908u, 0x08190808u, 0x192b1919u, 0x19080808u, 0x192b1919u,
    0x19081919u, 0x192b1919u, 0x2b2b1908u, 0x192b1919u, 0x08080819u, 0x192b2b08u, 0x192b2b2bu, 0x192b2b08u,
    0x082b1919u, 0x192b2b19u, 0x0808192bu, 0x192b2b2bu, 0x19191908u, 0x192b2b2bu, 0x192b082bu, 0x192b2b2bu,
    0x08080808u, 0x2b080808u, 0x0808082bu, 0x2b080808u, 0x08081919u, 0x2b080808u, 0x08082b08u, 0x2b080808u,
    0x08190819u, 0x2b080808u, 0x08191908u, 0x2b080808u, 0x082b0808u, 0x2b080808u, 0x082b2b2bu, 0x2b080808u,
    0x19080819u, 0x2b080808u, 0x19081908u, 0x2b080808u, 0x19190808u, 0x2b080808u, 0x2b080808u, 0x2b080808u,
    0x2b08082bu, 0x2b080808u, 0x2b2b2b08u, 0x2b080808u, 0x2b2b2b2bu, 0x2b080808u, 0x08080819u, 0x2b080819u,
    0x08081908u, 0x2b080819u, 0x0808192bu, 0x2b080819u, 0x08190808u, 0x2b080819u, 0x19080808u, 0x2b080819u,
    0x19190819u, 0x2b080819u, 0x19192b19u, 0x2b080819u, 0x08080808u, 0x2b08082bu, 0x082b0808u, 0x2b08082bu,
    0x2b080808u, 0x2b08082bu, 0x2b08082bu, 0x2b08082bu, 0x2b2b0808u, 0x2b08082bu, 0x2b2b2b08u, 0x2b08082bu,
    0x08080819u, 0x2b081908u, 0x08081908u, 0x2b081908u, 0x08190808u, 0x2b081908u, 0x0819082bu, 0x2b081908u,
    0x08191919u, 0x2b081908u, 0x19080808u, 0x2b081908u, 0x192b0808u, 0x2b081908u, 0x2b082b19u, 0x2b081908u,
    0x08080808u, 0x2b081919u, 0x19081908u, 0x2b081919u, 0x2b2b1919u, 0x2b081919u, 0x08192b08u, 0x2b08192bu,
    0x192b2b2bu, 0x2b08192bu, 0x08080808u, 0x2b082b08u, 0x08082b08u, 0x2b082b08u, 0x082b1919u, 0x2b082b08u,
    0x19192b2bu, 0x2b082b08u, 0x2b080808u, 0x2b082b08u, 0x2b08082bu, 0x2b082b08u, 0x2b2b2b08u, 0x2b082b08u,
    0x0808192bu, 0x2b082b19u, 0x082b082bu, 0x2b082b2bu, 0x2b080808u, 0x2b082b2bu, 0x2b082b08u, 0x2b082b2bu,
    0x2b19192bu, 0x2b082b2bu, 0x2b2b2b08u, 0x2b082b2bu, 0x08080819u, 0x2b190808u, 0x08081908u, 0x2b190808u,
    0x08190808u, 0x2b190808u, 0x19080808u, 0x2b190808u, 0x1919192bu, 0x2b190808u, 0x2b081908u, 0x2b190808u,
    0x08080808u, 0x2b190819u, 0x082b082bu, 0x2b190819u, 0x192b1908u, 0x2b190819u, 0x1919192bu, 0x2b19082bu,
    0x2b082b19u, 0x2b19082bu, 0x08080808u, 0x2b191908u, 0x08081919u, 0x2b191908u, 0x19081908u, 0x2b191908u,
    0x19190808u, 0x2b191908u, 0x19192b08u, 0x2b191908u, 0x082b2b19u, 0x2b191919u, 0x2b190808u, 0x2b191919u,
    0x2b19082bu, 0x2b191919u, 0x19080819u, 0x2b19192bu, 0x19190819u, 0x2b192b08u, 0x2b2b192bu, 0x2b192b08u,
    0x19082b19u, 0x2b192b19u, 0x08191919u, 0x2b192b2bu, 0x192b0808u, 0x2b192b2bu, 0x08080808u, 0x2b2b0808u,
    0x0808082bu, 0x2b2b0808u, 0x08082b08u, 0x2b2b0808u, 0x08082b2bu, 0x2b2b0808u, 0x082b0808u, 0x2b2b0808u,
    0x082b2b2bu, 0x2b2b0808u, 0x2b2b0808u, 0x2b2b0808u, 0x19190819u, 0x2b2b0819u, 0x19192b19u, 0x2b2b0819u,
    0x2b2b192bu, 0x2b2b0819u, 0x08080808u, 0x2b2b082bu, 0x0808082bu, 0x2b2b082bu, 0x08082b08u, 0x2b2b082bu,
    0x082b2b2bu, 0x2b2b082bu, 0x2b080808u, 0x2b2b082bu, 0x2b2b0808u, 0x2b2b082bu, 0x19080808u, 0x2b2b1908u,
    0x2b191919u, 0x2b2b1908u, 0x192b1919u, 0x2b2b192bu, 0x2b192b08u, 0x2b2b192bu, 0x08082b2bu, 0x2b2b2b08u,
    0x082b0808u, 0x2b2b2b08u, 0x082b082bu, 0x2b2b2b08u, 0x082b2b08u, 0x2b2b2b08u, 0x2b2b0808u, 0x2b2b2b08u,
    0x2b2b2b08u, 0x2b2b2b08u, 0x08081908u, 0x2b2b2b19u, 0x2b081908u, 0x2b2b2b19u, 0x2b08192bu, 0x2b2b2b19u,
    0x082b2b08u, 0x2b2b2b2bu, 0x082b2b2bu, 0x2b2b2b2bu, 0x2b190819u, 0x2b2b2b2bu, 0x2b2b2b2bu, 0x2b2b2b2bu,
    0x08080808u, 0x08080808u, 0x0808082bu, 0x08080808u, 0x08081919u, 0x08080808u, 0x08082b08u, 0x08080808u,
    0x08082b2bu, 0x08080808u, 0x08190819u, 0x08080808u, 0x08191908u, 0x08080808u, 0x0819192bu, 0x08080808u,
    0x08192b19u, 0x08080808u, 0x082b0808u, 0x08080808u, 0x082b082bu, 0x08080808u, 0x082b1919u, 0x08080808u,
    0x082b2b08u, 0x08080808u, 0x19080819u, 0x08080808u, 0x19081908u, 0x08080808u, 0x1908192bu, 0x08080808u,
    0x19082b19u, 0x08080808u, 0x19190808u, 0x08080808u, 0x1919082bu, 0x08080808u, 0x19191919u, 0x08080808u,
    0x19192b08u, 0x08080808u, 0x192b0819u, 0x08080808u, 0x192b1908u, 0x08080808u, 0x192b192bu, 0x08080808u,
    0x192b2b19u, 0x08080808u, 0x2b080808u, 0x08080808u, 0x2b08082bu, 0x08080808u, 0x2b081919u, 0x08080808u,
    0x2b082b08u, 0x08080808u, 0x2b190819u, 0x08080808u, 0x2b191908u, 0x08080808u, 0x2b2b0808u, 0x08080808u,
    0x2b2b1919u, 0x08080808u, 0x2b2b2b2bu, 0x08080808u, 0x08080819u, 0x08080819u, 0x08081908u, 0x08080819u,
    0x0808192bu, 0x08080819u, 0x08082b19u, 0x08080819u, 0x08190808u, 0x08080819u, 0x0819082bu, 0x08080819u,
    0x08191919u, 0x08080819u, 0x08192b08u, 0x08080819u, 0x082b0819u, 0x08080819u, 0x082b1908u, 0x08080819u,
    0x19080808u, 0x08080819u, 0x1908082bu, 0x08080819u, 0x19081919u, 0x08080819u, 0x19082b08u, 0x08080819u,
    0x19190819u, 0x08080819u, 0x19191908u, 0x08080819u, 0x1919192bu, 0x08080819u, 0x19192b19u, 0x08080819u,
    0x192b0808u, 0x08080819u, 0x192b1919u, 0x08080819u, 0x192b2b08u, 0x08080819u, 0x2b080819u, 0x08080819u,
    0x2b081908u, 0x08080819u, 0x2b190808u, 0x08080819u, 0x2b19082bu, 0x08080819u, 0x2b191919u, 0x08080819u,
    0x2b2b0819u, 0x08080819u, 0x2b2b1908u, 0x08080819u, 0x08080808u, 0x0808082bu, 0x0808082bu, 0x0808082bu,
    0x08081919u, 0x0808082bu, 0x08082b08u, 0x0808082bu, 0x08190819u, 0x0808082bu, 0x08191908u, 0x0808082bu,
    0x082b0808u, 0x0808082bu, 0x082b2b2bu, 0x0808082bu, 0x19080819u, 0x0808082bu, 0x19081908u, 0x0808082bu,
    0x1908192bu, 0x0808082bu, 0x19082b19u, 0x0808082bu, 0x19190808u, 0x0808082bu, 0x19191919u, 0x0808082bu,
    0x2b080808u, 0x0808082bu, 0x2b081919u, 0x0808082bu, 0x2b082b2bu, 0x0808082bu, 0x2b191908u, 0x0808082bu,
    0x2b2b082bu, 0x0808082bu, 0x08080819u, 0x08081908u, 0x08081908u, 0x08081908u, 0x0808192bu, 0x08081908u,
    0x08082b19u, 0x08081908u, 0x08190808u, 0x08081908u, 0x0819082bu, 0x08081908u, 0x08191919u, 0x08081908u,
    0x08192b08u, 0x08081908u, 0x082b0819u, 0x08081908u, 0x082b1908u, 0x08081908u, 0x082b192bu, 0x08081908u,
    0x082b2b19u, 0x08081908u, 0x19080808u, 0x08081908u, 0x1908082bu, 0x08081908u, 0x19081919u, 0x08081908u,
    0x19082b08u, 0x08081908u, 0x19082b2bu, 0x08081908u, 0x19190819u, 0x08081908u, 0x19191908u, 0x08081908u,
    0x1919192bu, 0x08081908u, 0x19192b19u, 0x08081908u, 0x192b0808u, 0x08081908u, 0x192b082bu, 0x08081908u,
    0x192b1919u, 0x08081908u, 0x2b080819u, 0x08081908u, 0x2b081908u, 0x08081908u, 0x2b08192bu, 0x08081908u,
    0x2b082b19u, 0x08081908u, 0x2b190808u, 0x08081908u, 0x2b191919u, 0x08081908u, 0x2b192b08u, 0x08081908u,
    0x2b2b0819u, 0x08081908u, 0x2b2b1908u, 0x08081908u, 0x08080808u, 0x08081919u, 0x0808082bu, 0x08081919u,
    0x08081919u, 0x08081919u, 0x08082b08u, 0x08081919u, 0x08082b2bu, 0x08081919u, 0x08190819u, 0x08081919u,
    0x08191908u, 0x08081919u, 0x0819192bu, 0x08081919u, 0x08192b19u, 0x08081919u, 0x082b0808u, 0x08081919u,
    0x082b1919u, 0x08081919u, 0x082b2b08u, 0x08081919u, 0x19080819u, 0x08081919u, 0x19081908u, 0x08081919u,
    0x1908192bu, 0x08081919u, 0x19082b19u, 0x08081919u, 0x19190808u, 0x08081919u, 0x1919082bu, 0x08081919u,
    0x19191919u, 0x08081919u, 0x19192b08u, 0x08081919u, 0x192b0819u, 0x08081919u, 0x192b1908u, 0x08081919u,
    0x2b080808u, 0x08081919u, 0x2b08082bu, 0x08081919u, 0x2b081919u, 0x08081919u, 0x2b082b08u, 0x08081919u,
    0x2b190819u, 0x08081919u, 0x2b191908u, 0x08081919u, 0x2b2b0808u, 0x08081919u, 0x08080819u, 0x0808192bu,
    0x08081908u, 0x0808192bu, 0x0808192bu, 0x0808192bu, 0x08082b19u, 0x0808192bu, 0x08190808u, 0x0808192bu,
    0x08191919u, 0x0808192bu, 0x19080808u, 0x0808192bu, 0x19081919u, 0x0808192bu, 0x19082b08u, 0x0808192bu,
    0x19190819u, 0x0808192bu, 0x19191908u, 0x0808192bu, 0x192b0808u, 0x0808192bu, 0x2b080819u, 0x0808192bu,
    0x2b081908u, 0x0808192bu, 0x2b190808u, 0x0808192bu, 0x08080808u, 0x08082b08u, 0x0808082bu, 0x08082b08u,
    0x08081919u, 0x08082b08u, 0x08082b08u, 0x08082b08u, 0x08190819u, 0x08082b08u, 0x08191908u, 0x08082b08u,
    0x0819192bu, 0x08082b08u, 0x08192b19u, 0x08082b08u, 0x082b0808u, 0x08082b08u, 0x082b1919u, 0x08082b08u,
    0x082b2b2bu, 0x08082b08u, 0x19080819u, 0x08082b08u, 0x19081908u, 0x08082b08u, 0x1908192bu, 0x08082b08u,
    0x19082b19u, 0x08082b08u, 0x19190808u, 0x08082b08u, 0x1919082bu, 0x08082b08u, 0x19191919u, 0x08082b08u,
    0x19192b08u, 0x08082b08u, 0x192b0819u, 0x08082b08u, 0x192b1908u, 0x08082b08u, 0x2b080808u, 0x08082b08u,
    0x2b081919u, 0x08082b08u, 0x2b191908u, 0x08082b08u, 0x2b2b2b2bu, 0x08082b08u, 0x08080819u, 0x08082b19u,
    0x08081908u, 0x08082b19u, 0x08190808u, 0x08082b19u, 0x0819082bu, 0x08082b19u, 0x08191919u, 0x08082b19u,
    0x08192b08u, 0x08082b19u, 0x082b0819u, 0x08082b19u, 0x19080808u, 0x08082b19u, 0x19081919u, 0x08082b19u,
    0x19082b08u, 0x08082b19u, 0x19190819u, 0x08082b19u, 0x19191908u, 0x08082b19u, 0x192b0808u, 0x08082b19u,
    0x2b080819u, 0x08082b19u, 0x2b190808u, 0x08082b19u, 0x08080808u, 0x08082b2bu, 0x08190819u, 0x08082b2bu,
    0x08191908u, 0x08082b2bu, 0x082b082bu, 0x08082b2bu, 0x082b2b08u, 0x08082b2bu, 0x082b2b2bu, 0x08082b2bu,
    0x19190808u, 0x08082b2bu, 0x2b192b19u, 0x08082b2bu, 0x08080819u, 0x08190808u, 0x08081908u, 0x08190808u,
    0x0808192bu, 0x08190808u, 0x08082b19u, 0x08190808u, 0x08190808u, 0x08190808u, 0x0819082bu, 0x08190808u,
    0x08191919u, 0x08190808u, 0x08192b08u, 0x08190808u, 0x082b0819u, 0x08190808u, 0x082b1908u, 0x08190808u,
    0x082b192bu, 0x08190808u, 0x19080808u, 0x08190808u, 0x1908082bu, 0x08190808u, 0x19081919u, 0x08190808u,
    0x19082b08u, 0x08190808u, 0x19190819u, 0x08190808u, 0x19191908u, 0x08190808u, 0x1919192bu, 0x08190808u,
    0x19192b19u, 0x08190808u, 0x192b0808u, 0x08190808u, 0x192b082bu, 0x08190808u, 0x192b1919u, 0x08190808u,
    0x192b2b08u, 0x08190808u, 0x2b080819u, 0x08190808u, 0x2b081908u, 0x08190808u, 0x2b08192bu, 0x08190808u,
    0x2b190808u, 0x08190808u, 0x2b191919u, 0x08190808u, 0x2b192b08u, 0x08190808u, 0x2b2b0819u, 0x08190808u,
    0x2b2b1908u, 0x08190808u, 0x08080808u, 0x08190819u, 0x0808082bu, 0x08190819u, 0x08081919u, 0x08190819u,
    0x08082b08u, 0x08190819u, 0x08082b2bu, 0x08190819u, 0x08190819u, 0x08190819u, 0x08191908u, 0x08190819u,
    0x0819192bu, 0x08190819u, 0x08192b19u, 0x08190819u, 0x082b0808u, 0x08190819u, 0x082b082bu, 0x08190819u,
    0x082b1919u, 0x08190819u, 0x082b2b08u, 0x08190819u, 0x19080819u, 0x08190819u, 0x19081908u, 0x08190819u,
    0x1908192bu, 0x08190819u, 0x19082b19u, 0x08190819u, 0x19190808u, 0x08190819u, 0x1919082bu, 0x08190819u,
    0x19191919u, 0x08190819u, 0x19192b08u, 0x08190819u, 0x192b0819u, 0x08190819u, 0x192b1908u, 0x08190819u,
    0x2b080808u, 0x08190819u, 0x2b08082bu, 0x08190819u, 0x2b081919u, 0x08190819u, 0x2b082b08u, 0x08190819u,
    0x2b190819u, 0x08190819u, 0x2b191908u, 0x08190819u, 0x08080819u, 0x0819082bu, 0x08081908u, 0x0819082bu,
    0x08082b19u, 0x0819082bu, 0x08190808u, 0x0819082bu, 0x08191919u, 0x0819082bu, 0x082b0819u, 0x0819082bu,
    0x082b1908u, 0x0819082bu, 0x19080808u, 0x0819082bu, 0x19081919u, 0x0819082bu, 0x19190819u, 0x0819082bu,
    0x19191908u, 0x0819082bu, 0x2b080819u, 0x0819082bu, 0x2b081908u, 0x0819082bu, 0x2b190808u, 0x0819082bu,
    0x08080808u, 0x08191908u, 0x0808082bu, 0x08191908u, 0x08081919u, 0x08191908u, 0x08082b08u, 0x08191908u,
    0x08190819u, 0x08191908u, 0x08191908u, 0x08191908u, 0x0819192bu, 0x08191908u, 0x08192b19u, 0x08191908u,
    0x082b0808u, 0x08191908u, 0x082b1919u, 0x08191908u, 0x082b2b08u, 0x08191908u, 0x19080819u, 0x08191908u,
    0x19081908u, 0x08191908u, 0x1908192bu, 0x08191908u, 0x19082b19u, 0x08191908u, 0x19190808u, 0x08191908u,
    0x1919082bu, 0x08191908u, 0x19191919u, 0x08191908u, 0x19192b08u, 0x08191908u, 0x192b0819u, 0x08191908u,
    0x192b1908u, 0x08191908u, 0x2b080808u, 0x08191908u, 0x2b08082bu, 0x08191908u, 0x2b081919u, 0x08191908u,
    0x2b082b08u, 0x08191908u, 0x2b190819u, 0x08191908u, 0x2b191908u, 0x08191908u, 0x2b2b0808u, 0x08191908u,
    0x08080819u, 0x08191919u, 0x08081908u, 0x08191919u, 0x0808192bu, 0x08191919u, 0x08082b19u, 0x08191919u,
    0x08190808u, 0x08191919u, 0x0819082bu, 0x08191919u, 0x08191919u, 0x08191919u, 0x08192b08u, 0x08191919u,
    0x082b0819u, 0x08191919u, 0x082b1908u, 0x08191919u, 0x19080808u, 0x08191919u, 0x1908082bu, 0x08191919u,
    0x19081919u, 0x08191919u, 0x19082b08u, 0x08191919u, 0x19190819u, 0x08191919u, 0x19191908u, 0x08191919u,
    0x192b0808u, 0x08191919u, 0x2b080819u, 0x08191919u, 0x2b081908u, 0x08191919u, 0x2b190808u, 0x08191919u,
    0x08080808u, 0x0819192bu, 0x08081919u, 0x0819192bu, 0x08082b08u, 0x0819192bu, 0x08190819u, 0x0819192bu,
    0x08191908u, 0x0819192bu, 0x082b0808u, 0x0819192bu, 0x19080819u, 0x0819192bu, 0x19081908u, 0x0819192bu,
    0x19190808u, 0x0819192bu, 0x2b080808u, 0x0819192bu, 0x2b2b2b2bu, 0x0819192bu, 0x08080819u, 0x08192b08u,
    0x08081908u, 0x08192b08u, 0x0808192bu, 0x08192b08u, 0x08082b19u, 0x08192b08u, 0x08190808u, 0x08192b08u,
    0x08191919u, 0x08192b08u, 0x08192b08u, 0x08192b08u, 0x082b0819u, 0x08192b08u, 0x19080808u, 0x08192b08u,
    0x1908082bu, 0x08192b08u, 0x19081919u, 0x08192b08u, 0x19082b08u, 0x08192b08u, 0x19190819u, 0x08192b08u,
    0x19191908u, 0x08192b08u, 0x192b0808u, 0x08192b08u, 0x2b080819u, 0x08192b08u, 0x2b081908u, 0x08192b08u,
    0x08080808u, 0x08192b19u, 0x0808082bu, 0x08192b19u, 0x08081919u, 0x08192b19u, 0x08082b08u, 0x08192b19u,
    0x08190819u, 0x08192b19u, 0x08191908u, 0x08192b19u, 0x082b0808u, 0x08192b19u, 0x19080819u, 0x08192b19u,
    0x19081908u, 0x08192b19u, 0x19190808u, 0x08192b19u, 0x192b2b19u, 0x08192b19u, 0x2b2b082bu, 0x08192b19u,
    0x08081908u, 0x08192b2bu, 0x08190808u, 0x08192b2bu, 0x19080808u, 0x08192b2bu, 0x1919192bu, 0x08192b2bu,
    0x08080808u, 0x082b0808u, 0x0808082bu, 0x082b0808u, 0x08081919u, 0x082b0808u, 0x08082b08u, 0x082b0808u,
    0x08190819u, 0x082b0808u, 0x08191908u, 0x082b0808u, 0x0819192bu, 0x082b0808u, 0x08192b19u, 0x082b0808u,
    0x082b0808u, 0x082b0808u, 0x082b1919u, 0x082b0808u, 0x082b2b2bu, 0x082b0808u, 0x19080819u, 0x082b0808u,
    0x19081908u, 0x082b0808u, 0x19190808u, 0x082b0808u, 0x1919082bu, 0x082b0808u, 0x19191919u, 0x082b0808u,
    0x192b1908u, 0x082b0808u, 0x2b080808u, 0x082b0808u, 0x2b082b2bu, 0x082b0808u, 0x2b191908u, 0x082b0808u,
    0x2b2b2b2bu, 0x082b0808u, 0x08080819u, 0x082b0819u, 0x08081908u, 0x082b0819u, 0x08190808u, 0x082b0819u,
    0x0819082bu, 0x082b0819u, 0x08191919u, 0x082b0819u, 0x082b0819u, 0x082b0819u, 0x19080808u, 0x082b0819u,
    0x1908082bu, 0x082b0819u, 0x19081919u, 0x082b0819u, 0x19190819u, 0x082b0819u, 0x19191908u, 0x082b0819u,
    0x192b0808u, 0x082b0819u, 0x2b080819u, 0x082b0819u, 0x2b081908u, 0x082b0819u, 0x2b190808u, 0x082b0819u,
    0x08080808u, 0x082b082bu, 0x08082b2bu, 0x082b082bu, 0x082b082bu, 0x082b082bu, 0x082b2b08u, 0x082b082bu,
    0x082b2b2bu, 0x082b082bu, 0x19081908u, 0x082b082bu, 0x19190808u, 0x082b082bu, 0x2b082b08u, 0x082b082bu,
    0x2b082b2bu, 0x082b082bu, 0x2b2b2b08u, 0x082b082bu, 0x08080819u, 0x082b1908u, 0x08081908u, 0x082b1908u,
    0x0808192bu, 0x082b1908u, 0x08082b19u, 0x082b1908u, 0x08190808u, 0x082b1908u, 0x08191919u, 0x082b1908u,
    0x08192b08u, 0x082b1908u, 0x082b0819u, 0x082b1908u, 0x082b1908u, 0x082b1908u, 0x19080808u, 0x082b1908u,
    0x1908082bu, 0x082b1908u, 0x19081919u, 0x082b1908u, 0x19082b08u, 0x082b1908u, 0x19190819u, 0x082b1908u,
    0x19191908u, 0x082b1908u, 0x192b0808u, 0x082b1908u, 0x2b080819u, 0x082b1908u, 0x2b081908u, 0x082b1908u,
    0x2b190808u, 0x082b1908u, 0x08080808u, 0x082b1919u, 0x08081919u, 0x082b1919u, 0x08082b08u, 0x082b1919u,
    0x08190819u, 0x082b1919u, 0x08191908u, 0x082b1919u, 0x082b0808u, 0x082b1919u, 0x19080819u, 0x082b1919u,
    0x19081908u, 0x082b1919u, 0x19190808u, 0x082b1919u, 0x192b192bu, 0x082b1919u, 0x2b080808u, 0x082b1919u,
    0x08080819u, 0x082b192bu, 0x08081908u, 0x082b192bu, 0x08190808u, 0x082b192bu, 0x19080808u, 0x082b192bu,
    0x19192b19u, 0x082b192bu, 0x08080808u, 0x082b2b08u, 0x08081919u, 0x082b2b08u, 0x08190819u, 0x082b2b08u,
    0x08191908u, 0x082b2b08u, 0x19080819u, 0x082b2b08u, 0x19081908u, 0x082b2b08u, 0x19190808u, 0x082b2b08u,
    0x2b082b2bu, 0x082b2b08u, 0x2b2b2b2bu, 0x082b2b08u, 0x08080819u, 0x082b2b19u, 0x08081908u, 0x082b2b19u,
    0x08190808u, 0x082b2b19u, 0x2b191919u, 0x082b2b19u, 0x08082b2bu, 0x082b2b2bu, 0x082b082bu, 0x082b2b2bu,
    0x192b1908u, 0x082b2b2bu, 0x2b082b08u, 0x082b2b2bu, 0x2b082b2bu, 0x082b2b2bu, 0x08080819u, 0x19080808u,
    0x08081908u, 0x19080808u, 0x0808192bu, 0x19080808u, 0x08082b19u, 0x19080808u, 0x08190808u, 0x19080808u,
    0x0819082bu, 0x19080808u, 0x08191919u, 0x19080808u, 0x08192b08u, 0x19080808u, 0x08192b2bu, 0x19080808u,
    0x082b0819u, 0x19080808u, 0x082b1908u, 0x19080808u, 0x082b192bu, 0x19080808u, 0x19080808u, 0x19080808u,
    0x1908082bu, 0x19080808u, 0x19081919u, 0x19080808u, 0x19082b08u, 0x19080808u, 0x19082b2bu, 0x19080808u,
    0x19190819u, 0x19080808u, 0x19191908u, 0x19080808u, 0x1919192bu, 0x19080808u, 0x19192b19u, 0x19080808u,
    0x192b0808u, 0x19080808u, 0x192b082bu, 0x19080808u, 0x192b1919u, 0x19080808u, 0x2b080819u, 0x19080808u,
    0x2b081908u, 0x19080808u, 0x2b190808u, 0x19080808u, 0x2b191919u, 0x19080808u, 0x2b192b08u, 0x19080808u,
    0x2b2b0819u, 0x19080808u, 0x2b2b1908u, 0x19080808u, 0x08080808u, 0x19080819u, 0x0808082bu, 0x19080819u,
    0x08081919u, 0x19080819u, 0x08082b08u, 0x19080819u, 0x08190819u, 0x19080819u, 0x08191908u, 0x19080819u,
    0x0819192bu, 0x19080819u, 0x08192b19u, 0x19080819u, 0x082b0808u, 0x19080819u, 0x082b082bu, 0x19080819u,
    0x082b1919u, 0x19080819u, 0x19080819u, 0x19080819u, 0x19081908u, 0x19080819u, 0x1908192bu, 0x19080819u,
    0x19082b19u, 0x19080819u, 0x19190808u, 0x19080819u, 0x1919082bu, 0x19080819u, 0x19191919u, 0x19080819u,
    0x19192b08u, 0x19080819u, 0x192b0819u, 0x19080819u, 0x192b1908u, 0x19080819u, 0x2b080808u, 0x19080819u,
    0x2b08082bu, 0x19080819u, 0x2b081919u, 0x19080819u, 0x2b082b08u, 0x19080819u, 0x2b190819u, 0x19080819u,
    0x2b191908u, 0x19080819u, 0x2b2b0808u, 0x19080819u, 0x08080819u, 0x1908082bu, 0x08081908u, 0x1908082bu,
    0x08190808u, 0x1908082bu, 0x0819082bu, 0x1908082bu, 0x08191919u, 0x1908082bu, 0x08192b08u, 0x1908082bu,
    0x082b1908u, 0x1908082bu, 0x19080808u, 0x1908082bu, 0x19081919u, 0x1908082bu, 0x19082b08u, 0x1908082bu,
    0x19190819u, 0x1908082bu, 0x19191908u, 0x1908082bu, 0x192b0808u, 0x1908082bu, 0x2b080819u, 0x1908082bu,
    0x2b081908u, 0x1908082bu, 0x08080808u, 0x19081908u, 0x0808082bu, 0x19081908u, 0x08081919u, 0x19081908u,
    0x08082b08u, 0x19081908u, 0x08082b2bu, 0x19081908u, 0x08190819u, 0x19081908u, 0x08191908u, 0x19081908u,
    0x0819192bu, 0x19081908u, 0x08192b19u, 0x19081908u, 0x082b0808u, 0x19081908u, 0x082b082bu, 0x19081908u,
    0x082b1919u, 0x19081908u, 0x082b2b08u, 0x19081908u, 0x19080819u, 0x19081908u, 0x19081908u, 0x19081908u,
    0x1908192bu, 0x19081908u, 0x19082b19u, 0x19081908u, 0x19190808u, 0x19081908u, 0x1919082bu, 0x19081908u,
    0x19191919u, 0x19081908u, 0x19192b08u, 0x19081908u, 0x192b0819u, 0x19081908u, 0x192b1908u, 0x19081908u,
    0x2b080808u, 0x19081908u, 0x2b08082bu, 0x19081908u, 0x2b081919u, 0x19081908u, 0x2b082b08u, 0x19081908u,
    0x2b190819u, 0x19081908u, 0x2b191908u, 0x19081908u, 0x2b2b0808u, 0x19081908u, 0x08080819u, 0x19081919u,
    0x08081908u, 0x19081919u, 0x0808192bu, 0x19081919u, 0x08082b19u, 0x19081919u, 0x08190808u, 0x19081919u,
    0x0819082bu, 0x19081919u, 0x08191919u, 0x19081919u, 0x08192b08u, 0x19081919u, 0x082b0819u, 0x19081919u,
    0x082b1908u, 0x19081919u, 0x19080808u, 0x19081919u, 0x1908082bu, 0x19081919u, 0x19081919u, 0x19081919u,
    0x19082b08u, 0x19081919u, 0x19190819u, 0x19081919u, 0x19191908u, 0x19081919u, 0x192b0808u, 0x19081919u,
    0x192b2b2bu, 0x19081919u, 0x2b080819u, 0x19081919u, 0x2b081908u, 0x19081919u, 0x2b190808u, 0x19081919u,
    0x08080808u, 0x1908192bu, 0x0808082bu, 0x1908192bu, 0x08081919u, 0x1908192bu, 0x08082b08u, 0x1908192bu,
    0x08190819u, 0x1908192bu, 0x08191908u, 0x1908192bu, 0x082b0808u, 0x1908192bu, 0x19080819u, 0x1908192bu,
    0x19081908u, 0x1908192bu, 0x19190808u, 0x1908192bu, 0x2b080808u, 0x1908192bu, 0x2b2b1919u, 0x1908192bu,
    0x08080819u, 0x19082b08u, 0x08081908u, 0x19082b08u, 0x08082b19u, 0x19082b08u, 0x08190808u, 0x19082b08u,
    0x0819082bu, 0x19082b08u, 0x08191919u, 0x19082b08u, 0x08192b08u, 0x19082b08u, 0x082b0819u, 0x19082b08u,
    0x082b1908u, 0x19082b08u, 0x19080808u, 0x19082b08u, 0x1908082bu, 0x19082b08u, 0x19081919u, 0x19082b08u,
    0x19082b08u, 0x19082b08u, 0x19190819u, 0x19082b08u, 0x19191908u, 0x19082b08u, 0x192b0808u, 0x19082b08u,
    0x2b081908u, 0x19082b08u, 0x2b190808u, 0x19082b08u, 0x08080808u, 0x19082b19u, 0x0808082bu, 0x19082b19u,
    0x08081919u, 0x19082b19u, 0x08082b08u, 0x19082b19u, 0x08190819u, 0x19082b19u, 0x08191908u, 0x19082b19u,
    0x082b0808u, 0x19082b19u, 0x19080819u, 0x19082b19u, 0x19081908u, 0x19082b19u, 0x19190808u, 0x19082b19u,
    0x2b080808u, 0x19082b19u, 0x2b19192bu, 0x19082b19u, 0x08080819u, 0x19082b2bu, 0x08081908u, 0x19082b2bu,
    0x08190808u, 0x19082b2bu, 0x19080808u, 0x19082b2bu, 0x08080808u, 0x19190808u, 0x0808082bu, 0x19190808u,
    0x08081919u, 0x19190808u, 0x08082b08u, 0x19190808u, 0x08190819u, 0x19190808u, 0x08191908u, 0x19190808u,
    0x0819192bu, 0x19190808u, 0x08192b19u, 0x19190808u, 0x082b0808u, 0x19190808u, 0x082b082bu, 0x19190808u,
    0x082b1919u, 0x19190808u, 0x082b2b08u, 0x19190808u, 0x19080819u, 0x19190808u, 0x19081908u, 0x19190808u,
    0x1908192bu, 0x19190808u, 0x19082b19u, 0x19190808u, 0x19190808u, 0x19190808u, 0x1919082bu, 0x19190808u,
    0x19191919u, 0x19190808u, 0x19192b08u, 0x19190808u, 0x192b0819u, 0x19190808u, 0x192b1908u, 0x19190808u,
    0x2b080808u, 0x19190808u, 0x2b08082bu, 0x19190808u, 0x2b081919u, 0x19190808u, 0x2b082b08u, 0x19190808u,
    0x2b190819u, 0x19190808u, 0x2b191908u, 0x19190808u, 0x08080819u, 0x19190819u, 0x08081908u, 0x19190819u,
    0x0808192bu, 0x19190819u, 0x08082b19u, 0x19190819u, 0x08190808u, 0x19190819u, 0x0819082bu, 0x19190819u,
    0x08191919u, 0x19190819u, 0x08192b08u, 0x19190819u, 0x082b0819u, 0x19190819u, 0x082b1908u, 0x19190819u,
    0x19080808u, 0x19190819u, 0x1908082bu, 0x19190819u, 0x19081919u, 0x19190819u, 0x19082b08u, 0x19190819u,
    0x19190819u, 0x19190819u, 0x19191908u, 0x19190819u, 0x192b0808u, 0x19190819u, 0x2b080819u, 0x19190819u,
    0x2b081908u, 0x19190819u, 0x2b190808u, 0x19190819u, 0x08080808u, 0x1919082bu, 0x08081919u, 0x1919082bu,
    0x08082b08u, 0x1919082bu, 0x08190819u, 0x1919082bu, 0x08191908u, 0x1919082bu, 0x082b0808u, 0x1919082bu,
    0x19080819u, 0x1919082bu, 0x19081908u, 0x1919082bu, 0x19190808u, 0x1919082bu, 0x192b2b19u, 0x1919082bu,
    0x2b080808u, 0x1919082bu, 0x08080819u, 0x19191908u, 0x08081908u, 0x19191908u, 0x0808192bu, 0x19191908u,
    0x08082b19u, 0x19191908u, 0x08190808u, 0x19191908u, 0x0819082bu, 0x19191908u, 0x08191919u, 0x19191908u,
    0x08192b08u, 0x19191908u, 0x082b0819u, 0x19191908u, 0x082b1908u, 0x19191908u, 0x19080808u, 0x19191908u,
    0x1908082bu, 0x19191908u, 0x19081919u, 0x19191908u, 0x19082b08u, 0x19191908u, 0x19190819u, 0x19191908u,
    0x19191908u, 0x19191908u, 0x192b0808u, 0x19191908u, 0x2b080819u, 0x19191908u, 0x2b081908u, 0x19191908u,
    0x2b190808u, 0x19191908u, 0x08080808u, 0x19191919u, 0x0808082bu, 0x19191919u, 0x08081919u, 0x19191919u,
    0x08082b08u, 0x19191919u, 0x08190819u, 0x19191919u, 0x08191908u, 0x19191919u, 0x082b0808u, 0x19191919u,
    0x19080819u, 0x19191919u, 0x19081908u, 0x19191919u, 0x19190808u, 0x19191919u, 0x2b080808u, 0x19191919u,
    0x08080819u, 0x1919192bu, 0x08081908u, 0x1919192bu, 0x08190808u, 0x1919192bu, 0x082b192bu, 0x1919192bu,
    0x19080808u, 0x1919192bu, 0x08080808u, 0x19192b08u, 0x0808082bu, 0x19192b08u, 0x08081919u, 0x19192b08u,
    0x08082b08u, 0x19192b08u, 0x08190819u, 0x19192b08u, 0x08191908u, 0x19192b08u, 0x082b0808u, 0x19192b08u,
    0x19080819u, 0x19192b08u, 0x19081908u, 0x19192b08u, 0x19190808u, 0x19192b08u, 0x19192b2bu, 0x19192b08u,
    0x2b080808u, 0x19192b08u, 0x08080819u, 0x19192b19u, 0x08081908u, 0x19192b19u, 0x08190808u, 0x19192b19u,
    0x19080808u, 0x19192b19u, 0x08080808u, 0x19192b2bu, 0x08192b19u, 0x19192b2bu, 0x2b081919u, 0x19192b2bu,
    0x2b2b2b08u, 0x19192b2bu, 0x08080819u, 0x192b0808u, 0x08081908u, 0x192b0808u, 0x0808192bu, 0x192b0808u,
    0x08190808u, 0x192b0808u, 0x0819082bu, 0x192b0808u, 0x08191919u, 0x192b0808u, 0x08192b08u, 0x192b0808u,
    0x082b0819u, 0x192b0808u, 0x082b1908u, 0x192b0808u, 0x19080808u, 0x192b0808u, 0x19081919u, 0x192b0808u,
    0x19082b08u, 0x192b0808u, 0x19190819u, 0x192b0808u, 0x19191908u, 0x192b0808u, 0x192b0808u, 0x192b0808u,
    0x2b081908u, 0x192b0808u, 0x2b190808u, 0x192b0808u, 0x08080808u, 0x192b0819u, 0x0808082bu, 0x192b0819u,
    0x08081919u, 0x192b0819u, 0x08082b08u, 0x192b0819u, 0x08190819u, 0x192b0819u, 0x08191908u, 0x192b0819u,
    0x082b0808u, 0x192b0819u, 0x19080819u, 0x192b0819u, 0x19081908u, 0x192b0819u, 0x19190808u, 0x192b0819u,
    0x2b080808u, 0x192b0819u, 0x2b192b19u, 0x192b0819u, 0x08081908u, 0x192b082bu, 0x08190808u, 0x192b082bu,
    0x19080808u, 0x192b082bu, 0x1919192bu, 0x192b082bu, 0x2b2b0819u, 0x192b082bu, 0x08080808u, 0x192b1908u,
    0x08081919u, 0x192b1908u, 0x08082b08u, 0x192b1908u, 0x08190819u, 0x192b1908u, 0x08191908u, 0x192b1908u,
    0x082b0808u, 0x192b1908u, 0x19080819u, 0x192b1908u, 0x19081908u, 0x192b1908u, 0x19190808u, 0x192b1908u,
    0x2b080808u, 0x192b1908u, 0x08080819u, 0x192b1919u, 0x08081908u, 0x192b1919u, 0x08190808u, 0x192b1919u,
    0x19080808u, 0x192b1919u, 0x19082b2bu, 0x192b1919u, 0x192b2b08u, 0x192b1919u, 0x2b19082bu, 0x192b1919u,
    0x08080808u, 0x192b192bu, 0x2b191908u, 0x192b192bu, 0x08080819u, 0x192b2b08u, 0x08081908u, 0x192b2b08u,
    0x08190808u, 0x192b2b08u, 0x192b1919u, 0x192b2b08u, 0x2b192b08u, 0x192b2b08u, 0x08080808u, 0x192b2b19u,
    0x082b2b2bu, 0x192b2b19u, 0x1908082bu, 0x192b2b2bu, 0x2b2b0819u, 0x192b2b2bu, 0x08080808u, 0x2b080808u,
    0x0808082bu, 0x2b080808u, 0x08081919u, 0x2b080808u, 0x08082b08u, 0x2b080808u, 0x08190819u, 0x2b080808u,
    0x08191908u, 0x2b080808u, 0x08192b19u, 0x2b080808u, 0x082b0808u, 0x2b080808u, 0x082b1919u, 0x2b080808u,
    0x19080819u, 0x2b080808u, 0x19081908u, 0x2b080808u, 0x19190808u, 0x2b080808u, 0x1919082bu, 0x2b080808u,
    0x19191919u, 0x2b080808u, 0x19192b08u, 0x2b080808u, 0x192b0819u, 0x2b080808u, 0x2b080808u, 0x2b080808u,
    0x2b081919u, 0x2b080808u, 0x2b190819u, 0x2b080808u, 0x2b191908u, 0x2b080808u, 0x08080819u, 0x2b080819u,
    0x08081908u, 0x2b080819u, 0x08082b19u, 0x2b080819u, 0x08190808u, 0x2b080819u, 0x0819082bu, 0x2b080819u,
    0x08191919u, 0x2b080819u, 0x08192b08u, 0x2b080819u, 0x082b0819u, 0x2b080819u, 0x082b1908u, 0x2b080819u,
    0x19080808u, 0x2b080819u, 0x1908082bu, 0x2b080819u, 0x19081919u, 0x2b080819u, 0x19082b08u, 0x2b080819u,
    0x19190819u, 0x2b080819u, 0x19191908u, 0x2b080819u, 0x2b080819u, 0x2b080819u, 0x2b081908u, 0x2b080819u,
    0x2b190808u, 0x2b080819u, 0x2b2b2b19u, 0x2b080819u, 0x08080808u, 0x2b08082bu, 0x08081919u, 0x2b08082bu,
    0x08082b2bu, 0x2b08082bu, 0x08190819u, 0x2b08082bu, 0x08191908u, 0x2b08082bu, 0x19080819u, 0x2b08082bu,
    0x19081908u, 0x2b08082bu, 0x19190808u, 0x2b08082bu, 0x08080819u, 0x2b081908u, 0x08081908u, 0x2b081908u,
    0x0808192bu, 0x2b081908u, 0x08082b19u, 0x2b081908u, 0x08190808u, 0x2b081908u, 0x0819082bu, 0x2b081908u,
    0x08191919u, 0x2b081908u, 0x08192b08u, 0x2b081908u, 0x082b0819u, 0x2b081908u, 0x19080808u, 0x2b081908u,
    0x1908082bu, 0x2b081908u, 0x19081919u, 0x2b081908u, 0x19082b08u, 0x2b081908u, 0x19190819u, 0x2b081908u,
    0x19191908u, 0x2b081908u, 0x192b0808u, 0x2b081908u, 0x2b080819u, 0x2b081908u, 0x2b081908u, 0x2b081908u,
    0x2b190808u, 0x2b081908u, 0x08080808u, 0x2b081919u, 0x0808082bu, 0x2b081919u, 0x08081919u, 0x2b081919u,
    0x08082b08u, 0x2b081919u, 0x08190819u, 0x2b081919u, 0x08191908u, 0x2b081919u, 0x082b0808u, 0x2b081919u,
    0x19080819u, 0x2b081919u, 0x19081908u, 0x2b081919u, 0x19190808u, 0x2b081919u, 0x2b080808u, 0x2b081919u,
    0x2b082b2bu, 0x2b081919u, 0x08080819u, 0x2b08192bu, 0x08081908u, 0x2b08192bu, 0x08190808u, 0x2b08192bu,
    0x082b2b19u, 0x2b08192bu, 0x19080808u, 0x2b08192bu, 0x08080808u, 0x2b082b08u, 0x08081919u, 0x2b082b08u,
    0x08190819u, 0x2b082b08u, 0x08191908u, 0x2b082b08u, 0x19080819u, 0x2b082b08u, 0x19081908u, 0x2b082b08u,
    0x19190808u, 0x2b082b08u, 0x2b2b082bu, 0x2b082b08u, 0x08080819u, 0x2b082b19u, 0x08081908u, 0x2b082b19u,
    0x19080808u, 0x2b082b19u, 0x192b1919u, 0x2b082b19u, 0x082b082bu, 0x2b082b2bu, 0x19192b08u, 0x2b082b2bu,
    0x19192b2bu, 0x2b082b2bu, 0x2b08082bu, 0x2b082b2bu, 0x2b2b082bu, 0x2b082b2bu, 0x08080819u, 0x2b190808u,
    0x08081908u, 0x2b190808u, 0x08082b19u, 0x2b190808u, 0x08190808u, 0x2b190808u, 0x0819082bu, 0x2b190808u,
    0x08191919u, 0x2b190808u, 0x08192b08u, 0x2b190808u, 0x082b1908u, 0x2b190808u, 0x19080808u, 0x2b190808u,
    0x1908082bu, 0x2b190808u, 0x19081919u, 0x2b190808u, 0x19082b08u, 0x2b190808u, 0x19190819u, 0x2b190808u,
    0x19191908u, 0x2b190808u, 0x192b0808u, 0x2b190808u, 0x2b080819u, 0x2b190808u, 0x2b081908u, 0x2b190808u,
    0x2b190808u, 0x2b190808u, 0x08080808u, 0x2b190819u, 0x08081919u, 0x2b190819u, 0x08190819u, 0x2b190819u,
    0x08191908u, 0x2b190819u, 0x19080819u, 0x2b190819u, 0x19081908u, 0x2b190819u, 0x19190808u, 0x2b190819u,
    0x19192b2bu, 0x2b190819u, 0x08080819u, 0x2b19082bu, 0x08081908u, 0x2b19082bu, 0x08190808u, 0x2b19082bu,
    0x19080808u, 0x2b19082bu, 0x2b2b192bu, 0x2b19082bu, 0x08080808u, 0x2b191908u, 0x0808082bu, 0x2b191908u,
    0x08081919u, 0x2b191908u, 0x08082b08u, 0x2b191908u, 0x08190819u, 0x2b191908u, 0x08191908u, 0x2b191908u,
    0x082b0808u, 0x2b191908u, 0x19080819u, 0x2b191908u, 0x19081908u, 0x2b191908u, 0x19190808u, 0x2b191908u,
    0x2b080808u, 0x2b191908u, 0x2b19192bu, 0x2b191908u, 0x08080819u, 0x2b191919u, 0x08081908u, 0x2b191919u,
    0x08190808u, 0x2b191919u, 0x19080808u, 0x2b191919u, 0x2b192b08u, 0x2b191919u, 0x2b2b0819u, 0x2b191919u,
    0x08080808u, 0x2b19192bu, 0x1908192bu, 0x2b19192bu, 0x192b1908u, 0x2b19192bu, 0x08080819u, 0x2b192b08u,
    0x08081908u, 0x2b192b08u, 0x08190808u, 0x2b192b08u, 0x082b192bu, 0x2b192b08u, 0x19080808u, 0x2b192b08u,
    0x2b2b2b19u, 0x2b192b08u, 0x08080808u, 0x2b192b19u, 0x19082b19u, 0x2b192b19u, 0x1919082bu, 0x2b192b19u,
    0x2b190808u, 0x2b192b2bu, 0x08080808u, 0x2b2b0808u, 0x08081919u, 0x2b2b0808u, 0x08082b2bu, 0x2b2b0808u,
    0x08191908u, 0x2b2b0808u, 0x082b082bu, 0x2b2b0808u, 0x082b2b2bu, 0x2b2b0808u, 0x19080819u, 0x2b2b0808u,
    0x19081908u, 0x2b2b0808u, 0x19190808u, 0x2b2b0808u, 0x2b2b082bu, 0x2b2b0808u, 0x2b2b2b2bu, 0x2b2b0808u,
    0x19080808u, 0x2b2b0819u, 0x192b1919u, 0x2b2b0819u, 0x0808082bu, 0x2b2b082bu, 0x08082b2bu, 0x2b2b082bu,
    0x082b082bu, 0x2b2b082bu, 0x082b2b08u, 0x2b2b082bu, 0x082b2b2bu, 0x2b2b082bu, 0x2b08082bu, 0x2b2b082bu,
    0x2b082b08u, 0x2b2b082bu, 0x2b082b2bu, 0x2b2b082bu, 0x2b2b2b08u, 0x2b2b082bu, 0x08080819u, 0x2b2b1908u,
    0x08081908u, 0x2b2b1908u, 0x08190808u, 0x2b2b1908u, 0x19080808u, 0x2b2b1908u, 0x2b082b19u, 0x2b2b1908u,
    0x2b2b1908u, 0x2b2b1908u, 0x08080808u, 0x2b2b1919u, 0x08192b19u, 0x2b2b1919u, 0x19190819u, 0x2b2b192bu,
    0x08082b2bu, 0x2b2b2b08u, 0x082b2b08u, 0x2b2b2b08u, 0x2b2b082bu, 0x2b2b2b08u, 0x19191908u, 0x2b2b2b19u,
    0x2b08192bu, 0x2b2b2b19u, 0x08082b08u, 0x2b2b2b2bu, 0x08082b2bu, 0x2b2b2b2bu, 0x082b0808u, 0x2b2b2b2bu,
    0x082b082bu, 0x2b2b2b2bu, 0x082b2b08u, 0x2b2b2b2bu, 0x2b082b08u, 0x2b2b2b2bu, 0x2b2b2b2bu, 0x2b2b2b2bu,
    0x04040404u, 0x04040414u, 0x04040424u, 0x04040c0cu, 0x04040c1cu, 0x04040c3eu, 0x04041404u, 0x04041414u,
    0x04041c0cu, 0x04042414u, 0x04043e1cu, 0x04043e2cu, 0x040c040cu, 0x040c041cu, 0x040c0c04u, 0x040c0c14u,
    0x040c140cu, 0x040c142cu, 0x040c1c04u, 0x040c1c14u, 0x040c240cu, 0x040c2c24u, 0x040c3e04u, 0x04140404u,
    0x04140414u, 0x04140424u, 0x04140c0cu, 0x04141404u, 0x04141414u, 0x04141c0cu, 0x04141c1cu, 0x04141c3eu,
    0x04142c0cu, 0x04142c3eu, 0x04143e2cu, 0x041c040cu, 0x041c043eu, 0x041c0c04u, 0x041c0c14u, 0x041c142cu,
    0x041c3e04u, 0x04240c1cu, 0x04241c3eu, 0x04242424u, 0x04242c3eu, 0x04243e1cu, 0x04243e2cu, 0x042c040cu,
    0x042c043eu, 0x042c1c14u, 0x042c2c14u, 0x04341c2cu, 0x04343424u, 0x043e0c04u, 0x043e0c24u, 0x043e0c34u,
    0x043e241cu, 0x043e340cu, 0x0c04040cu, 0x0c04041cu, 0x0c040c04u, 0x0c040c14u, 0x0c04140cu, 0x0c04141cu,
    0x0c041c04u, 0x0c041c14u, 0x0c041c24u, 0x0c04243eu, 0x0c042c04u, 0x0c0c0404u, 0x0c0c0414u, 0x0c0c0c0cu,
    0x0c0c1404u, 0x0c0c1414u, 0x0c14040cu, 0x0c14041cu, 0x0c140c04u, 0x0c140c14u, 0x0c14140cu, 0x0c141c04u,
    0x0c143e14u, 0x0c1c0404u, 0x0c1c0414u, 0x0c1c1404u, 0x0c1c1c0cu, 0x0c1c2434u, 0x0c1c3434u, 0x0c24040cu,
    0x0c24042cu, 0x0c242c04u, 0x0c2c1404u, 0x0c2c1424u, 0x0c2c2434u, 0x0c2c3e0cu, 0x0c34042cu, 0x0c3e1414u,
    0x0c3e2404u, 0x14040404u, 0x14040414u, 0x14040c0cu, 0x14040c1cu, 0x14041404u, 0x14041414u, 0x14041434u,
    0x14041c0cu, 0x14042414u, 0x140c040cu, 0x140c041cu, 0x140c042cu, 0x140c0c04u, 0x140c0c14u, 0x140c140cu,
    0x140c1c04u, 0x140c341cu, 0x140c343eu, 0x140c3e04u, 0x14140404u, 0x14140414u, 0x14140c0cu, 0x14140c3eu,
    0x14141404u, 0x14141414u, 0x14141c3eu, 0x14142404u, 0x14142c2cu, 0x141c040cu, 0x141c0c04u, 0x141c0c24u,
    0x141c3e04u, 0x141c3e24u, 0x14241c2cu, 0x14242c1cu, 0x142c041cu, 0x142c143eu, 0x142c240cu, 0x142c3e24u,
    0x143e040cu, 0x143e041cu, 0x143e0c34u, 0x143e242cu, 0x1c04040cu, 0x1c040c04u, 0x1c040c14u, 0x1c04140cu,
    0x1c04141cu, 0x1c042c04u, 0x1c04342cu, 0x1c043e14u, 0x1c0c0404u, 0x1c0c0414u, 0x1c0c1404u, 0x1c0c1c0cu,
    0x1c0c2424u, 0x1c0c2434u, 0x1c14040cu, 0x1c14041cu, 0x1c140c04u, 0x1c14142cu, 0x1c142c14u, 0x1c143e14u,
    0x1c1c0c0cu, 0x1c1c1c1cu, 0x1c241c04u, 0x1c24243eu, 0x1c243e14u, 0x1c2c0404u, 0x1c2c0434u, 0x1c2c1414u,
    0x1c2c2c2cu, 0x1c340c24u, 0x1c341c34u, 0x1c34341cu, 0x1c3e1c1cu, 0x1c3e3404u, 0x24040424u, 0x24040c3eu,
    0x24041c2cu, 0x24041c3eu, 0x24042c1cu, 0x24042c3eu, 0x240c3e24u, 0x24141404u, 0x24141c3eu, 0x24142404u,
    0x24143404u, 0x24143434u, 0x241c043eu, 0x241c242cu, 0x24240424u, 0x24242c0cu, 0x24243424u, 0x242c142cu,
    0x242c241cu, 0x242c3e04u, 0x243e042cu, 0x243e0c04u, 0x243e0c14u, 0x243e1c04u, 0x2c040c14u, 0x2c04240cu,
    0x2c043e04u, 0x2c0c0404u, 0x2c0c0434u, 0x2c0c1434u, 0x2c0c2c2cu, 0x2c140c24u, 0x2c141c14u, 0x2c143e14u,
    0x2c1c0414u, 0x2c1c2c1cu, 0x2c240c04u, 0x2c24141cu, 0x2c24143eu, 0x2c243e14u, 0x2c2c0414u, 0x2c2c1c0cu,
    0x2c342c04u, 0x2c3e1424u, 0x2c3e2414u, 0x34041424u, 0x34042424u, 0x34042434u, 0x34043424u, 0x340c140cu,
    0x340c340cu, 0x34140c3eu, 0x34143424u, 0x341c1c04u, 0x341c1c34u, 0x34242424u, 0x342c042cu, 0x342c2c14u,
    0x34341c1cu, 0x343e041cu, 0x343e140cu, 0x3e04041cu, 0x3e04042cu, 0x3e04043eu, 0x3e040c04u, 0x3e041c14u,
    0x3e042c14u, 0x3e0c1434u, 0x3e0c2404u, 0x3e140c14u, 0x3e14242cu, 0x3e142c14u, 0x3e1c0404u, 0x3e1c0c2cu,
    0x3e1c1c1cu, 0x3e1c3404u, 0x3e24140cu, 0x3e24240cu, 0x3e2c0404u, 0x3e2c0414u, 0x3e2c1424u, 0x3e341c04u,
    0x01010101u, 0x01010103u, 0x01010105u, 0x0101010bu, 0x0101010fu, 0x01010301u, 0x01010303u, 0x01010305u,
    0x01010309u, 0x0101030du, 0x01010501u, 0x01010503u, 0x0101050bu, 0x01010707u, 0x01010901u, 0x01010905u,
    0x0101090bu, 0x0101090fu, 0x01010b03u, 0x01010b07u, 0x01010d01u, 0x01010d05u, 0x01010f03u, 0x01010f09u,
    0x01010f0fu, 0x01030101u, 0x01030103u, 0x01030105u, 0x01030109u, 0x01030301u, 0x01030303u, 0x0103030bu,
    0x01030501u, 0x01030507u, 0x0103050fu, 0x01030703u, 0x0103070bu, 0x01030909u, 0x01030d03u, 0x01030d0bu,
    0x01030f05u, 0x01050101u, 0x01050103u, 0x0105010bu, 0x0105010fu, 0x01050301u, 0x01050307u, 0x0105030du,
    0x01050503u, 0x0105050bu, 0x01050701u, 0x01050709u, 0x01050905u, 0x0105090bu, 0x0105090fu, 0x01050b03u,
    0x01050b07u, 0x01050f01u, 0x01050f07u, 0x01070107u, 0x01070303u, 0x0107030bu, 0x01070501u, 0x01070505u,
    0x01070703u, 0x01070707u, 0x0107070du, 0x01070909u, 0x01070b01u, 0x01070b05u, 0x01070d0fu, 0x01070f03u,
    0x01070f0bu, 0x01090101u, 0x01090307u, 0x0109030fu, 0x01090503u, 0x01090509u, 0x01090705u, 0x01090901u,
    0x01090907u, 0x01090b03u, 0x01090f01u, 0x010b0105u, 0x010b0109u, 0x010b0501u, 0x010b0505u, 0x010b050du,
    0x010b0707u, 0x010b0903u, 0x010b090bu, 0x010b090fu, 0x010b0d0du, 0x010b0f07u, 0x010d010du, 0x010d0303u,
    0x010d0307u, 0x010d0703u, 0x010d0b05u, 0x010d0f03u, 0x010f0101u, 0x010f0105u, 0x010f0109u, 0x010f0501u,
    0x010f0505u, 0x010f050du, 0x010f0707u, 0x010f0b01u, 0x010f0b09u, 0x03010101u, 0x03010103u, 0x03010105u,
    0x03010109u, 0x03010301u, 0x03010303u, 0x03010307u, 0x0301030bu, 0x0301030fu, 0x03010501u, 0x03010505u,
    0x03010703u, 0x03010709u, 0x0301070du, 0x03010b09u, 0x03010b0du, 0x03010d03u, 0x03010f05u, 0x03030101u,
    0x03030103u, 0x03030107u, 0x0303010du, 0x03030301u, 0x03030309u, 0x03030503u, 0x03030701u, 0x03030707u,
    0x03030903u, 0x03030b01u, 0x03030b05u, 0x03030f01u, 0x03030f0du, 0x03050101u, 0x03050305u, 0x0305030bu,
    0x0305030fu, 0x03050501u, 0x03050509u, 0x03050705u, 0x03050901u, 0x03050907u, 0x03050b0bu, 0x03050d01u,
    0x03050f05u, 0x03070103u, 0x03070109u, 0x0307010fu, 0x03070301u, 0x03070307u, 0x03070503u, 0x0307050fu,
    0x03070701u, 0x03070709u, 0x03070903u, 0x03070d05u, 0x03070f01u, 0x03090107u, 0x0309010bu, 0x03090305u,
    0x03090309u, 0x03090703u, 0x03090707u, 0x03090905u, 0x0309090du, 0x03090b01u, 0x03090b09u, 0x030b0103u,
    0x030b0301u, 0x030b0307u, 0x030b0503u, 0x030b0701u, 0x030b0705u, 0x030b0b03u, 0x030d0501u, 0x030d0509u,
    0x030d050fu, 0x030d0909u, 0x030d090du, 0x030f0103u, 0x030f0107u, 0x030f0301u, 0x030f0305u, 0x030f0503u,
    0x030f070bu, 0x030f0903u, 0x030f0d05u, 0x030f0f01u, 0x05010101u, 0x05010103u, 0x05010107u, 0x0501010bu,
    0x0501010fu, 0x05010301u, 0x05010305u, 0x05010309u, 0x0501030du, 0x05010503u, 0x05010507u, 0x0501050fu,
    0x05010701u, 0x05010705u, 0x05010903u, 0x05010907u, 0x0501090bu, 0x05010b01u, 0x05010b05u, 0x05010d0fu,
    0x05010f01u, 0x05010f07u, 0x05010f0bu, 0x05030101u, 0x05030105u, 0x05030301u, 0x05030307u, 0x0503030fu,
    0x05030505u, 0x0503050bu, 0x05030703u, 0x05030709u, 0x05030905u, 0x05030b03u, 0x05050103u, 0x05050109u,
    0x0505010fu, 0x05050503u, 0x05050507u, 0x05050701u, 0x0505070fu, 0x05050903u, 0x05050b07u, 0x05050b0fu,
    0x05050f03u, 0x05050f09u, 0x05070101u, 0x05070105u, 0x0507010bu, 0x05070303u, 0x05070505u, 0x05070509u,
    0x05070703u, 0x05070707u, 0x05070905u, 0x05070b01u, 0x05070d0du, 0x05090103u, 0x0509010fu, 0x05090501u,
    0x05090507u, 0x05090705u, 0x0509070bu, 0x05090903u, 0x05090f05u, 0x05090f0bu, 0x050b0109u, 0x050b0303u,
    0x050b0505u, 0x050b070fu, 0x050b0901u, 0x050b0b07u, 0x050b0f01u, 0x050d0101u, 0x050d0105u, 0x050d010fu,
    0x050d0503u, 0x050d0b0bu, 0x050d0d03u, 0x050f010bu, 0x050f0303u, 0x050f050du, 0x050f0701u, 0x050f0907u,
    0x050f0b01u, 0x07010105u, 0x07010303u, 0x07010307u, 0x0701030bu, 0x0701030fu, 0x07010505u, 0x07010703u,
    0x07010707u, 0x0701070bu, 0x07010905u, 0x07010909u, 0x0701090fu, 0x07010b03u, 0x07010d07u, 0x07010f03u,
    0x07030103u, 0x07030107u, 0x0703010bu, 0x07030309u, 0x07030503u, 0x07030507u, 0x07030901u, 0x07030d01u,
    0x07030f05u, 0x07030f0du, 0x07050101u, 0x07050305u, 0x07050501u, 0x07050705u, 0x07050709u, 0x07050b01u,
    0x07070103u, 0x07070301u, 0x07070309u, 0x07070503u, 0x07070507u, 0x0707050fu, 0x07070701u, 0x07070903u,
    0x07070907u, 0x0707090fu, 0x07070b0bu, 0x07070f07u, 0x07090107u, 0x07090303u, 0x0709030du, 0x07090505u,
    0x07090703u, 0x07090b05u, 0x07090d01u, 0x07090d09u, 0x070b0103u, 0x070b0301u, 0x070b0305u, 0x070b050bu,
    0x070b0705u, 0x070b0909u, 0x070b0b0du, 0x070b0f07u, 0x070d030du, 0x070d0903u, 0x070f0103u, 0x070f0107u,
    0x070f0501u, 0x070f0505u, 0x070f070bu, 0x09010101u, 0x09010109u, 0x09010305u, 0x09010501u, 0x09010509u,
    0x0901050fu, 0x09010705u, 0x09010903u, 0x09010b01u, 0x09010f01u, 0x09030105u, 0x0903010fu, 0x09030303u,
    0x09030307u, 0x09030505u, 0x09030701u, 0x0903070bu, 0x09030907u, 0x09030b03u, 0x09030b0bu, 0x09050103u,
    0x09050107u, 0x09050301u, 0x0905030bu, 0x09050503u, 0x09050707u, 0x09050901u, 0x09050b0fu, 0x09050d05u,
    0x09050f01u, 0x09070109u, 0x09070303u, 0x09070307u, 0x09070501u, 0x09070505u, 0x09070703u, 0x0907070bu,
    0x09090101u, 0x09090105u, 0x09090509u, 0x0909070fu, 0x09090901u, 0x09090f03u, 0x090b010bu, 0x090b010fu,
    0x090b0503u, 0x090b0d05u, 0x090d0307u, 0x090d0709u, 0x090d0d01u, 0x090f0301u, 0x090f030bu, 0x090f0701u,
    0x090f0907u, 0x090f0b03u, 0x0b010105u, 0x0b010301u, 0x0b010309u, 0x0b010505u, 0x0b010901u, 0x0b010909u,
    0x0b01090fu, 0x0b010b05u, 0x0b010d0du, 0x0b010f09u, 0x0b030103u, 0x0b030107u, 0x0b03010bu, 0x0b030305u,
    0x0b030503u, 0x0b030705u, 0x0b030f05u, 0x0b050101u, 0x0b050303u, 0x0b050507u, 0x0b050701u, 0x0b05070du,
    0x0b050b07u, 0x0b070105u, 0x0b07010fu, 0x0b070301u, 0x0b07050fu, 0x0b070909u, 0x0b070b03u, 0x0b070d0bu,
    0x0b070f07u, 0x0b090103u, 0x0b090109u, 0x0b090501u, 0x0b090705u, 0x0b09090du, 0x0b0b0305u, 0x0b0b050du,
    0x0b0b0b03u, 0x0b0b0b07u, 0x0b0d0905u, 0x0b0f0105u, 0x0b0f0109u, 0x0b0f0505u, 0x0d010303u, 0x0d010307u,
    0x0d01030bu, 0x0d010703u, 0x0d010707u, 0x0d010d01u, 0x0d030101u, 0x0d030501u, 0x0d03050fu, 0x0d030d09u,
    0x0d050305u, 0x0d050709u, 0x0d050905u, 0x0d050b0bu, 0x0d050d05u, 0x0d050f01u, 0x0d070101u, 0x0d070309u,
    0x0d070503u, 0x0d070901u, 0x0d09050bu, 0x0d090907u, 0x0d090d05u, 0x0d0b0101u, 0x0d0b0107u, 0x0d0b0709u,
    0x0d0b0d01u, 0x0d0d010bu, 0x0d0d0901u, 0x0d0f0303u, 0x0d0f0307u, 0x0f010101u, 0x0f010109u, 0x0f01010fu,
    0x0f010501u, 0x0f010505u, 0x0f01070du, 0x0f010901u, 0x0f010b09u, 0x0f010d05u, 0x0f030105u, 0x0f030303u,
    0x0f030509u, 0x0f030907u, 0x0f03090bu, 0x0f050103u, 0x0f050109u, 0x0f050301u, 0x0f05030du, 0x0f050503u,
    0x0f050701u, 0x0f050b03u, 0x0f070105u, 0x0f070705u, 0x0f07070bu, 0x0f070b07u, 0x0f090103u, 0x0f09010bu,
    0x0f090307u, 0x0f090501u, 0x0f090b01u, 0x0f0b0505u, 0x0f0b0905u, 0x0f0d0105u, 0x0f0d0703u, 0x0f0f0101u,
    0x03828100u, 0x87060584u, 0x8b0a0988u, 0x0f8e8d0cu, 0x93121190u, 0x17969514u, 0x1b9a9918u, 0x9f1e1d9cu,
    0xa32221a0u, 0x27a6a524u, 0x2baaa928u, 0xaf2e2dacu, 0x33b2b130u, 0xb73635b4u, 0xbb3a39b8u, 0x3fbebd3cu,
    0xc34241c0u, 0x47c6c544u, 0x4bcac948u, 0xcf4e4dccu, 0x53d2d150u, 0xd75655d4u, 0xdb5a59d8u, 0x5fdedd5cu,
    0x63e2e160u, 0xe76665e4u, 0xeb6a69e8u, 0x6feeed6cu, 0xf37271f0u, 0x77f6f574u, 0x7bfaf978u, 0xff7e7dfcu,
};

static const int8_t g_kvalues_iq4nl[16] = { -127, -104, -83, -65, -49, -35, -22, -10, 1, 13, 25, 38, 53, 69, 89, 113 };

const uint32_t *ink_ggml_grid_table(size_t *out_u32_count) {
    if (out_u32_count) *out_u32_count = INK_GRID_TOTAL_U32;
    return g_ggml_grids;
}

static float ggml_fp16_to_f32(uint16_t h) {
    uint32_t s = (uint32_t)(h >> 15) << 31;
    uint32_t e = (h >> 10) & 0x1fu;
    uint32_t m = h & 0x3ffu;
    uint32_t bits;
    if (e == 0) {
        if (m == 0) bits = s;
        else {
            float v = ldexpf((float)m, -24);
            return s ? -v : v;
        }
    } else if (e == 31) {
        bits = s | 0x7f800000u | (m << 13);
    } else {
        bits = s | ((e + 112u) << 23) | (m << 13);
    }
    float out;
    memcpy(&out, &bits, 4);
    return out;
}

static float ggml_bf16_to_f32(uint16_t b) {
    uint32_t bits = (uint32_t)b << 16;
    float out;
    memcpy(&out, &bits, 4);
    return out;
}

static uint16_t rd_le16(const uint8_t *p) { return (uint16_t)p[0] | ((uint16_t)p[1] << 8); }
static uint32_t rd_le32(const uint8_t *p) {
    return (uint32_t)p[0] | ((uint32_t)p[1] << 8) | ((uint32_t)p[2] << 16) | ((uint32_t)p[3] << 24);
}

static void get_scale_min_k4_ref(int j, const uint8_t *q, uint8_t *d, uint8_t *m) {
    if (j < 4) {
        *d = q[j] & 63u;
        *m = q[j + 4] & 63u;
    } else {
        *d = (uint8_t)((q[j + 4] & 0x0fu) | ((q[j - 4] >> 6) << 4));
        *m = (uint8_t)((q[j + 4] >> 4)    | ((q[j]     >> 6) << 4));
    }
}

int ink_ggml_dequant_row(uint32_t type, const uint8_t *src, float *dst, int64_t n) {
    int be = ink_ggml_block_elems(type);
    int bb = ink_ggml_block_bytes(type);
    if (be <= 0 || bb <= 0 || n < 0 || (n % be) != 0) return 1;
    int64_t nb = n / be;
    const uint8_t *ksigns = (const uint8_t *)(g_ggml_grids + INK_GRID_OFF_KSIGNS);

    switch (type) {
        case INK_GGML_F32:
            memcpy(dst, src, (size_t)n * sizeof(float));
            return 0;
        case INK_GGML_F16:
            for (int64_t i = 0; i < n; i++) dst[i] = ggml_fp16_to_f32(rd_le16(src + i * 2));
            return 0;
        case INK_GGML_BF16:
            for (int64_t i = 0; i < n; i++) dst[i] = ggml_bf16_to_f32(rd_le16(src + i * 2));
            return 0;
        case INK_GGML_Q8_0:
            for (int64_t i = 0; i < nb; i++) {
                const uint8_t *b = src + i * 34;
                float d = ggml_fp16_to_f32(rd_le16(b));
                for (int j = 0; j < 32; j++) dst[i * 32 + j] = (float)((int8_t)b[2 + j]) * d;
            }
            return 0;
        case INK_GGML_Q4_0:
            for (int64_t i = 0; i < nb; i++) {
                const uint8_t *b = src + i * 18;
                float d = ggml_fp16_to_f32(rd_le16(b));
                for (int j = 0; j < 16; j++) {
                    dst[i * 32 + j]      = (float)((int)(b[2 + j] & 0x0fu) - 8) * d;
                    dst[i * 32 + j + 16] = (float)((int)(b[2 + j] >> 4)    - 8) * d;
                }
            }
            return 0;
        case INK_GGML_Q4_1:
            for (int64_t i = 0; i < nb; i++) {
                const uint8_t *b = src + i * 20;
                float d = ggml_fp16_to_f32(rd_le16(b));
                float m = ggml_fp16_to_f32(rd_le16(b + 2));
                for (int j = 0; j < 16; j++) {
                    dst[i * 32 + j]      = (float)(b[4 + j] & 0x0fu) * d + m;
                    dst[i * 32 + j + 16] = (float)(b[4 + j] >> 4)    * d + m;
                }
            }
            return 0;
        case INK_GGML_Q5_0:
            for (int64_t i = 0; i < nb; i++) {
                const uint8_t *b = src + i * 22;
                float d = ggml_fp16_to_f32(rd_le16(b));
                uint32_t qh = rd_le32(b + 2);
                const uint8_t *qs = b + 6;
                for (int j = 0; j < 16; j++) {
                    uint8_t xh0 = (uint8_t)(((qh >> j) << 4) & 0x10u);
                    uint8_t xh1 = (uint8_t)((qh >> (j + 12)) & 0x10u);
                    dst[i * 32 + j]      = (float)((int)((qs[j] & 0x0fu) | xh0) - 16) * d;
                    dst[i * 32 + j + 16] = (float)((int)((qs[j] >> 4)    | xh1) - 16) * d;
                }
            }
            return 0;
        case INK_GGML_Q5_1:
            for (int64_t i = 0; i < nb; i++) {
                const uint8_t *b = src + i * 24;
                float d = ggml_fp16_to_f32(rd_le16(b));
                float m = ggml_fp16_to_f32(rd_le16(b + 2));
                uint32_t qh = rd_le32(b + 4);
                const uint8_t *qs = b + 8;
                for (int j = 0; j < 16; j++) {
                    uint8_t xh0 = (uint8_t)(((qh >> j) << 4) & 0x10u);
                    uint8_t xh1 = (uint8_t)((qh >> (j + 12)) & 0x10u);
                    dst[i * 32 + j]      = (float)((qs[j] & 0x0fu) | xh0) * d + m;
                    dst[i * 32 + j + 16] = (float)((qs[j] >> 4)    | xh1) * d + m;
                }
            }
            return 0;
        case INK_GGML_IQ4_NL:
            for (int64_t i = 0; i < nb; i++) {
                const uint8_t *b = src + i * 18;
                float d = ggml_fp16_to_f32(rd_le16(b));
                for (int j = 0; j < 16; j++) {
                    dst[i * 32 + j]      = (float)g_kvalues_iq4nl[b[2 + j] & 0x0fu] * d;
                    dst[i * 32 + j + 16] = (float)g_kvalues_iq4nl[b[2 + j] >> 4]    * d;
                }
            }
            return 0;
        case INK_GGML_Q2_0:
            for (int64_t i = 0; i < nb; i++) {
                const uint8_t *b = src + i * 18;
                float d = ggml_fp16_to_f32(rd_le16(b));
                const uint8_t *qs = b + 2;
                for (int j = 0; j < 64; j++) {
                    int q = (int)((qs[j >> 2] >> ((j & 3) * 2)) & 3u);
                    dst[i * 64 + j] = (float)(q - 1) * d;
                }
            }
            return 0;
        case INK_GGML_Q2_K:
            for (int64_t i = 0; i < nb; i++) {
                const uint8_t *b = src + i * 84;
                const uint8_t *scales = b;
                const uint8_t *q = b + 16;
                float d = ggml_fp16_to_f32(rd_le16(b + 80));
                float dmin = ggml_fp16_to_f32(rd_le16(b + 82));
                float *y = dst + i * 256;
                int is = 0;
                for (int half = 0; half < 2; half++) {
                    for (int j = 0; j < 4; j++) {
                        int shift = j * 2;
                        uint8_t sc = scales[is++];
                        float dl = d * (float)(sc & 0x0fu), ml = dmin * (float)(sc >> 4);
                        for (int l = 0; l < 16; l++) *y++ = dl * (float)((q[l] >> shift) & 3u) - ml;
                        sc = scales[is++];
                        dl = d * (float)(sc & 0x0fu); ml = dmin * (float)(sc >> 4);
                        for (int l = 0; l < 16; l++) *y++ = dl * (float)((q[l + 16] >> shift) & 3u) - ml;
                    }
                    q += 32;
                }
            }
            return 0;
        case INK_GGML_Q3_K:
            for (int64_t i = 0; i < nb; i++) {
                const uint8_t *b = src + i * 110;
                const uint8_t *hm = b;
                const uint8_t *q = b + 32;
                uint32_t aux[4];
                aux[0] = rd_le32(b + 96);
                aux[1] = rd_le32(b + 100);
                aux[2] = rd_le32(b + 104);
                uint32_t tmp = aux[2];
                const uint32_t km1 = 0x03030303u, km2 = 0x0f0f0f0fu;
                aux[2] = ((aux[0] >> 4) & km2) | (((tmp >> 4) & km1) << 4);
                aux[3] = ((aux[1] >> 4) & km2) | (((tmp >> 6) & km1) << 4);
                aux[0] = (aux[0] & km2) | (((tmp >> 0) & km1) << 4);
                aux[1] = (aux[1] & km2) | (((tmp >> 2) & km1) << 4);
                const int8_t *sc = (const int8_t *)aux;
                float d_all = ggml_fp16_to_f32(rd_le16(b + 108));
                float *y = dst + i * 256;
                uint8_t m = 1;
                int is = 0;
                for (int half = 0; half < 2; half++) {
                    for (int j = 0; j < 4; j++) {
                        int shift = j * 2;
                        float dl = d_all * (float)((int)sc[is++] - 32);
                        for (int l = 0; l < 16; l++) {
                            int qv = (int)((q[l] >> shift) & 3u) - ((hm[l] & m) ? 0 : 4);
                            *y++ = dl * (float)qv;
                        }
                        dl = d_all * (float)((int)sc[is++] - 32);
                        for (int l = 0; l < 16; l++) {
                            int qv = (int)((q[l + 16] >> shift) & 3u) - ((hm[l + 16] & m) ? 0 : 4);
                            *y++ = dl * (float)qv;
                        }
                        m <<= 1;
                    }
                    q += 32;
                }
            }
            return 0;
        case INK_GGML_Q4_K:
            for (int64_t i = 0; i < nb; i++) {
                const uint8_t *b = src + i * 144;
                float d = ggml_fp16_to_f32(rd_le16(b));
                float dmin = ggml_fp16_to_f32(rd_le16(b + 2));
                const uint8_t *scales = b + 4;
                const uint8_t *q = b + 16;
                float *y = dst + i * 256;
                for (int j = 0; j < 4; j++) {
                    uint8_t sc1, m1b, sc2, m2b;
                    get_scale_min_k4_ref(j * 2 + 0, scales, &sc1, &m1b);
                    get_scale_min_k4_ref(j * 2 + 1, scales, &sc2, &m2b);
                    float d1 = d * (float)sc1, m1 = dmin * (float)m1b;
                    float d2 = d * (float)sc2, m2 = dmin * (float)m2b;
                    for (int l = 0; l < 32; l++) *y++ = d1 * (float)(q[l] & 0x0fu) - m1;
                    for (int l = 0; l < 32; l++) *y++ = d2 * (float)(q[l] >> 4)    - m2;
                    q += 32;
                }
            }
            return 0;
        case INK_GGML_Q5_K:
            for (int64_t i = 0; i < nb; i++) {
                const uint8_t *b = src + i * 176;
                float d = ggml_fp16_to_f32(rd_le16(b));
                float dmin = ggml_fp16_to_f32(rd_le16(b + 2));
                const uint8_t *scales = b + 4;
                const uint8_t *qh = b + 16;
                const uint8_t *ql = b + 48;
                float *y = dst + i * 256;
                uint8_t u1 = 1, u2 = 2;
                for (int j = 0; j < 4; j++) {
                    uint8_t sc1, m1b, sc2, m2b;
                    get_scale_min_k4_ref(j * 2 + 0, scales, &sc1, &m1b);
                    get_scale_min_k4_ref(j * 2 + 1, scales, &sc2, &m2b);
                    float d1 = d * (float)sc1, m1 = dmin * (float)m1b;
                    float d2 = d * (float)sc2, m2 = dmin * (float)m2b;
                    for (int l = 0; l < 32; l++) *y++ = d1 * (float)((ql[l] & 0x0fu) + ((qh[l] & u1) ? 16 : 0)) - m1;
                    for (int l = 0; l < 32; l++) *y++ = d2 * (float)((ql[l] >> 4)    + ((qh[l] & u2) ? 16 : 0)) - m2;
                    ql += 32;
                    u1 <<= 2; u2 <<= 2;
                }
            }
            return 0;
        case INK_GGML_Q6_K:
            for (int64_t i = 0; i < nb; i++) {
                const uint8_t *b = src + i * 210;
                const uint8_t *ql = b;
                const uint8_t *qh = b + 128;
                const int8_t *sc = (const int8_t *)(b + 192);
                float d = ggml_fp16_to_f32(rd_le16(b + 208));
                float *y = dst + i * 256;
                for (int half = 0; half < 2; half++) {
                    for (int l = 0; l < 32; l++) {
                        int is = l / 16;
                        int q1 = (int)((ql[l]      & 0x0fu) | (((qh[l] >> 0) & 3u) << 4)) - 32;
                        int q2 = (int)((ql[l + 32] & 0x0fu) | (((qh[l] >> 2) & 3u) << 4)) - 32;
                        int q3 = (int)((ql[l]      >> 4)    | (((qh[l] >> 4) & 3u) << 4)) - 32;
                        int q4 = (int)((ql[l + 32] >> 4)    | (((qh[l] >> 6) & 3u) << 4)) - 32;
                        y[l +  0] = d * (float)sc[is + 0] * (float)q1;
                        y[l + 32] = d * (float)sc[is + 2] * (float)q2;
                        y[l + 64] = d * (float)sc[is + 4] * (float)q3;
                        y[l + 96] = d * (float)sc[is + 6] * (float)q4;
                    }
                    y += 128; ql += 64; qh += 32; sc += 8;
                }
            }
            return 0;
        case INK_GGML_IQ2_XXS:
            for (int64_t i = 0; i < nb; i++) {
                const uint8_t *b = src + i * 66;
                float d = ggml_fp16_to_f32(rd_le16(b));
                float *y = dst + i * 256;
                for (int ib32 = 0; ib32 < 8; ib32++) {
                    uint32_t a0 = rd_le32(b + 2 + ib32 * 8);
                    uint32_t a1 = rd_le32(b + 2 + ib32 * 8 + 4);
                    float db = d * (0.5f + (float)(a1 >> 28)) * 0.25f;
                    for (int l = 0; l < 4; l++) {
                        uint32_t gidx = (a0 >> (l * 8)) & 0xffu;
                        uint8_t signs = ksigns[(a1 >> (7 * l)) & 127u];
                        uint32_t lo = g_ggml_grids[INK_GRID_OFF_IQ2XXS + gidx * 2];
                        uint32_t hi = g_ggml_grids[INK_GRID_OFF_IQ2XXS + gidx * 2 + 1];
                        for (int j = 0; j < 4; j++) {
                            float gv0 = (float)((lo >> (j * 8)) & 0xffu);
                            float gv1 = (float)((hi >> (j * 8)) & 0xffu);
                            y[j]     = db * gv0 * ((signs & (1u << j))       ? -1.0f : 1.0f);
                            y[j + 4] = db * gv1 * ((signs & (1u << (j + 4))) ? -1.0f : 1.0f);
                        }
                        y += 8;
                    }
                }
            }
            return 0;
        case INK_GGML_IQ2_XS:
            for (int64_t i = 0; i < nb; i++) {
                const uint8_t *b = src + i * 74;
                float d = ggml_fp16_to_f32(rd_le16(b));
                const uint8_t *scales = b + 66;
                float *y = dst + i * 256;
                for (int ib32 = 0; ib32 < 8; ib32++) {
                    float db0 = d * (0.5f + (float)(scales[ib32] & 0x0fu)) * 0.25f;
                    float db1 = d * (0.5f + (float)(scales[ib32] >> 4))    * 0.25f;
                    for (int l = 0; l < 4; l++) {
                        uint16_t qw = rd_le16(b + 2 + (ib32 * 4 + l) * 2);
                        uint32_t gidx = qw & 511u;
                        uint8_t signs = ksigns[qw >> 9];
                        float db = (l < 2) ? db0 : db1;
                        uint32_t lo = g_ggml_grids[INK_GRID_OFF_IQ2XS + gidx * 2];
                        uint32_t hi = g_ggml_grids[INK_GRID_OFF_IQ2XS + gidx * 2 + 1];
                        for (int j = 0; j < 4; j++) {
                            float gv0 = (float)((lo >> (j * 8)) & 0xffu);
                            float gv1 = (float)((hi >> (j * 8)) & 0xffu);
                            y[j]     = db * gv0 * ((signs & (1u << j))       ? -1.0f : 1.0f);
                            y[j + 4] = db * gv1 * ((signs & (1u << (j + 4))) ? -1.0f : 1.0f);
                        }
                        y += 8;
                    }
                }
            }
            return 0;
        case INK_GGML_IQ2_S:
            for (int64_t i = 0; i < nb; i++) {
                const uint8_t *b = src + i * 82;
                float d = ggml_fp16_to_f32(rd_le16(b));
                const uint8_t *qs = b + 2;
                const uint8_t *signs_p = b + 34;
                const uint8_t *qh = b + 66;
                const uint8_t *scales = b + 74;
                float *y = dst + i * 256;
                for (int ib32 = 0; ib32 < 8; ib32++) {
                    float db0 = d * (0.5f + (float)(scales[ib32] & 0x0fu)) * 0.25f;
                    float db1 = d * (0.5f + (float)(scales[ib32] >> 4))    * 0.25f;
                    for (int l = 0; l < 4; l++) {
                        uint32_t gidx = (uint32_t)qs[l] | (((uint32_t)qh[ib32] << (8 - 2 * l)) & 0x300u);
                        uint8_t signs = signs_p[l];
                        float db = (l < 2) ? db0 : db1;
                        uint32_t lo = g_ggml_grids[INK_GRID_OFF_IQ2S + gidx * 2];
                        uint32_t hi = g_ggml_grids[INK_GRID_OFF_IQ2S + gidx * 2 + 1];
                        for (int j = 0; j < 4; j++) {
                            float gv0 = (float)((lo >> (j * 8)) & 0xffu);
                            float gv1 = (float)((hi >> (j * 8)) & 0xffu);
                            y[j]     = db * gv0 * ((signs & (1u << j))       ? -1.0f : 1.0f);
                            y[j + 4] = db * gv1 * ((signs & (1u << (j + 4))) ? -1.0f : 1.0f);
                        }
                        y += 8;
                    }
                    qs += 4;
                    signs_p += 4;
                }
            }
            return 0;
        case INK_GGML_IQ3_XXS:
            for (int64_t i = 0; i < nb; i++) {
                const uint8_t *b = src + i * 98;
                float d = ggml_fp16_to_f32(rd_le16(b));
                const uint8_t *qs = b + 2;
                const uint8_t *sas = b + 66;
                float *y = dst + i * 256;
                for (int ib32 = 0; ib32 < 8; ib32++) {
                    uint32_t aux32 = rd_le32(sas + ib32 * 4);
                    float db = d * (0.5f + (float)(aux32 >> 28)) * 0.5f;
                    for (int l = 0; l < 4; l++) {
                        uint8_t signs = ksigns[(aux32 >> (7 * l)) & 127u];
                        uint32_t g1 = g_ggml_grids[INK_GRID_OFF_IQ3XXS + qs[2 * l]];
                        uint32_t g2 = g_ggml_grids[INK_GRID_OFF_IQ3XXS + qs[2 * l + 1]];
                        for (int j = 0; j < 4; j++) {
                            y[j]     = db * (float)((g1 >> (j * 8)) & 0xffu) * ((signs & (1u << j))       ? -1.0f : 1.0f);
                            y[j + 4] = db * (float)((g2 >> (j * 8)) & 0xffu) * ((signs & (1u << (j + 4))) ? -1.0f : 1.0f);
                        }
                        y += 8;
                    }
                    qs += 8;
                }
            }
            return 0;
        case INK_GGML_IQ3_S:
            for (int64_t i = 0; i < nb; i++) {
                const uint8_t *b = src + i * 110;
                float d = ggml_fp16_to_f32(rd_le16(b));
                const uint8_t *qs = b + 2;
                const uint8_t *qh = b + 66;
                const uint8_t *signs_p = b + 74;
                const uint8_t *scales = b + 106;
                float *y = dst + i * 256;
                for (int ib32 = 0; ib32 < 8; ib32++) {
                    int s_nib = (ib32 & 1) ? (scales[ib32 >> 1] >> 4) : (scales[ib32 >> 1] & 0x0fu);
                    float db = d * (float)(1 + 2 * s_nib);
                    uint8_t qhb = qh[ib32];
                    for (int l = 0; l < 4; l++) {
                        uint32_t idx1 = (uint32_t)qs[2 * l]     | (((uint32_t)qhb << (8 - 2 * l)) & 256u);
                        uint32_t idx2 = (uint32_t)qs[2 * l + 1] | (((uint32_t)qhb << (7 - 2 * l)) & 256u);
                        uint8_t signs = signs_p[l];
                        uint32_t g1 = g_ggml_grids[INK_GRID_OFF_IQ3S + idx1];
                        uint32_t g2 = g_ggml_grids[INK_GRID_OFF_IQ3S + idx2];
                        for (int j = 0; j < 4; j++) {
                            y[j]     = db * (float)((g1 >> (j * 8)) & 0xffu) * ((signs & (1u << j))       ? -1.0f : 1.0f);
                            y[j + 4] = db * (float)((g2 >> (j * 8)) & 0xffu) * ((signs & (1u << (j + 4))) ? -1.0f : 1.0f);
                        }
                        y += 8;
                    }
                    qs += 8;
                    signs_p += 4;
                }
            }
            return 0;
        case INK_GGML_IQ4_XS:
            for (int64_t i = 0; i < nb; i++) {
                const uint8_t *b = src + i * 136;
                float d = ggml_fp16_to_f32(rd_le16(b));
                uint16_t sh = rd_le16(b + 2);
                const uint8_t *sl = b + 4;
                const uint8_t *qs = b + 8;
                float *y = dst + i * 256;
                for (int ib = 0; ib < 8; ib++) {
                    int ls = (int)(((sl[ib >> 1] >> (4 * (ib & 1))) & 0x0fu) | (((sh >> (2 * ib)) & 3u) << 4));
                    float dl = d * (float)(ls - 32);
                    for (int j = 0; j < 16; j++) {
                        y[j]      = dl * (float)g_kvalues_iq4nl[qs[j] & 0x0fu];
                        y[j + 16] = dl * (float)g_kvalues_iq4nl[qs[j] >> 4];
                    }
                    y += 32;
                    qs += 16;
                }
            }
            return 0;
        default:
            return 1;
    }
}

/* ---- GGUF LoRA & direction-ablation adapter loader ---- */

static void bind_lora_tensor(InkLora *l, const char *name, uint64_t nlen, const InkTensor *t) {
    char buf[128];
    if (nlen >= sizeof buf) return;
    memcpy(buf, name, nlen);
    buf[nlen] = '\0';

    if (!strcmp(buf, "token_embd.weight.lora_a")) { l->tok_embd_a = *t; return; }
    if (!strcmp(buf, "token_embd.weight.lora_b")) { l->tok_embd_b = *t; return; }
    if (strncmp(buf, "blk.", 4) != 0) return;
    char *dot = strchr(buf + 4, '.');
    if (!dot) return;
    int il = atoi(buf + 4);
    if (il < 0 || il >= 32) return;
    const char *s = dot + 1;
    if (!strcmp(s, "attn_output.weight.lora_a"))        l->layers[il].attn_output_a = *t;
    else if (!strcmp(s, "attn_output.weight.lora_b"))   l->layers[il].attn_output_b = *t;
    else if (!strcmp(s, "ffn_down_exps.weight.lora_a")) l->layers[il].ffn_down_exps_a = *t;
    else if (!strcmp(s, "ffn_down_exps.weight.lora_b")) l->layers[il].ffn_down_exps_b = *t;
}

int ink_lora_open(const char *path, InkLora *l, char *err, size_t err_bytes) {
    memset(l, 0, sizeof *l);
    l->alpha = 1.0f;
    if (ink_map_file(path, &l->map, &l->file_size, &l->map_size, err, err_bytes) != 0) return 1;

    Reader r = { l->map, l->map + l->file_size };
    uint32_t magic = 0, ver = 0;
    uint64_t n_tensors = 0, n_kv = 0;
    if (rd_u32(&r, &magic) || magic != GGUF_MAGIC || rd_u32(&r, &ver) || (ver != 2 && ver != 3) ||
        rd_u64(&r, &n_tensors) || rd_u64(&r, &n_kv)) {
        snprintf(err, err_bytes, "invalid LoRA GGUF header in %s", path);
        ink_lora_close(l); return 1;
    }

    uint64_t alignment = 32;
    int arch_ok = 0, type_ok = 0;
    for (uint64_t i = 0; i < n_kv; i++) {
        const char *k; uint64_t kl; uint32_t vt;
        if (rd_str_view(&r, &k, &kl) || rd_u32(&r, &vt)) {
            snprintf(err, err_bytes, "truncated LoRA GGUF KV");
            ink_lora_close(l); return 1;
        }
        if (key_eq(k, kl, "general.alignment")) {
            int64_t a = 32; if (rd_scalar_i64(&r, vt, &a)) goto fail_lora;
            if (a > 0) alignment = (uint64_t)a;
        } else if (key_eq(k, kl, "general.architecture")) {
            if (vt != GT_STRING) goto fail_lora;
            const char *v; uint64_t vl; if (rd_str_view(&r, &v, &vl)) goto fail_lora;
            arch_ok = key_eq(v, vl, "inkling2_loop");
        } else if (key_eq(k, kl, "adapter.type")) {
            if (vt != GT_STRING) goto fail_lora;
            const char *v; uint64_t vl; if (rd_str_view(&r, &v, &vl)) goto fail_lora;
            type_ok = key_eq(v, vl, "lora");
        } else if (key_eq(k, kl, "adapter.lora_alpha")) {
            float a = 1.0f; if (rd_scalar_f32(&r, vt, &a)) goto fail_lora;
            l->alpha = a;
        } else {
            if (skip_val(&r, vt)) goto fail_lora;
        }
    }
    if (!arch_ok || !type_ok) {
        snprintf(err, err_bytes, "LoRA %s is not an inkling2_loop LoRA adapter", path);
        ink_lora_close(l); return 1;
    }

    typedef struct { const char *name; uint64_t nlen; InkTensor t; } RawLT;
    RawLT *raw = calloc((size_t)n_tensors, sizeof(RawLT));
    if (!raw) { snprintf(err, err_bytes, "oom LoRA tensor table"); ink_lora_close(l); return 1; }
    for (uint64_t i = 0; i < n_tensors; i++) {
        const char *name; uint64_t nlen; uint32_t nd = 0, tt = 0; uint64_t off = 0;
        if (rd_str_view(&r, &name, &nlen) || rd_u32(&r, &nd) || nd > 4) {
            free(raw); snprintf(err, err_bytes, "bad LoRA tensor header"); ink_lora_close(l); return 1;
        }
        InkTensor t; memset(&t, 0, sizeof t);
        t.ndims = nd; t.present = 1;
        for (uint32_t d = 0; d < nd; d++) if (rd_u64(&r, &t.dims[d])) { free(raw); ink_lora_close(l); return 1; }
        if (rd_u32(&r, &tt) || rd_u64(&r, &off)) { free(raw); ink_lora_close(l); return 1; }
        t.type = tt; t.offset = off; t.nbytes = tensor_nbytes(tt, nd, t.dims);
        raw[i].name = name; raw[i].nlen = nlen; raw[i].t = t;
    }
    uint64_t pos = (uint64_t)(r.p - l->map);
    l->data_offset = (pos + alignment - 1) & ~(alignment - 1);
    for (uint64_t i = 0; i < n_tensors; i++) {
        raw[i].t.offset += l->data_offset;
        bind_lora_tensor(l, raw[i].name, raw[i].nlen, &raw[i].t);
    }
    free(raw);

    /* Compute 64-bit fingerprint over file size + header + sampled bytes for cache isolation. */
    uint64_t fp = 14695981039346656037ULL ^ (uint64_t)l->file_size;
    fp *= 1099511628211ULL;
    size_t head_n = l->file_size < 65536 ? l->file_size : 65536;
    for (size_t i = 0; i < head_n; i++) { fp ^= l->map[i]; fp *= 1099511628211ULL; }
    for (size_t i = head_n; i < l->file_size; i += 4096) { fp ^= l->map[i]; fp *= 1099511628211ULL; }
    l->fp = fp ? fp : 1ULL;
    return 0;

fail_lora:
    snprintf(err, err_bytes, "failed parsing LoRA GGUF KV in %s", path);
    ink_lora_close(l);
    return 1;
}

void ink_lora_close(InkLora *l) {
    if (!l) return;
    if (l->map && l->map_size) ink_unmap_file(l->map, l->map_size);
    memset(l, 0, sizeof *l);
}

static int lora_read_vec(const InkLora *l, const InkTensor *t, int64_t elem_off, float *dst, int64_t n) {
    if (!t->present || t->offset >= l->file_size) return 1;
    int bb = ink_ggml_block_bytes(t->type);
    int be = ink_ggml_block_elems(t->type);
    if (bb <= 0 || be <= 0 || (elem_off % be) != 0 || (n % be) != 0) return 1;
    uint64_t byte_off = t->offset + (uint64_t)(elem_off / be) * (uint64_t)bb;
    uint64_t byte_len = (uint64_t)(n / be) * (uint64_t)bb;
    if (byte_off + byte_len > l->file_size) return 1;
    return ink_ggml_dequant_row(t->type, l->map + byte_off, dst, n);
}

int ink_lora_extract_ablate(const InkLora *l, int n_phys, int n_embd, int n_expert,
                            float *out_dirs, int *out_has_emb, uint8_t *out_has_layer,
                            char *err, size_t err_bytes) {
    if (!l || !l->map || !out_dirs || n_phys <= 0 || n_phys > 32 || n_embd <= 0) {
        snprintf(err, err_bytes, "invalid arguments to ink_lora_extract_ablate");
        return 1;
    }
    memset(out_dirs, 0, (size_t)(1 + n_phys) * (size_t)n_embd * sizeof(float));
    if (out_has_emb) *out_has_emb = 0;
    if (out_has_layer) memset(out_has_layer, 0, (size_t)n_phys);

    float *tmp = (float *)malloc((size_t)n_embd * sizeof(float));
    if (!tmp) { snprintf(err, err_bytes, "oom ablate extract"); return 1; }

    int any = 0;
    /* 1. Optional token_embd direction vector (rank-1: dims = [1, n_embd] or [n_embd, 1]) */
    if (l->tok_embd_b.present) {
        uint64_t ne = 1;
        for (uint32_t d = 0; d < l->tok_embd_b.ndims; d++) ne *= l->tok_embd_b.dims[d];
        if ((int64_t)ne != n_embd || lora_read_vec(l, &l->tok_embd_b, 0, out_dirs, n_embd) != 0) {
            free(tmp);
            snprintf(err, err_bytes, "LoRA token_embd.weight.lora_b has unexpected shape (expected %d)", n_embd);
            return 1;
        }
        double n2 = 0.0;
        for (int j = 0; j < n_embd; j++) n2 += (double)out_dirs[j] * (double)out_dirs[j];
        double norm = sqrt(n2);
        if (fabs(norm * (double)l->alpha - 1.0) > 2e-2) {
            free(tmp);
            snprintf(err, err_bytes, "LoRA token_embd direction norm %.4f (alpha %.4f) is not unit; non-abliteration LoRA unsupported", norm, l->alpha);
            return 1;
        }
        float inv = (float)(1.0 / norm);
        for (int j = 0; j < n_embd; j++) out_dirs[j] *= inv;
        if (out_has_emb) *out_has_emb = 1;
        any = 1;
    }

    /* 2. Per-layer direction vectors (shared by attn_output & all experts of ffn_down_exps) */
    for (int ip = 0; ip < n_phys; ip++) {
        const InkLoraLayer *L = &l->layers[ip];
        if (!L->attn_output_b.present && !L->ffn_down_exps_b.present) continue;
        float *dst = out_dirs + (size_t)(1 + ip) * (size_t)n_embd;
        if (!L->attn_output_b.present || lora_read_vec(l, &L->attn_output_b, 0, dst, n_embd) != 0) {
            free(tmp);
            snprintf(err, err_bytes, "LoRA layer %d missing or malformed attn_output.weight.lora_b", ip);
            return 1;
        }
        double n2 = 0.0;
        for (int j = 0; j < n_embd; j++) n2 += (double)dst[j] * (double)dst[j];
        double norm = sqrt(n2);
        if (fabs(norm * (double)l->alpha - 1.0) > 2e-2) {
            free(tmp);
            snprintf(err, err_bytes, "LoRA layer %d direction norm %.4f is not unit", ip, norm);
            return 1;
        }
        float inv = (float)(1.0 / norm);
        for (int j = 0; j < n_embd; j++) dst[j] *= inv;

        if (L->ffn_down_exps_b.present) {
            /* Verify expert directions align with layer's attention direction (check first, mid, last). */
            int check_e[3] = { 0, n_expert / 2, n_expert - 1 };
            for (int ci = 0; ci < 3; ci++) {
                int e = check_e[ci];
                if (e < 0 || e >= n_expert) continue;
                if (lora_read_vec(l, &L->ffn_down_exps_b, (int64_t)e * n_embd, tmp, n_embd) != 0) {
                    free(tmp);
                    snprintf(err, err_bytes, "LoRA layer %d ffn_down_exps.weight.lora_b read failed at expert %d", ip, e);
                    return 1;
                }
                double dot = 0.0;
                for (int j = 0; j < n_embd; j++) dot += (double)tmp[j] * (double)dst[j];
                if (fabs(dot * (double)l->alpha - 1.0) > 2e-2) {
                    free(tmp);
                    snprintf(err, err_bytes, "LoRA layer %d expert %d direction dot=%.4f != 1.0", ip, e, dot);
                    return 1;
                }
            }
        }
        if (out_has_layer) out_has_layer[ip] = 1;
        any = 1;
    }
    free(tmp);
    if (!any) {
        snprintf(err, err_bytes, "LoRA contains no supported Inkling 2 Mini direction-ablation tensors");
        return 1;
    }
    return 0;
}
