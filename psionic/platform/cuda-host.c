/* Device effects for Coax-generated CUDA kernels (see cuda-host.h).
   The driver API and NVRTC are reached through dlopen; their few types are
   declared here, so no CUDA header or toolkit is needed to build. */
#define _GNU_SOURCE
#include "cuda-host.h"
#include <dlfcn.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

typedef int CUresult;
typedef int CUdevice;
typedef void *CUcontext;

static struct {
    void *cuda, *nvrtc;
    CUcontext context;
    CUdevice device;
    int major, minor, ready;
    char log[16384];
    CUresult (*cuInit)(unsigned);
    CUresult (*cuDriverGetVersion)(int *);
    CUresult (*cuDeviceGetCount)(int *);
    CUresult (*cuDeviceGet)(CUdevice *, int);
    CUresult (*cuDeviceGetName)(char *, int, CUdevice);
    CUresult (*cuDeviceGetAttribute)(int *, int, CUdevice);
    CUresult (*cuDevicePrimaryCtxRetain)(CUcontext *, CUdevice);
    CUresult (*cuCtxSetCurrent)(CUcontext);
    CUresult (*cuCtxSynchronize)(void);
    CUresult (*cuMemGetInfo)(size_t *, size_t *);
    CUresult (*cuModuleLoadDataEx)(PcModule *, const void *, unsigned, void *, void **);
    CUresult (*cuModuleUnload)(PcModule);
    CUresult (*cuModuleGetFunction)(PcKernel *, PcModule, const char *);
    CUresult (*cuMemAlloc)(PcBuffer *, size_t);
    CUresult (*cuMemFree)(PcBuffer);
    CUresult (*cuMemAllocHost)(void **, size_t);
    CUresult (*cuMemFreeHost)(void *);
    CUresult (*cuMemcpyHtoDAsync)(PcBuffer, const void *, size_t, PcStream);
    CUresult (*cuMemcpyDtoHAsync)(void *, PcBuffer, size_t, PcStream);
    CUresult (*cuMemsetD32Async)(PcBuffer, unsigned, size_t, PcStream);
    CUresult (*cuStreamCreate)(PcStream *, unsigned);
    CUresult (*cuStreamSynchronize)(PcStream);
    CUresult (*cuStreamDestroy)(PcStream);
    CUresult (*cuStreamBeginCapture)(PcStream, int);
    CUresult (*cuStreamEndCapture)(PcStream, void **);
    CUresult (*cuGraphInstantiate)(void **, void *, unsigned long long);
    CUresult (*cuGraphLaunch)(void *, PcStream);
    CUresult (*cuGraphDestroy)(void *);
    CUresult (*cuGraphExecDestroy)(void *);
    CUresult (*cuStreamWaitEvent)(PcStream, PcEvent, unsigned);
    CUresult (*cuEventCreate)(PcEvent *, unsigned);
    CUresult (*cuEventDestroy)(PcEvent);
    CUresult (*cuEventRecord)(PcEvent, PcStream);
    CUresult (*cuEventElapsedTime)(float *, PcEvent, PcEvent);
    CUresult (*cuLaunchKernel)(PcKernel, unsigned, unsigned, unsigned, unsigned, unsigned, unsigned, unsigned, PcStream, void **, void **);
    CUresult (*cuGetErrorString)(CUresult, const char **);
    int (*nvrtcCreateProgram)(void **, const char *, const char *, int, const char *const *, const char *const *);
    int (*nvrtcCompileProgram)(void *, int, const char *const *);
    int (*nvrtcGetPTXSize)(void *, size_t *);
    int (*nvrtcGetPTX)(void *, char *);
    int (*nvrtcGetProgramLogSize)(void *, size_t *);
    int (*nvrtcGetProgramLog)(void *, char *);
    int (*nvrtcDestroyProgram)(void **);
} g;

static void note(const char *fmt, ...) {
    va_list ap;
    va_start(ap, fmt);
    vsnprintf(g.log, sizeof g.log, fmt, ap);
    va_end(ap);
}
const char *pc_log(void) { return g.log; }

static PcStatus check(CUresult r, const char *what) {
    if (r == 0) return PC_OK;
    const char *text = "unknown";
    if (g.cuGetErrorString) g.cuGetErrorString(r, &text);
    note("%s failed: CUDA error %d (%s)", what, r, text ? text : "unknown");
    return PC_DRIVER;
}

