#include <assert.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include "qwen-linear-layer-memory.h"
#include "qwen-linear-oracle.h"
#if __BYTE_ORDER__ != __ORDER_LITTLE_ENDIAN__
#error fixture uses little-endian F32 originals
#endif
static coax_v_f32 fv(const float *p,int64_t n) {return (coax_v_f32){(float *)p,n};}
static coax_v_u8 bv(const float *p,int64_t n) {return (coax_v_u8){(uint8_t *)p,n*(int64_t)sizeof(float)};}
static void near(const float *a,const float *b,int n) {for(int i=0;i<n;i++)assert(isfinite(a[i]) && fabsf(a[i]-b[i])<0.000003f);}
int main(void) {
    coax_t_QlShape s={3,2,{1,2,2,2,0.125f},COAX_QmDtype_f32};
    float down[6];memcpy(down,fixture_down,sizeof down);
    coax_t_QlWeights w={fv(fixture_inorm,3),fv(fixture_pnorm,3),fv(fixture_conv,32),fv(fixture_alog,2),fv(fixture_dt,2),fv(fixture_gamma,2),
        bv(fixture_qkv,24),bv(fixture_z,12),bv(fixture_a,6),bv(fixture_b,6),bv(fixture_mixer,12),bv(fixture_gate,6),bv(fixture_up,6),bv(down,6)};
    float history[25],state[9],out[4],work[175];memcpy(history,fixture_initial_history,sizeof fixture_initial_history);memcpy(state,fixture_initial_state,sizeof fixture_initial_state);
    history[24]=state[8]=out[3]=work[174]=-9;
    const float *inputs[]={fixture_input0,fixture_input1},*outputs[]={fixture_output0,fixture_output1};
    const float *histories[]={fixture_history0,fixture_history1},*states[]={fixture_state0,fixture_state1};
    for(int t=0;t<2;t++) {
        coax_t_QlResult r=psionic_qwen_linear_step(s,w,fv(inputs[t],3),fv(history,24),fv(state,8),fv(out,4),fv(work,175));
        assert(r.status==COAX_QlStatus_ok && r.written==3);near(out,outputs[t],3);near(history,histories[t],24);near(state,states[t],8);
        assert(history[24]==-9 && state[8]==-9 && out[3]==-9 && work[174]==-9);
    }
    float oh[24],os[8],oo[3];memcpy(oh,history,sizeof oh);memcpy(os,state,sizeof os);memcpy(oo,out,sizeof oo);
    down[5]=INFINITY;
    coax_t_QlResult r=psionic_qwen_linear_step(s,w,fv(inputs[0],3),fv(history,24),fv(state,8),fv(out,3),fv(work,174));
    assert(r.status==COAX_QlStatus_number && r.written==0 && !memcmp(oh,history,sizeof oh) && !memcmp(os,state,sizeof os) && !memcmp(oo,out,sizeof oo));
    w.delta_norm=fv(history,2);
    r=psionic_qwen_linear_step(s,w,fv(inputs[0],3),fv(history,24),fv(state,8),fv(out,3),fv(work,174));
    assert(r.status==COAX_QlStatus_alias && !memcmp(oh,history,sizeof oh));
    puts("Qwen linear layer: independent two-step outputs and both state types pass");
    puts("Qwen linear layer: late failure and nested alias preserve published state");
}
