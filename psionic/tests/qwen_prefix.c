#include <assert.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include "qwen-prefix-memory.h"
static coax_v_f32 fv(float *p,int64_t n) {return (coax_v_f32){p,n};}
int main(void) {
    coax_t_QpShape s={2,2,3,{1,1,1,1,1},{1,1,2,0,1}};coax_a32_u8 id={{0}};id.e[0]=1;
    coax_t_QpClaim claim={id,2,s};float src[44],dst[44],work[36];
    for(int i=0;i<44;i++){src[i]=(float)(i+1);dst[i]=-7;}src[28]=NAN;
    coax_t_QpResult r=psionic_qwen_fork(s,id,claim,fv(src,44),fv(dst,44),fv(work,36));
    assert(r.status==COAX_QpStatus_ok && r.written==36 && r.claim.prefix==2);
    for(int i=0;i<20;i++)assert(dst[i]==src[i]);
    for(int i=20;i<28;i++)assert(dst[i]==src[i]);
    for(int i=28;i<32;i++)assert(dst[i]==-7);
    for(int i=32;i<40;i++)assert(dst[i]==src[i]);
    for(int i=40;i<44;i++)assert(dst[i]==-7);
    float before[44];memcpy(before,dst,sizeof before);src[21]=NAN;
    r=psionic_qwen_fork(s,id,claim,fv(src,44),fv(dst,44),fv(work,36));
    assert(r.status==COAX_QpStatus_number && r.written==0 && !memcmp(before,dst,sizeof before));
    r=psionic_qwen_fork(s,id,claim,fv(src,44),fv(src,44),fv(work,36));
    assert(r.status==COAX_QpStatus_alias && r.written==0);
    puts("Qwen prefix: convolution, recurrent matrices and live KV prefixes fork independently");
    puts("Qwen prefix: unused KV tails, identity, alias and non-finite refusals are bounded");
}
