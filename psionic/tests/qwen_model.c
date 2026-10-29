#include "qwen-model-memory.h"
#include <assert.h>
#include <stdio.h>
#include <string.h>
static coax_v_f32 fs(float *p,int n) {return (coax_v_f32){p,n};}
static coax_v_u8 us(unsigned char *p,int n) {return (coax_v_u8){p,n};}
int main(void) {
    coax_t_PdShape delta={1,1,1,1,1};coax_t_QfShape att={1,1,2,2,1};
    coax_t_QeShape s={.linear={2,1,delta,COAX_QmDtype_bf16},.full={2,1,3,att,COAX_QmDtype_bf16},.state={1,1,3,delta,att}};
    unsigned char matrix[16]={0},bad[4]={0,0,0x80,0x7f};float norm[2]={0},conv[12]={0},scalar[1]={0},gain[1]={1};
    coax_t_QlWeights lw={fs(norm,2),fs(norm,2),fs(conv,12),fs(scalar,1),fs(scalar,1),fs(gain,1),us(matrix,12),us(matrix,4),us(matrix,4),us(matrix,4),us(matrix,4),us(matrix,4),us(matrix,4),us(matrix,4)};
    coax_t_QhWeights fw={fs(norm,2),fs(norm,2),fs(norm,2),fs(norm,2),us(matrix,16),us(matrix,8),us(matrix,8),us(matrix,8),us(matrix,4),us(matrix,4),us(matrix,4)};
    coax_t_QeLayer layers[2];memset(layers,0,sizeof(layers));layers[0].kind=PSIONIC_QE_LINEAR;layers[0].linear=lw;layers[1].kind=PSIONIC_QE_FULL;layers[1].full=fw;
    coax_v_t_QeLayer lv={layers,2};float x[4]={1,2,3,4},rope[4]={1,0,1,0},state[22]={0},out[5]={-7,-7,-7,-7,-7},work[400]={0};
    state[9]=4;for(int i=18;i<22;i++)state[i]=-5;
    coax_t_QeResult r=psionic_qwen_model_block(s,lv,fs(x,4),fs(rope,4),0,2,fs(state,22),fs(out,5),fs(work,400));
    assert(r.status==COAX_QeStatus_ok && r.written==4 && state[9]==1 && out[4]==-7);
    for(int i=0;i<4;i++) {assert(out[i]==x[i]);}
    for(int i=18;i<22;i++) {assert(state[i]==-5);}
    float before[22];memcpy(before,state,sizeof(state));for(int i=0;i<5;i++)out[i]=-7;layers[1].full.down=us(bad,4);
    r=psionic_qwen_model_block(s,lv,fs(x,4),fs(rope,4),0,2,fs(state,22),fs(out,5),fs(work,400));
    assert(r.status==COAX_QeStatus_number && r.written==0 && !memcmp(state,before,sizeof(state)));
    for(int i=0;i<5;i++) {assert(out[i]==-7);}
    layers[1].full.down=us(matrix,4);
    r=psionic_qwen_model_block(s,lv,fs(x,4),fs(rope,4),0,2,fs(state,22),fs(work,5),fs(work,400));
    assert(r.status==COAX_QeStatus_alias);
    layers[0].linear.input_norm=fs(out,2);
    r=psionic_qwen_model_block(s,lv,fs(x,4),fs(rope,4),0,2,fs(state,22),fs(out,5),fs(work,400));
    assert(r.status==COAX_QeStatus_alias && !memcmp(state,before,sizeof(state)));
    puts("hybrid model native: complete block, preserved state and nested alias checks pass");
}
