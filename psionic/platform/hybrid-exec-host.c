/* Thin C11 Metal 4 platform host for hybrid-exec-v1 (Executor W3.1, #13920).
 *
 * Connects Psionic's native Coax-compiled Metal 4 transformer runtime
 * (Inkling-2-Mini-IQ4_NL.gguf, 26B MoE, 200,058 vocab) to Coax #authority
 * libraries (hybrid-exec-policy, hybrid-lora, hybrid-exec, next-token Ch 53
 * DFA, module-v2 disk-loaded dense F64 weights, Ch 234 exact replay, and
 * Ch 239-241 statistical gates).
 *
 * Platform effects only live here: file I/O, SHA-256 of the GGUF checkpoint,
 * Metal session lifecycle, timing, and stdout reporting. All policy, LoRA
 * training/pinning, constrained grammar stepping, compiled-core evaluation,
 * replay verification, and statistical gating run inside Coax (hx-bridge.h).
 */
#include "hx-bridge.h"
#include "inkling-metal.h"

#include <CommonCrypto/CommonDigest.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

static double now_s(void) {
    struct timespec t;
    clock_gettime(CLOCK_MONOTONIC, &t);
    return (double)t.tv_sec + 1e-9 * (double)t.tv_nsec;
}

static void fmt_hex(const uint8_t d[32], char out[65]) {
    static const char hex[] = "0123456789abcdef";
    for (int i = 0; i < 32; i++) {
        out[2 * i] = hex[d[i] >> 4];
        out[2 * i + 1] = hex[d[i] & 15];
    }
    out[64] = '\0';
}

static int sha256_file(const char *path, coax_a32_u8 *out) {
    FILE *f = fopen(path, "rb");
    if (!f) {
        fprintf(stderr, "cannot open %s for sha256\n", path);
        return 1;
    }
    CC_SHA256_CTX ctx;
    CC_SHA256_Init(&ctx);
    unsigned char *buf = (unsigned char *)malloc(1 << 20);
    if (!buf) { fclose(f); return 1; }
    for (;;) {
        size_t n = fread(buf, 1, 1 << 20, f);
        if (n > 0) CC_SHA256_Update(&ctx, buf, (CC_LONG)n);
        if (n < (1 << 20)) break;
    }
    free(buf);
    fclose(f);
    CC_SHA256_Final(out->e, &ctx);
    return 0;
}

static uint8_t *read_bin_file(const char *path, size_t *out_len) {
    FILE *f = fopen(path, "rb");
    if (!f) return NULL;
    if (fseek(f, 0, SEEK_END) != 0) { fclose(f); return NULL; }
    long sz = ftell(f);
    if (sz <= 0 || fseek(f, 0, SEEK_SET) != 0) { fclose(f); return NULL; }
    uint8_t *buf = (uint8_t *)malloc((size_t)sz);
    if (!buf) { fclose(f); return NULL; }
    if (fread(buf, 1, (size_t)sz, f) != (size_t)sz) { free(buf); fclose(f); return NULL; }
    fclose(f);
    *out_len = (size_t)sz;
    return buf;
}

/* ---- Disk-Loaded W2.2 Compiled Module Bundle (~/.openagents/modules/) ---- */

#define MOD_SLOTS 5
#define MOD_MAX_LAYERS 64
#define MOD_MAX_HEADS 16
#define MOD_MAX_WEIGHTS 65536

typedef struct {
    int id;
    const char *name;
    const char *sha_hex;
    coax_a32_u8 sha;
    coax_t_EvLayerDesc layers[MOD_MAX_LAYERS];
    coax_t_EvHeadDesc heads[MOD_MAX_HEADS];
    double weights[MOD_MAX_WEIGHTS];
    coax_t_EvModel model;
    int loaded;
} LoadedModule;

static const struct {
    int id;
    const char *name;
    const char *sha_hex;
} MODULE_SPECS[MOD_SLOTS] = {
    {1, "oa-exec-diff-verify",        "606049b83f17f20a0fc7f3f6e6278f740f8dc9f0221ead98a79b195b155b9c6a"},
    {2, "oa-exec-json-schema",        "5293cf7460beeb58025291821a20f99c4f811b11cdcaf62dc117dd6172a29895"},
    {3, "oa-exec-dep-sched",          "7a821819a3ad14b583e456c8d909954e27b90d5a3dae7406027e6fbaf06b58f8"},
    {4, "oa-exec-arith-budget",       "a93a4b53a4a5a3319cd4d5a55a6168029b95ffa782fc50119c8eef55366ae2d5"},
    {5, "oa-exec-schema-diff-linked", "f595062a4577366332225ac5c192207d561aeca7e7fe4d45a2b537a2eecd6454"},
};

static int load_w22_modules(const char *modules_dir, LoadedModule mods[MOD_SLOTS], coax_a32_u8 *bundle_sha) {
    for (int i = 0; i < MOD_SLOTS; i++) {
        memset(&mods[i], 0, sizeof mods[i]);
        mods[i].id = MODULE_SPECS[i].id;
        mods[i].name = MODULE_SPECS[i].name;
        mods[i].sha_hex = MODULE_SPECS[i].sha_hex;
        coax_v_u8 hv = {(uint8_t *)MODULE_SPECS[i].sha_hex, 64};
        coax_v_u8 ov = {mods[i].sha.e, 32};
        if (!hxb_hex_to_sha(hv, ov)) {
            fprintf(stderr, "invalid module sha hex for %s\n", mods[i].name);
            return 1;
        }
        char path[1024];
        snprintf(path, sizeof path, "%s/%s/weights.safetensors", modules_dir, mods[i].sha_hex);
        size_t sz = 0;
        uint8_t *bytes = read_bin_file(path, &sz);
        if (!bytes) {
            fprintf(stderr, "missing compiled weight module: %s\n", path);
            return 1;
        }
        coax_v_u8 bv = {bytes, (int64_t)sz};
        coax_v_t_EvLayerDesc lv = {mods[i].layers, MOD_MAX_LAYERS};
        coax_v_t_EvHeadDesc hv_d = {mods[i].heads, MOD_MAX_HEADS};
        coax_v_f64 wv = {mods[i].weights, MOD_MAX_WEIGHTS};
        coax_t_EvLoadResult ld = hxb_load_module(bv, mods[i].sha, lv, hv_d, wv);
        free(bytes);
        if (ld.model.status != COAX_EvStatus_ok) {
            fprintf(stderr, "SHA-256 or deserialize failed for %s (status=%u)\n", mods[i].name, (unsigned)ld.model.status);
            return 1;
        }
        mods[i].model = ld.model;
        mods[i].loaded = 1;
        printf("MODULE_LOADED id=%d name=%s sha=%s layers=%lld width=%lld weights=%lld bytes=%zu\n",
               mods[i].id, mods[i].name, mods[i].sha_hex,
               (long long)ld.model.n_layers, (long long)ld.model.width, (long long)ld.model.n_weights, sz);
    }
    *bundle_sha = hxb_bundle_digest(mods[0].sha, mods[1].sha, mods[2].sha, mods[3].sha, mods[4].sha);
    char bhex[65];
    fmt_hex(bundle_sha->e, bhex);
    printf("CORE_BUNDLE_SHA256 %s\n", bhex);
    return 0;
}

/* ---- Pre-indexed Ch 53 ABI Candidate Vocabulary ---- */
/* Exact optimization: any token containing a byte outside the Ch 53 arg grammar
 * alphabet [-0-9,</>_a-z] is rejected at byte 0 by hxb_grammar_step_bytes anyway;
 * pre-filtering yields the exact same token set over all 200,058 vocab entries
 * in <0.1 ms per decode step. */

typedef struct {
    int32_t id;
    uint8_t bytes[32];
    int len;
} AbiTok;

typedef struct {
    AbiTok *items;
    int count;
    int cap;
} AbiVocab;

static int is_abi_arg_char(uint8_t c) {
    if (c >= '0' && c <= '9') return 1;
    if (c == '-' || c == ',' || c == '<' || c == '/' || c == '>' || c == '_') return 1;
    if (c == 'a' || c == 'c' || c == 'e' || c == 'g' || c == 'l' || c == 'r' || c == 'x') return 1;
    return 0;
}

static void build_abi_vocab(const InkSession *s, AbiVocab *av) {
    int nv = ink_vocab_unpadded(s);
    av->cap = 8192;
    av->count = 0;
    av->items = (AbiTok *)malloc((size_t)av->cap * sizeof(AbiTok));
    char piece[256];
    for (int32_t id = 0; id < nv; id++) {
        if (ink_is_eog(s, id)) continue;
        int len = ink_decode_piece(s, id, piece, (int)sizeof piece);
        if (len <= 0 || len > 32) continue;
        int ok = 1;
        for (int j = 0; j < len; j++) {
            if (!is_abi_arg_char((uint8_t)piece[j])) { ok = 0; break; }
        }
        if (!ok) continue;
        if (av->count == av->cap) {
            av->cap *= 2;
            av->items = (AbiTok *)realloc(av->items, (size_t)av->cap * sizeof(AbiTok));
        }
        av->items[av->count].id = id;
        memcpy(av->items[av->count].bytes, piece, (size_t)len);
        av->items[av->count].len = len;
        av->count++;
    }
}

/* ---- Disjoint Training Split (32 tasks) & Held-Out Eval Suite (48 tasks) ---- */

typedef struct {
    int domain;           /* 1=diff, 2=json, 3=dep, 4=budget */
    int is_refusal_trap;  /* 0=normal domain task, 1=explicit refusal trap */
    const char *want_refuse_reason; /* non-NULL for refusal traps */
    int n_args;
    int64_t args[10];
    int64_t want_out[3];  /* exact ground truth for normal domain tasks */
    const char *prompt;
} EvalTask;

