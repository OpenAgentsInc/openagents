#define _POSIX_C_SOURCE 200809L
#include "deepseek-hot-memory.h"
#include "spans.h"
#include <sys/mman.h>
#include <sys/stat.h>
#include <unistd.h>
#include <errno.h>
#include <string.h>
_Static_assert(sizeof(size_t)>=8&&sizeof(off_t)>=8,"Expert file effects require 64-bit extents");
typedef struct {const void *data;int64_t count;size_t width;} DcSpan;
#define DC_SPAN(x) (DcSpan){(x).data,(x).count,sizeof(*(x).data)}
static _Bool dc_alias(DcSpan *w,size_t nw,DcSpan *r,size_t nr) {
    for(size_t i=0;i<nw;i++) {
        for(size_t j=0;j<i;j++)if(psionic_spans_overlap(w[i].data,w[i].count,w[i].width,w[j].data,w[j].count,w[j].width))return 1;
        for(size_t j=0;j<nr;j++)if(psionic_spans_overlap(w[i].data,w[i].count,w[i].width,r[j].data,r[j].count,r[j].width))return 1;
    }return 0;
}
coax_t_DeResult psionic_dc_expert(int64_t width,int64_t hidden,coax_v_u8 packet,coax_v_f32 input,float route,float limit,coax_v_f32 output,coax_t_DeWork work) {
    coax_t_DcBound b=dc_bind(width,hidden,packet);if(b.status)return (coax_t_DeResult){.status=b.status};
    DcSpan w[]={DC_SPAN(output),DC_SPAN(work.floats),DC_SPAN(work.wide)},r[]={DC_SPAN(packet),DC_SPAN(input)};
    return de_expert(b.value,input,route,limit,output,work,dc_alias(w,3,r,2));
}
coax_t_DcResult psionic_dc_seed(coax_t_DcConfig c,coax_v_s64 profile,coax_v_s64 heat,coax_v_s64 slots,coax_v_s64 hw,coax_v_s64 sw) {
    DcSpan w[]={DC_SPAN(heat),DC_SPAN(slots),DC_SPAN(hw),DC_SPAN(sw)},r[]={DC_SPAN(profile),DC_SPAN(c.identity)};
    return dc_seed(c,profile,heat,slots,hw,sw,dc_alias(w,4,r,2));
}
coax_t_DcResult psionic_dc_dispatch(coax_t_DcConfig c,coax_t_DcState state,int64_t layer,coax_v_s64 ids,coax_v_t_DcLane out,coax_v_t_DcLane work) {
    DcSpan w[]={DC_SPAN(out),DC_SPAN(work)},r[]={DC_SPAN(state.heat),DC_SPAN(state.slots),DC_SPAN(ids),DC_SPAN(c.identity),DC_SPAN(state.head.identity)};
    return dc_dispatch(c,state,layer,ids,out,work,dc_alias(w,2,r,5));
}
coax_t_DcResult psionic_dc_prefetch(coax_t_DcConfig c,coax_t_DcState state,int64_t layer,coax_v_s64 ids,coax_v_s64 out,coax_v_s64 work) {
    DcSpan w[]={DC_SPAN(out),DC_SPAN(work)},r[]={DC_SPAN(state.heat),DC_SPAN(state.slots),DC_SPAN(ids),DC_SPAN(c.identity),DC_SPAN(state.head.identity)};
    return dc_prefetch(c,state,layer,ids,out,work,dc_alias(w,2,r,5));
}
coax_t_DcResult psionic_dc_finish(coax_t_DcConfig c,coax_t_DcState state,coax_v_s64 used,coax_v_s64 out,coax_v_s64 work) {
    DcSpan w[]={DC_SPAN(out),DC_SPAN(work)},r[]={DC_SPAN(state.heat),DC_SPAN(state.slots),DC_SPAN(used),DC_SPAN(c.identity),DC_SPAN(state.head.identity)};
    return dc_finish(c,state,used,out,work,dc_alias(w,2,r,5));
}
coax_t_DcPlan psionic_dc_plan(coax_t_DcConfig c,coax_t_DcState state,coax_v_s64 proposed,coax_v_t_DcMove moves,coax_v_s64 slots_work,coax_v_t_DcMove moves_work) {
    DcSpan w[]={DC_SPAN(proposed),DC_SPAN(moves),DC_SPAN(slots_work),DC_SPAN(moves_work)},r[]={DC_SPAN(state.heat),DC_SPAN(state.slots),DC_SPAN(c.identity),DC_SPAN(state.head.identity)};
    return dc_plan(c,state,proposed,moves,slots_work,moves_work,dc_alias(w,4,r,4));
}
coax_t_DcResult psionic_dc_install(coax_t_DcConfig c,coax_t_DcState state,coax_t_DcTicket ticket,coax_v_s64 proposed,coax_v_t_DcMove moves,coax_v_u8 ready,coax_v_u8 pending,coax_v_u8 resident) {
    DcSpan w[]={DC_SPAN(state.slots),DC_SPAN(resident)},r[]={DC_SPAN(state.heat),DC_SPAN(proposed),DC_SPAN(moves),DC_SPAN(ready),DC_SPAN(pending),DC_SPAN(c.identity),DC_SPAN(state.head.identity),DC_SPAN(ticket.identity)};
    return dc_install(c,state,ticket,proposed,moves,ready,pending,resident,dc_alias(w,2,r,8));
}
coax_t_DcResult psionic_dc_fetch(int fd,_Bool mapped,int64_t keys,int64_t stride,int64_t base,int64_t file_bytes,coax_v_s64 ids,coax_v_u8 output,coax_v_u8 work,coax_v_s64 offsets) {
    DcSpan w[]={DC_SPAN(output),DC_SPAN(work),DC_SPAN(offsets)},r[]={DC_SPAN(ids)};
    coax_t_DcResult ready=dc_fetch_ready(ids.count,stride,output.count,work.count,offsets.count,dc_alias(w,3,r,1));
    if(ready.status)return ready;
    int64_t draft[16512];
    coax_t_DcResult ranges=dc_ranges(keys,stride,base,file_bytes,ids,offsets,(coax_v_s64){draft,16512},0);
    if(ranges.status)return ranges;
    if(!ids.count)return ready;
    struct stat st;if(fstat(fd,&st)||dc_observed(file_bytes,st.st_size))return (coax_t_DcResult){.status=COAX_DsStatus_weights};
    void *mapping=MAP_FAILED;
    if(mapped){mapping=mmap(NULL,(size_t)st.st_size,PROT_READ,MAP_PRIVATE,fd,0);if(mapping==MAP_FAILED)return (coax_t_DcResult){.status=COAX_DsStatus_weights};}
    for(int64_t i=0;i<ids.count;i++) {
        if(mapped)memcpy(work.data+i*stride,(const unsigned char *)mapping+offsets.data[i],(size_t)stride);
        else {
            int64_t done=0;while(done<stride) {
                ssize_t got=pread(fd,work.data+i*stride+done,(size_t)(stride-done),(off_t)(offsets.data[i]+done));
                if(got<0&&errno==EINTR)continue;
                if(got<=0)return (coax_t_DcResult){.status=COAX_DsStatus_weights};done+=got;
            }
        }
    }
    if(mapped&&munmap(mapping,(size_t)st.st_size))return (coax_t_DcResult){.status=COAX_DsStatus_weights};
    return dc_fetch_commit((coax_v_u8){work.data,ready.written},output,0);
}
