#include <assert.h>
#include <float.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include "delta-memory.h"
#include "delta-oracle.h"
_Static_assert(FLT_RADIX==2 && FLT_MANT_DIG==24 && DBL_MANT_DIG==53,"binary32/64 required");
static coax_v_f32 view(float *data,int64_t count) {return (coax_v_f32){data,count};}
static void near(const float *a,const float *b,int n) {
    for(int i=0;i<n;i++) assert(isfinite(a[i]) && fabsf(a[i]-b[i])<0.000002f);
}
int main(void) {
    coax_t_PdShape s={2,4,2,2,0.125f};float history[49],state[17],out[9],work[89],input[106];
    memcpy(history,initial_history,sizeof initial_history);memcpy(state,initial_state,sizeof initial_state);
    history[48]=-9;state[16]=-9;out[8]=-9;work[88]=-9;
    const float *samples[]={input0,input1,input2},*outputs[]={expected0,expected1,expected2};
    const float *states[]={expected_state0,expected_state1,expected_state2};
    const float *histories[]={expected_history0,expected_history1,expected_history2};
    for(int t=0;t<3;t++) {
        memcpy(input,samples[t],sizeof input);
        coax_t_PdResult r=psionic_delta_step(s,view(input,106),view(history,48),view(state,16),view(out,9),view(work,89));
        assert(r.status==COAX_PdStatus_ok && r.written==8);
        near(out,outputs[t],8);near(state,states[t],16);near(history,histories[t],48);
        assert(history[48]==-9 && state[16]==-9 && out[8]==-9 && work[88]==-9);
    }
    float before_history[48],before_state[16],before_out[8];
    memcpy(before_history,history,sizeof before_history);memcpy(before_state,state,sizeof before_state);memcpy(before_out,out,sizeof before_out);
    coax_t_PdResult r=psionic_delta_step(s,view(input,106),view(history,48),view(state,16),view(state,8),view(work,89));
    assert(r.status==COAX_PdStatus_alias && r.written==0);
    r=psionic_delta_step(s,view(input,105),view(history,48),view(state,16),view(out,8),view(work,89));
    assert(r.status==COAX_PdStatus_capacity && r.written==0);
    r=psionic_delta_step(s,view(input,106),view(history,48),view(state,16),view(out,8),view(work,87));
    assert(r.status==COAX_PdStatus_capacity && r.written==0);
    input[0]=NAN;r=psionic_delta_step(s,view(input,106),view(history,48),view(state,16),view(out,8),view(work,89));
    assert(r.status==COAX_PdStatus_number && r.written==0);input[0]=0;
    history[0]=INFINITY;r=psionic_delta_step(s,view(input,106),view(history,48),view(state,16),view(out,8),view(work,89));
    assert(r.status==COAX_PdStatus_number);memcpy(history,before_history,sizeof before_history);
    state[0]=NAN;r=psionic_delta_step(s,view(input,106),view(history,48),view(state,16),view(out,8),view(work,89));
    assert(r.status==COAX_PdStatus_number);memcpy(state,before_state,sizeof before_state);
    s.key_heads=INT64_MAX;r=psionic_delta_step(s,view(input,106),view(history,48),view(state,16),view(out,8),view(work,89));
    assert(r.status==COAX_PdStatus_shape);
    assert(!memcmp(history,before_history,sizeof before_history) && !memcmp(state,before_state,sizeof before_state) && !memcmp(out,before_out,sizeof before_out));
    puts("delta: three independent recurrent states and convolution histories pass");
    puts("delta: alias, capacity, non-finite and shape refusals preserve public state");
}
