#include "retained-reader.h"
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

typedef struct { uint8_t *bytes; long count; } ReaderFile;
static ReaderFile reader_file(const char *path) {
    ReaderFile r = {0}; FILE *f = fopen(path, "rb");
    if (!f || fseek(f, 0, SEEK_END)) { fprintf(stderr, "open/seek failed: %s\n", path); exit(2); }
    long n = ftell(f);
    if (n <= 0 || n > 16000000 || fseek(f, 0, SEEK_SET)) { fprintf(stderr, "file size refused: %s\n", path); exit(2); }
    r.bytes = malloc((size_t)n); r.count = n;
    if (!r.bytes || fread(r.bytes, 1, (size_t)n, f) != (size_t)n || fgetc(f) != EOF || fclose(f)) {
        fprintf(stderr, "file read failed: %s\n", path); exit(2);
    }
    return r;
}
int main(int argc, char **argv) {
    if (argc != 9) { fprintf(stderr, "usage: reader IMAGES LABELS MLP_STATE MLP_SCORES CNN_STATE CNN_SCORES RES_STATE RES_SCORES\n"); return 2; }
    ReaderFile images = reader_file(argv[1]), labels = reader_file(argv[2]);
    const char *names[3] = {"MLP", "CNN", "residual-MLP"};
    for (int kind = 0; kind < 3; ++kind) {
        ReaderFile state = reader_file(argv[3 + 2*kind]), expected = reader_file(argv[4 + 2*kind]);
        float scores[10];
        if (expected.count != 81920 || !retained_saved_row0(kind,
            (coax_v_u8){state.bytes,state.count}, (coax_v_u8){images.bytes,images.count},
            (coax_v_u8){labels.bytes,labels.count}, (coax_v_f32){scores,10})) {
            fprintf(stderr, "load/infer failed: %s\n", names[kind]); return 1;
        }
        if (memcmp(scores, expected.bytes, sizeof scores)) { fprintf(stderr, "score mismatch: %s\n", names[kind]); return 1; }
        for (int j = 0; j < 10; ++j) scores[j] = 123.0f;
        if (retained_saved_row0(kind, (coax_v_u8){state.bytes,state.count-1},
            (coax_v_u8){images.bytes,images.count}, (coax_v_u8){labels.bytes,labels.count},
            (coax_v_f32){scores,10})) { fprintf(stderr, "truncated state accepted: %s\n", names[kind]); return 1; }
        for (int j = 0; j < 10; ++j) if (scores[j] != 123.0f) { fprintf(stderr, "refusal published: %s\n", names[kind]); return 1; }
        printf("%s: %ld state bytes; row0 all10 scores match retained capture bitwise; truncated-state refusal preserved10 outputs\n", names[kind], state.count);
        free(state.bytes); free(expected.bytes);
    }
    free(images.bytes); free(labels.bytes);
    puts("new public-API retained-artifact consumer: 30 score words matched; optimizer updates0");
    return 0;
}
