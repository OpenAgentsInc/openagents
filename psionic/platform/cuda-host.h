/* Device effects for Coax-generated CUDA kernels: load the driver, compile
   generated CUDA C with NVRTC, move bytes, launch, wait. No arithmetic or
   scheduling decisions live here; Coax makes them. Both libraries are opened
   with dlopen, so a build needs no CUDA headers and a machine without a GPU
   reports PC_ABSENT. */
#ifndef PSIONIC_CUDA_HOST_H
#define PSIONIC_CUDA_HOST_H
#include <stddef.h>
#include <stdint.h>

typedef enum {
    PC_OK = 0,
    PC_ABSENT = 1,    /* no driver library or no device */
    PC_NO_NVRTC = 2,  /* the driver is present but NVRTC is not */
    PC_COMPILE = 3,   /* NVRTC refused the source; see pc_log */
    PC_DRIVER = 4,    /* a driver call failed; see pc_log */
    PC_ARGUMENT = 5,  /* the host was asked for something malformed */
    PC_MEMORY = 6     /* an allocation failed */
} PcStatus;

typedef uint64_t PcBuffer; /* a device address */
typedef void *PcModule;
typedef void *PcKernel;
typedef void *PcStream;
typedef void *PcEvent;

typedef struct {
    int major, minor, sms;
    char name[128];
    size_t free_bytes, total_bytes;
    int driver_version;
} PcDeviceInfo;

/* Kernel arguments, in parameter order after the work-item count. */
typedef enum { PC_ARG_INT = 0, PC_ARG_U32 = 1, PC_ARG_F32 = 2, PC_ARG_F64 = 3, PC_ARG_VIEW = 4 } PcArgKind;
typedef struct {
    PcArgKind kind;
    int64_t i;      /* INT, U32 */
    double f;       /* F32, F64 */
    PcBuffer data;  /* VIEW: device address of element 0 */
    int64_t count;  /* VIEW: elements */
} PcArg;

PcStatus pc_open(PcDeviceInfo *info);
const char *pc_log(void); /* the last error or compiler log */
PcStatus pc_memory(size_t *free_bytes, size_t *total_bytes);

/* Compile generated CUDA C (coax cuda output) for this device and load it. */
PcStatus pc_compile(const char *source, size_t length, const char *name, PcModule *module, double *seconds);
PcStatus pc_kernel(PcModule module, const char *name, PcKernel *kernel);
PcStatus pc_unload(PcModule module);

PcStatus pc_alloc(size_t bytes, PcBuffer *buffer);
PcStatus pc_free(PcBuffer buffer);
PcStatus pc_host_alloc(size_t bytes, void **pinned);
PcStatus pc_host_free(void *pinned);
PcStatus pc_upload(PcBuffer dst, const void *src, size_t bytes, PcStream stream);
PcStatus pc_download(void *dst, PcBuffer src, size_t bytes, PcStream stream);
PcStatus pc_fill(PcBuffer dst, uint32_t value, size_t words, PcStream stream);

PcStatus pc_stream(PcStream *stream);
PcStatus pc_event(PcEvent *event);
PcStatus pc_destroy_stream(PcStream stream);
PcStatus pc_destroy_event(PcEvent event);
PcStatus pc_record(PcEvent event, PcStream stream);
PcStatus pc_wait(PcStream stream, PcEvent event);
PcStatus pc_event_seconds(PcEvent start, PcEvent end, double *seconds);
PcStatus pc_sync_stream(PcStream stream);
PcStatus pc_sync(void);

/* items work items in blocks of block threads (1..1024). */
PcStatus pc_launch(PcKernel kernel, int64_t items, int block, const PcArg *args, int count, PcStream stream);
/* A warp kernel: `warps` work items of 32 lanes each; block is a multiple of 32. */
PcStatus pc_launch_warps(PcKernel kernel, int64_t warps, int block, const PcArg *args, int count, PcStream stream);

#endif
