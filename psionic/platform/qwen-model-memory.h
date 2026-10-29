#ifndef PSIONIC_QWEN_MODEL_MEMORY_H
#define PSIONIC_QWEN_MODEL_MEMORY_H
#include "qwen-model.h"
/* Values of the Coax QeKind discriminant. */
enum { PSIONIC_QE_LINEAR=0, PSIONIC_QE_FULL=1 };
coax_t_QeResult psionic_qwen_model_block(coax_t_QeShape,coax_v_t_QeLayer,coax_v_f32,coax_v_f32,int64_t,int64_t,coax_v_f32,coax_v_f32,coax_v_f32);
#endif
