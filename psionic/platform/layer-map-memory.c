#include "layer-map-memory.h"
#include "spans.h"
typedef struct {const void *data;int64_t count;size_t width;} Span;
#define SPAN(v) ((Span){(v).data,(v).count,sizeof(*(v).data)})
static bool touches(Span a,Span b) {return psionic_spans_overlap(a.data,a.count,a.width,b.data,b.count,b.width);}
static bool observes(Span *w,size_t nw,Span *r,size_t nr) {
    for(size_t i=0;i<nw;i++) {for(size_t j=i+1;j<nw;j++)if(touches(w[i],w[j]))return true;for(size_t j=0;j<nr;j++)if(touches(w[i],r[j]))return true;}return false;
}
coax_t_PmResult psionic_map_layer(coax_t_QeShape shape,int64_t layer,coax_v_t_P5Tensor inventory,coax_v_t_PaFile files,coax_v_f32 out,coax_v_f32 work) {
    Span w[]={SPAN(out),SPAN(work)},r[]={SPAN(inventory),SPAN(files)};
    bool alias=observes(w,2,r,2);
    if(inventory.count<1 || inventory.count>4096 || files.count<1 || files.count>16) return pm_layer(shape,layer,inventory,files,out,work,true);
    if(!alias) for(int64_t i=0;i<inventory.count;i++) {Span x[]={SPAN(inventory.data[i].name)};if(observes(w,2,x,1)){alias=true;break;}}
    if(!alias) for(int64_t i=0;i<files.count;i++) {Span x[]={SPAN(files.data[i].data)};if(observes(w,2,x,1)){alias=true;break;}}
    return pm_layer(shape,layer,inventory,files,out,work,alias);
}