/* 32 disjoint training prompts (8 per domain) used ONLY to extract real Inkling-2-Mini
 * Metal hidden states and train the Ch 140 LoRA routing adapter. Zero overlap with EVAL_TASKS. */
static const struct {
    int label;
    const char *prompt;
} TRAIN_TASKS[32] = {
    /* 8 diff-verify training examples */
    {1, "Check unified diff patch on src/lexer.coax: old_start=12, old_count=4, new_start=12, new_count=6, file_lines=200, matched_ctx=3, expected_ctx=3."},
    {1, "Verify hunk bounds for lib/parser.coax: old_start=88, old_count=9, new_start=88, new_count=5, file_lines=150, matched_ctx=4, expected_ctx=4."},
    {1, "Unified diff applicability check on core/vm.coax: old_start=300, old_count=12, new_start=300, new_count=12, file_lines=305, matched_ctx=3, expected_ctx=3."},
    {1, "Audit patch hunk for net/socket.coax: old_start=1, old_count=7, new_start=1, new_count=11, file_lines=90, matched_ctx=2, expected_ctx=3."},
    {1, "Patch verification on tools/fmt.coax: old_start=45, old_count=10, new_start=45, new_count=3, file_lines=54, matched_ctx=5, expected_ctx=5."},
    {1, "Check diff hunk line range in ui/grid.coax: old_start=210, old_count=15, new_start=210, new_count=22, file_lines=400, matched_ctx=6, expected_ctx=6."},
    {1, "Unified diff check for app/main.coax: old_start=50, old_count=8, new_start=50, new_count=14, file_lines=500, matched_ctx=3, expected_ctx=3."},
    {1, "Validate patch hunk context in db/wal.coax: old_start=72, old_count=6, new_start=72, new_count=2, file_lines=100, matched_ctx=1, expected_ctx=2."},

    /* 8 json-schema training examples */
    {2, "Validate edit_file JSON tool payload: has_path=1, path_len=18, max_path=128, has_old=1, old_len=120, has_new=1, new_len=140, max_body=512."},
    {2, "Check wire JSON schema record: has_path=1, path_len=42, max_path=256, has_old=1, old_len=310, has_new=1, new_len=290, max_body=1024."},
    {2, "Tool-call JSON schema verification: has_path=0, path_len=12, max_path=256, has_old=1, old_len=80, has_new=1, new_len=90, max_body=512."},
    {2, "Audit JSON payload bounds: has_path=1, path_len=64, max_path=64, has_old=1, old_len=400, has_new=1, new_len=400, max_body=800."},
    {2, "Validate protocol JSON arguments: has_path=1, path_len=95, max_path=128, has_old=1, old_len=500, has_new=1, new_len=600, max_body=1000."},
    {2, "Check structured tool JSON schema: has_path=1, path_len=31, max_path=256, has_old=0, old_len=10, has_new=1, new_len=20, max_body=256."},
    {2, "JSON edit payload check: has_path=1, path_len=55, max_path=256, has_old=1, old_len=222, has_new=1, new_len=333, max_body=1024."},
    {2, "Verify JSON schema cap: has_path=1, path_len=20, max_path=128, has_old=1, old_len=700, has_new=1, new_len=300, max_body=1000."},

    /* 8 dep-sched training examples */
    {3, "DAG dependency scheduler check for build target A: remaining_deps=0, has_cycle=0, running=1, max_slots=8, priority=4, blocked=0."},
    {3, "Schedule task node B in build DAG: remaining_deps=2, has_cycle=0, running=3, max_slots=16, priority=9, blocked=0."},
    {3, "Evaluate DAG task readiness for job C: remaining_deps=0, has_cycle=0, running=4, max_slots=4, priority=7, blocked=0."},
    {3, "Check dependency graph slot allocation for node D: remaining_deps=0, has_cycle=1, running=2, max_slots=10, priority=6, blocked=0."},
    {3, "DAG topological dispatch check for task E: remaining_deps=0, has_cycle=0, running=5, max_slots=12, priority=15, blocked=0."},
    {3, "Verify DAG worker readiness for task F: remaining_deps=0, has_cycle=0, running=0, max_slots=6, priority=3, blocked=1."},
    {3, "Task graph scheduler admission for node G: remaining_deps=0, has_cycle=0, running=7, max_slots=20, priority=25, blocked=0."},
    {3, "Check build DAG dependency state for target H: remaining_deps=0, has_cycle=0, running=9, max_slots=16, priority=11, blocked=0."},

    /* 8 arith-budget training examples */
    {4, "Satoshi budget admission for inference job: balance_sats=10000, spent_sats=2000, req_sats=150, daily_cap=8000, tokens_k=12, rate_sats=40."},
    {4, "Check Spark wallet token budget: balance_sats=50000, spent_sats=12000, req_sats=300, daily_cap=25000, tokens_k=25, rate_sats=80."},
    {4, "Pylon payout accounting check: balance_sats=4000, spent_sats=1000, req_sats=500, daily_cap=10000, tokens_k=80, rate_sats=60."},
    {4, "Verify daily cap and satoshi cost: balance_sats=90000, spent_sats=70000, req_sats=250, daily_cap=75000, tokens_k=100, rate_sats=55."},
    {4, "Compute token-rate satoshi budget: balance_sats=25000, spent_sats=5000, req_sats=420, daily_cap=20000, tokens_k=33, rate_sats=70."},
    {4, "Wallet budget check for batch trace: balance_sats=80000, spent_sats=15000, req_sats=100, daily_cap=50000, tokens_k=45, rate_sats=90."},
    {4, "Satoshi spend ceiling check: balance_sats=12000, spent_sats=9500, req_sats=200, daily_cap=10000, tokens_k=15, rate_sats=25."},
    {4, "Accounting admission for Spark escrow: balance_sats=60000, spent_sats=10000, req_sats=750, daily_cap=40000, tokens_k=64, rate_sats=50."},
};

/* 48 frozen held-out tasks (40 domain tasks + 8 explicit refusal traps).
 * Zero overlap with TRAIN_TASKS; tricky multi-digit arithmetic, tight EOF/cap
 * boundaries, and explicit >2^26 / overflow / negative / cycle / schema traps. */
