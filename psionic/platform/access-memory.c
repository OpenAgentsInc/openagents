#include "access-memory.h"
#include "spans.h"
coax_t_PwResult psionic_access_decode(coax_t_PaView view,coax_v_f32 out,coax_v_f32 work) {
    bool overlap=psionic_spans_overlap(out.data,out.count,sizeof(float),work.data,work.count,sizeof(float)) ||
      psionic_spans_overlap(out.data,out.count,sizeof(float),view.data.data,view.data.count,sizeof(uint8_t)) ||
      psionic_spans_overlap(work.data,work.count,sizeof(float),view.data.data,view.data.count,sizeof(uint8_t));
    return pa_decode(view,out,work,overlap);
}
