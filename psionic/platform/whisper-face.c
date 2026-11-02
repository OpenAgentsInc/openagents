/* psionic-whisper: process I/O for Psionic's Whisper. It maps files,
 * allocates the arenas psionic/lib/whisper/engine.coax lays out, calls its
 * exports and prints. Every decision about the model, the audio, the search
 * and the output is Coax.
 *   psionic/platform/whisper-build.sh OUT
 *   OUT/psionic-whisper info MODEL
 *   OUT/psionic-whisper filters MODEL
 *   OUT/psionic-whisper pcm WAV          the 16 kHz mono samples, f32le on stdout
 *   OUT/psionic-whisper transcribe MODEL WAV [--text|--srt|--vtt|--json|--lines] [--language CODE|auto]
 *       [--threads N] [--name NAME] [--beam N] [--best-of N] [--no-fallback] [--prompt TEXT] [--words A,B,C] [--gpu]
 * WAV is any PCM (8, 16, 24 or 32-bit) or float WAV; wav.coax mixes it to mono
 * and resamples it to 16 kHz. Exit status 65: not a WAV this reads.
 * Built with -DPSIONIC_WHISPER_LIBRARY it has no main and is the library
 * psionic-stt.h declares. Design: psionic/docs/transcribe.md. */
#include "whisper.h"
#include "psionic-stt.h"
#include <fcntl.h>
#include <math.h>
#include <pthread.h>
#include <stdatomic.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <time.h>
#include <unistd.h>

enum { TENSORS = 1024, SEGMENTS = 4096 };

/* ---- the foreign kernels: split a range across threads ------------------
 * engine.coax's cxwh_* calls land here. Each splits its range into one
 * contiguous run per thread and calls the matching *_part export; each
 * output element belongs to exactly one run, so the result is the same for
 * any thread count. Small work runs on the calling thread. */
static int g_threads = 8;
typedef struct { int kind; int64_t lo, hi; coax_v_f32 a, b, c, d, e; coax_v_f64 f, g; int64_t i0, i1, i2, i3, i4, i5; _Bool flag; } job;
static void run_job(job *j) {
    if (j->kind == 0) psionic_whisper_linear_part(j->a, j->i0, j->i1, j->b, j->c, j->i2, j->d, j->lo, j->hi, j->i3);
    else if (j->kind == 1) psionic_whisper_attend_part(j->a, j->i0, j->b, j->c, j->i1, j->i2, j->i3, j->i4, j->flag, j->d, j->lo, j->hi);
    else if (j->kind == 3) psionic_whisper_mel_part(j->a, j->f, j->g, j->i0, j->b, j->lo, j->hi);
}
/* A pool of g_threads - 1 workers, started once. split() hands each one a
 * job and runs the first itself; workers spin briefly on the generation
 * counter, then sleep on a condition variable, so a burst of small calls
 * (the decoder's) costs no thread creation. */
static pthread_mutex_t g_mu = PTHREAD_MUTEX_INITIALIZER;
static pthread_cond_t g_go = PTHREAD_COND_INITIALIZER, g_done = PTHREAD_COND_INITIALIZER;
static job g_jobs[64];
static _Atomic long g_gen;
static _Atomic int g_pending;
static int g_workers;
static int g_active;
static void take(const job *base);
static void *worker(void *arg) {
    int me = (int)(intptr_t)arg; long seen = 0;
    for (;;) {
        long gen;
        int spins = 0;
        while ((gen = g_gen) == seen && spins < 200000) spins++;
        if (gen == seen) {
            pthread_mutex_lock(&g_mu);
            while ((gen = g_gen) == seen) pthread_cond_wait(&g_go, &g_mu);
            pthread_mutex_unlock(&g_mu);
        }
        seen = gen;
        if (me < g_active) take(&g_jobs[0]);
        if (--g_pending == 0) { pthread_mutex_lock(&g_mu); pthread_cond_signal(&g_done); pthread_mutex_unlock(&g_mu); }
    }
    return 0;
}
static void pool_start(void) {
    int n = g_threads - 1; if (n > 63) n = 63;
    for (int i = 1; i <= n; i++) {
        pthread_t id;
        if (pthread_create(&id, 0, worker, (void *)(intptr_t)i) != 0) break;
        pthread_detach(id); g_workers = i;
    }
}
static double g_prof[6]; static long g_calls[6]; static int g_small; static int g_profile;
static double now_ms(void);
/* Work is cut into chunks that every thread takes from one counter, so the
 * efficiency cores, which run slower, take fewer chunks instead of holding
 * up the rest. Each output element is still computed by the same code. */
