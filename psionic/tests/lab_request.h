/* Chapter 36: the Lab-style request (five typed questions about one
   message, sharing its prefix block) on a placed CUDA engine, four ways:
   whole decisions, sequential forks (chapter 23), one batched suffix pass
   (dp_plan_batch), and the batched pass again with the prefix kept from the
   previous request. Every round but the last starts without a kept prefix.
   The batched logits must equal the sequential forks' bit for bit; the
   forks must give the whole decisions' answers within 0.0005.
   Included by elephant_device.c and mammoth_device.c. */
#ifndef PSIONIC_LAB_REQUEST_H
#define PSIONIC_LAB_REQUEST_H
#include <math.h>
#include <stdio.h>
#include <string.h>
#include <time.h>

#define LAB_QUESTIONS 5
static const char *lab_questions[LAB_QUESTIONS] = {"Is the water wet?", "Is the message about water?", "Is the message a question?", "Is the message about fire?", "Is the message true?"};
static double lab_now(void) { struct timespec t; clock_gettime(CLOCK_MONOTONIC, &t); return (double)t.tv_sec + 1e-9 * (double)t.tv_nsec; }

/* prepare: the question's tokens about "Water is wet.", their count and prefix; 0 on success.
   read: p_yes and the choice from one question's logits; 0 on success. */
typedef int (*LabPrepare)(const char *question, int64_t *tokens, int64_t *written, int64_t *prefix);
typedef int (*LabRead)(const float *logits, double *p_yes, int *best);

static int lab_request(EgEngine *e, LabPrepare prepare, LabRead read, int rounds) {
    static int64_t qt[LAB_QUESTIONS][EG_MAX_TOKENS];
    static float out[4][LAB_QUESTIONS][EG_VOCAB];
    int64_t counts[LAB_QUESTIONS], prefix = 0;
    const int64_t *rows[LAB_QUESTIONS];
    for (int i = 0; i < LAB_QUESTIONS; i++) {
        int64_t p;
        if (prepare(lab_questions[i], qt[i], &counts[i], &p) || counts[i] > e->max_tokens || (i && p != prefix)) { fprintf(stderr, "lab question %d refused\n", i); return 1; }
        prefix = p;
        rows[i] = qt[i];
    }
    static const char *names[4] = {"whole decisions", "forked, one at a time", "batched, one suffix pass", "batched, prefix kept from the last request"};
    double best_ms[4] = {0};
    for (int mode = 0; mode < 4; mode++) {
        for (int round = 0; round < rounds; round++) {
            if (mode < 3) e->fork_length = 0; /* a request starts without a kept prefix */
            int64_t passes = e->prefix_passes;
            double t = lab_now();
            if (mode >= 2) {
                int r = eg_decide_batch(e, rows, counts, LAB_QUESTIONS, prefix, &out[mode][0][0], NULL, NULL);
                if (r || e->batched != LAB_QUESTIONS) { fprintf(stderr, "batched lab request status %d (batched %lld): %s\n", r, (long long)e->batched, eg_log()); return 1; }
            } else {
                for (int i = 0; i < LAB_QUESTIONS; i++) {
                    int r = mode ? eg_decide_fork(e, qt[i], counts[i], prefix, NULL, NULL) : eg_decide(e, qt[i], counts[i], prefix, NULL, NULL);
                    if (r) { fprintf(stderr, "lab question %d failed: %s\n", i, eg_log()); return 1; }
                    memcpy(out[mode][i], e->logits, sizeof out[mode][i]);
                }
            }
            double ms = (lab_now() - t) * 1e3;
            if (!round || ms < best_ms[mode]) best_ms[mode] = ms;
            printf("lab request (%s), round %d: five questions in %.1f ms, %lld prefix passes\n", names[mode], round + 1, ms, (long long)(e->prefix_passes - passes));
            fflush(stdout);
        }
    }
    int failed = 0;
    for (int i = 0; i < LAB_QUESTIONS; i++) {
        double p[4];
        int best[4];
        for (int mode = 0; mode < 4; mode++) if (read(out[mode][i], &p[mode], &best[mode])) return 1;
        int64_t differ = 0, kept = 0;
        for (int64_t v = 0; v < EG_VOCAB; v++) {
            if (memcmp(&out[1][i][v], &out[2][i][v], 4)) differ++;
            if (memcmp(&out[2][i][v], &out[3][i][v], 4)) kept++;
        }
        printf("  q%d %-28s tokens %lld prefix %lld: %s p_yes %.9g (forked %.9g, batched %.9g); batched vs forked: %lld of %d logits differ, kept prefix: %lld\n",
               i + 1, lab_questions[i], (long long)counts[i], (long long)prefix, best[0] == 0 ? "yes" : "no", p[0], p[1], p[2], (long long)differ, EG_VOCAB, (long long)kept);
        if (differ || kept || best[1] != best[2] || best[0] != best[1] || fabs(p[0] - p[1]) > 0.0005) failed = 1;
    }
    printf("lab request best of %d: whole %.1f ms, forked %.1f ms, batched %.1f ms, batched with the prefix kept %.1f ms\n", rounds, best_ms[0], best_ms[1], best_ms[2], best_ms[3]);
    if (failed) fprintf(stderr, "batched lab answers differ\n");
    return failed;
}
#endif
