/* Address observations only. Coax owns formats, budgets and publication. */
#include "deepseek-cache-memory.h"
#include "spans.h"
typedef struct {const void *data;int64_t count;size_t width;} DkSpan;
#define DK_SPAN(x) (DkSpan){(x).data,(x).count,sizeof(*(x).data)}
static _Bool dk_alias(DkSpan *write,size_t nw,DkSpan *read,size_t nr) {
    for(size_t i=0;i<nw;i++) {
        for(size_t j=0;j<i;j++) if(psionic_spans_overlap(write[i].data,write[i].count,write[i].width,write[j].data,write[j].count,write[j].width)) return 1;
        for(size_t j=0;j<nr;j++) if(psionic_spans_overlap(write[i].data,write[i].count,write[i].width,read[j].data,read[j].count,read[j].width)) return 1;
    }return 0;
}
coax_t_DkResult psionic_dk_pack(uint8_t kind,int64_t rows,int64_t width,coax_v_u16 input,coax_t_DkPacked output,coax_v_u8 stage) {
    DkSpan write[]={DK_SPAN(output.data),DK_SPAN(output.scales),DK_SPAN(stage)},read[]={DK_SPAN(input)};
    return dk_pack(kind,rows,width,input,output,stage,dk_alias(write,3,read,1));
}
coax_t_DkResult psionic_dk_unpack(uint8_t kind,int64_t rows,int64_t width,coax_t_DkPacked input,coax_v_u16 output,coax_v_u16 stage) {
    DkSpan write[]={DK_SPAN(output),DK_SPAN(stage)},read[]={DK_SPAN(input.data),DK_SPAN(input.scales)};
    return dk_unpack(kind,rows,width,input,output,stage,dk_alias(write,2,read,2));
}
