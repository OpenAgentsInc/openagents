/* Firm reason: 5 (thin wrapper with no logic: reads tekken.json into memory and writes the ids; parsing, indexing and encoding are Coax)
 *
 * Chapter 81's whole-tokenizer run: the Coax reader in
 * psionic/lib/ministral.coax (through ministral-tekken-runtime.coax) loads
 * every ordinary entry of the pinned tekken.json, indexes it, and encodes the
 * fixture strings. The file is authenticated by ministral_reference.py pin
 * first; ministral_reference.py tekken-check compares the ids.
 *
 *   ministral-tekken TEKKEN_JSON OUT_DIR     writes OUT_DIR/tekken-ids.txt
 */
#define _POSIX_C_SOURCE 200809L
#include "ministral-tekken-runtime.h"
#include <stdio.h>
#include <stdlib.h>
#include <time.h>

enum { ENTRIES = 130072, SLOTS = 1 << 18, STORE = 1 << 23, IDS = 256 };

static double now(void) { struct timespec t; clock_gettime(CLOCK_MONOTONIC, &t); return t.tv_sec + t.tv_nsec / 1e9; }

int main(int argc, char **argv) {
    if (argc != 3) { fprintf(stderr, "usage: ministral-tekken TEKKEN_JSON OUT_DIR\n"); return 2; }
    FILE *f = fopen(argv[1], "rb"); if (!f) { perror(argv[1]); return 2; }
    if (fseek(f, 0, SEEK_END)) { perror("seek"); return 2; }
    long size = ftell(f); rewind(f);
    char *json = malloc((size_t)size); uint8_t *store = malloc(STORE);
    coax_string *bytes = calloc(ENTRIES, sizeof *bytes); int64_t *ranks = calloc(ENTRIES, 8), *slots = calloc(SLOTS, 8), ids[IDS];
    if (!json || !store || !bytes || !ranks || !slots) { fprintf(stderr, "allocation failed\n"); return 2; }
    if (fread(json, 1, (size_t)size, f) != (size_t)size) { fprintf(stderr, "short read\n"); return 2; }
    fclose(f);
    double t0 = now();
    coax_t_MsResult r = mt_load((coax_string){json, size}, (coax_v_u8){store, STORE}, (coax_v_string){bytes, ENTRIES},
                                (coax_v_s64){ranks, ENTRIES}, (coax_v_s64){slots, SLOTS});
    if (r.status) { fprintf(stderr, "tekken.json refused: status %u cause %lld\n", r.status, (long long)r.cause); return 1; }
    char path[4096]; snprintf(path, sizeof path, "%s/tekken-ids.txt", argv[2]);
    FILE *out = fopen(path, "w"); if (!out) { perror(path); return 2; }
    fprintf(out, "tekken.json: %lld entries, %lld token bytes, read and indexed in %.2f s\n", (long long)r.written, (long long)r.cause, now() - t0);
    for (int64_t k = 0; k < mt_texts(); k++) {
        coax_t_MsResult e = mt_encode((coax_v_string){bytes, r.written}, (coax_v_s64){ranks, r.written}, (coax_v_s64){slots, SLOTS}, k, (coax_v_s64){ids, IDS});
        if (e.status) { fprintf(stderr, "string %lld refused: status %u cause %lld\n", (long long)k, e.status, (long long)e.cause); return 1; }
        for (int64_t i = 0; i < e.written; i++) fprintf(out, i ? " %lld" : "%lld", (long long)ids[i]);
        fprintf(out, "\n");
    }
    if (fclose(out)) { perror(path); return 2; }
    free(json); free(store); free(bytes); free(ranks); free(slots);
    return 0;
}
