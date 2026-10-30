/* Chapter 188: run one norm implementation (prefix PFX, chosen at build time
   with -DPFX=name) over chapter 188's cases, on the CPU or on a CUDA device,
   and time it. Every decision lives in Coax: the cases, the contract
   (ks_check), each implementation's knobs and plan. This file moves bytes,
   launches, and prints raw lines that the evaluator reads:
     cpu KNOB CASE RESULT
     device KNOB CASE RESULT
     time KNOB BLOCK T1 ... (seconds per call of both passes, one per trial)
     pass KNOB BLOCK P0 P1 (median seconds of each pass alone)
   RESULT is ok, wrong:N:FIRST, unwritten:N:FIRST, order (descending work
   items gave other bits), plan (a plan over the limits), trap (a bounds or
   other check stopped the run), cpu-differs:N or repeat-differs:N.
   Usage: kernel-search cpu | kernel-search device KERNEL.cu */
#include "cuda-host.h"
#include "kernel-search.h"
#include "candidate.h"
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/wait.h>
#include <unistd.h>

#define CAT2(a, b) a##b
#define CAT(a, b) CAT2(a, b)
#define STR2(a) #a
#define STR(a) STR2(a)
#define KNOBS CAT(PFX, _knobs)
#define PLAN CAT(PFX, _plan)
#define PASS0 CAT(PFX, _pass0)
#define PASS1 CAT(PFX, _pass1)

#define TRY(x) do { PcStatus s_ = (x); if (s_ != PC_OK) { printf("error %s: status %d: %s\n", #x, s_, pc_log()); fflush(stdout); exit(1); } } while (0)
enum { KNOBS_MAX = 64, TRIALS = 11, CALLS = 50, CASES = 10 };
static const int BLOCKS[] = {32, 64, 128, 256, 512, 1024};
static const float EPSILON = 1e-6f;

typedef struct { int64_t rows, width, items0, items1, work; float *x, *scale, *out, *other; double *scratch; } Run;

static void *must(size_t bytes) {
    void *p = calloc(1, bytes ? bytes : 1);
    if (!p) { printf("error out of host memory\n"); exit(2); }
    return p;
}
static void fill_words(void *p, uint64_t pattern, size_t count, size_t size) {
    for (size_t i = 0; i < count; i++) memcpy((char *)p + i * size, &pattern, size);
}
/* The plan for one knob and case, or 0 when it is over the limits: in each
   pass at most 64 items an output or 1,024 a row, whichever is more, and
   1,024 f64 of scratch a row. */
static int plan(int64_t knob, int c, Run *r) {
    coax_t_KsShape sh = ks_shape(c);
    int64_t p[3] = {-1, -1, -1};
    PLAN(0, sh.rows, sh.width, knob, (coax_v_s64){p, 3});
    r->rows = sh.rows;
    r->width = sh.width;
    r->items0 = p[0];
    r->items1 = p[1];
    r->work = p[2];
    int64_t n = sh.rows * sh.width;
    int64_t most = 64 * n > 1024 * sh.rows ? 64 * n : 1024 * sh.rows;
    return p[0] >= 0 && p[1] >= 0 && p[2] >= 1 && p[0] <= most && p[1] <= most && p[2] <= 1024 * sh.rows;
}
static void setup(int c, Run *r) {
    int64_t n = r->rows * r->width;
    r->x = must((size_t)n * 4);
    r->scale = must((size_t)r->width * 4);
    r->out = must((size_t)n * 4);
    r->other = must((size_t)n * 4);
    r->scratch = must((size_t)r->work * 8);
    ks_fill(c, (coax_v_f32){r->x, n}, (coax_v_f32){r->scale, r->width});
}
static void release(Run *r) { free(r->x); free(r->scale); free(r->out); free(r->other); free(r->scratch); }
/* Every item of pass 0, then of pass 1, ascending or descending. */
static void cpu_run(int64_t knob, Run *r, float *out, int descending) {
    int64_t n = r->rows * r->width;
    fill_words(out, 0x7f7fffffu, (size_t)n, 4);
    fill_words(r->scratch, 0x7ff8000000000000ull, (size_t)r->work, 8);
    coax_v_f32 x = {r->x, n}, scale = {r->scale, r->width}, o = {out, n};
    coax_v_f64 work = {r->scratch, r->work};
    for (int64_t k = 0; k < r->items0; k++) PASS0(descending ? r->items0 - 1 - k : k, r->rows, r->width, knob, EPSILON, x, scale, work, o);
    for (int64_t k = 0; k < r->items1; k++) PASS1(descending ? r->items1 - 1 - k : k, r->rows, r->width, knob, EPSILON, x, scale, work, o);
}
static int64_t differ(const float *a, const float *b, int64_t n) {
    int64_t d = 0;
    for (int64_t i = 0; i < n; i++) d += memcmp(&a[i], &b[i], 4) != 0;
    return d;
}
static void verdict(const char *where, int64_t knob, int c, Run *r) {
    int64_t n = r->rows * r->width;
    coax_t_KsCheck k = ks_check(c, (coax_v_f32){r->x, n}, (coax_v_f32){r->scale, r->width}, (coax_v_f32){r->out, n});
    if (k.unwritten) printf("%s %lld %d unwritten:%lld:%lld\n", where, (long long)knob, c, (long long)k.unwritten, (long long)k.first);
    else if (k.wrong) printf("%s %lld %d wrong:%lld:%lld\n", where, (long long)knob, c, (long long)k.wrong, (long long)k.first);
    else printf("%s %lld %d ok\n", where, (long long)knob, c);
}

