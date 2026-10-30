#define _POSIX_C_SOURCE 200809L
#include "deepseek-engram-memory.h"
#include "spans.h"
#include <sys/mman.h>
#include <sys/stat.h>
#include <unistd.h>
#include <errno.h>
#include <string.h>
_Static_assert(sizeof(size_t)>=8 && sizeof(off_t)>=8,"Engram file effects require 64-bit host extents");
typedef struct {const void *data;int64_t count;size_t width;} EgSpan;
#define EG_SPAN(x) (EgSpan){(x).data,(x).count,sizeof(*(x).data)}
static _Bool eg_alias(EgSpan *w,size_t nw,EgSpan *r,size_t nr) {
    for(size_t i=0;i<nw;i++) {
        for(size_t j=0;j<i;j++)if(psionic_spans_overlap(w[i].data,w[i].count,w[i].width,w[j].data,w[j].count,w[j].width))return 1;
        for(size_t j=0;j<nr;j++)if(psionic_spans_overlap(w[i].data,w[i].count,w[i].width,r[j].data,r[j].count,r[j].width))return 1;
    }return 0;
}
coax_t_EgHash psionic_eg_hash(coax_t_EgMap map,coax_t_EgHistory history,coax_v_s64 tokens,coax_v_u8 mask,coax_v_s64 output,coax_v_s64 stage) {
    EgSpan w[]={EG_SPAN(output),EG_SPAN(stage)},r[]={EG_SPAN(map.data),EG_SPAN(tokens),EG_SPAN(mask)};
    return eg_hash(map,history,tokens,mask,output,stage,eg_alias(w,2,r,3));
}
coax_t_EgResult psionic_eg_lookup(int fd,_Bool mapped,coax_t_EgTable table,coax_v_s64 ids,coax_v_u16 output,coax_v_u8 packet,coax_v_u16 stage) {
    EgSpan w[]={EG_SPAN(output),EG_SPAN(packet),EG_SPAN(stage)},r[]={EG_SPAN(ids)};
    uint8_t status=eg_lookup_ready(ids.count,packet.count,output.count,stage.count,eg_alias(w,3,r,1));
    if(status)return (coax_t_EgResult){.status=status};
    coax_t_EgRead reads[48],draft[48];
    coax_t_EgResult plan=eg_reads(table,ids,(coax_v_t_EgRead){reads,48},(coax_v_t_EgRead){draft,48},0);
    if(plan.status)return plan;
    struct stat st;if(fstat(fd,&st))return (coax_t_EgResult){.status=COAX_EgStatus_io};
    status=eg_observed(table,st.st_size);if(status)return (coax_t_EgResult){.status=status};
    void *base=MAP_FAILED;
    if(mapped) {
        base=mmap(NULL,(size_t)st.st_size,PROT_READ,MAP_PRIVATE,fd,0);
        if(base==MAP_FAILED)return (coax_t_EgResult){.status=COAX_EgStatus_io};
    }
    for(int64_t i=0;i<plan.written;i++) {
        coax_t_EgRead row=reads[i];
        if(mapped)memcpy(packet.data+row.destination,(const unsigned char *)base+row.offset,(size_t)row.bytes);
        else {
            int64_t done=0;
            while(done<row.bytes) {
                ssize_t got=pread(fd,packet.data+row.destination+done,(size_t)(row.bytes-done),(off_t)(row.offset+done));
                if(got<0&&errno==EINTR)continue;
                if(got<=0)return (coax_t_EgResult){.status=COAX_EgStatus_io};
                done+=got;
            }
        }
    }
    if(mapped&&munmap(base,(size_t)st.st_size))return (coax_t_EgResult){.status=COAX_EgStatus_io};
    return eg_decode(ids.count,packet,output,stage,0);
}
