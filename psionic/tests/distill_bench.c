/* Chapter 223: is the distilled student still within chapter 222's latency
   gate? The host reads the monotonic clock and writes the receipt; the
   distillation, the forward pass, parity, the card, percentiles and the
   verdict are Coax (psionic/lib/distill.coax and fast-encoder.coax).
   Usage: distill_bench RECEIPT RUNS BUILD COMMIT MACHINE
   Trains the student from the recorded teacher, installs its head, then
   runs chapter 222's frozen workload: parity and the card on 8 requests of
   512 token IDs, one untimed decision per engine, and RUNS interleaved pairs
   whose order alternates. Exit status: 0 when the gate is met, 1 when it is
   missed, 5 when refused. */
#define _POSIX_C_SOURCE 199309L
#include "distill.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

#define T 512
#define REQ 8
#define MAXRUNS 1024

static double w[8848], low[64], step[64], f[T * 192], feats[8192], teacher[1536], head[48];
static int64_t labels[512];
static coax_t_PkSplit split[512];
static uint8_t emb[1024];
static int8_t codes[7680], acodes[4096];
static float scales[480], offsets[480], a[4096], ascales[256];
static int32_t asums[256], dots[8192];
static int64_t ids[REQ][T], q8ns[MAXRUNS], refns[MAXRUNS];
static double ref[REQ * 9], der[REQ * 9], pref[REQ * 9], pder[REQ * 9], z[9];
static coax_t_PqItem items[REQ * 3];
static coax_t_FeQ8 q;
static coax_t_FeWork k;

static uint64_t splitmix(uint64_t seed, uint64_t counter) {
    uint64_t x = seed + 0x9e3779b97f4a7c15ull * (counter + 1);
    x = (x ^ (x >> 30)) * 0xbf58476d1ce4e5b9ull;
    x = (x ^ (x >> 27)) * 0x94d049bb133111ebull;
    return x ^ (x >> 31);
}
static int64_t now(void) {
    struct timespec t;
    clock_gettime(CLOCK_MONOTONIC, &t);
    return (int64_t)t.tv_sec * 1000000000 + t.tv_nsec;
}
static int64_t decide(coax_t_FeEngine e, int r, double *out) {
    int64_t t0 = now();
    coax_t_FeStatus s = fe_forward(e, (coax_v_f64){w, 8848}, q, k, (coax_v_s64){ids[r], T}, (coax_v_f64){out, 9});
    int64_t t1 = now();
    if (s != COAX_FeStatus_ok) { fprintf(stderr, "forward refused: %d\n", (int)s); exit(5); }
    return t1 - t0;
}
static void summary(FILE *o, const char *name, int64_t *x, int n) {
    coax_t_CmStats s = cm_welford((coax_v_s64){x, n});
    fprintf(o, "  \"%s\": {\"n\": %d, \"p50_ns\": %lld, \"p99_ns\": %lld, \"mean_ns\": %.1f, \"sd_ns\": %.1f},\n", name, n,
            (long long)fe_rank((coax_v_s64){x, n}, 50), (long long)fe_rank((coax_v_s64){x, n}, 99), s.mean, s.sd);
}
static void series(FILE *o, const char *name, const int64_t *x, int n, const char *end) {
    fprintf(o, "  \"%s\": [", name);
    for (int i = 0; i < n; i++) fprintf(o, "%s%lld", i ? "," : "", (long long)x[i]);
    fprintf(o, "]%s\n", end);
}