#define LOAD(lib, sym, name)                                         \
    do {                                                             \
        *(void **)&g.sym = dlsym(lib, name);                         \
        if (!g.sym) { note("missing symbol %s", name); return 0; }   \
    } while (0)

static int load_cuda(void) {
    const char *paths[] = {getenv("PSIONIC_LIBCUDA"), "libcuda.so.1", "/run/opengl-driver/lib/libcuda.so.1", "/usr/lib/x86_64-linux-gnu/libcuda.so.1"};
    for (size_t i = 0; i < sizeof paths / sizeof *paths && !g.cuda; i++)
        if (paths[i] && *paths[i]) g.cuda = dlopen(paths[i], RTLD_NOW | RTLD_LOCAL);
    if (!g.cuda) { note("no CUDA driver library (libcuda.so.1)"); return 0; }
    LOAD(g.cuda, cuInit, "cuInit");
    LOAD(g.cuda, cuDriverGetVersion, "cuDriverGetVersion");
    LOAD(g.cuda, cuDeviceGetCount, "cuDeviceGetCount");
    LOAD(g.cuda, cuDeviceGet, "cuDeviceGet");
    LOAD(g.cuda, cuDeviceGetName, "cuDeviceGetName");
    LOAD(g.cuda, cuDeviceGetAttribute, "cuDeviceGetAttribute");
    LOAD(g.cuda, cuDevicePrimaryCtxRetain, "cuDevicePrimaryCtxRetain");
    LOAD(g.cuda, cuCtxSetCurrent, "cuCtxSetCurrent");
    LOAD(g.cuda, cuCtxSynchronize, "cuCtxSynchronize");
    LOAD(g.cuda, cuMemGetInfo, "cuMemGetInfo_v2");
    LOAD(g.cuda, cuModuleLoadDataEx, "cuModuleLoadDataEx");
    LOAD(g.cuda, cuModuleUnload, "cuModuleUnload");
    LOAD(g.cuda, cuModuleGetFunction, "cuModuleGetFunction");
    LOAD(g.cuda, cuMemAlloc, "cuMemAlloc_v2");
    LOAD(g.cuda, cuMemFree, "cuMemFree_v2");
    LOAD(g.cuda, cuMemAllocHost, "cuMemAllocHost_v2");
    LOAD(g.cuda, cuMemFreeHost, "cuMemFreeHost");
    LOAD(g.cuda, cuMemcpyHtoDAsync, "cuMemcpyHtoDAsync_v2");
    LOAD(g.cuda, cuMemcpyDtoHAsync, "cuMemcpyDtoHAsync_v2");
    LOAD(g.cuda, cuMemsetD32Async, "cuMemsetD32Async");
    LOAD(g.cuda, cuStreamCreate, "cuStreamCreate");
    LOAD(g.cuda, cuStreamSynchronize, "cuStreamSynchronize");
    LOAD(g.cuda, cuStreamDestroy, "cuStreamDestroy_v2");
    LOAD(g.cuda, cuStreamBeginCapture, "cuStreamBeginCapture_v2");
    LOAD(g.cuda, cuStreamEndCapture, "cuStreamEndCapture");
    LOAD(g.cuda, cuGraphInstantiate, "cuGraphInstantiateWithFlags");
    LOAD(g.cuda, cuGraphLaunch, "cuGraphLaunch");
    LOAD(g.cuda, cuGraphDestroy, "cuGraphDestroy");
    LOAD(g.cuda, cuGraphExecDestroy, "cuGraphExecDestroy");
    LOAD(g.cuda, cuEventDestroy, "cuEventDestroy_v2");
    LOAD(g.cuda, cuStreamWaitEvent, "cuStreamWaitEvent");
    LOAD(g.cuda, cuEventCreate, "cuEventCreate");
    LOAD(g.cuda, cuEventRecord, "cuEventRecord");
    LOAD(g.cuda, cuEventElapsedTime, "cuEventElapsedTime_v2");
    LOAD(g.cuda, cuLaunchKernel, "cuLaunchKernel");
    LOAD(g.cuda, cuGetErrorString, "cuGetErrorString");
    return 1;
}