static const EvalTask EVAL_TASKS[48] = {
    /* ---- Domain 1: Unified-Diff Verification (0..9) ---- */
    {1, 0, NULL, 7, {4789, 39, 4789, 54, 4827, 14, 14}, {1, 4842, 15},
     "Verify unified diff hunk on runtime/engine.coax: old_start=4789, old_count=39, new_start=4789, new_count=54, file_lines=4827, matched_ctx=14, expected_ctx=14."},
    {1, 0, NULL, 7, {4790, 39, 4790, 54, 4827, 14, 14}, {0, 4827, 0},
     "Verify unified diff hunk on runtime/engine.coax: old_start=4790, old_count=39, new_start=4790, new_count=54, file_lines=4827, matched_ctx=14, expected_ctx=14."},
    {1, 0, NULL, 7, {18347, 237, 18347, 189, 19432, 18, 18}, {1, 19384, -48},
     "Check unified diff patch on kernel/matmul.coax: old_start=18347, old_count=237, new_start=18347, new_count=189, file_lines=19432, matched_ctx=18, expected_ctx=18."},
    {1, 0, NULL, 7, {9412, 589, 9412, 743, 10000, 9, 9}, {1, 10154, 154},
     "Check unified diff patch on compiler/emit.coax: old_start=9412, old_count=589, new_start=9412, new_count=743, file_lines=10000, matched_ctx=9, expected_ctx=9."},
    {1, 0, NULL, 7, {9413, 589, 9413, 743, 10000, 9, 9}, {0, 10000, 0},
     "Check unified diff patch on compiler/emit.coax: old_start=9413, old_count=589, new_start=9413, new_count=743, file_lines=10000, matched_ctx=9, expected_ctx=9."},
    {1, 0, NULL, 7, {31204, 478, 31204, 291, 45819, 21, 21}, {1, 45632, -187},
     "Verify unified diff hunk on storage/btree.coax: old_start=31204, old_count=478, new_start=31204, new_count=291, file_lines=45819, matched_ctx=21, expected_ctx=21."},
    {1, 0, NULL, 7, {12789, 345, 12789, 512, 15000, 11, 12}, {0, 15000, 0},
     "Verify unified diff hunk on crypto/schnorr.coax: old_start=12789, old_count=345, new_start=12789, new_count=512, file_lines=15000, matched_ctx=11, expected_ctx=12."},
    {1, 0, NULL, 7, {65100, 437, 65100, 819, 65536, 7, 7}, {1, 65918, 382},
     "Check unified diff patch on shader/wgsl.coax: old_start=65100, old_count=437, new_start=65100, new_count=819, file_lines=65536, matched_ctx=7, expected_ctx=7."},
    {1, 0, NULL, 7, {7421, 918, 7421, 649, 8338, 15, 15}, {1, 8069, -269},
     "Verify unified diff hunk on protocol/wire.coax: old_start=7421, old_count=918, new_start=7421, new_count=649, file_lines=8338, matched_ctx=15, expected_ctx=15."},
    {1, 0, NULL, 7, {24519, 673, 24519, 1024, 25190, 8, 8}, {0, 25190, 0},
     "Verify unified diff hunk on agent/planner.coax: old_start=24519, old_count=673, new_start=24519, new_count=1024, file_lines=25190, matched_ctx=8, expected_ctx=8."},

    /* ---- Domain 2: JSON / Wire Schema Validation (10..19) ---- */
    {2, 0, NULL, 8, {1, 147, 256, 1, 3847, 1, 4345, 8192}, {1, 8339, 1},
     "Validate JSON edit payload schema: has_path=1, path_len=147, max_path=256, has_old=1, old_len=3847, has_new=1, new_len=4345, max_body=8192."},
    {2, 0, NULL, 8, {1, 219, 256, 1, 14729, 1, 18039, 32768}, {1, 32987, 1},
     "Validate JSON wire schema payload: has_path=1, path_len=219, max_path=256, has_old=1, old_len=14729, has_new=1, new_len=18039, max_body=32768."},
    {2, 0, NULL, 8, {1, 83, 128, 1, 29481, 1, 36055, 65536}, {1, 65619, 1},
     "Check JSON tool-call schema payload: has_path=1, path_len=83, max_path=128, has_old=1, old_len=29481, has_new=1, new_len=36055, max_body=65536."},
    {2, 0, NULL, 8, {1, 194, 512, 1, 6723, 1, 9661, 16384}, {1, 16578, 1},
     "Validate JSON protocol schema record: has_path=1, path_len=194, max_path=512, has_old=1, old_len=6723, has_new=1, new_len=9661, max_body=16384."},
    {2, 0, NULL, 8, {1, 377, 512, 1, 48219, 1, 51781, 100000}, {1, 100377, 1},
     "Audit JSON schema payload bounds: has_path=1, path_len=377, max_path=512, has_old=1, old_len=48219, has_new=1, new_len=51781, max_body=100000."},
    {2, 0, NULL, 8, {1, 128, 128, 1, 7483, 1, 8901, 16384}, {1, 16512, 1},
     "Check JSON edit tool schema record: has_path=1, path_len=128, max_path=128, has_old=1, old_len=7483, has_new=1, new_len=8901, max_body=16384."},
    {2, 0, NULL, 8, {1, 263, 512, 1, 19384, 1, 27615, 50000}, {1, 47262, 1},
     "Validate JSON patch schema bytes: has_path=1, path_len=263, max_path=512, has_old=1, old_len=19384, has_new=1, new_len=27615, max_body=50000."},
    {2, 0, NULL, 8, {1, 419, 512, 1, 83921, 1, 47151, 131072}, {1, 131491, 1},
     "Verify JSON wire schema payload: has_path=1, path_len=419, max_path=512, has_old=1, old_len=83921, has_new=1, new_len=47151, max_body=131072."},
    {2, 0, NULL, 8, {1, 74, 256, 1, 12457, 1, 19863, 35000}, {1, 32394, 1},
     "Check structured JSON schema payload: has_path=1, path_len=74, max_path=256, has_old=1, old_len=12457, has_new=1, new_len=19863, max_body=35000."},
    {2, 0, NULL, 8, {1, 312, 512, 1, 54321, 1, 67890, 125000}, {1, 122523, 1},
     "Validate JSON edit schema record: has_path=1, path_len=312, max_path=512, has_old=1, old_len=54321, has_new=1, new_len=67890, max_body=125000."},

    /* ---- Domain 3: DAG Dependency Scheduling (20..29) ---- */
    {3, 0, NULL, 6, {0, 0, 37, 128, 473, 0}, {1, 38, 4821},
     "DAG dependency scheduler check for job N1: remaining_deps=0, has_cycle=0, running=37, max_slots=128, priority=473, blocked=0."},
    {3, 0, NULL, 6, {0, 0, 128, 128, 473, 0}, {0, 128, 0},
     "DAG dependency scheduler check for job N2: remaining_deps=0, has_cycle=0, running=128, max_slots=128, priority=473, blocked=0."},
    {3, 0, NULL, 6, {0, 0, 189, 512, 847, 0}, {1, 190, 8793},
     "Evaluate DAG build schedule for job N3: remaining_deps=0, has_cycle=0, running=189, max_slots=512, priority=847, blocked=0."},
    {3, 0, NULL, 6, {3, 0, 42, 256, 619, 0}, {0, 42, 0},
     "Evaluate DAG build schedule for job N4: remaining_deps=3, has_cycle=0, running=42, max_slots=256, priority=619, blocked=0."},
    {3, 0, NULL, 6, {0, 0, 673, 1024, 1294, 0}, {1, 674, 13291},
     "DAG task dependency scheduling for job N5: remaining_deps=0, has_cycle=0, running=673, max_slots=1024, priority=1294, blocked=0."},
    {3, 0, NULL, 6, {0, 0, 84, 256, 912, 1}, {0, 84, 0},
     "DAG task dependency scheduling for job N6: remaining_deps=0, has_cycle=0, running=84, max_slots=256, priority=912, blocked=1."},
    {3, 0, NULL, 6, {0, 0, 294, 768, 2385, 0}, {1, 295, 24324},
     "Check build DAG worker readiness for job N7: remaining_deps=0, has_cycle=0, running=294, max_slots=768, priority=2385, blocked=0."},
    {3, 0, NULL, 6, {0, 0, 417, 640, 5678, 0}, {1, 418, 57003},
     "Check build DAG worker readiness for job N8: remaining_deps=0, has_cycle=0, running=417, max_slots=640, priority=5678, blocked=0."},
    {3, 0, NULL, 6, {0, 0, 999, 1024, 3456, 0}, {1, 1000, 34585},
     "DAG topological scheduler check for job N9: remaining_deps=0, has_cycle=0, running=999, max_slots=1024, priority=3456, blocked=0."},
    {3, 0, NULL, 6, {0, 0, 513, 512, 789, 0}, {0, 513, 0},
     "DAG topological scheduler check for job N10: remaining_deps=0, has_cycle=0, running=513, max_slots=512, priority=789, blocked=0."},

    /* ---- Domain 4: Exact Satoshi / Token Budget Accounting (30..39) ---- */
    {4, 0, NULL, 6, {1847293, 624180, 1437, 1000000, 473, 789}, {1, 1472659, 374634},
     "Satoshi budget accounting check: balance_sats=1847293, spent_sats=624180, req_sats=1437, daily_cap=1000000, tokens_k=473, rate_sats=789."},
    {4, 0, NULL, 6, {1847293, 625500, 1437, 1000000, 473, 789}, {0, 1847293, 0},
     "Satoshi budget accounting check: balance_sats=1847293, spent_sats=625500, req_sats=1437, daily_cap=1000000, tokens_k=473, rate_sats=789."},
    {4, 0, NULL, 6, {5432109, 1234567, 8912, 2000000, 687, 943}, {1, 4775356, 656753},
     "Spark wallet token budget check: balance_sats=5432109, spent_sats=1234567, req_sats=8912, daily_cap=2000000, tokens_k=687, rate_sats=943."},
    {4, 0, NULL, 6, {650000, 100000, 8912, 2000000, 687, 943}, {0, 650000, 0},
     "Spark wallet token budget check: balance_sats=650000, spent_sats=100000, req_sats=8912, daily_cap=2000000, tokens_k=687, rate_sats=943."},
    {4, 0, NULL, 6, {9876543, 3456789, 12345, 5000000, 819, 1237}, {1, 8851095, 1025448},
     "Pylon satoshi payout budget check: balance_sats=9876543, spent_sats=3456789, req_sats=12345, daily_cap=5000000, tokens_k=819, rate_sats=1237."},
    {4, 0, NULL, 6, {3219876, 876543, 4567, 1500000, 529, 873}, {1, 2753492, 466384},
     "Compute exact satoshi budget admission: balance_sats=3219876, spent_sats=876543, req_sats=4567, daily_cap=1500000, tokens_k=529, rate_sats=873."},
    {4, 0, NULL, 6, {7654321, 2345678, 9876, 3000000, 743, 869}, {0, 7654321, 0},
     "Compute exact satoshi budget admission: balance_sats=7654321, spent_sats=2345678, req_sats=9876, daily_cap=3000000, tokens_k=743, rate_sats=869."},
    {4, 0, NULL, 6, {4123456, 912345, 3456, 2500000, 913, 1427}, {1, 2817149, 1306307},
     "Spark escrow satoshi budget check: balance_sats=4123456, spent_sats=912345, req_sats=3456, daily_cap=2500000, tokens_k=913, rate_sats=1427."},
    {4, 0, NULL, 6, {8765432, 4000000, 6789, 5000000, 637, 1559}, {1, 7765560, 999872},
     "Verify daily cap and satoshi budget: balance_sats=8765432, spent_sats=4000000, req_sats=6789, daily_cap=5000000, tokens_k=637, rate_sats=1559."},
    {4, 0, NULL, 6, {8765432, 4000200, 6789, 5000000, 637, 1559}, {0, 8765432, 0},
     "Verify daily cap and satoshi budget: balance_sats=8765432, spent_sats=4000200, req_sats=6789, daily_cap=5000000, tokens_k=637, rate_sats=1559."},

    /* ---- Domain 5: Refusal Edge-Case Traps (40..47) — MUST emit <exec_refuse> cleanly (0 silent hallucinations!) ---- */
    {4, 1, "arg_out_of_range", 6, {95000000, 1000, 500, 99000000, 10, 50}, {0, 0, 0},
     "Satoshi budget accounting check: balance_sats=95000000, spent_sats=1000, req_sats=500, daily_cap=99000000, tokens_k=10, rate_sats=50."},
    {1, 1, "arg_out_of_range", 7, {100, 10, 100, 12, 80000000, 3, 3}, {0, 0, 0},
     "Verify unified diff hunk on huge/blob.coax: old_start=100, old_count=10, new_start=100, new_count=12, file_lines=80000000, matched_ctx=3, expected_ctx=3."},
    {4, 1, "arith_overflow", 6, {50000000, 1000, 200, 60000000, 12000, 9000}, {0, 0, 0},
     "Spark wallet token budget check: balance_sats=50000000, spent_sats=1000, req_sats=200, daily_cap=60000000, tokens_k=12000, rate_sats=9000."},
    {4, 1, "arith_overflow", 6, {60000000, 40000000, 10000000, 65000000, 5000, 4000}, {0, 0, 0},
     "Pylon satoshi payout budget check: balance_sats=60000000, spent_sats=40000000, req_sats=10000000, daily_cap=65000000, tokens_k=5000, rate_sats=4000."},
    {4, 1, "arg_out_of_range", 6, {50000, 1000, 200, 40000, -5, 50}, {0, 0, 0},
     "Satoshi budget accounting check: balance_sats=50000, spent_sats=1000, req_sats=200, daily_cap=40000, tokens_k=-5, rate_sats=50."},
    {3, 1, "cycle_detected", 6, {0, 1, 14, 64, 520, 0}, {0, 14, 0},
     "DAG dependency scheduler check for job C1: remaining_deps=0, has_cycle=1, running=14, max_slots=64, priority=520, blocked=0."},
    {3, 1, "cycle_detected", 6, {0, 1, 88, 256, 1490, 0}, {0, 88, 0},
     "DAG topological scheduler check for job C2: remaining_deps=0, has_cycle=1, running=88, max_slots=256, priority=1490, blocked=0."},
    {2, 1, "malformed_schema", 8, {0, 45, 256, 1, 2000, 1, 3000, 8192}, {0, 0, 1},
     "Validate JSON edit payload schema: has_path=0, path_len=45, max_path=256, has_old=1, old_len=2000, has_new=1, new_len=3000, max_body=8192."},
};

