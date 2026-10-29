#include <assert.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include "qwen-decision-memory.h"
#include "qwen-decision-oracle.h"
#if __BYTE_ORDER__ != __ORDER_LITTLE_ENDIAN__
#error fixture uses little-endian F32 originals
#endif
static coax_v_f32 fv(const float *p,int64_t n) {return (coax_v_f32){(float *)p,n};}
static coax_v_u8 bv(const float *p,int64_t n) {return (coax_v_u8){(uint8_t *)p,n*(int64_t)sizeof(float)};}
static void near(const float *a,const float *b,int n) {for(int i=0;i<n;i++)assert(isfinite(a[i]) && fabsf(a[i]-b[i])<0.000003f);}
int main(void) {
    coax_t_QhShape s={3,2,3,{2,1,2,2,0.125f},COAX_QmDtype_f32};float down[6];memcpy(down,fixture_down,sizeof down);
    coax_t_QhWeights w={fv(fixture_inorm,3),fv(fixture_pnorm,3),fv(fixture_qnorm,2),fv(fixture_knorm,2),bv(fixture_qw,24),bv(fixture_kw,6),bv(fixture_vw,6),bv(fixture_mix,12),bv(fixture_gate,6),bv(fixture_up,6),bv(down,6)};
    float cache[12]={0},out[6],work[110];work[109]=-9;
    coax_t_QhResult r=psionic_qwen_decision_block(s,w,fv(fixture_x,6),fv(fixture_rope,4),0,2,fv(cache,12),fv(out,6),fv(work,110));
    assert(r.status==COAX_QhStatus_ok && r.written==6 && work[109]==-9);near(out,fixture_prefix,6);near(cache,fixture_prefix_cache,8);
    float a[12],b[12],old[12];memcpy(a,cache,sizeof a);memcpy(b,cache,sizeof b);memcpy(old,cache,sizeof old);
    r=psionic_qwen_decision_block(s,w,fv(fixture_xa,3),fv(fixture_ra,2),2,1,fv(a,12),fv(out,3),fv(work,110));
    assert(r.status==COAX_QhStatus_ok);near(out,fixture_ao,3);near(a,fixture_ac,12);
    r=psionic_qwen_decision_block(s,w,fv(fixture_xb,3),fv(fixture_rb,2),2,1,fv(b,12),fv(out,3),fv(work,110));
    assert(r.status==COAX_QhStatus_ok);near(out,fixture_bo,3);near(b,fixture_bc,12);assert(!memcmp(old,cache,sizeof old));
    float old_out[3];memcpy(old_out,out,sizeof old_out);down[5]=INFINITY;
    r=psionic_qwen_decision_block(s,w,fv(fixture_xa,3),fv(fixture_ra,2),2,1,fv(cache,12),fv(out,3),fv(work,110));
    assert(r.status==COAX_QhStatus_number && !memcmp(old,cache,sizeof old) && !memcmp(old_out,out,sizeof old_out));
    puts("Qwen decision: bidirectional prefix and independent suffix forks match references");
    puts("Qwen decision: late failures preserve prefix cache and output");
}
