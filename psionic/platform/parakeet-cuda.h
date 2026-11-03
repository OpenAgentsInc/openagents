/* CUDA effects for the Coax Parakeet encoder, the Linux counterpart of
 * parakeet-metal.h. The model is immutable while open. Calls serialize
 * internally; weights and device arenas stay resident until close. A missing
 * driver, NVRTC or device is an explicit refusal, never a CPU fallback.
 * Arithmetic and launch ordering live in psionic/lib/parakeet/{cuda,fast}.coax;
 * the driver and NVRTC are opened with dlopen, so building needs no CUDA. */
#ifndef PSIONIC_PARAKEET_CUDA_H
#define PSIONIC_PARAKEET_CUDA_H
#include <stdint.h>
int pk_cuda_available(void);
int pk_cuda_encode(const float *ew,int64_t ne,const float *x,int64_t nx,int64_t frames,float *work,int64_t nw,float *out,int64_t no);
void pk_cuda_close(void);
/* Discard the device and host work arenas after pending commands finish;
 * retain the uploaded weights, allocations and compiled kernels. */
void pk_cuda_clear_work(void);
const char *pk_cuda_error(void);
/* The last encode's device time (kernels and copies), and the one-time
 * kernel compile and weight upload. */
double pk_cuda_seconds(void);
double pk_cuda_compile_seconds(void);
double pk_cuda_upload_seconds(void);
/* The device's name, or "" before it is opened. */
const char *pk_cuda_device(void);
/* Verification entry: shape fields are mels/channels/dim/heads/hidden/kernel/
 * layers/vocab/xscale. A layer prefix of zero observes subsampling only.
 * It uses exactly the production Coax scheduler and generated kernels. */
int pk_cuda_encode_shape(const int64_t shape[9],const float *ew,int64_t ne,const float *x,int64_t nx,int64_t frames,float *out,int64_t no,int64_t prefix);
#endif
