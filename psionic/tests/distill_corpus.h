/* Chapter 141: record a yes-or-no decision for every request of a corpus on
   a placed CUDA engine, for distillation targets and student features.
   The corpus is psionic/fixtures/distill-mammoth/corpus.tsv: lines of
   item, family, gold, message and question separated by tabs; lines that
   start with '#' are comments. Each request is one whole decision (no kept
   prefix), read through the same readout as the harness's own decision.
   Included by elephant_device.c and mammoth_device.c when
   PSIONIC_DISTILL_CORPUS names the corpus. Prints one line per request:
     distill item=N tokens=T ms=M p_yes=P p_no=Q slot_mass=S */
#ifndef PSIONIC_DISTILL_CORPUS_H
#define PSIONIC_DISTILL_CORPUS_H
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

/* prepare: tokens for one typed yes-or-no question about message; 0 on success.
   read: the yes and no probabilities and the slot mass; 0 on success. */
typedef int (*DistillPrepare)(const char *message, const char *question, int64_t *tokens, int64_t *written, int64_t *prefix);
typedef int (*DistillRead)(const float *logits, double *p, double *mass);

static double distill_now(void) { struct timespec t; clock_gettime(CLOCK_MONOTONIC, &t); return (double)t.tv_sec + 1e-9 * (double)t.tv_nsec; }

static int distill_corpus(EgEngine *e, const char *path, DistillPrepare prepare, DistillRead read) {
    FILE *f = fopen(path, "r");
    if (!f) { perror(path); return 1; }
    static int64_t tokens[EG_MAX_TOKENS];
    char line[1024];
    int rows = 0;
    while (fgets(line, sizeof line, f)) {
        if (line[0] == '#' || line[0] == '\n') continue;
        line[strcspn(line, "\n")] = 0;
        char *field[5], *s = line;
        int n = 0;
        while (n < 5) {
            field[n++] = s;
            char *tab = strchr(s, '\t');
            if (!tab) break;
            *tab = 0;
            s = tab + 1;
        }
        if (n != 5) { fprintf(stderr, "corpus line %d: want 5 fields\n", rows); fclose(f); return 1; }
        int64_t written = 0, prefix = 0;
        if (prepare(field[3], field[4], tokens, &written, &prefix) || written > e->max_tokens) { fprintf(stderr, "corpus item %s refused\n", field[0]); fclose(f); return 1; }
        e->fork_length = 0;
        double t = distill_now();
        if (eg_decide(e, tokens, written, prefix, NULL, NULL)) { fprintf(stderr, "corpus item %s failed: %s\n", field[0], eg_log()); fclose(f); return 1; }
        double ms = (distill_now() - t) * 1e3, p[2], mass;
        if (read(e->logits, p, &mass)) { fprintf(stderr, "corpus item %s: readout refused\n", field[0]); fclose(f); return 1; }
        printf("distill item=%s tokens=%lld ms=%.1f p_yes=%.9f p_no=%.9f slot_mass=%.9f\n", field[0], (long long)written, ms, p[0], p[1], mass);
        fflush(stdout);
        rows++;
    }
    fclose(f);
    printf("distill rows=%d\n", rows);
    return 0;
}
#endif
