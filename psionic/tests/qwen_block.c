#include <assert.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include "qwen-block-memory.h"
#include "qwen-block-oracle.h"
static coax_v_f32 fv(float *p,int64_t n) {return (coax_v_f32){p,n};}
static coax_v_u8 bv(float *p,int64_t n) {return (coax_v_u8){(uint8_t *)p,n*(int64_t)sizeof(float)};}
int main(void) {
    float x[3],att[3],scale[3],gate[6],up[6],down[6],out[4]={-7,-7,-7,-7},work[21];
    memcpy(x,fixture_x,sizeof x);memcpy(att,fixture_att,sizeof att);memcpy(scale,fixture_scale,sizeof scale);
    memcpy(gate,fixture_gate,sizeof gate);memcpy(up,fixture_up,sizeof up);memcpy(down,fixture_down,sizeof down);work[20]=-9;
    coax_t_QmResult r=psionic_qwen_finish(COAX_QmDtype_f32,3,2,0.125f,fv(x,3),fv(att,3),fv(scale,3),bv(gate,6),bv(up,6),bv(down,6),fv(out,4),fv(work,21));
    assert(r.status==COAX_QmStatus_ok && r.written==3 && out[3]==-7 && work[20]==-9);
    for(int i=0;i<3;i++)assert(fabsf(out[i]-fixture_out[i])<0.000002f);
    float old[3];memcpy(old,out,sizeof old);
    r=psionic_qwen_finish(COAX_QmDtype_f32,3,2,0.125f,fv(x,3),fv(att,3),fv(scale,3),bv(gate,6),bv(up,6),bv(down,6),fv(out,3),fv(out,3));
    assert(r.status==COAX_QmStatus_capacity);
    r=psionic_qwen_finish(COAX_QmDtype_f32,3,2,0.125f,fv(x,3),fv(att,3),fv(scale,3),bv(gate,6),bv(up,6),bv(down,6),fv(work,3),fv(work,20));
    assert(r.status==COAX_QmStatus_alias);
    down[5]=INFINITY;r=psionic_qwen_finish(COAX_QmDtype_f32,3,2,0.125f,fv(x,3),fv(att,3),fv(scale,3),bv(gate,6),bv(up,6),bv(down,6),fv(out,3),fv(work,20));
    assert(r.status==COAX_QmStatus_number && !memcmp(out,old,sizeof old));
    float begin_x[2]={3,4},begin_scale[2]={1,-1},begin_out[2],begin_work[2];
    r=psionic_qwen_begin(2,3.5f,fv(begin_x,2),fv(begin_scale,2),fv(begin_out,2),fv(begin_work,2));
    assert(r.status==COAX_QmStatus_ok && begin_out[0]==1.5f && begin_out[1]==0);
    r=psionic_qwen_begin(2,3.5f,fv(begin_x,2),fv(begin_scale,2),fv(begin_x,2),fv(begin_work,2));
    assert(r.status==COAX_QmStatus_alias);
    puts("Qwen block: independent offset-RMS, residual and SwiGLU fixture passes");
    puts("Qwen block: capacity, alias and non-finite weight refusals preserve output");
}