static int64_t g_n, g_chunk;
static _Atomic int64_t g_next;
static void take(const job *base) {
    for (;;) {
        int64_t c = g_next++;
        int64_t lo = c * g_chunk;
        if (lo >= g_n) return;
        job j = *base; j.lo = lo; j.hi = lo + g_chunk < g_n ? lo + g_chunk : g_n;
        run_job(&j);
    }
}
static void split(job base, int64_t n, double work) {
    double t0 = g_profile ? now_ms() : 0;
    int t = g_workers + 1;
    if (work < 2e5 || t <= 1 || n < 2) { base.lo = 0; base.hi = n; run_job(&base); }
    else {
        int64_t chunks = (int64_t)t * 6;
        int64_t chunk = (n + chunks - 1) / chunks;
        if (base.kind == 0) chunk = (chunk + 3) / 4 * 4;
        if (chunk < 1) chunk = 1;
        g_jobs[0] = base; g_n = n; g_chunk = chunk; g_next = 0;
        int need = (int)((n + chunk - 1) / chunk) - 1;
        g_active = need < g_workers ? need + 1 : g_workers + 1;
        g_pending = g_workers;
        pthread_mutex_lock(&g_mu); g_gen++; pthread_cond_broadcast(&g_go); pthread_mutex_unlock(&g_mu);
        take(&base);
        int spins = 0;
        while (g_pending > 0 && spins < 2000000) spins++;
        if (g_pending > 0) { pthread_mutex_lock(&g_mu); while (g_pending > 0) pthread_cond_wait(&g_done, &g_mu); pthread_mutex_unlock(&g_mu); }
    }
    if (g_profile) { int k = base.kind + (g_small && base.kind < 2 ? 4 : 0); g_prof[k] += now_ms() - t0; g_calls[k]++; }
}
/* ---- the GPU (--gpu, Apple Metal) ----------------------------------------
 * With --gpu the weights and the engine's work arena live in shared Metal
 * buffers, and the encoder's projections and attention (the calls with at
 * least 64 rows) run psionic/lib/whisper/device.coax's kernels there. A
 * decoder step (cxwh_step) is one batch: its norms, products, attention,
 * residual adds and cache stores are launched into one command buffer,
 * which runs when the step ends, so the step costs one wait, not one per
 * call. A call the GPU cannot take ends the batch first, then runs on the
 * CPU. The CPU and GPU see the same bytes, so nothing is copied. */
#ifdef PSIONIC_WHISPER_METAL
#include "metal-host.h"
#include "whisper-metal.h"
static int g_gpu;
static PmBuffer g_buf[4]; static char *g_base[4]; static size_t g_size[4];
static PmKernel g_klinear, g_krows, g_kscores, g_ksoftmax, g_kvalues, g_knorm, g_kadd, g_kstore, g_kpart, g_kjoin, g_kcross, g_krowsh;
static double g_gpu_ms;
static PmBatch g_step; static int g_open, g_steps; static double g_step_ms;
static void gpu_close(void) {
    if (!g_open) return;
    double t = 0; g_open = 0;
    PmStatus st = pm_wait(g_step, &t);
    if (st != PM_OK) { fprintf(stderr, "psionic-whisper: GPU launch failed (%d): %s\n", (int)st, pm_log()); exit(1); }
    g_gpu_ms += t * 1e3; g_step_ms += t * 1e3;
}
static int gpu_view(coax_v_f32 v, PmArg *a) {
    for (int i = 0; i < 2; i++) {
        char *p = (char *)v.data;
        if (g_base[i] && p >= g_base[i] && p + v.count * sizeof(float) <= g_base[i] + g_size[i]) {
            *a = (PmArg){PM_ARG_VIEW, 0, 0, g_buf[i], (int64_t)(p - g_base[i]), v.count};
            return 1;
        }
    }
    return 0;
}
static PmArg gpu_int(int64_t i) { return (PmArg){PM_ARG_INT, i, 0, 0, 0, 0}; }
#define WD_ROWS_OUT 4 /* device.coax: outputs per group of wd_rows and wd_rowsh */
static int gpu_run(PmKernel k, int64_t items, int lanes, int block, const PmArg *a, int n) {
    if (g_open) {
        if (pm_launch(g_step, k, items, lanes, block, a, n) != PM_OK) { fprintf(stderr, "psionic-whisper: GPU launch refused: %s\n", pm_log()); exit(1); }
        return 1;
    }
    PmBatch b; double t = 0;
    if (pm_begin(&b) != PM_OK || pm_launch(b, k, items, lanes, block, a, n) != PM_OK) return 0;
    PmStatus st = pm_wait(b, &t);
    if (st != PM_OK) { fprintf(stderr, "psionic-whisper: GPU launch failed (%d): %s\n", (int)st, pm_log()); exit(1); }
    g_gpu_ms += t * 1e3;
    return 1;
}
/* A call the open step cannot take: the step's launches run first. */
static int gpu_refuse(void) { gpu_close(); return 0; }
static int gpu_linear(coax_v_f32 x, int64_t rows, int64_t n_in, coax_v_f32 w, coax_v_f32 b, int64_t n_out, coax_v_f32 y, int64_t act) {
    if (!g_gpu || (rows < 64 && !(g_open && rows <= 8))) return g_open ? gpu_refuse() : 0;
    PmArg a[9];
    a[0] = gpu_int(rows); a[1] = gpu_int(n_in); a[2] = gpu_int(n_out); a[3] = gpu_int(act); a[4] = gpu_int(b.count > 0);
    if (!gpu_view(x, &a[5]) || !gpu_view(w, &a[6]) || !gpu_view(y, &a[8])) return g_open ? gpu_refuse() : 0;
    if (b.count > 0) { if (!gpu_view(b, &a[7])) return g_open ? gpu_refuse() : 0; } else a[7] = a[8];
    if (rows <= 8) {
        /* The weights as halves when the model's are (gpu_halves). */
        int64_t at = (int64_t)((char *)w.data - g_base[0]) / 4;
        if (g_base[3] && a[6].buffer == g_buf[0] && at % 2 == 0 && n_in % 2 == 0) {
            a[6] = (PmArg){PM_ARG_VIEW, 0, 0, g_buf[3], at * 2, w.count / 2};
            return gpu_run(g_krowsh, (n_out + WD_ROWS_OUT - 1) / WD_ROWS_OUT, 128, 128, a, 9);
        }
        return gpu_run(g_krows, (n_out + WD_ROWS_OUT - 1) / WD_ROWS_OUT, 128, 128, a, 9);
    }
    return gpu_run(g_klinear, ((rows + 63) / 64) * ((n_out + 31) / 32), 128, 128, a, 9);
}
/* wd_part then wd_join (device.coax): causal queries read slot and position
 * from sp, 12 bits each. */
