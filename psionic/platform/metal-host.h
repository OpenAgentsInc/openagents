/* Device effects for Coax-generated Metal compute kernels (coax
   metal-compute): open the GPU, compile generated Metal Shading Language,
   allocate shared buffers, encode launches, wait. No arithmetic or scheduling
   decisions live here; Coax makes them. There is no CPU fallback: a machine
   without a Metal GPU reports PM_ABSENT, and a launch whose Coax checks
   failed reports PM_CHECK and must not be used.

   Launch ABI (coax metal-compute): buffer(0) is one constant argument block,
   the work-item count (8 bytes) and then each parameter after the work item
   in an 8-byte slot (a view takes 16: the device address of element 0, then
   the element count; an f64 is two f32, hi then lo); buffer(1) is the
   launch's two error words (failed checks, largest failing source line). */
#ifndef PSIONIC_METAL_HOST_H
#define PSIONIC_METAL_HOST_H
#include <stddef.h>
#include <stdint.h>

typedef enum {
    PM_OK = 0,
    PM_ABSENT = 1,   /* no Metal device */
    PM_COMPILE = 3,  /* the Metal compiler refused the source; see pm_log */
    PM_DEVICE = 4,   /* a command buffer failed; see pm_log */
    PM_ARGUMENT = 5, /* the host was asked for something malformed */
    PM_MEMORY = 6,   /* an allocation failed */
    PM_CHECK = 7     /* a Coax check failed inside a kernel; results are void */
} PmStatus;

typedef void *PmBuffer; /* a shared MTLBuffer: the CPU and GPU see the same bytes */
typedef void *PmModule; /* an MTLLibrary */
typedef void *PmKernel; /* an MTLComputePipelineState */
typedef void *PmBatch;  /* one command buffer with one serial compute encoder */

typedef struct {
    char name[128];
    size_t working_set_bytes; /* recommendedMaxWorkingSetSize */
    size_t max_buffer_bytes;  /* maxBufferLength */
    size_t allocated_bytes;   /* currentAllocatedSize when asked */
    int unified_memory, apple_family; /* the highest MTLGPUFamilyAppleN supported */
} PmDeviceInfo;

/* Kernel arguments after the work item, in parameter order; the same kinds as
   the CUDA host (PcArgKind) and the device plan's DpArg. */
typedef enum { PM_ARG_INT = 0, PM_ARG_U32 = 1, PM_ARG_F32 = 2, PM_ARG_F64 = 3, PM_ARG_VIEW = 4 } PmArgKind;
typedef struct {
    PmArgKind kind;
    int64_t i;        /* INT, U32 */
    double f;         /* F32, F64 */
    PmBuffer buffer;  /* VIEW: the buffer */
    int64_t offset;   /* VIEW: byte offset of element 0 */
    int64_t count;    /* VIEW: elements */
} PmArg;

PmStatus pm_open(PmDeviceInfo *info);
const char *pm_log(void); /* the last error or compiler log */
PmStatus pm_memory(PmDeviceInfo *info);

/* Compile generated Metal (coax metal-compute output) with fast math off. */
PmStatus pm_compile(const char *source, size_t length, const char *name, PmModule *module, double *seconds);
PmStatus pm_kernel(PmModule module, const char *name, PmKernel *kernel);
/* Release only after every batch using these objects has completed. */
PmStatus pm_kernel_free(PmKernel kernel);
PmStatus pm_unload(PmModule module);

/* Shared storage: pm_contents is the CPU address of the same bytes. */
PmStatus pm_alloc(size_t bytes, PmBuffer *buffer);
PmStatus pm_free(PmBuffer buffer);
void *pm_contents(PmBuffer buffer);

/* A batch encodes launches in order; each launch sees every earlier launch's
   writes. pm_wait commits, waits and reports the first failure: the command
   buffer's error, or the Coax checks that failed inside it. */
PmStatus pm_begin(PmBatch *batch);
/* items work items; lanes is 1, 32 for a warp kernel (one simdgroup per
   item; block threads per threadgroup, a multiple of 32 lowered to what the
   pipeline allows), or a group kernel's size T (a multiple of 32 above 32,
   up to 1024: one threadgroup of exactly T threads per item; block is
   ignored). A group kernel of 32 threads is launched with lanes 32 and block
   32. */
PmStatus pm_launch(PmBatch batch, PmKernel kernel, int64_t items, int lanes, int block, const PmArg *args, int count);
PmStatus pm_wait(PmBatch batch, double *gpu_seconds);

#endif
