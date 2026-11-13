/* A realm script's C runtime: see stdio.h. Only what becomes WebAssembly instructions: sin, cos, exp and
   log are Coax's own kernels in the emitted C; tan and pow would be libm imports, which the module check
   refuses. */
#ifndef RS_MATH_H
#define RS_MATH_H
#define isfinite(x) __builtin_isfinite(x)
#define isnan(x) __builtin_isnan(x)
#define isinf(x) __builtin_isinf(x)
#define fabs(x) __builtin_fabs(x)
#define fabsf(x) __builtin_fabsf(x)
#define sqrt(x) __builtin_sqrt(x)
#define sqrtf(x) __builtin_sqrtf(x)
#define floor(x) __builtin_floor(x)
#define floorf(x) __builtin_floorf(x)
#define INFINITY (__builtin_inff())
#define NAN (__builtin_nanf(""))
double tan(double x); float tanf(float x); double pow(double x, double y); float powf(float x, float y);
#endif
