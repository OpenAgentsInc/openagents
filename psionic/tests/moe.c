#include "moe-memory.h"
#include "moe-oracle.h"
#include <assert.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

static coax_v_f32 fv(float *p,int64_t n){return (coax_v_f32){p,n};}
static coax_v_t_MoExpert ev(coax_t_MoExpert *p,int64_t n){return (coax_v_t_MoExpert){p,n};}
static float from_bits(uint32_t word){float value;memcpy(&value,&word,4);return value;}

int main(void){
    for(size_t c=0;c<sizeof(moe_cases)/sizeof(*moe_cases);c++){
        int n=moe_cases[c].n,k=moe_cases[c].k;float logits[128],matrix[128],bias[128],x=2,out[2]={-7,-7};coax_t_MoExpert experts[128];
        for(int i=0;i<n;i++){logits[i]=from_bits(moe_cases[c].logits[i]);matrix[i]=(float)i+1;bias[i]=(float)i;}
        for(int i=0;i<n;i++){int id=n-1-i;experts[i]=(coax_t_MoExpert){id,fv(matrix+id,1),fv(bias+id,1)};}
        coax_t_MoRoute r=mo_route(fv(logits,n),k);assert(r.status==COAX_MoStatus_ok && r.count==k);
        double total=0;
        for(int i=0;i<k;i++){assert(r.ids.e[i]==moe_cases[c].ids[i] && fabs((double)r.weights.e[i]-moe_cases[c].weights[i])<1.2e-7);total+=r.weights.e[i];}
        assert(fabs(total-1)<2e-7);
        coax_t_MoResult result=psionic_moe_forward(fv(logits,n),k,fv(&x,1),ev(experts,n),n*8,fv(out,2));
        assert(result.status==COAX_MoStatus_ok && result.written==1 && result.resident_bytes==n*8 && out[1]==-7);
        assert(fabs((double)out[0]-moe_cases[c].mixed)<3e-7*(1+fabs(moe_cases[c].mixed)));
        for(int i=0;i<k;i++)assert(result.route.ids.e[i]==r.ids.e[i] && result.route.weights.e[i]==r.weights.e[i]);
    }
    float logits[3]={0,2,2},x[2]={1,2},matrix[3][4]={{1,0,0,1},{2,0,0,2},{4,0,0,4}},bias[3][2]={{-100,-100},{1,-1},{-1,1}};
    coax_t_MoExpert experts[3];for(int i=0;i<3;i++)experts[i]=(coax_t_MoExpert){i,fv(matrix[i],4),fv(bias[i],2)};
    float out[3]={-7,-7,-7},before[3];memcpy(before,out,sizeof out);
    coax_t_MoResult r=psionic_moe_forward(fv(logits,3),2,fv(x,2),ev(experts,3),72,fv(out,3));
    assert(r.status==COAX_MoStatus_ok && out[0]==3 && out[1]==6 && out[2]==-7);
    memcpy(out,before,sizeof out);logits[0]=NAN;
    assert(mo_route(fv(logits,3),2).status==COAX_MoStatus_number);
    logits[0]=INFINITY;assert(mo_route(fv(logits,3),2).status==COAX_MoStatus_number);
    logits[0]=-INFINITY;assert(mo_route(fv(logits,3),2).status==COAX_MoStatus_number);logits[0]=0;
    matrix[0][0]=NAN;r=psionic_moe_forward(fv(logits,3),2,fv(x,2),ev(experts,3),72,fv(out,3));assert(r.status==COAX_MoStatus_ok);
    matrix[0][0]=1;memcpy(out,before,sizeof out);matrix[2][0]=NAN;
    r=psionic_moe_forward(fv(logits,3),2,fv(x,2),ev(experts,3),72,fv(out,3));
    assert(r.status==COAX_MoStatus_arithmetic && r.cause==COAX_PsionicOpStatus_number && !memcmp(out,before,sizeof out));matrix[2][0]=4;
    x[1]=NAN;r=psionic_moe_forward(fv(logits,3),2,fv(x,2),ev(experts,3),72,fv(out,3));
    assert(r.status==COAX_MoStatus_arithmetic && r.cause==COAX_PsionicOpStatus_number && !memcmp(out,before,sizeof out));x[1]=2;
    r=psionic_moe_forward(fv(logits,3),2,fv(x,2),ev(experts,2),72,fv(out,3));assert(r.status==COAX_MoStatus_resident);
    r=psionic_moe_forward(fv(logits,3),2,fv(x,2),ev(experts,3),71,fv(out,3));assert(r.status==COAX_MoStatus_budget);
    r=psionic_moe_forward(fv(logits,3),2,fv(x,2),ev(experts,3),72,fv(out,1));assert(r.status==COAX_MoStatus_capacity);
    r=psionic_moe_forward(fv(logits,3),2,fv(x,2),ev(experts,3),72,fv(logits,3));assert(r.status==COAX_MoStatus_alias);
    r=psionic_moe_forward(fv(logits,3),2,fv(x,2),ev(experts,3),72,fv(x,2));assert(r.status==COAX_MoStatus_alias);
    r=psionic_moe_forward(fv(logits,3),2,fv(x,2),ev(experts,3),72,fv(matrix[0],4));assert(r.status==COAX_MoStatus_alias);
    r=psionic_moe_forward(fv(logits,3),2,fv(x,2),ev(experts,3),72,fv(bias[2],2));assert(r.status==COAX_MoStatus_alias);
    union{coax_t_MoExpert experts[3];float output[64];} shared;memset(&shared,0,sizeof shared);
    r=psionic_moe_forward(fv(logits,3),2,fv(x,2),ev(shared.experts,3),72,fv(shared.output,2));assert(r.status==COAX_MoStatus_alias);
    float scores[128]={0},large_x[16],weights[128][256],biases[128][16],large_out[17];coax_t_MoExpert large[128];
    memset(weights,0,sizeof weights);for(int j=0;j<16;j++)large_x[j]=(float)j;large_out[16]=-7;
    for(int i=0;i<128;i++){
        for(int j=0;j<16;j++){weights[i][j*16+j]=1;biases[i][j]=(float)i;}
        large[127-i]=(coax_t_MoExpert){i,fv(weights[i],256),fv(biases[i],16)};
    }
    r=psionic_moe_forward(fv(scores,128),4,fv(large_x,16),ev(large,128),139264,fv(large_out,17));
    assert(r.status==COAX_MoStatus_ok && r.written==16 && r.resident_bytes==139264 && large_out[16]==-7);
    for(int j=0;j<16;j++)assert(large_out[j]==(float)j+1.5f);
    assert(psionic_moe_forward(fv(scores,128),4,fv(large_x,16),ev(large,128),139263,fv(large_out,17)).status==COAX_MoStatus_budget);
    puts("MoE native: independent routes, selected expert dispatch, nested aliases and maximum residency pass");
}