static int knob_list(int64_t *knobs) {
    int64_t k[KNOBS_MAX + 1];
    for (int i = 0; i <= KNOBS_MAX; i++) k[i] = -2;
    KNOBS(0, (coax_v_s64){k, KNOBS_MAX + 1});
    int n = 0;
    while (n < KNOBS_MAX && k[n] >= 0) { knobs[n] = k[n]; n++; }
    return n;
}

/* Each knob and case in a child process, so a trap stops only that run. */
static int cpu_mode(void) {
    int64_t knobs[KNOBS_MAX];
    int count = knob_list(knobs);
    for (int i = 0; i < count; i++) {
        for (int c = 0; c < CASES; c++) {
            fflush(stdout);
            pid_t pid = fork();
            if (pid == 0) {
                Run r;
                if (!plan(knobs[i], c, &r)) { printf("cpu %lld %d plan\n", (long long)knobs[i], c); fflush(stdout); _exit(0); }
                setup(c, &r);
                cpu_run(knobs[i], &r, r.other, 1);
                cpu_run(knobs[i], &r, r.out, 0);
                int64_t d = differ(r.out, r.other, r.rows * r.width);
                if (d) printf("cpu %lld %d order\n", (long long)knobs[i], c);
                else verdict("cpu", knobs[i], c, &r);
                fflush(stdout);
                _exit(0);
            }
            int status = 0;
            waitpid(pid, &status, 0);
            if (!WIFEXITED(status) || WEXITSTATUS(status) != 0) printf("cpu %lld %d trap\n", (long long)knobs[i], c);
        }
    }
    return 0;
}

typedef struct { PcBuffer x, scale, out, work; } Device;
static PcKernel k_pass0, k_pass1;
static void launch(int64_t knob, Run *r, Device d, int block, int which) {
    int64_t n = r->rows * r->width;
    PcArg a[] = {{PC_ARG_INT, r->rows}, {PC_ARG_INT, r->width}, {PC_ARG_INT, knob}, {PC_ARG_F32, .f = EPSILON},
                 {PC_ARG_VIEW, .data = d.x, .count = n}, {PC_ARG_VIEW, .data = d.scale, .count = r->width},
                 {PC_ARG_VIEW, .data = d.work, .count = r->work}, {PC_ARG_VIEW, .data = d.out, .count = n}};
    if (which != 1 && r->items0) TRY(pc_launch(k_pass0, r->items0, block, a, 8, NULL));
    if (which != 0 && r->items1) TRY(pc_launch(k_pass1, r->items1, block, a, 8, NULL));
}
static void device_once(int64_t knob, Run *r, Device d, float *out) {
    int64_t n = r->rows * r->width;
    TRY(pc_fill(d.out, 0x7f7fffffu, (size_t)n, NULL));
    TRY(pc_fill(d.work, 0x7ff80000u, (size_t)r->work * 2, NULL));
    launch(knob, r, d, 256, 2);
    TRY(pc_sync());
    TRY(pc_download(out, d.out, (size_t)n * 4, NULL));
    TRY(pc_sync());
}
static double seconds_of(int64_t knob, Run *r, Device d, int block, int which, PcEvent a, PcEvent b) {
    TRY(pc_record(a, NULL));
    for (int i = 0; i < CALLS; i++) launch(knob, r, d, block, which);
    TRY(pc_record(b, NULL));
    TRY(pc_sync());
    double s;
    TRY(pc_event_seconds(a, b, &s));
    return s / CALLS;
}
static int by_value(const void *a, const void *b) { double x = *(const double *)a, y = *(const double *)b; return (x > y) - (x < y); }

static char *read_file(const char *path, size_t *n) {
    FILE *f = fopen(path, "rb");
    if (!f) return NULL;
    fseek(f, 0, SEEK_END);
    long size = ftell(f);
    fseek(f, 0, SEEK_SET);
    char *text = malloc((size_t)size + 1);
    if (!text || fread(text, 1, (size_t)size, f) != (size_t)size) { fclose(f); free(text); return NULL; }
    fclose(f);
    text[size] = 0;
    *n = (size_t)size;
    return text;
}

