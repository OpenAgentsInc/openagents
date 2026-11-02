/* Platform effects for the Coax Parakeet encoder. The model is immutable
 * while open. Calls serialize internally; weights and GPU arenas stay
 * resident until close. GPU failure is explicit; this never falls back to CPU.
 * Arithmetic and launch ordering live in psionic/lib/parakeet/{device,fast}.coax. */
#ifndef PSIONIC_PARAKEET_METAL_H
#define PSIONIC_PARAKEET_METAL_H
#include <stdint.h>
int pk_metal_available(void);
int pk_metal_encode(const float *ew,int64_t ne,const float *x,int64_t nx,int64_t frames,float *work,int64_t nw,float *out,int64_t no);
void pk_metal_close(void);
const char *pk_metal_error(void);
double pk_metal_seconds(void);
/* Verification entry: shape fields are mels/channels/dim/heads/hidden/kernel/
 * layers/vocab/xscale. A layer prefix of zero observes subsampling only.
 * It uses exactly the production Coax scheduler and generated kernels. */
int pk_metal_encode_shape(const int64_t shape[9],const float *ew,int64_t ne,const float *x,int64_t nx,int64_t frames,float *out,int64_t no,int64_t prefix);
#endif
