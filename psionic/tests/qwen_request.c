#include "qwen-request-memory.h"
#include <assert.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
static coax_v_f32 fs(float *p,int64_t n){return (coax_v_f32){p,n};}
static coax_v_u8 us(unsigned char *p,int64_t n){return (coax_v_u8){p,n};}
int main(void){
    coax_t_PdShape delta={1,1,1,1,1};coax_t_QfShape att={1,1,64,64,1};
    coax_t_QeShape shape={.linear={2,1,delta,COAX_QmDtype_bf16},.full={2,1,3,att,COAX_QmDtype_bf16},.state={1,1,3,delta,att}};
    unsigned char zero[512]={0};float norm[2]={0},qnorm[64]={0},conv[12]={0},scalar[1]={0},gain[1]={1};
    coax_t_QlWeights lw={fs(norm,2),fs(norm,2),fs(conv,12),fs(scalar,1),fs(scalar,1),fs(gain,1),us(zero,12),us(zero,4),us(zero,4),us(zero,4),us(zero,4),us(zero,4),us(zero,4),us(zero,4)};
    coax_t_QhWeights fw={fs(norm,2),fs(norm,2),fs(qnorm,64),fs(qnorm,64),us(zero,512),us(zero,256),us(zero,256),us(zero,256),us(zero,4),us(zero,4),us(zero,4)};
    coax_t_QeLayer layers[2];memset(layers,0,sizeof(layers));layers[0].kind=0;layers[0].linear=lw;layers[1].kind=1;layers[1].full=fw;
    unsigned char embedding[16]={0,64,128,64,0,0,0,0,0,0,0,0,0,0,0,0};
    unsigned char head[16]={128,63,0,0,0,0,128,63,128,191,0,0,0,0,128,191};
    coax_t_QxModel model={shape,{layers,2},4,us(embedding,16),us(head,16),fs(norm,2)};
    int64_t tokens[2]={0,0},slots[2]={0,1};float state[394],before[394];for(int i=0;i<394;i++)state[i]=-7;
    double out[3]={-7,-7,-7},prob[4];float *work=calloc(71000,sizeof(float));assert(work);
    coax_t_QxResult r=psionic_qwen_request(model,(coax_v_s64){tokens,2},1,fs(state,394),COAX_QrKind_choice,1,(coax_v_s64){slots,2},(coax_v_f64){out,3},(coax_v_f64){prob,4},fs(work,71000));
    assert(r.status==COAX_QxStatus_ok && r.answer.best==1 && fabs(out[0]-0.35365245499220305)<1e-6 && out[2]==-7);
    for(int i=0;i<266;i++){assert(state[i]==0);}
    for(int i=266;i<394;i++){assert(state[i]==-7);}
    memcpy(before,state,sizeof(state));out[0]=out[1]=-7;tokens[1]=4;
    r=psionic_qwen_request(model,(coax_v_s64){tokens,2},1,fs(state,394),COAX_QrKind_choice,1,(coax_v_s64){slots,2},(coax_v_f64){out,3},(coax_v_f64){prob,4},fs(work,71000));
    assert(r.status==COAX_QxStatus_index && out[0]==-7 && out[1]==-7 && !memcmp(before,state,sizeof(state)));
    tokens[1]=0;r=psionic_qwen_request(model,(coax_v_s64){tokens,2},1,fs(state,394),COAX_QrKind_choice,1,(coax_v_s64){slots,2},(coax_v_f64){prob,2},(coax_v_f64){prob,4},fs(work,71000));
    assert(r.status==COAX_QxStatus_alias && !memcmp(before,state,sizeof(state)));
    free(work);puts("decision request native: block split, rotary, atomic failure and aliases pass");
}
