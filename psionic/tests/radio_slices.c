/* Original-weight slices of C-RADIOv3-B (chapter 144): the header and the
 * inventory of every tensor against the config, the stored summary indices
 * and conditioner, then the encoder on a 64 by 48 image (patches, embedding,
 * block 0, and all 12 blocks to the summary and spatial features), the
 * embedding of a 48 by 64 image and the patches of a 224 by 224 image. All
 * model arithmetic is Coax (radio-slices-runtime.coax); this file maps the
 * file and writes the outputs.
 *
 *   radio-slices SAFETENSORS OUT.f32
 */
#define _POSIX_C_SOURCE 200809L
#include "radio-slices-runtime.h"
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <time.h>
#include <unistd.h>

static double now(void) { struct timespec t; clock_gettime(CLOCK_MONOTONIC, &t); return t.tv_sec + t.tv_nsec / 1e9; }

int main(int argc, char **argv) {
    if (argc != 3) { fprintf(stderr, "usage: radio-slices SAFETENSORS OUT.f32\n"); return 2; }
    int fd = open(argv[1], O_RDONLY); if (fd < 0) { perror("open"); return 2; }
    struct stat st; if (fstat(fd, &st)) { perror("stat"); return 2; }
    void *map = mmap(NULL, (size_t)st.st_size, PROT_READ, MAP_PRIVATE, fd, 0); close(fd);
    if (map == MAP_FAILED) { perror("mmap"); return 2; }
    coax_t_RdTensor *inv = calloc(512, sizeof *inv); int64_t *begins = calloc(512, sizeof *begins);
    if (!inv || !begins) { fprintf(stderr, "allocation failed\n"); return 2; }
    coax_v_u8 bytes = {map, st.st_size};
    coax_t_PwResult scan = rs_scan(bytes, (coax_v_t_RdTensor){inv, 512}, (coax_v_s64){begins, 512});
    if (scan.status) { fprintf(stderr, "header %u\n", scan.status); return 1; }
    coax_v_t_RdTensor inventory = {inv, scan.written};
    coax_t_RdResult check = rs_inventory(inventory);
    printf("c-radio: %lld tensors in the file, inventory status %u (cause %lld), %lld elements\n",
           (long long)scan.written, check.status, (long long)check.cause, (long long)check.written);
    if (check.status) return 1;
    coax_v_s64 starts = {begins, scan.written};
    coax_t_RdResult buffers = rs_buffers(bytes, inventory, starts);
    printf("c-radio: summary indices and conditioner status %u (%lld values)\n", buffers.status, (long long)buffers.written);
    if (buffers.status) return 1;

    int64_t total = 0, count = rs_count();
    for (int64_t i = 0; i < count; i++) total += rs_elements(i);
    float *weights = malloc((size_t)total * sizeof(float));
    int64_t out_n = rs_output(), work_n = rs_work();
    float *out = calloc((size_t)out_n, sizeof(float)), *work = calloc((size_t)work_n, sizeof(float));
    if (!weights || !out || !work) { fprintf(stderr, "allocation failed\n"); return 2; }
    double t0 = now();
    for (int64_t i = 0, at = 0; i < count; i++) {
        int64_t n = rs_elements(i);
        if (!n) continue;
        coax_t_RdResult d = rs_decode(i, bytes, inventory, starts, (coax_v_f32){weights + at, n});
        if (d.status) { fprintf(stderr, "decode %lld: status %u cause %lld\n", (long long)i, d.status, (long long)d.cause); return 1; }
        at += n;
    }
    double t1 = now();
    coax_t_RdResult r = rs_slice((coax_v_f32){weights, total}, (coax_v_f32){out, out_n}, (coax_v_f32){work, work_n});
    double t2 = now();
    if (r.status) { fprintf(stderr, "slice status %u cause %lld\n", r.status, (long long)r.cause); return 1; }
    printf("c-radio: decoded %lld weights in %.2f s; slices and the full encoder in %.2f s\n", (long long)total, t1 - t0, t2 - t1);
    FILE *f = fopen(argv[2], "wb");
    if (!f || fwrite(out, sizeof(float), (size_t)out_n, f) != (size_t)out_n || fclose(f)) { fprintf(stderr, "cannot write %s\n", argv[2]); return 1; }
    free(weights); free(out); free(work); free(inv); free(begins); munmap(map, (size_t)st.st_size);
    return 0;
}
