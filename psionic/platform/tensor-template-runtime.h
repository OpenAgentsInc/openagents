#ifndef PSIONIC_TENSOR_TEMPLATE_RUNTIME_H
#define PSIONIC_TENSOR_TEMPLATE_RUNTIME_H
#include "tensor-ir-runtime.h"
typedef struct Ptg Ptg;
typedef struct {uint64_t hits,specializations,evictions,module_compiles;int64_t cached_bytes;} PtgStats;
coax_t_TiStatus ptg_open(coax_t_TgTemplate model,coax_t_TiTarget target,const char *module,size_t bytes,Ptg **out);
coax_t_TiStatus ptg_run(Ptg *model,coax_t_TgBinding binding,const float *input,int64_t count,float *out,int64_t capacity);
PtgStats ptg_stats(const Ptg *model);
coax_t_TiTarget ptg_target(const Ptg *model);
bool ptg_matches(const Ptg *model,coax_t_TgTemplate proposed,coax_t_TiTarget target);
coax_t_TiStatus ptg_close(Ptg **model);
#endif
