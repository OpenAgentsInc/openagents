#include <assert.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include "qwen-full-layer-memory.h"
#include "qwen-full-oracle.h"
#if __BYTE_ORDER__ != __ORDER_LITTLE_ENDIAN__
#error fixture uses little-endian F32 originals
#endif
static coax_v_f32 fv(const float *p,int64_t n) {return (coax_v_f32){(float *)p,n};}
static coax_v_u8 bv(const float *p,int64_t n) {return (coax_v_u8){(uint8_t *)p,n*(int64_t)sizeof(float)};}
static void near(const float *a,const float *b,int n) {for(int i=0;i<n;i++)assert(isfinite(a[i]) && fabsf(a[i]-b[i])<0.000003f);}
int main(void) {
    coax_t_QhShape s={3,2,3,{2,1,2,2,0.125f},COAX_QmDtype_f32};
    float down[6];memcpy(down,fixture_down,sizeof down);
    coax_t_QhWeights w={fv(fixture_inorm,3),fv(fixture_pnorm,3),fv(fixture_qnorm,2),fv(fixture_knorm,2),
        bv(fixture_qw,24),bv(fixture_kw,6),bv(fixture_vw,6),bv(fixture_mix,12),bv(fixture_gate,6),bv(fixture_up,6),bv(down,6)};
    float cache[13]={0},out[4],work[99];cache[12]=out[3]=work[98]=-9;
    const float *inputs[]={fixture_input0,fixture_input1,fixture_input2},*ropes[]={fixture_rope0,fixture_rope1,fixture_rope2};
    const float *outputs[]={fixture_output0,fixture_output1,fixture_output2},*caches[]={fixture_cache0,fixture_cache1,fixture_cache2};
    for(int t=0;t<3;t++) {
        coax_t_QhResult r=psionic_qwen_full_step(s,w,fv(inputs[t],3),fv(ropes[t],2),t,fv(cache,12),fv(out,4),fv(work,99));
        assert(r.status==COAX_QhStatus_ok && r.written==3);near(out,outputs[t],3);near(cache,caches[t],4*(t+1));
        assert(cache[12]==-9 && out[3]==-9 && work[98]==-9);
    }
    float old_cache[12],old_out[3];memcpy(old_cache,cache,sizeof old_cache);memcpy(old_out,out,sizeof old_out);
    down[5]=INFINITY;
    coax_t_QhResult r=psionic_qwen_full_step(s,w,fv(inputs[0],3),fv(ropes[0],2),0,fv(cache,12),fv(out,3),fv(work,99));
    assert(r.status==COAX_QhStatus_number && r.written==0 && !memcmp(old_cache,cache,sizeof old_cache) && !memcmp(old_out,out,sizeof old_out));
    w.q_norm=fv(cache,2);
    r=psionic_qwen_full_step(s,w,fv(inputs[0],3),fv(ropes[0],2),0,fv(cache,12),fv(out,3),fv(work,99));
    assert(r.status==COAX_QhStatus_alias && !memcmp(old_cache,cache,sizeof old_cache));
    puts("Qwen full layer: independent three-step outputs and KV caches pass");
    puts("Qwen full layer: late failure and nested alias preserve published state");
}
