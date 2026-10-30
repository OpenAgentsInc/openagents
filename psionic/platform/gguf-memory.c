/* C observes borrowed spans; all wire parsing and admission are Coax. */
#include "gguf-memory.h"
#include "spans.h"
coax_t_PgResult psionic_pg_read(coax_v_u8 header,int64_t file_bytes,coax_v_t_PgMeta meta,coax_v_t_PgMeta mw,coax_v_t_PgTensor tensors,coax_v_t_PgTensor tw){
    const void *p[]={meta.data,mw.data,tensors.data,tw.data};int64_t n[]={meta.count,mw.count,tensors.count,tw.count};size_t w[]={sizeof(*meta.data),sizeof(*mw.data),sizeof(*tensors.data),sizeof(*tw.data)};_Bool alias=0;
    for(size_t i=0;i<4;i++){alias|=psionic_spans_overlap(p[i],n[i],w[i],header.data,header.count,1);for(size_t j=0;j<i;j++)alias|=psionic_spans_overlap(p[i],n[i],w[i],p[j],n[j],w[j]);}
    return pg_read(header,file_bytes,meta,mw,tensors,tw,alias);
}
