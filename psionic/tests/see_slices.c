/* Original-weight slices of the vision tower (chapter 143): the inventory of
 * every model.visual tensor against the config, then the encoder on a 2 by 4
 * patch grid (patch embedding with positions, block 0, the merger) and the
 * processor's patches for a 320 by 256 image. All model arithmetic is Coax
 * (see-slices-runtime.coax); this file maps the file and writes the outputs.
 *
 *   see-slices SAFETENSORS elephant|mammoth OUT.f32
 */
#define _POSIX_C_SOURCE 200809L
#include "see-slices-runtime.h"
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <time.h>
#include <unistd.h>

static double now(void) { struct timespec t; clock_gettime(CLOCK_MONOTONIC, &t); return t.tv_sec + t.tv_nsec / 1e9; }

int main(int argc, char **argv) {
    if (argc != 4 || (strcmp(argv[2], "elephant") && strcmp(argv[2], "mammoth"))) {
        fprintf(stderr, "usage: see-slices SAFETENSORS elephant|mammoth OUT.f32\n");
        return 2;
    }
    int64_t model = strcmp(argv[2], "mammoth") == 0;
    int fd = open(argv[1], O_RDONLY); if (fd < 0) { perror("open"); return 2; }
    struct stat st; if (fstat(fd, &st)) { perror("stat"); return 2; }
    void *map = mmap(NULL, (size_t)st.st_size, PROT_READ, MAP_PRIVATE, fd, 0); close(fd);
    if (map == MAP_FAILED) { perror("mmap"); return 2; }
    coax_t_P5Tensor *inv = calloc(4096, sizeof *inv), *draft = calloc(4096, sizeof *draft);
    if (!inv || !draft) { fprintf(stderr, "allocation failed\n"); return 2; }
    coax_v_u8 bytes = {map, st.st_size};
    coax_t_PwResult scan = p5_inspect_checked(bytes, (coax_v_t_P5Tensor){inv, 4096}, (coax_v_t_P5Tensor){draft, 4096}, 0);
    if (scan.status) { fprintf(stderr, "header %u\n", scan.status); return 1; }
    coax_v_t_P5Tensor inventory = {inv, scan.written};
    coax_t_SvResult check = ss_inventory(model, inventory);
    printf("%s: %lld tensors in the file, %lld under model.visual, inventory status %u (entry %lld), %lld vision parameters\n",
           argv[2], (long long)scan.written, (long long)check.cause, check.status, (long long)check.cause, (long long)check.written);
    if (check.status) return 1;

    coax_t_PaFile file = {.data = bytes}; coax_v_t_PaFile files = {&file, 1};
    int64_t total = 0, count = ss_count();
    for (int64_t i = 0; i < count; i++) total += ss_elements(model, i);
    float *weights = calloc((size_t)total, sizeof(float));
    int64_t out_n = ss_output(model), work_n = ss_work(model);
    float *out = calloc((size_t)out_n, sizeof(float)), *work = calloc((size_t)work_n, sizeof(float));
    if (!weights || !out || !work) { fprintf(stderr, "allocation failed\n"); return 2; }
    double t0 = now();
    for (int64_t i = 0, at = 0; i < count; i++) {
        int64_t n = ss_elements(model, i);
        coax_t_SvResult d = ss_decode(model, i, inventory, files, (coax_v_f32){weights + at, n});
        if (d.status) { fprintf(stderr, "decode %lld: status %u cause %lld\n", (long long)i, d.status, (long long)d.cause); return 1; }
        at += n;
    }
    double t1 = now();
    coax_t_SvResult r = ss_slice(model, (coax_v_f32){weights, total}, (coax_v_f32){out, out_n}, (coax_v_f32){work, work_n});
    double t2 = now();
    if (r.status) { fprintf(stderr, "slice status %u\n", r.status); return 1; }
    printf("%s: decoded %lld weights in %.2f s; slice in %.2f s\n", argv[2], (long long)total, t1 - t0, t2 - t1);
    FILE *f = fopen(argv[3], "wb");
    if (!f || fwrite(out, sizeof(float), (size_t)out_n, f) != (size_t)out_n || fclose(f)) { fprintf(stderr, "cannot write %s\n", argv[3]); return 1; }
    free(weights); free(out); free(work); free(inv); free(draft); munmap(map, (size_t)st.st_size);
    return 0;
}