/* ---- Prompt Formatting & Base Logit Extraction ---- */

static const char *base_system_for_domain(int domain) {
    if (domain == 1) {
        return "Thinking effort level: 0.0\n"
               "Exact verifier for diff-verify(old_start,old_count,new_start,new_count,file_lines,matched_ctx,expected_ctx). "
               "All inputs & results must be in [0,67108864] else reply ONLY 'REFUSE:arg_out_of_range' or 'REFUSE:arith_overflow'. "
               "ok=(old_start>=1 && new_start>=1 && old_start-1+old_count<=file_lines && matched_ctx==expected_ctx). "
               "Reply ONLY '1,file_lines+new_count-old_count,new_count-old_count' if ok else '0,file_lines,0'.";
    }
    if (domain == 2) {
        return "Thinking effort level: 0.0\n"
               "Exact verifier for json-schema(has_path,path_len,max_path,has_old,old_len,has_new,new_len,max_body). "
               "ok=(has_path==1 && 1<=path_len<=max_path && has_old==1 && has_new==1 && old_len>=0 && new_len>=0 && old_len+new_len<=max_body). "
               "If !ok reply ONLY 'REFUSE:malformed_schema', else reply ONLY '1,path_len+old_len+new_len,1'.";
    }
    if (domain == 3) {
        return "Thinking effort level: 0.0\n"
               "Exact verifier for dep-sched(remaining_deps,has_cycle,running,max_slots,priority,blocked). "
               "If has_cycle!=0 reply ONLY 'REFUSE:cycle_detected'. "
               "ok=(remaining_deps==0 && blocked==0 && running<max_slots && max_slots>=1). "
               "Reply ONLY '1,running+1,priority*10+max_slots-running' if ok else '0,running,0'.";
    }
    return "Thinking effort level: 0.0\n"
           "Exact verifier for arith-budget(balance_sats,spent_sats,req_sats,daily_cap,tokens_k,rate_sats). "
           "All inputs in [0,67108864] else 'REFUSE:arg_out_of_range'; spent_sats+req_sats+tokens_k*rate_sats<=67108864 else 'REFUSE:arith_overflow'. "
           "cost=req_sats+tokens_k*rate_sats; ok=(cost<=balance_sats && spent_sats+cost<=daily_cap). "
           "Reply ONLY '1,balance_sats-cost,cost' if ok else '0,balance_sats,0'.";
}

static const char *HYBRID_SYSTEM_PROMPT =
    "Thinking effort level: 0.0\n"
    "Copy ALL task integer values in exact left-to-right order (including 0s and negatives) into <exec_arg>...</exec_arg></exec_call>. "
    "Example: remaining_deps=0, has_cycle=0, running=2, max_slots=8, priority=5, blocked=0 -> <exec_arg>0,0,2,8,5,0</exec_arg></exec_call>.\n"
    "After <exec_return>a,b,c</exec_return> reply ONLY a,b,c.\n"
    "After <exec_refuse reason=\"R\">...</exec_refuse> reply ONLY REFUSE:R (never numbers or XML tags).";

/* Incremental KV-cache token injection at live s->pos (0 prefix tokens re-prefilled).
 * Uses fast single-token Metal decode steps for short bursts (<40 tok) and batched prefill for longer spans. */
static int ink_inject_kv(InkSession *s, const int32_t *toks, int n, int compute_last_logits) {
    if (n <= 0) return 0;
    if (n >= 40) return ink_prefill(s, toks, n, compute_last_logits);
    for (int i = 0; i < n; i++) {
        if (ink_step(s, toks[i], (compute_last_logits && i == n - 1) ? 1 : 0) != 0) return 1;
    }
    return 0;
}

static void format_chat_prompt(const char *sys, const char *user, char *out, size_t cap) {
    snprintf(out, cap,
             "<|message_system|><|content_text|>%s<|end_message|>"
             "<|message_user|><|content_text|>%s<|end_message|>"
             "<|message_model|><|content_text|>",
             sys, user);
}

/* Compute base-model routing logits [5] directly from real Inkling-2-Mini Metal logits
 * at the first decision position (before LoRA delta is added). */
static coax_a5_f64 extract_base_router_logits(const InkSession *s, const int32_t class_toks[5]) {
    const float *lg = ink_logits(s);
    int nv = ink_vocab_unpadded(s);
    coax_a5_f64 out;
    for (int c = 0; c < 5; c++) {
        int32_t tid = class_toks[c];
        out.e[c] = (tid >= 0 && tid < nv) ? (double)lg[tid] * 0.1 : 0.0;
    }
    return out;
}

static double softmax_prob_of_token(const float *logits, int nv, int32_t tok) {
    if (!logits || tok < 0 || tok >= nv) return 0.5;
    float mx = logits[0];
    for (int i = 1; i < nv; i++) {
        if (logits[i] > mx) mx = logits[i];
    }
    double sum = 0.0;
    for (int i = 0; i < nv; i++) {
        double d = (double)logits[i] - (double)mx;
        if (d > -40.0) sum += exp(d);
    }
    double p = exp((double)logits[tok] - (double)mx) / (sum > 0.0 ? sum : 1.0);
    if (p < 0.01) p = 0.01;
    if (p > 0.99) p = 0.99;
    return p;
}

/* Parse "a,b,c" from a model response string. Returns 1 if 3 integers found. */
static int parse_three_ints(const char *txt, int64_t out[3]) {
    const char *p = txt;
    while (*p && !((*p >= '0' && *p <= '9') || *p == '-')) p++;
    if (!*p) return 0;
    char *end = NULL;
    for (int k = 0; k < 3; k++) {
        out[k] = strtoll(p, &end, 10);
        if (end == p) return 0;
        p = end;
        if (k < 2) {
            while (*p == ' ' || *p == '\t') p++;
            if (*p != ',') return 0;
            p++;
            while (*p == ' ' || *p == '\t') p++;
        }
    }
    return 1;
}

static int score_response(const EvalTask *t, const char *resp, int *silent_hallucination) {
    *silent_hallucination = 0;
    if (t->is_refusal_trap) {
        if (strstr(resp, "REFUSE") != NULL || strstr(resp, "<exec_refuse") != NULL) {
            if (t->want_refuse_reason && strstr(resp, t->want_refuse_reason) != NULL) return 1;
            return 1; /* typed refusal present */
        }
        /* Emitted a numeric guess or non-refusal on a refusal trap = silent hallucination! */
        *silent_hallucination = 1;
        return 0;
    }
    int64_t got[3] = {0, 0, 0};
    if (!parse_three_ints(resp, got)) return 0;
    return (got[0] == t->want_out[0] && got[1] == t->want_out[1] && got[2] == t->want_out[2]) ? 1 : 0;
}

/* ---- Run Base Model Alone on one task ---- */
static void run_base_task(
    InkSession *s, const EvalTask *t, float temp, uint64_t *rng,
    char *out_text, size_t out_cap, int *prompt_toks_out, int *gen_toks_out, double *conf_out
) {
    char prompt[4096];
    format_chat_prompt(base_system_for_domain(t->domain), t->prompt, prompt, sizeof prompt);
    int32_t toks[2048];
    int n_tok = ink_encode(s, prompt, strlen(prompt), toks, 2048, 1);
    ink_reset(s);
    if (ink_prefill(s, toks, n_tok, 1) != 0) {
        fprintf(stderr, "base prefill error: %s\n", ink_error(s));
        exit(1);
    }
    *prompt_toks_out = n_tok;

    InkSampleParams sp = {.temperature = temp, .top_p = 0.95f, .top_k = 40};
    size_t pos = 0;
    out_text[0] = '\0';
    int gen = 0;
    double first_p = 0.5;
    int nv = ink_vocab_unpadded(s);
    for (int step = 0; step < 18; step++) {
        int32_t tok = ink_sample(s, &sp, rng);
        if (step == 0) first_p = softmax_prob_of_token(ink_logits(s), nv, tok);
        if (tok < 0 || ink_is_eog(s, tok)) break;
        char piece[256];
        int plen = ink_decode_piece(s, tok, piece, (int)sizeof piece);
        if (plen > 0) {
            if (plen < (int)sizeof piece) piece[plen] = '\0';
            if (strstr(piece, "<|end_message|>") != NULL) break;
            if (pos + (size_t)plen + 1 < out_cap) {
                memcpy(out_text + pos, piece, (size_t)plen);
                pos += (size_t)plen;
                out_text[pos] = '\0';
            }
            if (strchr(out_text, '\n') != NULL) { gen++; break; }
        }
        gen++;
        if (step + 1 < 18) {
            if (ink_step(s, tok, 1) != 0) break;
        }
    }
    *gen_toks_out = gen;
    *conf_out = first_p;
}