static int gpu_split(coax_v_f32 q, int64_t nq, coax_v_f32 k, coax_v_f32 v, int64_t nk, int64_t d, int64_t heads, int64_t ctx, int64_t sp, int causal, coax_v_f32 out) {
    int64_t chunks = (nk + 127) / 128, items = heads * nq * chunks;
    if (nq > 8 || (causal && nq > 5) || d != heads * 64 || (size_t)(items * 66) * sizeof(float) > g_size[2]) return gpu_refuse();
    PmArg a[12], j[6], part = {PM_ARG_VIEW, 0, 0, g_buf[2], 0, items * 66};
    a[0] = gpu_int(nq); a[1] = gpu_int(nk); a[2] = gpu_int(d); a[3] = gpu_int(heads); a[4] = gpu_int(ctx); a[5] = gpu_int(sp); a[6] = gpu_int(causal);
    if (!gpu_view(q, &a[7]) || !gpu_view(k, &a[8]) || !gpu_view(v, &a[9]) || !gpu_view(out, &j[5])) return gpu_refuse();
    a[10] = part;
    j[0] = a[0]; j[1] = a[1]; j[2] = a[2]; j[3] = part; j[4] = j[5];
    if (!causal) {
        PmArg c[9] = {a[0], a[1], a[2], a[3], a[7], a[8], a[9], part};
        return gpu_run(g_kcross, heads * chunks, 128, 128, c, 8) && gpu_run(g_kjoin, heads * nq, 64, 64, j, 5);
    }
    return gpu_run(g_kpart, items, 128, 128, a, 11) && gpu_run(g_kjoin, heads * nq, 64, 64, j, 5);
}
static int gpu_few(coax_v_f32 q, int64_t nq, coax_v_f32 k, coax_v_f32 v, int64_t nk, int64_t d, int64_t heads, int64_t first, _Bool causal, coax_v_f32 out) {
    if (causal && (nq != 1 || first >= 512)) return gpu_refuse();
    return gpu_split(q, nq, k, v, nk, d, heads, 0, causal ? first : 0, causal, out);
}
static int gpu_attend(coax_v_f32 q, int64_t nq, coax_v_f32 k, coax_v_f32 v, int64_t nk, int64_t d, int64_t heads, int64_t first, _Bool causal, coax_v_f32 out) {
    if (g_open) return gpu_few(q, nq, k, v, nk, d, heads, first, causal, out);
    if (!g_gpu || causal || nq < 64 || d != heads * 64 || (size_t)(heads * nq * nk) * sizeof(float) > g_size[2]) return 0;
    PmArg a[8], m[2], c[8], sv = {PM_ARG_VIEW, 0, 0, g_buf[2], 0, heads * nq * nk};
    a[0] = gpu_int(nq); a[1] = gpu_int(nk); a[2] = gpu_int(d); a[3] = gpu_int(heads);
    if (!gpu_view(q, &a[4]) || !gpu_view(k, &a[5]) || !gpu_view(v, &c[5]) || !gpu_view(out, &c[6])) return 0;
    a[6] = sv; m[0] = gpu_int(nk); m[1] = sv;
    c[0] = a[0]; c[1] = a[1]; c[2] = a[2]; c[3] = a[3]; c[4] = sv;
    PmBatch b; double t = 0;
    if (pm_begin(&b) != PM_OK) return 0;
    int64_t qt = (nq + 63) / 64;
    if (pm_launch(b, g_kscores, heads * qt * ((nk + 31) / 32), 128, 128, a, 7) != PM_OK ||
        pm_launch(b, g_ksoftmax, heads * nq, 128, 128, m, 2) != PM_OK ||
        pm_launch(b, g_kvalues, heads * qt * 2, 128, 128, c, 7) != PM_OK) return 0;
    PmStatus st = pm_wait(b, &t);
    if (st != PM_OK) { fprintf(stderr, "psionic-whisper: GPU launch failed (%d): %s\n", (int)st, pm_log()); exit(1); }
    g_gpu_ms += t * 1e3;
    return 1;
}
/* Opens the GPU and compiles the kernels; 0 when there is no Metal device. */
static int gpu_open(void) {
    PmDeviceInfo info; PmModule mod; double secs;
    if (pm_open(&info) != PM_OK) return 0;
    if (pm_compile(whisper_metal_source, sizeof whisper_metal_source - 1, "whisper", &mod, &secs) != PM_OK) { fprintf(stderr, "psionic-whisper: %s\n", pm_log()); return 0; }
    if (pm_kernel(mod, "wd_linear", &g_klinear) != PM_OK || pm_kernel(mod, "wd_rows", &g_krows) != PM_OK || pm_kernel(mod, "wd_scores", &g_kscores) != PM_OK ||
        pm_kernel(mod, "wd_softmax", &g_ksoftmax) != PM_OK || pm_kernel(mod, "wd_values", &g_kvalues) != PM_OK || pm_kernel(mod, "wd_norm", &g_knorm) != PM_OK ||
        pm_kernel(mod, "wd_add", &g_kadd) != PM_OK || pm_kernel(mod, "wd_store", &g_kstore) != PM_OK || pm_kernel(mod, "wd_part", &g_kpart) != PM_OK || pm_kernel(mod, "wd_join", &g_kjoin) != PM_OK || pm_kernel(mod, "wd_cross", &g_kcross) != PM_OK || pm_kernel(mod, "wd_rowsh", &g_krowsh) != PM_OK) return 0;
    return 1;
}
static int gpu_step(int begin) {
    if (!begin) { gpu_close(); return 1; }
    if (!g_gpu || g_open || pm_begin(&g_step) != PM_OK) return 0;
    g_open = 1; g_steps++;
    return 1;
}
static int gpu_norm(coax_v_f32 x, int64_t rows, int64_t d, coax_v_f32 g, coax_v_f32 b, coax_v_f32 y) {
    if (!g_open) return 0;
    PmArg a[5]; a[0] = gpu_int(d);
    if (!gpu_view(x, &a[1]) || !gpu_view(g, &a[2]) || !gpu_view(b, &a[3]) || !gpu_view(y, &a[4])) return gpu_refuse();
    return gpu_run(g_knorm, rows, 128, 128, a, 5);
}
static int gpu_add(coax_v_f32 x, coax_v_f32 h) {
    if (!g_open) return 0;
    PmArg a[2];
    if (x.count != h.count || !gpu_view(x, &a[0]) || !gpu_view(h, &a[1])) return gpu_refuse();
    return gpu_run(g_kadd, x.count, 1, 256, a, 2);
}
static int gpu_store(coax_v_f32 ka, coax_v_f32 va, coax_v_f32 kb, coax_v_f32 vb) {
    if (!g_open) return 0;
    PmArg a[4];
    if (!gpu_view(ka, &a[0]) || !gpu_view(va, &a[1]) || !gpu_view(kb, &a[2]) || !gpu_view(vb, &a[3])) return gpu_refuse();
    return gpu_run(g_kstore, ka.count, 1, 256, a, 4);
}
/* A copy of the weights as halves for the decoder's products, kept only
 * when every weight is a half exactly (an f16 model; a quantized model's
 * products stay in f32). */
