// Chapter 178: measure a replica's start on this host, cold and warm. The host
// owns the effects: files, mappings, page-cache observation (mincore), the
// eviction hint (posix_fadvise DONTNEED on our own file), the monotonic clock
// and the arena memory. Coax (psionic/lib/startup.coax) owns every decision:
// the predicted phases (cs_plan), whether a verified record admits a file
// (pv_admits, chapter 19), the hash check (pw_digest, chapter 7), the
// readiness machine (cs_step, cs_ready), the measured phases (cs_measured)
// and the stand-in golden pass (cs_golden). Prints one JSON receipt.
// Build and run with psionic/tests/startup.sh.
#define _GNU_SOURCE
#include <fcntl.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <sys/utsname.h>
#include <time.h>
#include <unistd.h>
#include "startup.h"

enum { GOLDEN = 3 };

static int64_t now_us(void) {
    struct timespec t;
    clock_gettime(CLOCK_MONOTONIC, &t);
    return (int64_t)t.tv_sec * 1000000 + t.tv_nsec / 1000;
}
static coax_string str(const char *s) { return (coax_string){s, (int64_t)strlen(s)}; }
static int64_t page_size(void) { return (int64_t)sysconf(_SC_PAGESIZE); }

typedef struct {
    int fd;
    uint8_t *data;
    int64_t size, cached, pages;
    coax_t_PaStat st;
} Mapped;

// Open, fstat and map on one descriptor, then count resident pages before
// anything touches them.
static int map_open(const char *path, Mapped *m) {
    memset(m, 0, sizeof *m);
    m->fd = open(path, O_RDONLY);
    if (m->fd < 0) return -1;
    struct stat st;
    if (fstat(m->fd, &st) || st.st_size <= 0) { close(m->fd); return -1; }
    m->size = st.st_size;
    m->st = (coax_t_PaStat){(int64_t)st.st_dev, (int64_t)st.st_ino, (int64_t)st.st_size,
                            (int64_t)st.st_mtim.tv_sec * 1000000000 + st.st_mtim.tv_nsec,
                            (int64_t)st.st_ctim.tv_sec * 1000000000 + st.st_ctim.tv_nsec};
    void *p = mmap(NULL, (size_t)m->size, PROT_READ, MAP_SHARED, m->fd, 0);
    if (p == MAP_FAILED) { close(m->fd); return -1; }
    m->data = p;
    int64_t page = page_size();
    m->pages = (m->size + page - 1) / page;
    unsigned char *vec = malloc((size_t)m->pages);
    m->cached = -1;
    if (vec && !mincore(p, (size_t)m->size, vec)) {
        m->cached = 0;
        for (int64_t i = 0; i < m->pages; i++) m->cached += vec[i] & 1;
    }
    free(vec);
    return 0;
}
static void map_close(Mapped *m) {
    munmap(m->data, (size_t)m->size);
    close(m->fd);
}
static int evict(const char *path) {
    int fd = open(path, O_RDONLY);
    if (fd < 0) return -1;
    int r = posix_fadvise(fd, 0, 0, POSIX_FADV_DONTNEED);
    close(fd);
    return r;
}
static volatile uint8_t sink;
static void touch(const uint8_t *p, int64_t n) {
    uint8_t s = 0;
    for (int64_t i = 0; i < n; i += page_size()) s ^= p[i];
    sink = s;
}
static const char *label(int64_t cached, int64_t pages) {
    if (cached < 0 || pages <= 0) return "unknown";
    if (cached == 0) return "cold";
    if (cached == pages) return "warm";
    return "mixed";
}
static int64_t least(int64_t a, int64_t b) { return a < b ? a : b; }
static int64_t rate(int64_t bytes, int64_t us) { return us < 1 ? bytes : (bytes + us - 1) / us; }
static int64_t load_record(const char *path, uint8_t *out, int64_t cap) {
    FILE *f = path ? fopen(path, "rb") : NULL;
    if (!f) return 0;
    int64_t n = (int64_t)fread(out, 1, (size_t)cap, f);
    fclose(f);
    return n;
}
static void save_record(const char *path, const uint8_t *r, int64_t n) {
    FILE *f = fopen(path, "wb");
    if (!f) return;
    fwrite(r, 1, (size_t)n, f);
    fclose(f);
}
static const char *state_name(coax_t_CsState s) {
    static const char *n[] = {"verify", "map", "arenas", "warmup", "ready", "failed"};
    return n[s];
}
static const char *status_name(coax_t_CsStatus s) {
    static const char *n[] = {"ok", "shape", "rate", "clock", "order", "stale", "check"};
    return n[s];
}
static void phases(const char *key, coax_t_CsPlan p) {
    printf("\"%s\": {\"verify_us\": %lld, \"map_us\": %lld, \"arena_us\": %lld, \"warm_us\": %lld, \"total_us\": %lld}", key,
           (long long)p.verify_us, (long long)p.map_us, (long long)p.arena_us, (long long)p.warm_us, (long long)p.total_us);
}

