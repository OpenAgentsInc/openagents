#define _POSIX_C_SOURCE 200809L
#include "deepseek-prefill-memory.h"
#include "deepseek-hot-memory.h"
#include "deepseek-expert-memory.h"
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <time.h>
#include <unistd.h>
#define V(p,n) ((coax_v_s64){(int64_t *)(p),(n)})
#define B(p,n) ((coax_v_u8){(uint8_t *)(p),(n)})
#define F(p,n) ((coax_v_f32){(float *)(p),(n)})
#define D(p,n) ((coax_v_f64){(double *)(p),(n)})
#define A(p,n) ((coax_v_t_PsionicAccumulator){(p),(n)})
static int64_t now(void){struct timespec t;assert(!clock_gettime(CLOCK_MONOTONIC,&t));return t.tv_sec*1000000000LL+t.tv_nsec;}
static void read_exact(const char *path,void *p,size_t n){FILE *f=fopen(path,"rb");assert(f&&fread(p,1,n,f)==n&&fgetc(f)==EOF);fclose(f);}
int main(int argc,char **argv){
    assert(argc==5);FILE *file=fopen(argv[1],"rb");assert(file);struct stat st;assert(!fstat(fileno(file),&st));coax_t_DsArtifact pin=de_test_original_pin();assert(st.st_size==pin.bytes);uint8_t *map=mmap(NULL,(size_t)st.st_size,PROT_READ,MAP_PRIVATE,fileno(file),0);assert(map!=MAP_FAILED);
    const int64_t n=4,d=5120,h=2304,k=6;coax_t_DeExpert experts[384];assert(de_test_original_all(B(map,st.st_size),(coax_v_t_DeExpert){experts,384})==384);coax_t_DeExpert shared=de_test_original(B(map,st.st_size),1);
    float *input=malloc((size_t)n*d*4),*expected=malloc((size_t)d*4),*baseline=malloc((size_t)n*d*4),*output=malloc(((size_t)n*d+1)*4);assert(input&&expected&&baseline&&output);read_exact(argv[2],input,(size_t)d*4);read_exact(argv[3],expected,(size_t)d*4);for(int t=1;t<n;t++)memcpy(input+t*d,input,(size_t)d*4);
    float logits[384],ls[384];double raw[384],ranked[384];coax_t_DrRoute route=de_test_original_route(B(map,st.st_size),F(input,d),F(logits,384),F(ls,384),D(raw,384),D(ranked,384));assert(!route.status&&route.count==k);
    int64_t ids[24];float weights[24];assert(de_test_route_values(route,V(ids,k),F(weights,k))==k);for(int t=1;t<n;t++){memcpy(ids+t*k,ids,(size_t)k*8);memcpy(weights+t*k,weights,(size_t)k*4);}
    coax_t_DePlan ep=de_plan(shared);float *sf=malloc((size_t)(ep.floats+2*d)*4);double *sq=malloc((size_t)ep.wide*8);coax_t_PsionicAccumulator *acc=malloc((size_t)d*sizeof(*acc));assert(sf&&sq&&acc);coax_t_DeWork serial={F(sf,ep.floats+2*d),D(sq,ep.wide)};
    int64_t began=now();for(int t=0;t<n;t++){coax_t_DeResult r=psionic_de_mixture((coax_v_t_DeExpert){experts,384},shared,F(input+t*d,d),V(ids+t*k,k),F(weights+t*k,k),10,F(baseline+t*d,d),serial,A(acc,d));assert(!r.status&&!memcmp(baseline+t*d,expected,(size_t)d*4));}int64_t serial_ns=now()-began;
    int64_t order[24],offsets[385],keys[384],so[24],os[385],ks[384];coax_t_DpResult groups=psionic_dp_groups(384,n,k,V(ids,24),F(weights,24),V(order,24),V(offsets,385),V(keys,384),V(so,24),V(os,385),V(ks,384));assert(!groups.status&&groups.groups==6);
    float *gx=malloc((size_t)n*d*4),*go=malloc((size_t)n*d*4),*contrib=malloc((size_t)n*k*d*4),*shared_out=malloc((size_t)n*d*4),*stage=malloc((size_t)n*d*4);coax_t_DpPlan p=dp_plan(shared,n);float *wf=malloc((size_t)p.floats*4);double *wq=malloc((size_t)p.wide*8);uint8_t *flags=malloc((size_t)p.flags);coax_t_DpWork work={F(wf,p.floats),D(wq,p.wide),B(flags,p.flags)};assert(gx&&go&&contrib&&shared_out&&stage&&wf&&wq&&flags);
    coax_t_DcPacket packet_plan=dc_packet(d,h);uint8_t *packet=malloc((size_t)packet_plan.bytes);assert(packet&&packet_plan.bytes==18800640);float route_work[4],ones[4]={1,1,1,1};uint8_t ready[384]={0};int64_t decoded=0;began=now();
    for(int group=0;group<groups.groups;group++){
        int64_t id=keys[group];coax_t_DeExpert e=experts[id];coax_v_u8 spans[]={e.gate.data,e.gate.scales,e.up.data,e.up.scales,e.down.data,e.down.scales};
        for(int j=0;j<6;j++){assert(spans[j].count==packet_plan.begin.e[j+1]-packet_plan.begin.e[j]);memcpy(packet+packet_plan.begin.e[j],spans[j].data,(size_t)spans[j].count);}coax_t_DcBound bound=dc_bind(d,h,B(packet,packet_plan.bytes));assert(!bound.status);
        coax_t_DpResult r=psionic_dp_gather(384,n,k,d,id,V(ids,24),F(weights,24),V(order,24),V(offsets,385),F(input,n*d),F(gx,n*d),F(route_work,n));assert(!r.status&&r.groups==n);
        r=psionic_dp_expert(bound.value,n,F(gx,n*d),F(route_work,n),10,F(go,n*d),work);assert(!r.status);decoded+=r.coefficients;
        r=psionic_dp_scatter(384,n,k,d,id,V(ids,24),V(order,24),V(offsets,385),F(go,n*d),F(contrib,n*k*d));assert(!r.status);ready[id]=1;
        printf("Original prefill: expert %lld packet %lld bytes staged once for %lld routed rows\n",(long long)id,(long long)packet_plan.bytes,(long long)n);
    }
    coax_t_DpResult r=psionic_dp_expert(shared,n,F(input,n*d),F(ones,n),10,F(shared_out,n*d),work);assert(!r.status);decoded+=r.coefficients;output[n*d]=77;
    r=psionic_dp_combine(384,n,k,d,V(ids,24),F(weights,24),B(ready,384),1,F(contrib,n*k*d),F(shared_out,n*d),F(output,n*d+1),F(stage,n*d));assert(!r.status&&!memcmp(output,baseline,(size_t)n*d*4)&&output[n*d]==77);int64_t grouped_ns=now()-began;
    FILE *result=fopen(argv[4],"wb");assert(result&&fwrite(output,1,(size_t)n*d*4,result)==(size_t)n*d*4);fclose(result);
    printf("Original prefill: full 384/6 routed plus shared, 4 tokens, 6 routed packet stages for 24 routed uses, %lld coefficient decodes versus %lld serial; all 20480 output words bit-identical to serial and retained independent reference; serial %lld ns, grouped %lld ns\n",(long long)decoded,(long long)(decoded*n),(long long)serial_ns,(long long)grouped_ns);
    free(input);free(expected);free(baseline);free(output);free(sf);free(sq);free(acc);free(gx);free(go);free(contrib);free(shared_out);free(stage);free(wf);free(wq);free(flags);free(packet);munmap(map,(size_t)st.st_size);fclose(file);return 0;
}
