#include <assert.h>
#include <float.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include "attention-memory.h"
#include "attention-oracle.h"
_Static_assert(FLT_RADIX==2 && FLT_MANT_DIG==24 && DBL_MANT_DIG==53,"binary32/64 required");
static coax_v_f32 view(float *p,int64_t n) {return (coax_v_f32){p,n};}
static void close_values(const float *got,const float *want,size_t n) {
    for(size_t i=0;i<n;i++) assert(isfinite(got[i]) && fabsf(got[i]-want[i]) < 0.000002f);
}
static void case_check(coax_t_PaShape s,const float *wf,size_t nw,const float *rf,size_t nr,
        const float *xf,const float *expected,const float *expected_cache) {
    float w[4000],rope[256],x[256],full[260],incremental[260],cache[516];
    assert(nw<4000 && nr<256);memcpy(w,wf,nw*sizeof(float));memcpy(rope,rf,nr*sizeof(float));
    size_t nx=(size_t)(s.capacity*s.width), nc=(size_t)(2*s.capacity*s.kv_heads*s.head_width);
    memcpy(x,xf,nx*sizeof(float));for(size_t i=0;i<260;i++) full[i]=incremental[i]=-9;
    for(size_t i=0;i<516;i++) cache[i]=-7;
    coax_t_PaResult r=psionic_attention_forward(s,view(w,nw),view(rope,nr),view(x,nx),s.capacity,view(full,nx+1));
    assert(r.status==COAX_PaStatus_ok && r.written==(int64_t)nx && full[nx]==-9);
    close_values(full,expected,nx);
    for(int64_t p=0;p<s.capacity;p++) {
        r=psionic_attention_step(s,view(w,nw),view(rope,nr),view(x+p*s.width,s.width),p,view(cache,nc+1),view(incremental+p*s.width,s.width));
        assert(r.status==COAX_PaStatus_ok && r.written==s.width);
    }
    close_values(incremental,expected,nx); close_values(cache,expected_cache,nc);
    assert(!memcmp(full,incremental,nx*sizeof(float)) && cache[nc]==-7);
    /* Changing a future row must not change any earlier result. */
    for(int64_t i=0;i<s.width;i++) x[(s.capacity-1)*s.width+i]+=4;
    r=psionic_attention_forward(s,view(w,nw),view(rope,nr),view(x,nx),s.capacity,view(incremental,nx));
    assert(r.status==COAX_PaStatus_ok && !memcmp(full,incremental,(nx-(size_t)s.width)*sizeof(float)));
}
static void refusals(void) {
    coax_t_PaShape s={2,1,1,2,1,2,1};float w[26]={0},rope[4]={1,0,1,0},x[4]={1,2,3,4};
    float cache[8],out[4];for(int i=0;i<8;i++)cache[i]=-9;for(int i=0;i<4;i++)out[i]=-7;
    coax_t_PaResult r=psionic_attention_step(s,view(w,26),view(rope,4),view(x,2),0,view(cache,8),view(cache+1,2));
    assert(r.status==COAX_PaStatus_alias && r.written==0);
    r=psionic_attention_forward(s,view(w,26),view(rope,4),view(x,4),2,view(x+1,3));
    assert(r.status==COAX_PaStatus_capacity && r.written==0);
    r=psionic_attention_forward(s,view(w,26),view(rope,4),view(x,4),2,view(x,4));
    assert(r.status==COAX_PaStatus_alias && r.written==0);
    x[1]=NAN;r=psionic_attention_step(s,view(w,26),view(rope,4),view(x,2),0,view(cache,8),view(out,4));
    assert(r.status==COAX_PaStatus_number && r.written==0);x[1]=2;
    w[0]=INFINITY;r=psionic_attention_step(s,view(w,26),view(rope,4),view(x,2),0,view(cache,8),view(out,4));
    assert(r.status==COAX_PaStatus_number);w[0]=0;
    rope[0]=NAN;r=psionic_attention_step(s,view(w,26),view(rope,4),view(x,2),0,view(cache,8),view(out,4));
    assert(r.status==COAX_PaStatus_number);rope[0]=1;
    cache[0]=NAN;r=psionic_attention_step(s,view(w,26),view(rope,4),view(x,2),1,view(cache,8),view(out,4));
    assert(r.status==COAX_PaStatus_number);cache[0]=-9;
    s.heads=INT64_MAX;r=psionic_attention_step(s,view(w,26),view(rope,4),view(x,2),0,view(cache,8),view(out,4));
    assert(r.status==COAX_PaStatus_shape);s.heads=1;
    r=psionic_attention_step(s,view(w,26),view(rope,4),view(x,2),0,view(cache,7),view(out,4));
    assert(r.status==COAX_PaStatus_capacity);
    r=psionic_attention_step(s,view(w,26),view(rope,4),view(x,2),0,view(cache,8),view(w,2));
    assert(r.status==COAX_PaStatus_alias);
    r=psionic_attention_step(s,view(w,26),view(rope,4),view(x,2),0,view(cache,8),view(rope,2));
    assert(r.status==COAX_PaStatus_alias);
    r=psionic_attention_step(s,view(w,26),view(rope,4),view(x,2),0,view(cache,8),view(x,2));
    assert(r.status==COAX_PaStatus_alias);
    r=psionic_attention_step(s,view(w,26),view(rope,4),view(x,2),0,view(w,8),view(out,4));
    assert(r.status==COAX_PaStatus_alias);
    w[0]=FLT_MAX;x[0]=FLT_MAX;x[1]=0;
    r=psionic_attention_step(s,view(w,26),view(rope,4),view(x,2),0,view(cache,8),view(out,4));
    assert(r.status==COAX_PaStatus_arithmetic && r.written==0);
    for(int i=0;i<4;i++) assert(out[i]==-7);
    for(int i=0;i<8;i++) assert(cache[i]==-9);
}
#define CASE(i,d,h,kv,hd,ff) case_check((coax_t_PaShape){d,h,kv,hd,ff,3,0.125f},fixture_##i##_w,sizeof fixture_##i##_w/sizeof(float),fixture_##i##_rope,sizeof fixture_##i##_rope/sizeof(float),fixture_##i##_x,fixture_##i##_out,fixture_##i##_cache)
int main(void) {
    CASE(0,4,2,1,2,3);CASE(1,4,1,1,4,5);CASE(2,6,3,3,2,4);
    refusals();puts("attention: independent dense, GQA, RoPE, causal and cached fixtures pass");
    puts("attention: non-finite, shape, alias and capacity refusals preserve published storage");
    return 0;
}