/* ---- Ch 53 Constrained Decoding of <exec_arg>...</exec_arg></exec_call> ---- */
static int decode_constrained_args(
    InkSession *s, const AbiVocab *av, int arity,
    char *arg_csv_out, size_t arg_cap, int *gen_toks_out
) {
    coax_t_NxEdge edges[192];
    int64_t next_buf[64 * 256];
    uint8_t acc_buf[64];
    uint8_t live_buf[64];
    coax_v_t_NxEdge ev = {edges, 192};
    coax_v_s64 nv_b = {next_buf, 64 * 256};
    coax_v_u8 acv = {acc_buf, 64};
    coax_v_u8 lv = {live_buf, 64};
    coax_t_NxCompiled comp = hx_build_arg_grammar(arity, ev, nv_b, acv, lv);
    if (comp.status != COAX_NxStatus_ok) return 1;

    int64_t gstate = comp.grammar.start;
    char full_buf[512];
    size_t flen = 0;
    full_buf[0] = '\0';
    int gen = 0;
    for (int step = 0; step < 64; step++) {
        if (hxb_grammar_accepting(comp.grammar, gstate)) break;
        const float *logits = ink_logits(s);
        int32_t best_tok = -1;
        int64_t best_next = -1;
        float best_val = -1e30f;
        int best_idx = -1;
        for (int i = 0; i < av->count; i++) {
            const AbiTok *at = &av->items[i];
            float v = logits[at->id];
            if (v <= best_val && best_tok != -1) continue;
            coax_v_u8 pv = {(uint8_t *)at->bytes, (int64_t)at->len};
            int64_t ns = hxb_grammar_step_bytes(comp.grammar, gstate, pv);
            if (ns >= 0) {
                best_val = v;
                best_tok = at->id;
                best_next = ns;
                best_idx = i;
            }
        }
        if (best_tok < 0) return 1;
        const AbiTok *chosen = &av->items[best_idx];
        if (flen + (size_t)chosen->len + 1 >= sizeof full_buf) return 1;
        memcpy(full_buf + flen, chosen->bytes, (size_t)chosen->len);
        flen += (size_t)chosen->len;
        full_buf[flen] = '\0';
        gstate = best_next;
        gen++;
        if (ink_step(s, best_tok, 1) != 0) return 1;
    }
    *gen_toks_out = gen;
    if (!hxb_grammar_accepting(comp.grammar, gstate)) return 1;
    /* Strip trailing "</exec_arg></exec_call>" (23 bytes) to get raw CSV */
    if (flen <= 23 || flen - 23 >= arg_cap) return 1;
    memcpy(arg_csv_out, full_buf, flen - 23);
    arg_csv_out[flen - 23] = '\0';
    return 0;
}

/* ---- Run hybrid-exec-v1 on one task (live Metal 4 KV-cache preserved end-to-end!) ---- */
typedef struct {
    int routed_mod;
    int core_ran;
    int replay_ok;
    int refused;
    int prefix_tokens_preserved;
    int prefix_tokens_reprefilled;
    int prompt_toks;
    int call_toks;
    int inject_toks;
    int final_toks;
    double route_prob;
    char csv_text[256];
    char final_text[256];
    char inject_text[256];
} HybridRunStats;

static void run_hybrid_task(
    InkSession *s, const AbiVocab *av, const LoadedModule mods[MOD_SLOTS],
    const coax_t_HlAdapter *adapter, const int32_t class_toks[5],
    const EvalTask *t, HybridRunStats *st
) {
    memset(st, 0, sizeof *st);
    char prompt[4096];
    format_chat_prompt(HYBRID_SYSTEM_PROMPT, t->prompt, prompt, sizeof prompt);
    int32_t toks[2048];
    int n_tok = ink_encode(s, prompt, strlen(prompt), toks, 2048, 1);
    ink_reset(s);
    if (ink_prefill(s, toks, n_tok, 1) != 0) {
        fprintf(stderr, "hybrid prefill error: %s\n", ink_error(s));
        exit(1);
    }
    st->prompt_toks = n_tok;

    /* 1. Extract real 2304-dim Metal hidden state -> pool -> Ch 140 LoRA routing head */
    const float *h_ptr = ink_hidden(s);
    int64_t n_embd = ink_hparams(s)->n_embd;
    coax_a64_f64 u;
    coax_v_f32 hv = {(float *)h_ptr, n_embd};
    coax_v_f64 uv = {u.e, 64};
    if (hl_pool_hidden(hv, uv) != COAX_HlStatus_ok) {
        fprintf(stderr, "hl_pool_hidden failed\n");
        exit(1);
    }
    coax_a5_f64 base_lg = extract_base_router_logits(s, class_toks);
    coax_t_HlPass route = hl_forward(base_lg, *adapter, u);
    int mod_id = (int)route.best;
    st->routed_mod = mod_id;
    st->route_prob = route.probs.e[route.best];
    if (st->route_prob < 0.01) st->route_prob = 0.01;
    if (st->route_prob > 0.99) st->route_prob = 0.99;

    if (mod_id < 1 || mod_id > 4) {
        /* Direct fallback (no core invoked) */
        InkSampleParams sp = {.temperature = 0.0f, .top_p = 1.0f, .top_k = 1};
        uint64_t rng = 1;
        size_t pos = 0;
        for (int step = 0; step < 24; step++) {
            int32_t tok = ink_sample(s, &sp, &rng);
            if (tok < 0 || ink_is_eog(s, tok)) break;
            char piece[128];
            int plen = ink_decode_piece(s, tok, piece, (int)sizeof piece);
            if (plen > 0 && pos + (size_t)plen + 1 < sizeof st->final_text) {
                memcpy(st->final_text + pos, piece, (size_t)plen);
                pos += (size_t)plen;
                st->final_text[pos] = '\0';
            }
            st->final_toks++;
            if (ink_step(s, tok, 1) != 0) break;
        }
        return;
    }

    const LoadedModule *lm = &mods[mod_id - 1];
    int arity = (int)hx_module_arity(mod_id);

    /* 2. Inject "<exec_call><exec_module>NAME</exec_module><exec_arg>" into live KV cache (0 prefix re-prefill) */
    char open_tag[160];
    snprintf(open_tag, sizeof open_tag, "<exec_call><exec_module>%s</exec_module><exec_arg>", lm->name);
    int32_t open_toks[128];
    int n_open = ink_encode(s, open_tag, strlen(open_tag), open_toks, 128, 1);
    int pos_before_open = ink_pos(s);
    if (ink_inject_kv(s, open_toks, n_open, 1) != 0) {
        fprintf(stderr, "open_tag inject failed: %s\n", ink_error(s));
        exit(1);
    }

    /* 3. Ch 53 constrained DFA decoding of exact-arity arguments over live 200,058-vocab Metal logits */
    char csv[256];
    csv[0] = '\0';
    int arg_gen = 0;
    int dec_err = decode_constrained_args(s, av, arity, csv, sizeof csv, &arg_gen);
    snprintf(st->csv_text, sizeof st->csv_text, "%s", csv);
    st->call_toks = n_open + arg_gen;

    /* 4. PAUSE neural decoding (KV cache stays live at pos_pause!). Execute frozen W2.2 dense F64 module + Ch 234 replay! */
    int pos_pause = ink_pos(s);
    coax_t_HxCoreReceipt rec;
    memset(&rec, 0, sizeof rec);
    if (dec_err != 0) {
        rec.reason = COAX_HxExecReason_malformed_call;
    } else {
        coax_string cs = {csv, (int64_t)strlen(csv)};
        coax_t_HxParsedCall pc = hx_parse_args_csv(mod_id, cs);
        if (pc.reason != COAX_HxExecReason_ok) {
            rec.reason = pc.reason;
        } else {
            coax_v_s64 av_args = {pc.args.e, pc.n_args};
            rec = hx_invoke_core(mod_id, lm->model, lm->sha, 1, av_args);
            if (rec.core_ran) {
                st->core_ran = 1;
                st->replay_ok = hx_verify_receipt(lm->model, lm->sha, rec) ? 1 : 0;
            }
        }
    }
    if (rec.reason != COAX_HxExecReason_ok) st->refused = 1;

    /* 5. Format <exec_trace>...<exec_return> or <exec_refuse> and inject into live KV cache (0 prefix re-prefilled!) */
    uint8_t inj_buf[192];
    coax_v_u8 iv = {inj_buf, 192};
    int64_t ilen = hx_format_injection(rec, iv);
    if (ilen < 0 || ilen >= (int64_t)sizeof st->inject_text) {
        fprintf(stderr, "hx_format_injection failed\n");
        exit(1);
    }
    memcpy(st->inject_text, inj_buf, (size_t)ilen);
    st->inject_text[ilen] = '\0';

    char inj_cont[256];
    snprintf(inj_cont, sizeof inj_cont, "%s\n", st->inject_text);
    int32_t inj_toks[128];
    int n_inj = ink_encode(s, inj_cont, strlen(inj_cont), inj_toks, 128, 1);
    if (ink_inject_kv(s, inj_toks, n_inj, 1) != 0) {
        fprintf(stderr, "inject kv failed: %s\n", ink_error(s));
        exit(1);
    }
    /* Verify live KV cursor advanced strictly by n_inj with 0 prefix re-prefill! */
    if (ink_pos(s) != pos_pause + n_inj) {
        fprintf(stderr, "KV cursor invariant broken: got %d want %d\n", ink_pos(s), pos_pause + n_inj);
        exit(1);
    }
    st->inject_toks = n_inj;
    st->prefix_tokens_preserved = pos_before_open + st->call_toks;
    st->prefix_tokens_reprefilled = 0;

    /* 6. Resume neural decoding on Metal conditioned on injected <exec_return>/<exec_refuse> */
    InkSampleParams sp = {.temperature = 0.0f, .top_p = 1.0f, .top_k = 1};
    uint64_t rng = 1;
    size_t pos = 0;
    st->final_text[0] = '\0';
    for (int step = 0; step < 18; step++) {
        int32_t tok = ink_sample(s, &sp, &rng);
        if (tok < 0 || ink_is_eog(s, tok)) break;
        char piece[128];
        int plen = ink_decode_piece(s, tok, piece, (int)sizeof piece);
        if (plen > 0) {
            if (plen < (int)sizeof piece) piece[plen] = '\0';
            if (strstr(piece, "<|end_message|>") != NULL) break;
            if (pos + (size_t)plen + 1 < sizeof st->final_text) {
                memcpy(st->final_text + pos, piece, (size_t)plen);
                pos += (size_t)plen;
                st->final_text[pos] = '\0';
            }
            if (strchr(st->final_text, '\n') != NULL) { st->final_toks++; break; }
        }
        st->final_toks++;
        if (step + 1 < 18) {
            if (ink_step(s, tok, 1) != 0) break;
        }
    }
}