static void gpu_halves(const float *W, size_t count) {
    if (pm_alloc((count + 1) / 2 * 4, &g_buf[3]) != PM_OK) return;
    __fp16 *h = pm_contents(g_buf[3]);
    for (size_t i = 0; i < count; i++) {
        h[i] = (__fp16)W[i];
        if ((float)h[i] != W[i] || !isfinite(W[i])) { pm_free(g_buf[3]); g_buf[3] = 0; return; }
    }
    if (count % 2) h[count] = 0;
    g_base[3] = (char *)h; g_size[3] = (count + 1) / 2 * 4;
}
static float *gpu_alloc(int i, size_t count) {
    if (pm_alloc(count * sizeof(float), &g_buf[i]) != PM_OK) return 0;
    g_base[i] = pm_contents(g_buf[i]); g_size[i] = count * sizeof(float);
    memset(g_base[i], 0, g_size[i]);
    return (float *)g_base[i];
}
#else
static int g_gpu, g_steps;
static int g_open;
static double g_step_ms;
static double g_gpu_ms;
static int gpu_linear(coax_v_f32 x, int64_t rows, int64_t n_in, coax_v_f32 w, coax_v_f32 b, int64_t n_out, coax_v_f32 y, int64_t act) { (void)x; (void)rows; (void)n_in; (void)w; (void)b; (void)n_out; (void)y; (void)act; return 0; }
static int gpu_attend(coax_v_f32 q, int64_t nq, coax_v_f32 k, coax_v_f32 v, int64_t nk, int64_t d, int64_t heads, int64_t first, _Bool causal, coax_v_f32 out) { (void)q; (void)nq; (void)k; (void)v; (void)nk; (void)d; (void)heads; (void)first; (void)causal; (void)out; return 0; }
static int gpu_open(void) { return 0; }
static int gpu_step(int begin) { (void)begin; return 0; }
static int gpu_split(coax_v_f32 q, int64_t nq, coax_v_f32 k, coax_v_f32 v, int64_t nk, int64_t d, int64_t heads, int64_t ctx, int64_t sp, int causal, coax_v_f32 out) { (void)q; (void)nq; (void)k; (void)v; (void)nk; (void)d; (void)heads; (void)ctx; (void)sp; (void)causal; (void)out; return 0; }
static int gpu_norm(coax_v_f32 x, int64_t rows, int64_t d, coax_v_f32 g, coax_v_f32 b, coax_v_f32 y) { (void)x; (void)rows; (void)d; (void)g; (void)b; (void)y; return 0; }
static int gpu_add(coax_v_f32 x, coax_v_f32 h) { (void)x; (void)h; return 0; }
static int gpu_store(coax_v_f32 ka, coax_v_f32 va, coax_v_f32 kb, coax_v_f32 vb) { (void)ka; (void)va; (void)kb; (void)vb; return 0; }
static float *gpu_alloc(int i, size_t count) { (void)i; (void)count; return 0; }
static void gpu_halves(const float *W, size_t count) { (void)W; (void)count; }
#endif
int64_t cxwh_linear(coax_v_f32 x, int64_t rows, int64_t n_in, coax_v_f32 w, coax_v_f32 b, int64_t n_out, coax_v_f32 y, int64_t act) {
    double t0 = g_profile ? now_ms() : 0;
    if (gpu_linear(x, rows, n_in, w, b, n_out, y, act)) { if (g_profile) { g_prof[0] += now_ms() - t0; g_calls[0]++; } return 0; }
    job j = {0}; j.kind = 0; j.a = x; j.i0 = rows; j.i1 = n_in; j.b = w; j.c = b; j.i2 = n_out; j.d = y; j.i3 = act;
    g_small = rows < 256; split(j, n_out, (double)rows * n_in * n_out);
    return 0;
}
int64_t cxwh_attend(coax_v_f32 q, int64_t nq, coax_v_f32 k, coax_v_f32 v, int64_t nk, int64_t d, int64_t heads, int64_t first, _Bool causal, coax_v_f32 out) {
    double t0 = g_profile ? now_ms() : 0;
    if (gpu_attend(q, nq, k, v, nk, d, heads, first, causal, out)) { if (g_profile) { g_prof[1] += now_ms() - t0; g_calls[1]++; } return 0; }
    job j = {0}; j.kind = 1; j.a = q; j.i0 = nq; j.b = k; j.c = v; j.i1 = nk; j.i2 = d; j.i3 = heads; j.i4 = first; j.flag = causal; j.d = out;
    g_small = nq < 256; split(j, heads * ((nq + 3) / 4), (double)nq * nk * d * 2);
    return 0;
}
/* A decoder step's batch (begin 1 opens it, 0 runs it) and the calls only
 * an open step takes; each returns 1 when the GPU has the call and 0 when
 * the engine must do it on the CPU. */
