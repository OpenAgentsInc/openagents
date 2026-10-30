/* Psionic interchange ABI v1: LP64, little-endian IEEE binary32 hosts.
 * External callers use these PODs, never compiler-generated Coax records.
 * All data are borrowed for a call unless an explicit tensor lease is returned.
 * Functions require exclusive access; producer mutation must be coordinated.
 */
#ifndef PSIONIC_INTERCHANGE_H
#define PSIONIC_INTERCHANGE_H
#include <stdint.h>
#include <stddef.h>
#include "vendor/dlpack.h"
#ifdef __cplusplus
extern "C" {
#endif
enum { PS_OK, PS_ARGUMENT, PS_VERSION, PS_DTYPE, PS_SHAPE, PS_CAPACITY,
       PS_ALIAS, PS_NUMBER, PS_FORMAT, PS_NAME, PS_OWNERSHIP, PS_MEMORY,
       PS_RANGE, PS_TRAINING };
enum { PS_F32=0, PS_BF16=1, PS_READONLY=1, PS_DLPACK_LEGACY=0, PS_DLPACK_VERSIONED=1 };
typedef struct {
    uint32_t abi_version,dtype;
    int32_t rank;
    uint32_t flags;
    int64_t dims[4];
    const void *data;
    uint64_t byte_count;
} PsTensorV1;
typedef struct {
    char name[128];
    uint32_t name_bytes,dtype;
    int32_t rank;
    uint32_t reserved;
    int64_t dims[4];
    uint64_t data_offset,byte_count;
} PsParameterInfoV1;
typedef struct { const char *name; uint64_t name_bytes; PsTensorV1 tensor; } PsParameterV1;
typedef struct { uint32_t status,reserved; uint64_t written; } PsResultV1;
typedef struct PsTensorHandleV1 PsTensorHandleV1;
uint32_t psionic_interchange_abi_version(void);
PsResultV1 psionic_weights_size_v1(const PsParameterV1 *,uint64_t count);
PsResultV1 psionic_weights_write_v1(const PsParameterV1 *,uint64_t count,void *out,uint64_t capacity);
/* written is parameter count; offsets are absolute within the borrowed file. */
PsResultV1 psionic_weights_inspect_v1(const void *file,uint64_t bytes,PsParameterInfoV1 *out,uint64_t capacity);
uint32_t psionic_tensor_copy_v1(const PsTensorV1 *,PsTensorHandleV1 **out);
uint32_t psionic_tensor_view_v1(const PsTensorHandleV1 *,PsTensorV1 *out);
uint32_t psionic_tensor_retain_v1(PsTensorHandleV1 *,PsTensorHandleV1 **out);
uint32_t psionic_tensor_release_v1(PsTensorHandleV1 **);
/* Valid kind + nonnull carrier + empty output transfers the carrier even on
 * validation failure. Other argument errors do not consume it. Each carrier
 * is consumed once. Import is zero-copy CPU contiguous f32, rank <=4.
 */
uint32_t psionic_dlpack_import_v1(void *carrier,uint32_t kind,PsTensorHandleV1 **out);
/* Export owns a lease; the consumer must call its deleter exactly once.
 * Version 1.0, explicit strides, read-only flag. Original handle may be freed.
 */
uint32_t psionic_tensor_dlpack_v1(PsTensorHandleV1 *,DLManagedTensorVersioned **out);
void psionic_dlpack_dispose_v1(DLManagedTensorVersioned *);
uint64_t psionic_tensor_live_v1(void);
#ifdef __cplusplus
}
static_assert(sizeof(PsTensorV1)==64 && offsetof(PsTensorV1,data)==48,"tensor ABI v1");
static_assert(sizeof(PsParameterInfoV1)==192 && offsetof(PsParameterInfoV1,data_offset)==176 && sizeof(PsParameterV1)==80 && sizeof(PsResultV1)==16,"parameter ABI v1");
#else
_Static_assert(sizeof(PsTensorV1)==64 && offsetof(PsTensorV1,data)==48,"tensor ABI v1");
_Static_assert(sizeof(PsParameterInfoV1)==192 && offsetof(PsParameterInfoV1,data_offset)==176 && sizeof(PsParameterV1)==80 && sizeof(PsResultV1)==16,"parameter ABI v1");
#endif
#endif