int main(int argc, char **argv) {
    if (argc != 6) { fprintf(stderr, "usage: see the header comment\n"); return 2; }
    int runs = atoi(argv[2]);
    if (runs < 1 || runs > MAXRUNS) return 2;
    if (fe_weights((coax_v_f64){w, 8848}) != COAX_FeStatus_ok) return 5;
    uint64_t base = fe_model_id((coax_v_f64){w, 8848});
    k = (coax_t_FeWork){{f, T * 192}, {a, 4096}, {acodes, 4096}, {ascales, 256}, {asums, 256}, {dots, 8192}};
    coax_t_DsData d = {{feats, 8192}, {teacher, 1536}, {labels, 512}, {split, 512}};
    if (ds_recorded((coax_v_f64){teacher, 1536}, (coax_v_s64){labels, 512}) != COAX_DsStatus_ok) return 5;
    if (ds_data((coax_v_f64){w, 8848}, k, d, 0, 512) != COAX_DsStatus_ok) return 5;
    coax_t_DsRun run = ds_train(d, 2.0, ds_config(), (coax_v_f64){head, 48});
    if (run.status != COAX_DsStatus_ok) return 5;
    if (ds_install((coax_v_f64){w, 8848}, (coax_v_f64){head, 48}) != COAX_DsStatus_ok) return 5;
    q = (coax_t_FeQ8){{emb, 1024}, {low, 64}, {step, 64}, {codes, 7680}, {scales, 480}, {offsets, 480}};
    if (fe_quantize((coax_v_f64){w, 8848}, q) != COAX_FeStatus_ok) return 5;
    /* Chapter 222's requests: SplitMix64 seed 2230 + r, word i mod 64. */
    for (int r = 0; r < REQ; r++)
        for (int i = 0; i < T; i++) ids[r][i] = (int64_t)(splitmix(2230 + r, i) % 64);
    for (int r = 0; r < REQ; r++) { decide(COAX_FeEngine_reference, r, ref + 9 * r); decide(COAX_FeEngine_q8, r, der + 9 * r); }
    coax_t_FeParity p = fe_parity((coax_v_f64){ref, REQ * 9}, (coax_v_f64){der, REQ * 9}, REQ);
    for (int r = 0; r < REQ; r++) {
        fe_probs((coax_v_f64){ref + 9 * r, 9}, (coax_v_f64){pref + 9 * r, 9});
        fe_probs((coax_v_f64){der + 9 * r, 9}, (coax_v_f64){pder + 9 * r, 9});
    }
    coax_t_QzCard card = fe_card((coax_v_f64){pref, REQ * 9}, (coax_v_f64){pder, REQ * 9}, REQ, (coax_v_t_PqItem){items, REQ * 3});
    decide(COAX_FeEngine_q8, 0, z);
    decide(COAX_FeEngine_reference, 0, z);
    for (int i = 0; i < runs; i++) {
        int r = i % REQ;
        if (i % 2 == 0) { q8ns[i] = decide(COAX_FeEngine_q8, r, z); refns[i] = decide(COAX_FeEngine_reference, r, z); }
        else { refns[i] = decide(COAX_FeEngine_reference, r, z); q8ns[i] = decide(COAX_FeEngine_q8, r, z); }
    }
    coax_t_FeGate g = fe_gate(COAX_FeEngine_q8, T, p, card, (coax_v_s64){q8ns, runs}, 20000000);
    static const char *verdicts[] = {"refused", "met", "missed"};
    static const char *reasons[] = {"none", "engine", "workload", "parity", "card", "samples"};
    FILE *o = fopen(argv[1], "w");
    if (!o) return 2;
    fprintf(o, "{\n  \"profile\": \"fe-tiny-v1 with head 1 distilled (chapter 223)\",\n  \"base_model_id\": \"%016llx\",\n  \"student_model_id\": \"%016llx\",\n",
            (unsigned long long)base, (unsigned long long)fe_model_id((coax_v_f64){w, 8848}));
    fprintf(o, "  \"training\": {\"steps\": %lld, \"evaluations\": %lld, \"stopped\": %s, \"best_development_loss\": %.17g},\n",
            (long long)run.steps, (long long)run.evals, run.stopped ? "true" : "false", run.best);
    fprintf(o, "  \"engines\": [\"fe-q8-cpu-v1\", \"fe-f64-reference-v1\"],\n  \"tokens\": %d,\n  \"multiply_adds\": %lld,\n", T, (long long)fe_macs(T));
    fprintf(o, "  \"build\": \"%s\",\n  \"commit\": \"%s\",\n  \"machine\": \"%s\",\n  \"threads\": 1,\n", argv[3], argv[4], argv[5]);
    fprintf(o, "  \"parity\": {\"decisions\": %lld, \"equal\": %lld, \"worst_logit_difference\": %.17g, \"bound\": 0.125, \"ok\": %s},\n",
            (long long)p.decisions, (long long)p.equal, p.worst, p.ok ? "true" : "false");
    fprintf(o, "  \"card\": {\"items\": %lld, \"agreement\": %.17g, \"agreement_low\": %.17g, \"kl\": %.17g, \"kl_high\": %.17g, \"promoted\": %s},\n",
            (long long)card.items, card.agreement, card.agreement_low, card.kl, card.kl_high, card.promoted ? "true" : "false");
    summary(o, "warm_q8", q8ns, runs);
    summary(o, "warm_reference", refns, runs);
    fprintf(o, "  \"gate\": {\"engine\": \"fe-q8-cpu-v1\", \"p99_limit_ns\": %lld, \"verdict\": \"%s\", \"reason\": \"%s\", \"p50_ns\": %lld, \"p99_ns\": %lld},\n",
            (long long)g.limit, verdicts[g.verdict], reasons[g.reason], (long long)g.p50, (long long)g.p99);
    series(o, "warm_q8_ns", q8ns, runs, ",");
    series(o, "warm_reference_ns", refns, runs, "");
    fprintf(o, "}\n");
    fclose(o);
    printf("student %016llx: parity %s (%lld of %lld, worst %.3g); card %s; q8 warm P50 %.3f ms P99 %.3f ms; reference warm P50 %.3f ms; gate %s (%s)\n",
           (unsigned long long)fe_model_id((coax_v_f64){w, 8848}), p.ok ? "pass" : "fail", (long long)p.equal, (long long)p.decisions, p.worst,
           card.promoted ? "promoted" : "not promoted", g.p50 / 1e6, g.p99 / 1e6, fe_rank((coax_v_s64){refns, runs}, 50) / 1e6,
           verdicts[g.verdict], reasons[g.reason]);
    return g.verdict == COAX_FeVerdict_met ? 0 : g.verdict == COAX_FeVerdict_missed ? 1 : 5;
}
