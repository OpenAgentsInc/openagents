#include "readout-memory.h"
#include "spans.h"
coax_t_QrResult psionic_readout(coax_t_QrKind kind,float temperature,coax_v_f32 logits,coax_v_s64 slots,coax_v_f64 out,coax_v_f64 work) {
    bool overlap=psionic_spans_overlap(out.data,out.count,sizeof(double),work.data,work.count,sizeof(double)) ||
        psionic_spans_overlap(out.data,out.count,sizeof(double),logits.data,logits.count,sizeof(float)) ||
        psionic_spans_overlap(out.data,out.count,sizeof(double),slots.data,slots.count,sizeof(int64_t)) ||
        psionic_spans_overlap(work.data,work.count,sizeof(double),logits.data,logits.count,sizeof(float)) ||
        psionic_spans_overlap(work.data,work.count,sizeof(double),slots.data,slots.count,sizeof(int64_t));
    return qr_read(kind,temperature,logits,slots,out,work,overlap);
}