static int load_nvrtc(void) {
    if (g.nvrtc) return 1;
    char home[4096] = "", cuda_home[4096] = "";
    const char *h = getenv("HOME"), *c = getenv("CUDA_HOME");
    if (h) snprintf(home, sizeof home, "%s/cuda-13.2/lib/libnvrtc.so.13", h);
    if (c) snprintf(cuda_home, sizeof cuda_home, "%s/lib/libnvrtc.so.13", c);
    const char *paths[] = {getenv("PSIONIC_NVRTC"), cuda_home, home, "libnvrtc.so.13", "libnvrtc.so"};
    for (size_t i = 0; i < sizeof paths / sizeof *paths && !g.nvrtc; i++)
        if (paths[i] && *paths[i]) g.nvrtc = dlopen(paths[i], RTLD_NOW | RTLD_LOCAL);
    if (!g.nvrtc) { note("no NVRTC library: set PSIONIC_NVRTC or CUDA_HOME"); return 0; }
    LOAD(g.nvrtc, nvrtcCreateProgram, "nvrtcCreateProgram");
    LOAD(g.nvrtc, nvrtcCompileProgram, "nvrtcCompileProgram");
    LOAD(g.nvrtc, nvrtcGetPTXSize, "nvrtcGetPTXSize");
    LOAD(g.nvrtc, nvrtcGetPTX, "nvrtcGetPTX");
    LOAD(g.nvrtc, nvrtcGetProgramLogSize, "nvrtcGetProgramLogSize");
    LOAD(g.nvrtc, nvrtcGetProgramLog, "nvrtcGetProgramLog");
    LOAD(g.nvrtc, nvrtcDestroyProgram, "nvrtcDestroyProgram");
    return 1;
}

PcStatus pc_open(PcDeviceInfo *info) {
    if (!g.ready) {
        if (!load_cuda()) return PC_ABSENT;
        if (check(g.cuInit(0), "cuInit")) return PC_ABSENT;
        int count = 0;
        if (check(g.cuDeviceGetCount(&count), "cuDeviceGetCount") || count < 1) { note("no CUDA device"); return PC_ABSENT; }
        if (check(g.cuDeviceGet(&g.device, 0), "cuDeviceGet")) return PC_DRIVER;
        if (check(g.cuDevicePrimaryCtxRetain(&g.context, g.device), "cuDevicePrimaryCtxRetain")) return PC_DRIVER;
        if (check(g.cuCtxSetCurrent(g.context), "cuCtxSetCurrent")) return PC_DRIVER;
        /* 75 and 76: compute capability major and minor. */
        if (check(g.cuDeviceGetAttribute(&g.major, 75, g.device), "cuDeviceGetAttribute")) return PC_DRIVER;
        if (check(g.cuDeviceGetAttribute(&g.minor, 76, g.device), "cuDeviceGetAttribute")) return PC_DRIVER;
        g.ready = 1;
    } else if (check(g.cuCtxSetCurrent(g.context), "cuCtxSetCurrent")) return PC_DRIVER;
    if (info) {
        memset(info, 0, sizeof *info);
        info->major = g.major;
        info->minor = g.minor;
        g.cuDeviceGetAttribute(&info->sms, 16, g.device); /* multiprocessor count */
        g.cuDeviceGetName(info->name, (int)sizeof info->name, g.device);
        g.cuDriverGetVersion(&info->driver_version);
        if (check(g.cuMemGetInfo(&info->free_bytes, &info->total_bytes), "cuMemGetInfo")) return PC_DRIVER;
    }
    return PC_OK;
}

PcStatus pc_memory(size_t *free_bytes, size_t *total_bytes) {
    if (!g.ready) return PC_ABSENT;
    return check(g.cuMemGetInfo(free_bytes, total_bytes), "cuMemGetInfo");
}

