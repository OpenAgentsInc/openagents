#include "decision-answer-memory.h"
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
    unsigned char embedding[160]={0,64,128,64},head[160]={0};head[128]=128;head[129]=63;head[134]=128;head[135]=63;
    coax_t_QxModel model={shape,{layers,2},40,us(embedding,160),us(head,160),fs(norm,2)};
    int64_t tokens[2]={0,0};float state[394],before[394];for(int i=0;i<394;i++)state[i]=-7;
    double out[3]={-7,-7,-7},prob[4];float *work=calloc(71000,sizeof(float));assert(work);
    coax_t_PsSession session;memset(&session,0,sizeof(session));session.model=model;
    coax_t_PrResult prepared;memset(&prepared,0,sizeof(prepared));prepared.written=2;prepared.prefix=1;prepared.slots=2;prepared.kind=COAX_PtQuestionKind_choice;
    coax_t_PtOption options[2]={{{"yes",3},{COAX_PtEntryKind_none,{"",0}}},{{"no",2},{COAX_PtEntryKind_none,{"",0}}}};
    coax_t_PsAnswer r=psionic_answer(session,prepared,(coax_v_s64){tokens,2},(coax_v_t_PtOption){options,2},1,fs(state,394),(coax_v_f64){out,3},(coax_v_f64){prob,4},fs(work,71000));
    assert(r.status==COAX_QxStatus_ok && r.statistics.best==1 && r.choice.count==2 && !memcmp(r.choice.data,"no",2));
    assert(fabs(out[0]-0.35365245499220305)<1e-6 && out[2]==-7);
    memcpy(before,state,sizeof(state));out[0]=out[1]=-7;options[0].name=(coax_string){(char *)out,1};
    r=psionic_answer(session,prepared,(coax_v_s64){tokens,2},(coax_v_t_PtOption){options,2},1,fs(state,394),(coax_v_f64){out,3},(coax_v_f64){prob,4},fs(work,71000));
    assert(r.status==COAX_QxStatus_alias && out[0]==-7 && !memcmp(before,state,sizeof(state)));
    free(work);puts("typed answer native: pinned slots, choice label and nested alias checks pass");
}
