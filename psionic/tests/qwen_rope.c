#include "qwen-rope-memory.h"
#include <assert.h>
#include <math.h>
#include <stdio.h>
int main(void){
    float out[129],work[128];for(int i=0;i<129;i++)out[i]=-7;
    coax_t_QwResult r=psionic_qwen_rope(0,2,(coax_v_f32){out,129},(coax_v_f32){work,128});
    assert(r.status==COAX_QwStatus_ok && r.written==128 && out[128]==-7);
    for(int i=0;i<32;i++)assert(out[2*i]==1 && out[2*i+1]==0);
    for(int i=0;i<32;i++){
        double phase=pow(10000000.0,-2.0*i/64.0);
        assert(fabs((double)out[64+2*i]-cos(phase))<1e-6 && fabs((double)out[65+2*i]-sin(phase))<1e-6);
    }
    out[0]=-7;r=psionic_qwen_rope(0,1,(coax_v_f32){out,64},(coax_v_f32){out,64});assert(r.status==COAX_QwStatus_alias && out[0]==-7);
    r=psionic_qwen_rope(16383,2,(coax_v_f32){out,64},(coax_v_f32){work,64});assert(r.status==COAX_QwStatus_range && out[0]==-7);
    puts("rotary native: original theta, boundaries and aliases pass");
}