int64_t cxwh_step(int64_t begin) { return gpu_step((int)begin); }
int64_t cxwh_norm(coax_v_f32 x, int64_t rows, int64_t d, coax_v_f32 g, coax_v_f32 b, coax_v_f32 y) { return gpu_norm(x, rows, d, g, b, y); }
int64_t cxwh_add(coax_v_f32 x, coax_v_f32 h) { return gpu_add(x, h); }
int64_t cxwh_store(coax_v_f32 ka, coax_v_f32 va, coax_v_f32 kb, coax_v_f32 vb) { return gpu_store(ka, va, kb, vb); }
int64_t cxwh_slots(coax_v_f32 q, int64_t rows, coax_v_f32 ks, coax_v_f32 vs, int64_t ctx, int64_t d, int64_t heads, int64_t sp, int64_t nk, coax_v_f32 out) {
    return g_open ? gpu_split(q, rows, ks, vs, nk, d, heads, ctx, sp, 1, out) : 0;
}
int64_t cxwh_mel(coax_v_f32 x, coax_v_f64 window, coax_v_f64 bank, int64_t mels, coax_v_f32 cache, int64_t frames) {
    job j = {0}; j.kind = 3; j.a = x; j.f = window; j.g = bank; j.i0 = mels; j.b = cache;
    split(j, frames, (double)frames * 1e5);
    return 0;
}

