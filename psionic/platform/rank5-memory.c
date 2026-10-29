#include "rank5-memory.h"
#include "spans.h"
coax_t_PwResult psionic_rank5_inspect(coax_v_u8 file,coax_v_t_P5Tensor out,coax_v_t_P5Tensor draft) {
    bool overlap=psionic_spans_overlap(file.data,file.count,sizeof(uint8_t),out.data,out.count,sizeof(coax_t_P5Tensor)) ||
        psionic_spans_overlap(file.data,file.count,sizeof(uint8_t),draft.data,draft.count,sizeof(coax_t_P5Tensor)) ||
        psionic_spans_overlap(out.data,out.count,sizeof(coax_t_P5Tensor),draft.data,draft.count,sizeof(coax_t_P5Tensor));
    return p5_inspect_checked(file,out,draft,overlap);
}
