#include "decision-json-memory.h"
#include "spans.h"
/* The output must not overlap the probabilities, the option table or any option text. */
coax_t_PjResult psionic_answer_json(coax_t_QrKind kind,uint8_t fields,coax_v_f64 probabilities,coax_v_t_PtOption options,coax_v_u8 out){
    bool alias=psionic_spans_overlap(out.data,out.count,1,probabilities.data,probabilities.count,sizeof(double)) ||
        psionic_spans_overlap(out.data,out.count,1,options.data,options.count,sizeof(*options.data));
    for(int64_t i=0;!alias && i>=0 && i<options.count && options.count<=255;i++)
        alias=psionic_spans_overlap(out.data,out.count,1,options.data[i].name.data,options.data[i].name.count,1) ||
            psionic_spans_overlap(out.data,out.count,1,options.data[i].description.value.data,options.data[i].description.value.count,1);
    return pj_answer(kind,fields,probabilities,options,out,alias);
}