// One replica start: verify, map, allocate, warm up, in that order, each
// step reported to the readiness machine. A record admits the file only if
// pv_admits says so; otherwise the bytes are hashed, and a good hash writes
// the record.
static void start(const char *name, const char *path, const char *sha, const char *record_path, int cold,
                  int64_t arena, int64_t expected, coax_t_CsHost h, int last) {
    if (cold) evict(path);
    Mapped m;
    if (map_open(path, &m)) { fprintf(stderr, "startup: cannot map %s\n", path); exit(1); }
    uint8_t rec[512];
    int64_t n = load_record(record_path, rec, sizeof rec);
    coax_t_CsFile f = {str("weights.bin"), str(sha), m.st, {rec, n}, m.cached < 0 ? 0 : m.cached};
    coax_t_CsPlan plan = cs_plan((coax_v_t_CsFile){&f, 1}, h, arena, GOLDEN, 1, 1);
    coax_t_CsLife life = cs_start(GOLDEN);
    int64_t t[5];
    t[0] = now_us();
    int admitted = pv_admits(f.record, f.name, f.sha, m.st);
    int ok = admitted || pw_digest((coax_v_u8){m.data, m.size}, f.sha);
    life = cs_step(life, COAX_CsEvent_verified, ok);
    if (ok && !admitted) {
        uint8_t out[512];
        int64_t k = pv_record(f.name, f.sha, m.st, (coax_v_u8){out, sizeof out});
        if (k > 0) save_record(record_path, out, k);
    }
    t[1] = now_us();
    if (life.state == COAX_CsState_map) { touch(m.data, m.size); life = cs_step(life, COAX_CsEvent_mapped, 1); }
    t[2] = now_us();
    uint8_t *a = NULL;
    if (life.state == COAX_CsState_arenas) {
        void *p = mmap(NULL, (size_t)arena, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
        int good = p != MAP_FAILED;
        if (good) { a = p; memset(a, 0, (size_t)arena); }
        life = cs_step(life, COAX_CsEvent_allocated, good);
    }
    t[3] = now_us();
    for (int g = 0; g < GOLDEN && life.state == COAX_CsState_warmup; g++)
        life = cs_step(life, COAX_CsEvent_warmed, cs_golden((coax_v_u8){m.data, m.size}) == expected);
    t[4] = now_us();
    if (a) munmap(a, (size_t)arena);
    coax_t_CsPlan seen = cs_measured((coax_v_s64){t, 5});
    printf("    {\"start\": \"%s\", \"file_cache_at_open\": \"%s\", \"cached_pages\": %lld, \"pages\": %lld, \"record_admitted\": %s, "
           "\"hashed_bytes\": %lld, ",
           name, label(m.cached, m.pages), (long long)m.cached, (long long)m.pages, admitted ? "true" : "false",
           (long long)(admitted ? 0 : m.size));
    phases("predicted", plan);
    printf(", ");
    phases("measured", seen);
    printf(", \"state\": \"%s\", \"ready\": %s", state_name(life.state), cs_ready(life) ? "true" : "false");
    if (life.state == COAX_CsState_failed) printf(", \"failed_at\": \"%s\", \"why\": \"%s\"", state_name(life.at), status_name(life.why));
    printf("}%s\n", last ? "" : ",");
    map_close(&m);
}

// Elephant's original file: observe it, and time verification by record and
// mapping. The full hash runs only when asked (it reads 18.8 GB). Nothing is
// evicted and no record is written: the file and its record belong to the
// serving cache.
static void elephant(const char *path, const char *sha, const char *record_path, int hash, coax_t_CsHost h) {
    Mapped m;
    if (map_open(path, &m)) { fprintf(stderr, "startup: cannot map %s\n", path); exit(1); }
    uint8_t rec[512];
    int64_t n = load_record(record_path, rec, sizeof rec);
    coax_t_CsFile f = {str("model.safetensors"), str(sha), m.st, {rec, n}, m.cached < 0 ? 0 : m.cached};
    coax_t_CsPlan by_record = cs_plan((coax_v_t_CsFile){&f, 1}, h, 0, 1, 1, 1);
    coax_t_CsFile g = f;
    g.record = (coax_v_u8){rec, 0};
    coax_t_CsPlan by_hash = cs_plan((coax_v_t_CsFile){&g, 1}, h, 0, 1, 1, 1);
    int64_t t0 = now_us();
    int admitted = pv_admits(f.record, f.name, f.sha, m.st);
    int64_t t1 = now_us();
    touch(m.data, m.size);
    int64_t t2 = now_us();
    int64_t hash_us = -1;
    int hash_ok = -1;
    if (hash) {
        int64_t a = now_us();
        hash_ok = pw_digest((coax_v_u8){m.data, m.size}, f.sha);
        hash_us = now_us() - a;
    }
    printf("  \"elephant\": {\"bytes\": %lld, \"file_cache_at_open\": \"%s\", \"cached_pages\": %lld, \"pages\": %lld, "
           "\"record_admitted\": %s, \"record_check_us\": %lld, \"map_us\": %lld, \"predicted_map_us\": %lld, "
           "\"predicted_hash_us\": %lld, \"hash_us\": %lld, \"hash_matches\": %s},\n",
           (long long)m.size, label(m.cached, m.pages), (long long)m.cached, (long long)m.pages, admitted ? "true" : "false",
           (long long)(t1 - t0), (long long)(t2 - t1), (long long)by_record.map_us, (long long)by_hash.verify_us,
           (long long)hash_us, hash_ok < 0 ? "null" : hash_ok ? "true" : "false");
    map_close(&m);
}

int main(int argc, char **argv) {
    if (argc < 6) {
        fprintf(stderr, "usage: startup DIR BYTES SHA COMPILER FLAGS [ELEPHANT_FILE ELEPHANT_SHA ELEPHANT_RECORD HASH]\n");
        return 2;
    }
    const char *dir = argv[1], *sha = argv[3], *compiler = argv[4], *flags = argv[5];
    int64_t bytes = atoll(argv[2]), page = page_size(), arena = bytes / 4;
    char path[4096], record[4096];
    snprintf(path, sizeof path, "%s/weights.bin", dir);
    snprintf(record, sizeof record, "%s/weights.rec", dir);

    // Host rates, each the fastest of three timed passes, so a busy moment
    // on a shared host does not set them. The file was just written, so it
    // starts cached; each hash and touch pass maps it afresh, as a start does.
    int64_t fault_us = INT64_MAX, hash_us = INT64_MAX, decision_us = INT64_MAX, read_us = INT64_MAX, zero_us = INT64_MAX;
    int64_t expected = 0, size = 0;
    for (int r = 0; r < 3; r++) {
        Mapped m;
        if (map_open(path, &m)) { fprintf(stderr, "startup: cannot map %s\n", path); return 1; }
        size = m.size;
        int64_t a = now_us();
        int good = pw_digest((coax_v_u8){m.data, m.size}, str(sha));
        hash_us = least(hash_us, now_us() - a);
        if (!good) { fprintf(stderr, "startup: the pinned digest does not match the file\n"); return 1; }
        map_close(&m);
        if (map_open(path, &m)) return 1;
        a = now_us();
        touch(m.data, m.size);
        fault_us = least(fault_us, now_us() - a);
        a = now_us();
        expected = cs_golden((coax_v_u8){m.data, m.size});
        decision_us = least(decision_us, now_us() - a);
        map_close(&m);
    }
    // The disk rate is measured the way a start reads: evicted pages faulted
    // in through a fresh mapping, one touch per page.
    for (int r = 0; r < 3; r++) {
        evict(path);
        Mapped m;
        if (map_open(path, &m)) return 1;
        int64_t a = now_us();
        touch(m.data, m.size);
        read_us = least(read_us, now_us() - a);
        map_close(&m);
        uint8_t *z = mmap(NULL, (size_t)arena, PROT_READ | PROT_WRITE, MAP_PRIVATE | MAP_ANONYMOUS, -1, 0);
        a = now_us();
        memset(z, 0, (size_t)arena);
        zero_us = least(zero_us, now_us() - a);
        munmap(z, (size_t)arena);
    }
    coax_t_CsHost h = {rate(size, hash_us), rate(size, read_us), rate(size, fault_us), rate(arena, zero_us), page, decision_us};
    double load[3] = {0, 0, 0};
    FILE *la = fopen("/proc/loadavg", "r");
    if (la) { if (fscanf(la, "%lf %lf %lf", &load[0], &load[1], &load[2]) != 3) load[0] = -1; fclose(la); }

    struct utsname u;
    uname(&u);
    printf("{\n  \"host\": \"%s\", \"system\": \"%s %s %s\", \"cores\": %ld, \"load_average_1m\": %.2f, \"compiler\": \"%s\", \"flags\": \"%s\", \"golden\": %d,\n",
           u.nodename, u.sysname, u.release, u.machine, sysconf(_SC_NPROCESSORS_ONLN), load[0], compiler, flags, GOLDEN);
    printf("  \"rates_bytes_per_us\": {\"hash\": %lld, \"read\": %lld, \"fault\": %lld, \"zero\": %lld}, \"page\": %lld, "
           "\"decision_us\": %lld, \"weights_bytes\": %lld, \"arena_bytes\": %lld,\n",
           (long long)h.hash, (long long)h.read, (long long)h.fault, (long long)h.zero, (long long)page,
           (long long)decision_us, (long long)bytes, (long long)arena);
    if (argc >= 10) elephant(argv[6], argv[7], argv[8], atoi(argv[9]), h);
    printf("  \"starts\": [\n");
    unlink(record);
    start("cold, no record", path, sha, record, 1, arena, expected, h, 0);
    start("warm, record", path, sha, record, 0, arena, expected, h, 0);
    unlink(record);
    start("warm, no record", path, sha, record, 0, arena, expected, h, 0);
    start("cold, record", path, sha, record, 1, arena, expected, h, 0);
    // One changed byte: the change time moves, the record no longer admits
    // the file, the hash fails and the replica never reports ready.
    int w = open(path, O_RDWR);
    uint8_t flip = 0;
    if (w < 0 || pread(w, &flip, 1, bytes / 2) != 1) { fprintf(stderr, "startup: cannot read the file\n"); return 1; }
    flip ^= 0xff;
    if (pwrite(w, &flip, 1, bytes / 2) != 1) { fprintf(stderr, "startup: cannot change the file\n"); return 1; }
    close(w);
    start("one byte changed, record", path, sha, record, 0, arena, expected, h, 1);
    printf("  ]\n}\n");
    return 0;
}