static int device_mode(const char *path) {
    PcDeviceInfo info;
    PcStatus s = pc_open(&info);
    if (s == PC_ABSENT || s == PC_NO_NVRTC) { printf("SKIP: %s\n", pc_log()); return 0; }
    TRY(s);
    printf("gpu %s compute %d.%d, %d SMs, driver %d\n", info.name, info.major, info.minor, info.sms, info.driver_version);
    size_t length;
    char *source = read_file(path, &length);
    if (!source) { printf("error cannot read %s\n", path); return 1; }
    PcModule module;
    double compile;
    TRY(pc_compile(source, length, "candidate.cu", &module, &compile));
    printf("compiled in %.3f s\n", compile);
    TRY(pc_kernel(module, STR(PFX) "_pass0", &k_pass0));
    TRY(pc_kernel(module, STR(PFX) "_pass1", &k_pass1));
    PcEvent ea, eb;
    TRY(pc_event(&ea));
    TRY(pc_event(&eb));
    int64_t knobs[KNOBS_MAX];
    int count = knob_list(knobs);
    for (int i = 0; i < count; i++) {
        int all = 1;
        for (int c = 0; c < CASES; c++) {
            Run r;
            if (!plan(knobs[i], c, &r)) { printf("device %lld %d plan\n", (long long)knobs[i], c); all = 0; continue; }
            setup(c, &r);
            int64_t n = r.rows * r.width;
            Device d;
            TRY(pc_alloc((size_t)n * 4, &d.x));
            TRY(pc_alloc((size_t)r.width * 4, &d.scale));
            TRY(pc_alloc((size_t)n * 4, &d.out));
            TRY(pc_alloc((size_t)r.work * 8, &d.work));
            TRY(pc_upload(d.x, r.x, (size_t)n * 4, NULL));
            TRY(pc_upload(d.scale, r.scale, (size_t)r.width * 4, NULL));
            float *cpu = must((size_t)n * 4);
            cpu_run(knobs[i], &r, cpu, 0);
            device_once(knobs[i], &r, d, r.out);
            device_once(knobs[i], &r, d, r.other);
            int64_t dc = differ(r.out, cpu, n), dr = differ(r.out, r.other, n);
            if (dc) { printf("device %lld %d cpu-differs:%lld\n", (long long)knobs[i], c, (long long)dc); all = 0; }
            else if (dr) { printf("device %lld %d repeat-differs:%lld\n", (long long)knobs[i], c, (long long)dr); all = 0; }
            else {
                coax_t_KsCheck k = ks_check(c, (coax_v_f32){r.x, n}, (coax_v_f32){r.scale, r.width}, (coax_v_f32){r.out, n});
                verdict("device", knobs[i], c, &r);
                all &= k.wrong == 0 && k.unwritten == 0;
            }
            if (c == 0 && all) {
                double best = 1e9;
                int best_block = 0;
                for (size_t b = 0; b < sizeof BLOCKS / sizeof *BLOCKS; b++) {
                    double t[TRIALS];
                    seconds_of(knobs[i], &r, d, BLOCKS[b], 2, ea, eb);
                    printf("time %lld %d", (long long)knobs[i], BLOCKS[b]);
                    for (int k = 0; k < TRIALS; k++) { t[k] = seconds_of(knobs[i], &r, d, BLOCKS[b], 2, ea, eb); printf(" %.9f", t[k]); }
                    printf("\n");
                    qsort(t, TRIALS, sizeof *t, by_value);
                    if (t[TRIALS / 2] < best) { best = t[TRIALS / 2]; best_block = BLOCKS[b]; }
                }
                double p[2][TRIALS];
                for (int w = 0; w < 2; w++) {
                    for (int k = 0; k < TRIALS; k++) p[w][k] = seconds_of(knobs[i], &r, d, best_block, w, ea, eb);
                    qsort(p[w], TRIALS, sizeof *p[w], by_value);
                }
                printf("pass %lld %d %.9f %.9f\n", (long long)knobs[i], best_block, p[0][TRIALS / 2], p[1][TRIALS / 2]);
            }
            fflush(stdout);
            free(cpu);
            TRY(pc_free(d.x));
            TRY(pc_free(d.scale));
            TRY(pc_free(d.out));
            TRY(pc_free(d.work));
            release(&r);
        }
    }
    return 0;
}

int main(int argc, char **argv) {
    if (argc >= 2 && !strcmp(argv[1], "cpu")) return cpu_mode();
    if (argc >= 3 && !strcmp(argv[1], "device")) return device_mode(argv[2]);
    fprintf(stderr, "usage: kernel-search cpu | kernel-search device KERNEL.cu\n");
    return 2;
}
