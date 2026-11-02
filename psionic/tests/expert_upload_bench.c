/* Chapter 74: how fast gpt-oss-120b's experts cross the link to the card.
   A platform measurement, no model arithmetic: one original expert is
   13,236,480 bytes (MXFP4 blocks, scales and BF16 biases, chapter 33's
   rp_expert), and a decode token that misses every expert cache slot reads
   36 layers * 4 experts = 144 of them. The harness pins 288 distinct expert
   images in host memory (two tokens' worth, so consecutive tokens copy
   different bytes), copies them one expert at a time into 8 device slots
   (two layers' selections, double-buffered), and times each token's 144
   copies with device events. The first token is a warm-up. It then checks
   the last slot's bytes against its source.
   Usage: expert-upload-bench [TOKENS]   (default 11: 1 warm-up, 10 timed) */
#include "cuda-host.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define EXPERT 13236480ull
#define PER_TOKEN 144
#define IMAGES 288
#define SLOTS 8

static int fail(const char *what, PcStatus s) {
    fprintf(stderr, "expert-upload-bench: %s failed (%d): %s\n", what, (int)s, pc_log());
    return 1;
}

int main(int argc, char **argv) {
    int tokens = argc > 1 ? atoi(argv[1]) : 11;
    if (tokens < 2 || tokens > 100) { fprintf(stderr, "expert-upload-bench: TOKENS must be 2..100\n"); return 2; }
    PcDeviceInfo info;
    PcStatus s = pc_open(&info);
    if (s == PC_ABSENT) { printf("SKIP: no CUDA device\n"); return 0; }
    if (s) return fail("pc_open", s);
    printf("device: %s sm_%d%d, %zu bytes total, %zu free, driver %d\n", info.name, info.major, info.minor,
           info.total_bytes, info.free_bytes, info.driver_version);
    unsigned char *host = NULL;
    if ((s = pc_host_alloc(IMAGES * EXPERT, (void **)&host))) return fail("pc_host_alloc", s);
    /* A distinct byte pattern per image, so a copy of the wrong image is caught. */
    for (unsigned long long i = 0; i < IMAGES * EXPERT; i++) host[i] = (unsigned char)((i * 2654435761ull >> 13) ^ (i / EXPERT));
    PcBuffer slots;
    if ((s = pc_alloc(SLOTS * EXPERT, &slots))) return fail("pc_alloc", s);
    PcStream stream; PcEvent start, end;
    if ((s = pc_stream(&stream)) || (s = pc_event(&start)) || (s = pc_event(&end))) return fail("stream/event", s);
    printf("pinned %llu bytes in %d expert images; %d device slots of %llu bytes\n", IMAGES * EXPERT, IMAGES, SLOTS, EXPERT);
    unsigned long long total_us = 0;
    int last_image = 0;
    for (int t = 0; t < tokens; t++) {
        if ((s = pc_record(start, stream))) return fail("pc_record", s);
        for (int e = 0; e < PER_TOKEN; e++) {
            int image = (t % 2) * PER_TOKEN + e;
            last_image = image;
            if ((s = pc_upload(slots + (e % SLOTS) * EXPERT, host + image * EXPERT, EXPERT, stream))) return fail("pc_upload", s);
        }
        if ((s = pc_record(end, stream)) || (s = pc_sync_stream(stream))) return fail("record/sync", s);
        double seconds = 0;
        if ((s = pc_event_seconds(start, end, &seconds))) return fail("pc_event_seconds", s);
        unsigned long long us = (unsigned long long)(seconds * 1e6 + 0.999);
        unsigned long long bytes = PER_TOKEN * EXPERT;
        printf("token %d%s: %llu bytes in %llu us, %llu bytes/us\n", t, t == 0 ? " (warm-up)" : "", bytes, us, bytes / (us ? us : 1));
        if (t > 0) total_us += us;
    }
    unsigned char *back = malloc(EXPERT);
    if (!back) return fail("malloc", PC_MEMORY);
    if ((s = pc_download(back, slots + ((PER_TOKEN - 1) % SLOTS) * EXPERT, EXPERT, stream)) || (s = pc_sync_stream(stream))) return fail("download", s);
    int same = memcmp(back, host + last_image * EXPERT, EXPERT) == 0;
    unsigned long long timed = (unsigned long long)(tokens - 1) * PER_TOKEN * EXPERT;
    printf("timed: %d tokens, %llu bytes in %llu us, %llu bytes/us; last slot matches its image: %s\n", tokens - 1, timed, total_us,
           timed / (total_us ? total_us : 1), same ? "yes" : "no");
    free(back);
    pc_destroy_event(start); pc_destroy_event(end); pc_destroy_stream(stream);
    pc_free(slots); pc_host_free(host);
    return same ? 0 : 1;
}