static double now_ms(void) {
    struct timespec t;
    clock_gettime(CLOCK_MONOTONIC, &t);
    return t.tv_sec * 1e3 + t.tv_nsec / 1e6;
}
static coax_v_u8 map(const char *path) {
    coax_v_u8 v = {0, 0};
    int fd = open(path, O_RDONLY);
    struct stat st;
    if (fd < 0 || fstat(fd, &st) != 0 || st.st_size == 0) { if (fd >= 0) close(fd); return v; }
    void *p = mmap(0, (size_t)st.st_size, PROT_READ, MAP_PRIVATE, fd, 0);
    close(fd);
    if (p == MAP_FAILED) return v;
    v.data = p; v.count = st.st_size;
    return v;
}
/* ---- the library (psionic-stt.h) ----------------------------------------- */
struct psionic_stt {
    coax_v_u8 file; coax_t_WgModel m; coax_t_WeNet net; coax_v_t_WgTensor tv;
    float *W, *work; double *bank; int64_t *vocab;
    pthread_mutex_t mu;
};
static void stt_err(char *err, size_t cap, const char *msg) { if (err && cap) snprintf(err, cap, "%s", msg); }
psionic_stt *psionic_stt_open(const char *path, int threads, int gpu, char *err, size_t errcap) {
    static coax_t_WgTensor tensors[TENSORS];
    static int64_t offsets[TENSORS];
    static int opened;
    if (opened) { stt_err(err, errcap, "psionic: a model is already open"); return 0; }
    psionic_stt *s = calloc(1, sizeof *s);
    if (!s) { stt_err(err, errcap, "psionic: out of memory"); return 0; }
    s->file = map(path);
    if (!s->file.data) { stt_err(err, errcap, "psionic: cannot read the Whisper model"); free(s); return 0; }
    s->tv = (coax_v_t_WgTensor){tensors, TENSORS};
    s->m = psionic_whisper_read(s->file, s->tv);
    if (s->m.status != COAX_WgStatus_ok) { stt_err(err, errcap, "psionic: not a Whisper ggml file"); free(s); return 0; }
    coax_t_WgParams p = s->m.params;
    s->bank = calloc((size_t)(p.n_mels * 201), sizeof *s->bank);
    s->vocab = malloc((size_t)s->m.tokens * sizeof *s->vocab);
    if (!s->bank || !s->vocab || psionic_whisper_bank(p.n_mels, (coax_v_f64){s->bank, p.n_mels * 201}) != 0 ||
        psionic_whisper_vocab(s->file, s->m, (coax_v_s64){s->vocab, s->m.tokens}) != s->m.tokens) { stt_err(err, errcap, "psionic: model tables"); return 0; }
    s->net = psionic_whisper_net(s->file, s->m, s->tv, (coax_v_s64){offsets, TENSORS}, p.n_audio_ctx);
    if (s->net.status != COAX_WeStatus_ok) { stt_err(err, errcap, "psionic: model tensors"); return 0; }
    g_threads = threads < 1 ? 1 : threads > 64 ? 64 : threads;
    pool_start();
    g_gpu = gpu && gpu_open();
    if (g_gpu) {
        s->W = gpu_alloc(0, (size_t)s->net.weights); s->work = gpu_alloc(1, (size_t)s->net.work.total);
        if (!gpu_alloc(2, (size_t)(p.n_audio_head * p.n_audio_ctx * p.n_audio_ctx))) s->W = 0;
    } else { s->W = malloc((size_t)s->net.weights * sizeof *s->W); s->work = calloc((size_t)s->net.work.total, sizeof *s->work); }
    if (!s->W || !s->work) { stt_err(err, errcap, "psionic: out of memory"); return 0; }
    if (psionic_whisper_load(s->file, s->tv, s->m.tensor_count, (coax_v_s64){offsets, TENSORS}, (coax_v_f32){s->W, s->net.weights}) != COAX_WeStatus_ok) {
        stt_err(err, errcap, "psionic: model weights"); return 0;
    }
    if (g_gpu) gpu_halves(s->W, (size_t)s->net.weights);
    pthread_mutex_init(&s->mu, 0);
    opened = 1;
    return s;
}
int psionic_stt_gpu(const psionic_stt *s) { return s && g_gpu; }
int psionic_stt_text(psionic_stt *s, const float *samples, int64_t n, const char *words, char *out, size_t cap, char *err, size_t errcap) {
    if (!s || !out || cap == 0) { stt_err(err, errcap, "psionic: no model"); return 0; }
    out[0] = 0;
    if (n < 1) return 1;
    coax_t_WgParams p = s->m.params;
    int64_t cache_n = psionic_whisper_cache_size(n, p.n_mels);
    size_t text_cap = 1 << 16;
    float *cache = malloc((size_t)cache_n * sizeof *cache);
    uint8_t *text = malloc(text_cap), *line = malloc(text_cap);
    coax_t_WsSegment *segs = malloc(SEGMENTS * sizeof *segs);
    int ok = 0;
    if (!cache || !text || !line || !segs) { stt_err(err, errcap, "psionic: out of memory"); goto done; }
    coax_t_WeOptions opt = psionic_whisper_options();
    opt.language = psionic_whisper_language(s->net, (coax_string){"en", 2});
    opt.prompt = (coax_string){"", 0};
    opt.words = (coax_string){words ? words : "", words ? (int64_t)strlen(words) : 0};
    pthread_mutex_lock(&s->mu);
    coax_t_WeRun run = psionic_whisper_transcribe(s->net, (coax_v_f32){s->W, s->net.weights}, (coax_v_f32){s->work, s->net.work.total}, s->file, s->m,
                                                  (coax_v_s64){s->vocab, s->m.tokens}, (coax_v_f64){s->bank, p.n_mels * 201}, (coax_v_f32){(float *)samples, n},
                                                  (coax_v_f32){cache, cache_n}, opt, (coax_v_t_WsSegment){segs, SEGMENTS}, (coax_v_u8){text, (int64_t)text_cap});
    int64_t k = run.status == COAX_WeStatus_ok
        ? psionic_whisper_format((coax_v_t_WsSegment){segs, run.segments}, 0, (coax_string){"", 0}, (coax_string){"en", 2}, (coax_v_u8){line, (int64_t)text_cap})
        : -1;
    pthread_mutex_unlock(&s->mu);
    if (k < 0) { stt_err(err, errcap, "psionic: transcription failed"); goto done; }
    /* one line: the text format ends each segment with a newline */
    size_t o = 0;
    for (int64_t i = 0; i < k && o + 1 < cap; i++) out[o++] = line[i] == '\n' ? ' ' : (char)line[i];
    while (o > 0 && out[o - 1] == ' ') o--;
    out[o] = 0;
    ok = 1;
done:
    free(cache); free(text); free(line); free(segs);
    return ok;
}

#ifndef PSIONIC_WHISPER_LIBRARY
static int usage(void) {
    fputs("usage: psionic-whisper info|filters MODEL\n"
          "       psionic-whisper pcm WAV\n"
          "       psionic-whisper transcribe MODEL WAV [--text|--srt|--vtt|--json|--lines] [--language CODE|auto] [--threads N]\n"
          "                       [--name NAME] [--beam N] [--best-of N] [--no-fallback] [--prompt TEXT] [--words A,B,C] [--gpu]\n", stderr);
    return 64;
}

