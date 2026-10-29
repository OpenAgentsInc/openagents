/* C observes native addresses only. Coax owns model admission and execution. */
#include "deepseek-v2-memory.h"
#include "spans.h"
typedef struct {const void *data;int64_t count;size_t width;} DvSpan;
#define DV_SPAN(x) (DvSpan){(x).data,(x).count,sizeof(*(x).data)}
static _Bool dv_hit(DvSpan a,DvSpan b) {return psionic_spans_overlap(a.data,a.count,a.width,b.data,b.count,b.width);}
static _Bool dv_read(DvSpan *write,size_t n,DvSpan read) {for (size_t i=0;i<n;i++) if (dv_hit(write[i],read)) return 1;return 0;}
static _Bool dv_write(DvSpan *write,size_t n) {for (size_t i=0;i<n;i++) for (size_t j=0;j<i;j++) if (dv_hit(write[i],write[j])) return 1;return 0;}
coax_t_DsResult psionic_dv_decode(coax_v_u8 table,coax_v_s64 ids,coax_v_u8 out,coax_v_u8 stage) {
    DvSpan writes[]={DV_SPAN(out),DV_SPAN(stage)};
    _Bool alias=dv_write(writes,2)|dv_read(writes,2,DV_SPAN(table))|dv_read(writes,2,DV_SPAN(ids));
    return dv_decode_answer(table,ids,out,stage,alias);
}
coax_t_DsResult psionic_dv_step(coax_t_DvModel m,coax_t_MlMode mode,int64_t token,int64_t pos,
        coax_v_f32 cache,coax_t_DvOutput out,coax_t_DvWork work) {
    DvSpan writes[]={DV_SPAN(cache),DV_SPAN(out.logits),DV_SPAN(out.trace),DV_SPAN(out.routes),DV_SPAN(work.floats),DV_SPAN(work.scores),DV_SPAN(work.accumulators),DV_SPAN(work.routes)};
    _Bool alias=dv_write(writes,8);alias|=dv_read(writes,8,DV_SPAN(m.blocks));
    for (size_t i=0;i<3;i++) alias|=dv_read(writes,8,DV_SPAN(m.p.e[i]));
    for (int64_t layer=0;layer<m.blocks.count;layer++) {coax_t_DvBlock b=m.blocks.data[layer];
        for (size_t i=0;i<8;i++) alias|=dv_read(writes,8,DV_SPAN(b.p.e[i]));
        for (size_t i=0;i<3;i++) {alias|=dv_read(writes,8,DV_SPAN(b.dense.p.e[i]));alias|=dv_read(writes,8,DV_SPAN(b.shared.p.e[i]));}
        alias|=dv_read(writes,8,DV_SPAN(b.experts));
        for (int64_t id=0;id<b.experts.count;id++) for (size_t i=0;i<3;i++) alias|=dv_read(writes,8,DV_SPAN(b.experts.data[id].p.e[i]));
    }
    return dv_step(m,mode,token,pos,cache,out,work,alias);
}
coax_t_DsResult psionic_dv_bind(coax_t_DvProfile p,coax_v_t_DsFile files,coax_v_t_DvBlock blocks,
        coax_v_t_DvExpert experts,coax_v_t_DvBlock draft,coax_v_t_DvExpert de,coax_v_t_DvModel out) {
    DvSpan writes[]={DV_SPAN(blocks),DV_SPAN(experts),DV_SPAN(draft),DV_SPAN(de),DV_SPAN(out)};
    _Bool alias=dv_write(writes,5);alias|=dv_read(writes,5,DV_SPAN(files));alias|=dv_read(writes,5,DV_SPAN(p.pins));alias|=dv_read(writes,5,DV_SPAN(p.tensors));alias|=dv_read(writes,5,DV_SPAN(p.payload));
    for (int64_t i=0;i<files.count;i++) {alias|=dv_read(writes,5,DV_SPAN(files.data[i].name));alias|=dv_read(writes,5,DV_SPAN(files.data[i].data));}
    for (int64_t i=0;i<p.pins.count;i++) {alias|=dv_read(writes,5,DV_SPAN(p.pins.data[i].name));alias|=dv_read(writes,5,DV_SPAN(p.pins.data[i].sha));}
    for (int64_t i=0;i<p.tensors.count;i++) alias|=dv_read(writes,5,DV_SPAN(p.tensors.data[i].name));
    return dv_bind_profile(p,files,blocks,experts,draft,de,out,alias);
}
