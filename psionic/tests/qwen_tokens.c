#include "qwen-tokens-memory.h"
#include <assert.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
static coax_v_f32 fs(float *p,int64_t n){return (coax_v_f32){p,n};}
static coax_v_u8 us(unsigned char *p,int64_t n){return (coax_v_u8){p,n};}
int main(void){
    coax_t_PdShape delta={1,1,1,1,1};coax_t_QfShape att={1,1,2,2,1};
    coax_t_QeShape shape={.linear={2,1,delta,COAX_QmDtype_bf16},.full={2,1,1,att,COAX_QmDtype_bf16},.state={1,1,1,delta,att}};
    unsigned char zero[16]={0};float norm[2]={0},conv[12]={0},scalar[1]={0},gain[1]={1};
    coax_t_QlWeights lw={fs(norm,2),fs(norm,2),fs(conv,12),fs(scalar,1),fs(scalar,1),fs(gain,1),us(zero,12),us(zero,4),us(zero,4),us(zero,4),us(zero,4),us(zero,4),us(zero,4),us(zero,4)};
    coax_t_QhWeights fw={fs(norm,2),fs(norm,2),fs(norm,2),fs(norm,2),us(zero,16),us(zero,8),us(zero,8),us(zero,8),us(zero,4),us(zero,4),us(zero,4)};
    coax_t_QeLayer layers[2];memset(layers,0,sizeof(layers));layers[0].kind=0;layers[0].linear=lw;layers[1].kind=1;layers[1].full=fw;
    unsigned char embedding[16]={0,64,128,64,0,0,0,0,0,0,0,0,0,0,0,0};
    unsigned char head[16]={128,63,0,0,0,0,128,63,128,191,0,0,0,0,128,191};
    coax_t_QxModel model={shape,{layers,2},4,us(embedding,16),us(head,16),fs(norm,2)};
    int64_t token[1]={0},slots[2]={0,1};float rope[2]={1,0},state[14]={0};double out[3]={-7,-7,-7},prob[2];float *work=calloc(100000,sizeof(float));assert(work);
    coax_t_QxResult r=psionic_qwen_tokens(model,(coax_v_s64){token,1},fs(rope,2),0,fs(state,14),COAX_QrKind_choice,1,(coax_v_s64){slots,2},(coax_v_f64){out,3},(coax_v_f64){prob,2},fs(work,100000));
    assert(r.status==COAX_QxStatus_ok && r.answer.best==1 && fabs(out[0]-0.35365245499220305)<1e-6 && fabs(r.answer.mass-0.8592492040662238)<1e-6 && out[2]==-7);
    float before[14];memcpy(before,state,sizeof(state));out[0]=out[1]=-7;head[14]=0x80;head[15]=0x7f;
    r=psionic_qwen_tokens(model,(coax_v_s64){token,1},fs(rope,2),0,fs(state,14),COAX_QrKind_choice,1,(coax_v_s64){slots,2},(coax_v_f64){out,3},(coax_v_f64){prob,2},fs(work,100000));
    assert(r.status==COAX_QxStatus_model && r.answer.written==0 && out[0]==-7 && !memcmp(before,state,sizeof(state)));
    head[14]=128;head[15]=191;
    r=psionic_qwen_tokens(model,(coax_v_s64){token,1},fs(rope,2),0,fs(state,14),COAX_QrKind_choice,1,(coax_v_s64){slots,2},(coax_v_f64){out,3},(coax_v_f64){out,2},fs(work,100000));
    assert(r.status==COAX_QxStatus_alias && out[0]==-7 && !memcmp(before,state,sizeof(state)));
    const int64_t vocabulary=33000;unsigned char *big_embed=calloc((size_t)vocabulary*4,1),*big_head=calloc((size_t)vocabulary*4,1);assert(big_embed&&big_head);
    model.vocab=vocabulary;model.embedding=us(big_embed,vocabulary*4);model.head=us(big_head,vocabulary*4);
    r=psionic_qwen_tokens(model,(coax_v_s64){token,1},fs(rope,2),0,fs(state,14),COAX_QrKind_choice,1,(coax_v_s64){slots,2},(coax_v_f64){out,3},(coax_v_f64){prob,2},fs(work,100000));
    assert(r.status==COAX_QxStatus_ok && out[0]==.5 && out[1]==.5 && fabs(r.answer.mass-2.0/(double)vocabulary)<1e-15);
    free(big_embed);free(big_head);free(work);puts("token block native: full vocabulary chunks, atomic failure and aliases pass");
}