int main(int argc, char **argv) {
    if (argc < 3) return usage();
    const char *cmd = argv[1];
    double t0 = now_ms();
    if (strcmp(cmd, "pcm") == 0) {
        /* the 16 kHz mono samples a WAV becomes, as raw f32le on stdout */
        coax_v_u8 wav = map(argv[2]);
        if (!wav.data) { fprintf(stderr, "psionic-whisper: cannot map %s\n", argv[2]); return 1; }
        coax_t_WvInfo wi = psionic_whisper_wav_info(wav);
        if (wi.status != COAX_WvStatus_ok) { fprintf(stderr, "psionic-whisper: not a WAV this reads\n"); return 65; }
        float *mono = malloc((size_t)(wi.frames + 1) * sizeof *mono);
        int64_t n = psionic_whisper_wav_length(wi.frames, wi.rate);
        float *out = malloc((size_t)(n + 1) * sizeof *out);
        if (!mono || !out || psionic_whisper_wav_mono(wav, wi, (coax_v_f32){mono, wi.frames}) != wi.frames) return 1;
        if (psionic_whisper_resample((coax_v_f32){mono, wi.frames}, wi.rate, (coax_v_f32){out, n}) != n) return 1;
        fwrite(out, sizeof *out, (size_t)n, stdout);
        return 0;
    }
    coax_v_u8 file = map(argv[2]);
    if (!file.data) { fprintf(stderr, "psionic-whisper: cannot map %s\n", argv[2]); return 1; }
    static coax_t_WgTensor tensors[TENSORS];
    coax_v_t_WgTensor tv = {tensors, TENSORS};
    coax_t_WgModel m = psionic_whisper_read(file, tv);
    if (m.status != COAX_WgStatus_ok) { fprintf(stderr, "psionic-whisper: not a Whisper ggml file (status %d)\n", (int)m.status); return 1; }
    coax_t_WgParams p = m.params;
    if (strcmp(cmd, "info") == 0) {
        printf("vocab %lld\naudio %lld x %lld, %lld heads, %lld layers\ntext %lld x %lld, %lld heads, %lld layers\nmels %lld\nftype %lld\ntensors %lld\ntokens %lld\n",
               (long long)p.n_vocab, (long long)p.n_audio_ctx, (long long)p.n_audio_state, (long long)p.n_audio_head, (long long)p.n_audio_layer,
               (long long)p.n_text_ctx, (long long)p.n_text_state, (long long)p.n_text_head, (long long)p.n_text_layer,
               (long long)p.n_mels, (long long)p.ftype, (long long)m.tensor_count, (long long)m.tokens);
        return 0;
    }
    double *bank = calloc((size_t)(p.n_mels * 201), sizeof *bank);
    if (!bank || psionic_whisper_bank(p.n_mels, (coax_v_f64){bank, p.n_mels * 201}) != 0) { fputs("psionic-whisper: mel bank\n", stderr); return 1; }
    if (strcmp(cmd, "filters") == 0) {
        printf("max_abs_diff %.3e\n", psionic_whisper_filter_diff(file, m, (coax_v_f64){bank, p.n_mels * 201}));
        return 0;
    }
    if (strcmp(cmd, "transcribe") != 0 || argc < 4) return usage();
    g_profile = getenv("PSIONIC_WHISPER_PROFILE") != 0;
    int format = 0; const char *lang = "en", *name_arg = 0, *prompt = "", *words = ""; long ctx = p.n_audio_ctx;
    coax_t_WeOptions opt = psionic_whisper_options();
    for (int i = 4; i < argc; i++) {
        if (strcmp(argv[i], "--text") == 0) format = 0;
        else if (strcmp(argv[i], "--srt") == 0) format = 1;
        else if (strcmp(argv[i], "--vtt") == 0) format = 2;
        else if (strcmp(argv[i], "--json") == 0) format = 3;
        else if (strcmp(argv[i], "--lines") == 0) format = 4;
        else if (strcmp(argv[i], "--language") == 0 && i + 1 < argc) lang = argv[++i];
        else if (strcmp(argv[i], "--name") == 0 && i + 1 < argc) name_arg = argv[++i];
        else if (strcmp(argv[i], "--threads") == 0 && i + 1 < argc) g_threads = (int)strtol(argv[++i], 0, 10);
        else if (strcmp(argv[i], "--beam") == 0 && i + 1 < argc) opt.beam = strtol(argv[++i], 0, 10);
        else if (strcmp(argv[i], "--best-of") == 0 && i + 1 < argc) opt.best_of = strtol(argv[++i], 0, 10);
        else if (strcmp(argv[i], "--no-fallback") == 0) opt.fallback = 0;
        else if (strcmp(argv[i], "--prompt") == 0 && i + 1 < argc) prompt = argv[++i];
        else if (strcmp(argv[i], "--words") == 0 && i + 1 < argc) words = argv[++i];
        else if (strcmp(argv[i], "--word-push") == 0 && i + 2 < argc) { opt.word_first = strtod(argv[i + 1], 0); opt.word_next = strtod(argv[i + 2], 0); i += 2; }
        else if (strcmp(argv[i], "--gpu") == 0) g_gpu = 1;
        else return usage();
    }
    pool_start();
    static int64_t offsets[TENSORS];
    coax_t_WeNet *net = malloc(sizeof *net);
    if (!net) return 1;
    *net = psionic_whisper_net(file, m, tv, (coax_v_s64){offsets, TENSORS}, ctx);
    if (net->status != COAX_WeStatus_ok) { fprintf(stderr, "psionic-whisper: model tensors (status %d)\n", (int)net->status); return 1; }
    if (g_gpu && !gpu_open()) { fputs("psionic-whisper: --gpu needs an Apple GPU (Metal) build; running on the CPU\n", stderr); g_gpu = 0; }
    float *W, *work;
    if (g_gpu) {
        W = gpu_alloc(0, (size_t)net->weights); work = gpu_alloc(1, (size_t)net->work.total);
        if (!gpu_alloc(2, (size_t)(p.n_audio_head * p.n_audio_ctx * p.n_audio_ctx))) { fputs("psionic-whisper: out of GPU memory\n", stderr); return 1; }
    }
    else { W = malloc((size_t)net->weights * sizeof *W); work = calloc((size_t)net->work.total, sizeof *work); }
    if (!W || !work) { fputs("psionic-whisper: out of memory\n", stderr); return 1; }
    coax_t_WeStatus ls = psionic_whisper_load(file, tv, m.tensor_count, (coax_v_s64){offsets, TENSORS}, (coax_v_f32){W, net->weights});
    if (ls != COAX_WeStatus_ok) { fprintf(stderr, "psionic-whisper: weights (status %d)\n", (int)ls); return 1; }
    if (g_gpu) gpu_halves(W, (size_t)net->weights);
    double t_load = now_ms();
    coax_v_u8 wav = map(argv[3]);
    if (!wav.data) { fprintf(stderr, "psionic-whisper: cannot map %s\n", argv[3]); return 1; }
    coax_t_WvInfo wi = psionic_whisper_wav_info(wav);
    if (wi.status != COAX_WvStatus_ok || wi.frames < 1) {
        fprintf(stderr, "psionic-whisper: %s is not a WAV file this reads (PCM 8/16/24/32-bit or float 32/64-bit)\n", argv[3]);
        return 65;
    }
    float *mono = malloc((size_t)wi.frames * sizeof *mono);
    if (!mono || psionic_whisper_wav_mono(wav, wi, (coax_v_f32){mono, wi.frames}) != wi.frames) return 1;
    int64_t samples = psionic_whisper_wav_length(wi.frames, wi.rate);
    float *audio = mono;
    if (wi.rate != 16000) {
        audio = malloc((size_t)(samples > 0 ? samples : 1) * sizeof *audio);
        if (!audio || psionic_whisper_resample((coax_v_f32){mono, wi.frames}, wi.rate, (coax_v_f32){audio, samples}) != samples) return 1;
    }
    if (samples < 1) { fputs("psionic-whisper: the audio is empty\n", stderr); return 65; }

    int64_t cache_n = psionic_whisper_cache_size(samples, p.n_mels);
    float *cache = malloc((size_t)cache_n * sizeof *cache);
    if (!cache) return 1;
    int64_t *vocab = malloc((size_t)m.tokens * sizeof *vocab);
    if (!vocab || psionic_whisper_vocab(file, m, (coax_v_s64){vocab, m.tokens}) != m.tokens) return 1;
    opt.language = strcmp(lang, "auto") == 0 ? -2 : psionic_whisper_language(*net, (coax_string){lang, (int64_t)strlen(lang)});
    opt.prompt = (coax_string){prompt, (int64_t)strlen(prompt)};
    opt.words = (coax_string){words, (int64_t)strlen(words)};
    static coax_t_WsSegment segs[SEGMENTS];
    size_t text_cap = 1 << 22;
    uint8_t *text = malloc(text_cap), *out = malloc(text_cap * 2);
    if (!text || !out) return 1;
    coax_t_WeRun run = psionic_whisper_transcribe(*net, (coax_v_f32){W, net->weights}, (coax_v_f32){work, net->work.total}, file, m,
                                                  (coax_v_s64){vocab, m.tokens}, (coax_v_f64){bank, p.n_mels * 201}, (coax_v_f32){audio, samples},
                                                  (coax_v_f32){cache, cache_n}, opt,
                                                  (coax_v_t_WsSegment){segs, SEGMENTS}, (coax_v_u8){text, (int64_t)text_cap});
    double t_end = now_ms();
    if (run.status != COAX_WeStatus_ok) { fprintf(stderr, "psionic-whisper: transcribe (status %d)\n", (int)run.status); return 1; }
    const char *name = strrchr(argv[2], '/'); name = name_arg ? name_arg : name ? name + 1 : argv[2];
    coax_string code = psionic_whisper_language_code(*net, run.language);
    if (code.count > 0) lang = code.data; else if (strcmp(lang, "auto") == 0) lang = "en";
    int64_t lang_n = code.count > 0 ? code.count : (int64_t)strlen(lang);
    int64_t n = psionic_whisper_format((coax_v_t_WsSegment){segs, run.segments}, format, (coax_string){name, (int64_t)strlen(name)},
                                       (coax_string){lang, lang_n}, (coax_v_u8){out, (int64_t)text_cap * 2});
    if (n < 0) { fputs("psionic-whisper: output too long\n", stderr); return 1; }
    fwrite(out, 1, (size_t)n, stdout);
    fprintf(stderr, "psionic-whisper: %.0f ms load, %.0f ms transcribe (%s%d threads, %lld windows, %lld tokens, %lld fallbacks, %.*s, %.1f s audio)\n",
            t_load - t0, t_end - t_load, g_gpu ? "GPU and " : "", g_threads, (long long)run.windows, (long long)run.tokens, (long long)run.fallbacks, (int)lang_n, lang,
            samples / 16000.0);
    if (g_profile) fprintf(stderr, "psionic-whisper profile: GPU %.0f ms (%d decoder steps, %.0f ms); ", g_gpu_ms, g_steps, g_step_ms);
    if (g_profile)
        fprintf(stderr, "decoder linear %.0f ms (%ld), attend %.0f ms (%ld); encoder linear %.0f ms (%ld), attend %.0f ms (%ld), mel %.0f ms (%ld), other %.0f ms\n",
                g_prof[4], g_calls[4], g_prof[5], g_calls[5], g_prof[0], g_calls[0], g_prof[1], g_calls[1], g_prof[3], g_calls[3],
                (t_end - t_load) - g_prof[0] - g_prof[1] - g_prof[2] - g_prof[3] - g_prof[4] - g_prof[5]);
    return 0;
}
#endif