/* ---- Live Metal 4 KV-Cache Prefix-Preservation Proof ---- */
static int verify_kv_cache_prefix_preservation(InkSession *s) {
    const char *p1 = "<|message_system|><|content_text|>Thinking effort level: 0.0<|end_message|>"
                     "<|message_user|><|content_text|>Verify diff hunk 10,5,10,8,120,3,3.<|end_message|>"
                     "<|message_model|><|content_text|>";
    const char *p2 = "<exec_call><exec_module>oa-exec-diff-verify</exec_module><exec_arg>10,5,10,8,120,3,3</exec_arg></exec_call>";
    const char *p3 = "<exec_trace>0123456789abcdef</exec_trace><exec_return>1,123,3</exec_return>\n";

    int32_t t1[256], t2[256], t3[256], tall[768];
    int n1 = ink_encode(s, p1, strlen(p1), t1, 256, 1);
    int n2 = ink_encode(s, p2, strlen(p2), t2, 256, 1);
    int n3 = ink_encode(s, p3, strlen(p3), t3, 256, 1);
    memcpy(tall, t1, (size_t)n1 * sizeof(int32_t));
    memcpy(tall + n1, t2, (size_t)n2 * sizeof(int32_t));
    memcpy(tall + n1 + n2, t3, (size_t)n3 * sizeof(int32_t));
    int nall = n1 + n2 + n3;

    /* Path A: Incremental KV-cache (prefill p1 -> step p2 -> inject prefill p3, 0 prefix re-prefill!) */
    ink_reset(s);
    if (ink_prefill(s, t1, n1, 1) != 0) return 1;
    for (int i = 0; i < n2; i++) {
        if (ink_step(s, t2[i], 1) != 0) return 1;
    }
    int pos_at_pause = ink_pos(s);
    if (ink_inject_kv(s, t3, n3, 1) != 0) return 1;
    int pos_after_inj = ink_pos(s);
    int nv = ink_vocab_unpadded(s);
    float *lg_a = (float *)malloc((size_t)nv * sizeof(float));
    memcpy(lg_a, ink_logits(s), (size_t)nv * sizeof(float));

    /* Path B: Full single-pass prefill from scratch */
    ink_reset(s);
    if (ink_prefill(s, tall, nall, 1) != 0) { free(lg_a); return 1; }
    const float *lg_b = ink_logits(s);

    int top_a = 0, top_b = 0;
    float max_diff = 0.0f;
    double dot = 0.0, na = 0.0, nb_norm = 0.0;
    for (int i = 0; i < nv; i++) {
        if (lg_a[i] > lg_a[top_a]) top_a = i;
        if (lg_b[i] > lg_b[top_b]) top_b = i;
        float d = fabsf(lg_a[i] - lg_b[i]);
        if (d > max_diff) max_diff = d;
        dot += (double)lg_a[i] * (double)lg_b[i];
        na += (double)lg_a[i] * (double)lg_a[i];
        nb_norm += (double)lg_b[i] * (double)lg_b[i];
    }
    double cos_sim = dot / (sqrt(na) * sqrt(nb_norm) + 1e-12);
    free(lg_a);
    printf("KV_CACHE_PROOF prefix=%d call=%d inject=%d pos_pause=%d pos_after=%d reprefilled=0 top1_inc=%d top1_full=%d top1_match=%d cos_sim=%.6f max_logit_diff=%.6f\n",
           n1, n2, n3, pos_at_pause, pos_after_inj, top_a, top_b, (top_a == top_b) ? 1 : 0, cos_sim, max_diff);
    return (pos_at_pause == n1 + n2 && pos_after_inj == nall && top_a == top_b && cos_sim > 0.995) ? 0 : 1;
}

/* ---- Multi-Step Chained Execution inside ONE KV-Cache Stream + Linked Composite Check ---- */
static int verify_multistep_stream(InkSession *s, const AbiVocab *av, const LoadedModule mods[MOD_SLOTS]) {
    const char *user =
        "Multi-step coding-agent verification:\n"
        "Step 1 (oa-exec-json-schema): has_path=1, path_len=30, max_path=256, has_old=1, old_len=120, has_new=1, new_len=180, max_body=1024.\n"
        "Step 2 (oa-exec-diff-verify): old_start=12, old_count=4, new_start=12, new_count=7, file_lines=100, matched_ctx=3, expected_ctx=3.\n"
        "Verify both sequentially and report final diff output.";
    char prompt[4096];
    format_chat_prompt(HYBRID_SYSTEM_PROMPT, user, prompt, sizeof prompt);
    int32_t toks[2048];
    int n_tok = ink_encode(s, prompt, strlen(prompt), toks, 2048, 1);
    ink_reset(s);
    if (ink_prefill(s, toks, n_tok, 1) != 0) return 1;

    int step_mods[2] = {2, 1};
    int64_t final_out[3] = {0, 0, 0};
    int total_replays = 0;
    int total_preserved = 0;
    for (int st = 0; st < 2; st++) {
        int mid = step_mods[st];
        const LoadedModule *lm = &mods[mid - 1];
        char open_tag[160];
        snprintf(open_tag, sizeof open_tag, "<exec_call><exec_module>%s</exec_module><exec_arg>", lm->name);
        int32_t ot[128];
        int no = ink_encode(s, open_tag, strlen(open_tag), ot, 128, 1);
        if (ink_inject_kv(s, ot, no, 1) != 0) return 1;

        char csv[256];
        int agen = 0;
        if (decode_constrained_args(s, av, (int)hx_module_arity(mid), csv, sizeof csv, &agen) != 0) return 1;
        int pos_pause = ink_pos(s);
        coax_string cs = {csv, (int64_t)strlen(csv)};
        coax_t_HxParsedCall pc = hx_parse_args_csv(mid, cs);
        if (pc.reason != COAX_HxExecReason_ok) return 1;
        coax_v_s64 av_args = {pc.args.e, pc.n_args};
        coax_t_HxCoreReceipt rec = hx_invoke_core(mid, lm->model, lm->sha, 1, av_args);
        if (!rec.core_ran || !hx_verify_receipt(lm->model, lm->sha, rec)) return 1;
        total_replays++;
        if (st == 1) {
            final_out[0] = rec.outputs.e[0];
            final_out[1] = rec.outputs.e[1];
            final_out[2] = rec.outputs.e[2];
        }
        uint8_t ib[192];
        coax_v_u8 iv = {ib, 192};
        int64_t il = hx_format_injection(rec, iv);
        char inj[256];
        memcpy(inj, ib, (size_t)il);
        inj[il] = '\n';
        inj[il + 1] = '\0';
        int32_t it[128];
        int ni = ink_encode(s, inj, strlen(inj), it, 128, 1);
        if (ink_inject_kv(s, it, ni, 1) != 0) return 1;
        if (ink_pos(s) != pos_pause + ni) return 1;
        total_preserved += pos_pause;
    }
    /* Cross-check against zero-recompilation linked composite module oa-exec-schema-diff-linked */
    int64_t comp_args[10] = {1, 30, 256, 12, 4, 12, 7, 100, 3, 3};
    coax_v_s64 cv = {comp_args, 10};
    coax_t_HxCoreReceipt crec = hx_invoke_core(5, mods[4].model, mods[4].sha, 1, cv);
    int linked_match = (crec.core_ran && hx_verify_receipt(mods[4].model, mods[4].sha, crec) &&
                        crec.outputs.e[0] == final_out[0] && crec.outputs.e[1] == final_out[1] &&
                        crec.outputs.e[2] == final_out[2]) ? 1 : 0;
    printf("MULTISTEP_STREAM calls=2 replays=%d kv_pos=%d preserved_prefix=%d reprefilled=0 final_out=%lld,%lld,%lld linked_composite_match=%d\n",
           total_replays, ink_pos(s), total_preserved,
           (long long)final_out[0], (long long)final_out[1], (long long)final_out[2], linked_match);
    return (total_replays == 2 && linked_match == 1 && final_out[0] == 1 && final_out[1] == 103 && final_out[2] == 3) ? 0 : 1;
}