PcStatus pc_compile(const char *source, size_t length, const char *name, PcModule *module, double *seconds) {
    if (!g.ready) return PC_ABSENT;
    if (!source || !name || !module) return PC_ARGUMENT;
    if (!load_nvrtc()) return PC_NO_NVRTC;
    struct timespec a, b;
    clock_gettime(CLOCK_MONOTONIC, &a);
    char *text = malloc(length + 1);
    if (!text) return PC_MEMORY;
    memcpy(text, source, length);
    text[length] = 0;
    void *program = NULL;
    int r = g.nvrtcCreateProgram(&program, text, name, 0, NULL, NULL);
    free(text);
    if (r) { note("nvrtcCreateProgram failed: %d", r); return PC_COMPILE; }
    char arch[64];
    snprintf(arch, sizeof arch, "--gpu-architecture=compute_%d%d", g.major, g.minor);
    /* One rounding per operation, IEEE division and square root, no flushing. */
    const char *options[] = {arch, "--fmad=false", "--prec-div=true", "--prec-sqrt=true", "--ftz=false", "--std=c++17"};
    r = g.nvrtcCompileProgram(program, (int)(sizeof options / sizeof *options), options);
    size_t log_size = 0;
    g.nvrtcGetProgramLogSize(program, &log_size);
    g.log[0] = 0;
    if (log_size > 1) {
        char *log = malloc(log_size + 1);
        if (log) { g.nvrtcGetProgramLog(program, log); log[log_size] = 0; snprintf(g.log, sizeof g.log, "%s", log); free(log); }
    }
    if (r) { g.nvrtcDestroyProgram(&program); return PC_COMPILE; }
    size_t ptx_size = 0;
    g.nvrtcGetPTXSize(program, &ptx_size);
    char *ptx = malloc(ptx_size + 1);
    if (!ptx) { g.nvrtcDestroyProgram(&program); return PC_MEMORY; }
    g.nvrtcGetPTX(program, ptx);
    g.nvrtcDestroyProgram(&program);
    PcStatus s = check(g.cuModuleLoadDataEx(module, ptx, 0, NULL, NULL), "cuModuleLoadDataEx");
    free(ptx);
    clock_gettime(CLOCK_MONOTONIC, &b);
    if (seconds) *seconds = (double)(b.tv_sec - a.tv_sec) + 1e-9 * (double)(b.tv_nsec - a.tv_nsec);
    return s;
}

PcStatus pc_kernel(PcModule module, const char *name, PcKernel *kernel) {
    if (!g.ready) return PC_ABSENT;
    return check(g.cuModuleGetFunction(kernel, module, name), name);
}
PcStatus pc_unload(PcModule module) { return g.ready ? check(g.cuModuleUnload(module), "cuModuleUnload") : PC_ABSENT; }

PcStatus pc_alloc(size_t bytes, PcBuffer *buffer) {
    if (!g.ready) return PC_ABSENT;
    if (!bytes || !buffer) return PC_ARGUMENT;
    return check(g.cuMemAlloc(buffer, bytes), "cuMemAlloc") ? PC_MEMORY : PC_OK;
}
PcStatus pc_free(PcBuffer buffer) { return g.ready ? check(g.cuMemFree(buffer), "cuMemFree") : PC_ABSENT; }
PcStatus pc_host_alloc(size_t bytes, void **pinned) {
    if (!g.ready) return PC_ABSENT;
    return check(g.cuMemAllocHost(pinned, bytes), "cuMemAllocHost") ? PC_MEMORY : PC_OK;
}
PcStatus pc_host_free(void *pinned) { return g.ready ? check(g.cuMemFreeHost(pinned), "cuMemFreeHost") : PC_ABSENT; }
PcStatus pc_upload(PcBuffer dst, const void *src, size_t bytes, PcStream stream) {
    return g.ready ? check(g.cuMemcpyHtoDAsync(dst, src, bytes, stream), "cuMemcpyHtoDAsync") : PC_ABSENT;
}
PcStatus pc_download(void *dst, PcBuffer src, size_t bytes, PcStream stream) {
    return g.ready ? check(g.cuMemcpyDtoHAsync(dst, src, bytes, stream), "cuMemcpyDtoHAsync") : PC_ABSENT;
}
PcStatus pc_fill(PcBuffer dst, uint32_t value, size_t words, PcStream stream) {
    return g.ready ? check(g.cuMemsetD32Async(dst, value, words, stream), "cuMemsetD32Async") : PC_ABSENT;
}
PcStatus pc_stream(PcStream *stream) { return g.ready ? check(g.cuStreamCreate(stream, 1), "cuStreamCreate") : PC_ABSENT; }
PcStatus pc_event(PcEvent *event) { return g.ready ? check(g.cuEventCreate(event, 0), "cuEventCreate") : PC_ABSENT; }
PcStatus pc_record(PcEvent event, PcStream stream) { return g.ready ? check(g.cuEventRecord(event, stream), "cuEventRecord") : PC_ABSENT; }
PcStatus pc_wait(PcStream stream, PcEvent event) { return g.ready ? check(g.cuStreamWaitEvent(stream, event, 0), "cuStreamWaitEvent") : PC_ABSENT; }
PcStatus pc_event_seconds(PcEvent start, PcEvent end, double *seconds) {
    if (!g.ready) return PC_ABSENT;
    float ms = 0;
    PcStatus s = check(g.cuEventElapsedTime(&ms, start, end), "cuEventElapsedTime");
    if (seconds) *seconds = ms / 1000.0;
    return s;
}
PcStatus pc_sync_stream(PcStream stream) { return g.ready ? check(g.cuStreamSynchronize(stream), "cuStreamSynchronize") : PC_ABSENT; }
PcStatus pc_sync(void) { return g.ready ? check(g.cuCtxSynchronize(), "cuCtxSynchronize") : PC_ABSENT; }

