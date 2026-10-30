#ifndef PSIONIC_OPERATOR_RUNTIME_H
#define PSIONIC_OPERATOR_RUNTIME_H
#include "tensor-ir-runtime.h"
typedef struct Por Por;
typedef void (*PorCPU)(int64_t,coax_v_s64,float,coax_v_f32,coax_v_f32,coax_v_f32,coax_v_u8);
coax_t_TiStatus por_open(coax_t_TdDevice device,coax_t_TiTarget provenance,Por **out);
coax_t_TiStatus por_register(Por *registry,coax_t_OrSchema schema,PorCPU forward,PorCPU reference,const char *source,size_t bytes,int64_t *id);
coax_t_TiStatus por_compile(Por *registry,coax_t_TiProgram graph,coax_t_TiTarget target,bool reference,const char *base_module,size_t bytes,Pti **out);
coax_t_TiStatus por_close(Por **registry);
coax_t_OrRegistry por_info(const Por *registry);
#endif