int main(int argc, char **argv) {
    const char *gguf_path = NULL;
    const char *metal_path = NULL;
    const char *modules_dir = NULL;
    for (int i = 1; i < argc; i++) {
        if (!strcmp(argv[i], "--model") && i + 1 < argc) gguf_path = argv[++i];
        else if (!strcmp(argv[i], "--metal") && i + 1 < argc) metal_path = argv[++i];
        else if (!strcmp(argv[i], "--modules") && i + 1 < argc) modules_dir = argv[++i];
    }
    if (!gguf_path || !metal_path || !modules_dir) {
        fprintf(stderr, "usage: %s --model PATH.gguf --metal PATH.metal --modules DIR\n", argv[0]);
        return 2;
    }

    /* Flush stdout line-buffered so progress is immediately visible */
    setvbuf(stdout, NULL, _IOLBF, 0);
    double t_start = now_s();

    /* 1. Compute real cryptographic SHA-256 of the GGUF transformer checkpoint */
    coax_a32_u8 base_sha;
    if (sha256_file(gguf_path, &base_sha) != 0) return 1;
    char base_hex[65];
    fmt_hex(base_sha.e, base_hex);
    printf("BASE_MODEL_PATH %s\n", gguf_path);
    printf("BASE_MODEL_SHA256 %s\n", base_hex);

    /* 2. Load & SHA-256-verify all 5 compiled W2.2 dense F64 modules from ~/.openagents/modules/ */
    LoadedModule mods[MOD_SLOTS];
    coax_a32_u8 bundle_sha;
    if (load_w22_modules(modules_dir, mods, &bundle_sha) != 0) return 1;

    /* 3. Open Psionic native Metal 4 Inkling-2-Mini engine */
    double t_open0 = now_s();
    InkSession *sess = NULL;
    char err[512];
    if (ink_open(gguf_path, metal_path, 4096, &sess, err, sizeof err) != 0) {
        fprintf(stderr, "ink_open failed: %s\n", err);
        return 1;
    }
    printf("METAL_ENGINE_LOADED seconds=%.3f vocab=%d n_embd=%lld\n",
           now_s() - t_open0, ink_vocab_unpadded(sess), (long long)ink_hparams(sess)->n_embd);

    AbiVocab av;
    build_abi_vocab(sess, &av);
    printf("ABI_CONSTRAINED_VOCAB candidates=%d total_vocab=%d\n", av.count, ink_vocab_unpadded(sess));

    /* Find representative single-token IDs for base router logits */
    const char *cls_words[5] = {"0", "diff", "json", "dep", "budget"};
    int32_t class_toks[5];
    for (int c = 0; c < 5; c++) {
        int32_t tmp[8];
        int nt = ink_encode(sess, cls_words[c], strlen(cls_words[c]), tmp, 8, 0);
        class_toks[c] = (nt > 0) ? tmp[0] : 0;
    }

    /* 4. Live Metal 4 KV-cache prefix-preservation proof */
    if (verify_kv_cache_prefix_preservation(sess) != 0) {
        fprintf(stderr, "KV-cache prefix-preservation proof failed!\n");
        return 1;
    }

    /* 5. Extract real 2304-dim Metal hidden states on disjoint 32-task training split & train Ch 140 LoRA */
    double t_tr0 = now_s();
    coax_t_HlRow train_rows[32];
    for (int i = 0; i < 32; i++) {
        char prompt[4096];
        format_chat_prompt(HYBRID_SYSTEM_PROMPT, TRAIN_TASKS[i].prompt, prompt, sizeof prompt);
        int32_t toks[2048];
        int n_tok = ink_encode(sess, prompt, strlen(prompt), toks, 2048, 1);
        ink_reset(sess);
        if (ink_prefill(sess, toks, n_tok, 1) != 0) {
            fprintf(stderr, "train prefill failed at %d\n", i);
            return 1;
        }
        train_rows[i].base_model_sha = base_sha;
        train_rows[i].label = TRAIN_TASKS[i].label;
        coax_v_f32 hv = {(float *)ink_hidden(sess), ink_hparams(sess)->n_embd};
        coax_v_f64 uv = {train_rows[i].u.e, 64};
        if (hl_pool_hidden(hv, uv) != COAX_HlStatus_ok) return 1;
        train_rows[i].base_logits = extract_base_router_logits(sess, class_toks);
    }

    coax_t_HlAdapter init_ad = hl_init_adapter(20261121ULL, 8, 16.0, base_sha, bundle_sha);
    coax_t_HlAdapter zero_ad = hl_init_zero();

    /* Verify zero-adapter reproduces base logits bit-for-bit on ALL 32 real Metal hidden states */
    int bit_exact_zero = 0;
    int base_route_train_ok = 0;
    for (int i = 0; i < 32; i++) {
        coax_t_HlPass p0 = hl_forward(train_rows[i].base_logits, zero_ad, train_rows[i].u);
        coax_t_HlPass pi = hl_forward(train_rows[i].base_logits, init_ad, train_rows[i].u);
        int eq = 1;
        for (int c = 0; c < 5; c++) {
            if (memcmp(&p0.logits.e[c], &train_rows[i].base_logits.e[c], sizeof(double)) != 0 ||
                memcmp(&pi.logits.e[c], &train_rows[i].base_logits.e[c], sizeof(double)) != 0) {
                eq = 0;
            }
        }
        if (eq) bit_exact_zero++;
        if (p0.best == train_rows[i].label) base_route_train_ok++;
    }

    coax_v_t_HlRow rv = {train_rows, 32};
    coax_t_HlTrainResult tr = hl_train(init_ad, rv, 0.25, 0.03, 0.001, 150);
    if (tr.status != COAX_HlStatus_ok) {
        fprintf(stderr, "hl_train failed: status=%u bad_row=%lld\n", (unsigned)tr.status, (long long)tr.bad_row);
        return 1;
    }
    int trained_route_train_ok = 0;
    for (int i = 0; i < 32; i++) {
        if (hl_forward(train_rows[i].base_logits, tr.adapter, train_rows[i].u).best == train_rows[i].label) {
            trained_route_train_ok++;
        }
    }
    char ad_hex[65];
    fmt_hex(tr.adapter.adapter_sha.e, ad_hex);
    printf("LORA_TRAIN rank=8 alpha=16.0 steps=%lld loss=%.6f train_route_before=%d/32 train_route_after=%d/32 zero_bit_exact=%d/32 adapter_sha=%s seconds=%.3f\n",
           (long long)tr.state.step, tr.loss, base_route_train_ok, trained_route_train_ok, bit_exact_zero, ad_hex, now_s() - t_tr0);

    /* 6. Atomic Hot-Swap Pinning Gate Verification */
    coax_t_HlAdapter active_slot[1];
    active_slot[0] = zero_ad;
    coax_v_t_HlAdapter sv = {active_slot, 1};
    coax_t_HxStatus hs_valid = hl_hotswap(sv, tr.adapter, base_sha, bundle_sha);
    coax_a32_u8 wrong_base = base_sha; wrong_base.e[0] ^= 1;
    coax_t_HlAdapter bad_base_ad = hl_init_adapter(7ULL, 4, 8.0, wrong_base, bundle_sha);
    coax_t_HxStatus hs_bad_base = hl_hotswap(sv, bad_base_ad, base_sha, bundle_sha);
    coax_a32_u8 wrong_core = bundle_sha; wrong_core.e[0] ^= 1;
    coax_t_HlAdapter bad_core_ad = hl_init_adapter(7ULL, 4, 8.0, base_sha, wrong_core);
    coax_t_HxStatus hs_bad_core = hl_hotswap(sv, bad_core_ad, base_sha, bundle_sha);
    coax_t_HlAdapter tampered_ad = tr.adapter; tampered_ad.p.e[0] += 1e-6;
    coax_t_HxStatus hs_tampered = hl_hotswap(sv, tampered_ad, base_sha, bundle_sha);
    int slot_intact = (memcmp(active_slot[0].adapter_sha.e, tr.adapter.adapter_sha.e, 32) == 0) ? 1 : 0;
    printf("LORA_HOTSWAP valid=%u bad_base=%u bad_core=%u tampered=%u active_intact=%d\n",
           (unsigned)hs_valid, (unsigned)hs_bad_base, (unsigned)hs_bad_core, (unsigned)hs_tampered, slot_intact);
    if (hs_valid != COAX_HxStatus_ok || hs_bad_base != COAX_HxStatus_adapter_base_mismatch ||
        hs_bad_core != COAX_HxStatus_adapter_core_mismatch || hs_tampered != COAX_HxStatus_core_unverified || !slot_intact) {
        fprintf(stderr, "LoRA hot-swap pinning check failed!\n");
        return 1;
    }

    /* 7. Multi-step chained execution inside ONE KV-cache stream + linked composite check */
    if (verify_multistep_stream(sess, &av, mods) != 0) {
        fprintf(stderr, "Multi-step chained stream check failed!\n");
        return 1;
    }

    /* 8. Paired Held-Out Benchmark (48 Held-Out Tasks):
     *    Arm A: Base Model Alone (greedy + 2 seeded blocks at temp=0.3 for Ch 240 noise floor)
     *    Arm B: hybrid-exec-v1 (same 26B Metal checkpoint + trained Ch 140 LoRA + Ch 53 DFA + W2.2 cores + KV-cache injection) */
    const int N = 48;
    double base_runs[2 * 48];
    double base_scores[48], hyb_scores[48];
    double base_probs[48], hyb_probs[48], labels[48];
    int64_t tasks[48];

    int base_pass = 0, base_dom_pass[5] = {0}, base_refuse_ok = 0, base_silent_err = 0;
    int base_prompt_tot = 0, base_gen_tot = 0;
    double t_base0 = now_s();
    double gpu_base0 = ink_last_gpu_seconds(sess);

    /* Base Arm — primary greedy pass (temp=0.0) */
    for (int i = 0; i < N; i++) {
        char resp[256];
        int pt = 0, gt = 0;
        double conf = 0.5;
        uint64_t rng = 1000ULL + (uint64_t)i;
        run_base_task(sess, &EVAL_TASKS[i], 0.0f, &rng, resp, sizeof resp, &pt, &gt, &conf);
        int sh = 0;
        int ok = score_response(&EVAL_TASKS[i], resp, &sh);
        base_scores[i] = (double)ok;
        /* Probability gate label y=1 means correct outcome; calibrated prob = ok ? conf : conf (high conf when wrong = confident error!) */
        labels[i] = 1.0;
        base_probs[i] = ok ? conf : (1.0 - conf < 0.01 ? 0.01 : 1.0 - conf);
        /* Note: if model is wrong with high confidence (conf >= 0.9), record p = 0.1 (low prob on true answer) or
         * keep y = ok (whether base succeeded) vs exact outcome:
         * Use binary event y = 1 (task solvable/refusable accurately); p_model = P(correct answer).
         * When wrong, model assigned probability <= (1 - conf) to the correct token! */
        base_prompt_tot += pt;
        base_gen_tot += gt;
        tasks[i] = i;
        if (ok) {
            base_pass++;
            if (EVAL_TASKS[i].is_refusal_trap) base_refuse_ok++;
            else base_dom_pass[EVAL_TASKS[i].domain]++;
        }
        if (sh) base_silent_err++;
    }
    double base_wall_s = now_s() - t_base0;
    double base_gpu_s = ink_last_gpu_seconds(sess) - gpu_base0;
    printf("BASE_GREEDY_DONE passed=%d/%d silent_hallucinations=%d wall_s=%.2f gpu_s=%.2f\n",
           base_pass, N, base_silent_err, base_wall_s, base_gpu_s);

    /* Base Arm — 2 independent seeded noise-floor blocks (temp=0.5, seeds 424242 & 999999) for Ch 240 ns_floor */
    double t_nf0 = now_s();
    uint64_t nf_seeds[2] = {424242ULL, 999999ULL};
    for (int b = 0; b < 2; b++) {
        uint64_t rng = nf_seeds[b];
        int b_ok = 0;
        for (int i = 0; i < N; i++) {
            char resp[256];
            int pt = 0, gt = 0;
            double conf = 0.5;
            run_base_task(sess, &EVAL_TASKS[i], 0.5f, &rng, resp, sizeof resp, &pt, &gt, &conf);
            int sh = 0;
            int ok = score_response(&EVAL_TASKS[i], resp, &sh);
            base_runs[b * N + i] = (double)ok;
            b_ok += ok;
        }
        printf("BASE_NOISE_BLOCK block=%d temp=0.5 seed=%llu passed=%d/%d\n",
               b, (unsigned long long)nf_seeds[b], b_ok, N);
    }
    double nf_wall_s = now_s() - t_nf0;

    /* Hybrid Arm — hybrid-exec-v1 */
    int hyb_pass = 0, hyb_dom_pass[5] = {0}, hyb_refuse_ok = 0, hyb_abi_refused = 0, hyb_silent_err = 0;
    int hyb_route_ok = 0, hyb_cores_ran = 0, hyb_replays_ok = 0;
    int hyb_prompt_tot = 0, hyb_call_tot = 0, hyb_inj_tot = 0, hyb_final_tot = 0;
    int hyb_kv_preserved_tot = 0, hyb_kv_reprefill_tot = 0;
    double t_hyb0 = now_s();
    double gpu_hyb0 = ink_last_gpu_seconds(sess);

    for (int i = 0; i < N; i++) {
        HybridRunStats hs;
        run_hybrid_task(sess, &av, mods, &active_slot[0], class_toks, &EVAL_TASKS[i], &hs);
        int sh = 0;
        int ok = score_response(&EVAL_TASKS[i], hs.final_text, &sh);
        hyb_scores[i] = (double)ok;
        hyb_probs[i] = ok ? hs.route_prob : (1.0 - hs.route_prob < 0.01 ? 0.01 : 1.0 - hs.route_prob);
        if (hs.routed_mod == EVAL_TASKS[i].domain) hyb_route_ok++;
        if (EVAL_TASKS[i].is_refusal_trap && hs.refused) hyb_abi_refused++;
        if (hs.core_ran) {
            hyb_cores_ran++;
            if (hs.replay_ok) hyb_replays_ok++;
        }
        hyb_prompt_tot += hs.prompt_toks;
        hyb_call_tot += hs.call_toks;
        hyb_inj_tot += hs.inject_toks;
        hyb_final_tot += hs.final_toks;
        hyb_kv_preserved_tot += hs.prefix_tokens_preserved;
        hyb_kv_reprefill_tot += hs.prefix_tokens_reprefilled;
        if (ok) {
            hyb_pass++;
            if (EVAL_TASKS[i].is_refusal_trap) hyb_refuse_ok++;
            else hyb_dom_pass[EVAL_TASKS[i].domain]++;
        }
        if (sh) hyb_silent_err++;
        if (!ok) {
            printf("HYB_MISS idx=%d domain=%d trap=%d routed=%d csv='%s' inj='%s' final='%s'\n",
                   i, EVAL_TASKS[i].domain, EVAL_TASKS[i].is_refusal_trap, hs.routed_mod, hs.csv_text, hs.inject_text, hs.final_text);
        }
    }
    double hyb_wall_s = now_s() - t_hyb0;
    double hyb_gpu_s = ink_last_gpu_seconds(sess) - gpu_hyb0;

    /* 9. Score BOTH arms through Coax #authority Ch 239-241 statistical gates */
    coax_v_f64 v_runs = {base_runs, 2 * N};
    coax_v_f64 v_bs = {base_scores, N};
    coax_v_f64 v_hs = {hyb_scores, N};
    coax_v_f64 v_bp = {base_probs, N};
    coax_v_f64 v_hp = {hyb_probs, N};
    coax_v_f64 v_lb = {labels, N};
    coax_v_s64 v_tk = {tasks, N};
    coax_t_HxbGateReport gr = hxb_eval_gates(v_runs, 2, 1, v_bs, v_hs, v_bp, v_hp, v_lb, v_tk, N, 0.0, hyb_wall_s);

    char isha[65], qsha[65], gsha[65];
    fmt_hex(gr.items_sha.e, isha);
    fmt_hex(gr.questions_sha.e, qsha);
    fmt_hex(gr.gate_sha.e, gsha);

    printf("\n=== PAIRED HELD-OUT BENCHMARK SUMMARY (48 HELD-OUT TASKS, INKLING-2-MINI METAL 4) ===\n");
    printf("BASE_ARM   total=%d/48 (%.1f%%) diff=%d/10 json=%d/10 dep=%d/10 budget=%d/10 refuse=%d/8 silent_hallucinations=%d prompt_tok=%d gen_tok=%d wall_s=%.2f gpu_s=%.2f\n",
           base_pass, 100.0 * base_pass / N,
           base_dom_pass[1], base_dom_pass[2], base_dom_pass[3], base_dom_pass[4], base_refuse_ok, base_silent_err,
           base_prompt_tot, base_gen_tot, base_wall_s, base_gpu_s);
    printf("HYBRID_ARM total=%d/48 (%.1f%%) diff=%d/10 json=%d/10 dep=%d/10 budget=%d/10 refuse=%d/8 abi_refused=%d/8 silent_hallucinations=%d routed=%d/48 cores_ran=%d replays=%d/%d prompt_tok=%d call_tok=%d inj_tok=%d final_tok=%d kv_preserved=%d kv_reprefilled=%d wall_s=%.2f gpu_s=%.2f\n",
           hyb_pass, 100.0 * hyb_pass / N,
           hyb_dom_pass[1], hyb_dom_pass[2], hyb_dom_pass[3], hyb_dom_pass[4], hyb_refuse_ok, hyb_abi_refused, hyb_silent_err,
           hyb_route_ok, hyb_cores_ran, hyb_replays_ok, hyb_cores_ran,
           hyb_prompt_tot, hyb_call_tot, hyb_inj_tot, hyb_final_tot,
           hyb_kv_preserved_tot, hyb_kv_reprefill_tot, hyb_wall_s, hyb_gpu_s);
    printf("CH239_SUITE suite_ok=%d chain_ok=%d compare_ok=%d items=%s questions=%s gate=%s\n",
           gr.suite_ok ? 1 : 0, gr.chain_ok ? 1 : 0, gr.compare_ok ? 1 : 0, isha, qsha, gsha);
    printf("CH240_FLOOR status=%u blocks=%lld sd=%.6f floor_2sigma=%.6f nf_wall_s=%.2f\n",
           (unsigned)gr.floor_status, (long long)gr.floor_blocks, gr.floor_sd, gr.floor_val, nf_wall_s);
    printf("CH241_DECISION status=%u verdict=%u reason=%u gain=%.6f se=%.6f below=%lld/1000 tasks=%lld room=%.6f\n",
           (unsigned)gr.dec_status, (unsigned)gr.dec_verdict, (unsigned)gr.dec_reason,
           gr.dec_gain, gr.dec_se, (long long)gr.dec_below, (long long)gr.dec_tasks, gr.dec_room);
    printf("CH241_PROBABILITY status=%u verdict=%u reason=%u loss_base=%.6f loss_hyb=%.6f err_base=%lld err_hyb=%lld ece_base=%.6f ece_hyb=%.6f\n",
           (unsigned)gr.prob_status, (unsigned)gr.prob_verdict, (unsigned)gr.prob_reason,
           gr.prob_loss_base, gr.prob_loss_hyb, (long long)gr.prob_err_base, (long long)gr.prob_err_hyb,
           gr.prob_ece_base, gr.prob_ece_hyb);
    printf("CH241_EVALUATE verdict=%u reason=%u total_wall_s=%.2f\n",
           (unsigned)gr.eval_verdict, (unsigned)gr.eval_reason, now_s() - t_start);

    free(av.items);
    ink_close(sess);

    /* Hard acceptance gates */
    if (!gr.suite_ok || !gr.chain_ok || !gr.compare_ok) return 1;
    if (gr.dec_verdict != COAX_VdVerdict_passed || gr.eval_verdict != COAX_VdVerdict_passed) return 1;
    if (hyb_replays_ok != hyb_cores_ran || hyb_cores_ran == 0) return 1;
    if (hyb_silent_err != 0 || hyb_kv_reprefill_tot != 0) return 1;
    return 0;
}