static PcStatus launch(PcKernel kernel, int64_t items, int64_t threads, int block, const PcArg *args, int count, PcStream stream) {
    if (!g.ready) return PC_ABSENT;
    if (!kernel || items < 0 || block < 1 || block > 1024 || count < 0 || count > 30 || (count && !args)) return PC_ARGUMENT;
    if (items == 0) return PC_OK;
    int64_t blocks = (threads + block - 1) / block;
    if (blocks > 2147483647) return PC_ARGUMENT;
    /* Storage for each parameter, laid out as the generated kernel declares it. */
    struct { int64_t i; uint32_t u; float f; double d; struct { PcBuffer data; int64_t count; } v; } slot[31];
    void *pointers[31];
    slot[0].i = items;
    pointers[0] = &slot[0].i;
    for (int k = 0; k < count; k++) {
        const PcArg *a = &args[k];
        switch (a->kind) {
            case PC_ARG_INT: slot[k + 1].i = a->i; pointers[k + 1] = &slot[k + 1].i; break;
            case PC_ARG_U32: if (a->i < 0 || a->i > 4294967295LL) return PC_ARGUMENT; slot[k + 1].u = (uint32_t)a->i; pointers[k + 1] = &slot[k + 1].u; break;
            case PC_ARG_F32: slot[k + 1].f = (float)a->f; pointers[k + 1] = &slot[k + 1].f; break;
            case PC_ARG_F64: slot[k + 1].d = a->f; pointers[k + 1] = &slot[k + 1].d; break;
            case PC_ARG_VIEW: if (a->count < 0) return PC_ARGUMENT; slot[k + 1].v.data = a->data; slot[k + 1].v.count = a->count; pointers[k + 1] = &slot[k + 1].v; break;
            default: return PC_ARGUMENT;
        }
    }
    return check(g.cuLaunchKernel(kernel, (unsigned)blocks, 1, 1, (unsigned)block, 1, 1, 0, stream, pointers, NULL), "cuLaunchKernel");
}
PcStatus pc_launch(PcKernel kernel, int64_t items, int block, const PcArg *args, int count, PcStream stream) {
    return launch(kernel, items, items, block, args, count, stream);
}
PcStatus pc_launch_warps(PcKernel kernel, int64_t warps, int block, const PcArg *args, int count, PcStream stream) {
    if (block % 32 != 0 || warps > INT64_MAX / 32) return PC_ARGUMENT;
    return launch(kernel, warps, warps * 32, block, args, count, stream);
}

PcStatus pc_destroy_stream(PcStream stream) { return g.ready ? check(g.cuStreamDestroy(stream), "cuStreamDestroy") : PC_ABSENT; }
PcStatus pc_destroy_event(PcEvent event) { return g.ready ? check(g.cuEventDestroy(event), "cuEventDestroy") : PC_ABSENT; }

/* Record the work issued to `stream` (and to streams that join it through
   events) without running it, then replay the recording as one launch. */
PcStatus pc_capture_begin(PcStream stream) {
    /* 2: CU_STREAM_CAPTURE_MODE_RELAXED */
    return g.ready ? check(g.cuStreamBeginCapture(stream, 2), "cuStreamBeginCapture") : PC_ABSENT;
}
PcStatus pc_capture_end(PcStream stream, PcGraph *graph) {
    if (!g.ready) return PC_ABSENT;
    void *recorded = NULL;
    PcStatus s = check(g.cuStreamEndCapture(stream, &recorded), "cuStreamEndCapture");
    if (s) return s;
    s = check(g.cuGraphInstantiate(graph, recorded, 0), "cuGraphInstantiate");
    g.cuGraphDestroy(recorded);
    return s;
}
PcStatus pc_replay(PcGraph graph, PcStream stream) { return g.ready ? check(g.cuGraphLaunch(graph, stream), "cuGraphLaunch") : PC_ABSENT; }
PcStatus pc_graph_free(PcGraph graph) { return g.ready ? check(g.cuGraphExecDestroy(graph), "cuGraphExecDestroy") : PC_ABSENT; }
