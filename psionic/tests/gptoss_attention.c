#include "gptoss-attention-memory.h"
#include "gptoss-oracle.h"
#include <assert.h>
#include <float.h>
#include <math.h>
#include <stdio.h>
#include <string.h>
static coax_v_f32 fv(float *p,int64_t n){return (coax_v_f32){p,n};}
static void close_to(float a,float b,double tolerance){assert(isfinite(a) && fabs((double)a-b)<tolerance*(1+fabs(b)));}
static void references(void){
    for(size_t j=0;j<sizeof(yarn_cases)/sizeof(*yarn_cases);j++){
        float out[65]={0};out[64]=-7;coax_t_GoResult r=go_yarn(yarn_cases[j].position,1,fv(out,65));
        assert(r.status==COAX_GoStatus_ok && r.written==64 && out[64]==-7);
        for(int i=0;i<64;i++)close_to(out[i],yarn_cases[j].coefficients[i],2e-6);
    }
    for(size_t j=0;j<sizeof(activation_cases)/sizeof(*activation_cases);j++){
        float input[]={activation_cases[j].gate,activation_cases[j].linear},out[]={-7,-7};
        coax_t_GoResult r=psionic_gptoss_swiglu(fv(input,2),fv(out,2));
        assert(r.status==COAX_GoStatus_ok && r.written==1 && r.clamped==activation_cases[j].clamped && out[1]==-7);
        close_to(out[0],activation_cases[j].expected,2e-6);
    }
    for(size_t j=0;j<sizeof(attention_cases)/sizeof(*attention_cases);j++){
        int h=attention_cases[j].h,k=attention_cases[j].k,c=attention_cases[j].c,n=attention_cases[j].n;
        coax_t_GaShape s={h,k,attention_cases[j].w,c};
        float q[1280],keys[640],v[640],sinks[4],cache[769],out[257],work[897];
        memcpy(q,attention_cases[j].q,sizeof q);memcpy(keys,attention_cases[j].key,sizeof keys);
        memcpy(v,attention_cases[j].v,sizeof v);memcpy(sinks,attention_cases[j].sinks,sizeof sinks);
        int cache_n=2*c*k*64,out_n=h*64,work_n=(3*h+k)*64;
        for(int i=0;i<769;i++)cache[i]=-17;for(int i=0;i<257;i++)out[i]=-19;for(int i=0;i<897;i++)work[i]=-23;
        for(int p=0;p<n;p++){
            coax_t_GaResult r=psionic_gptoss_attention(s,p,fv(q+p*h*64,h*64),fv(keys+p*k*64,k*64),fv(v+p*k*64,k*64),fv(sinks,h),fv(cache,cache_n+1),fv(out,out_n+1),fv(work,work_n+1));
            assert(r.status==COAX_GoStatus_ok && r.written==out_n && r.next==p+1);
            for(int i=0;i<out_n;i++)close_to(out[i],attention_cases[j].expected[p*out_n+i],2e-5);
            assert(cache[cache_n]==-17 && out[out_n]==-19 && work[work_n]==-23);
            /* Unwritten future slots retain their sentinel. */
            if(p+1<c)for(int i=(p+1)*2*k*64;i<cache_n;i++)assert(cache[i]==-17);
        }
    }
}
static void attention_failures(void){
    coax_t_GaShape s={1,1,2,2};float q[64]={0},k[64]={0},v[64]={0},sink[1]={0};
    float cache[257],before[257],out[65],before_out[65],work[256];
    for(int i=0;i<257;i++)cache[i]=-7;for(int i=0;i<65;i++)out[i]=-9;
    memcpy(before,cache,sizeof cache);memcpy(before_out,out,sizeof out);
    for(int j=0;j<7;j++)for(int negative=0;negative<2;negative++){
        coax_v_f32 views[]={fv(q,64),fv(k,64),fv(v,64),fv(sink,1),fv(cache,257),fv(out,65),fv(work,256)};
        int minimum[]={64,64,64,1,256,64,256};views[j].count=negative?-1:minimum[j]-1;
        coax_t_GaResult r=psionic_gptoss_attention(s,0,views[0],views[1],views[2],views[3],views[4],views[5],views[6]);
        assert(r.status==COAX_GoStatus_capacity && !memcmp(before,cache,sizeof cache) && !memcmp(before_out,out,sizeof out));
    }
    float *inputs[]={q,k,v,sink};
    for(int j=0;j<4;j++)for(int n=0;n<3;n++){
        inputs[j][0]=n==0?NAN:n==1?INFINITY:-INFINITY;
        coax_t_GaResult r=psionic_gptoss_attention(s,0,fv(q,64),fv(k,64),fv(v,64),fv(sink,1),fv(cache,257),fv(out,65),fv(work,256));
        assert(r.status==COAX_GoStatus_number && r.written==0 && r.next==0);
        assert(!memcmp(before,cache,sizeof cache) && !memcmp(before_out,out,sizeof out));inputs[j][0]=0;
    }
    for(int j=0;j<2;j++){
        inputs[j][63]=FLT_MAX;
        coax_t_GaResult r=psionic_gptoss_attention(s,0,fv(q,64),fv(k,64),fv(v,64),fv(sink,1),fv(cache,257),fv(out,65),fv(work,256));
        assert(r.status==COAX_GoStatus_arithmetic && !memcmp(before,cache,sizeof cache) && !memcmp(before_out,out,sizeof out));inputs[j][63]=0;
    }
    /* All unused/future slots may be nonfinite. Only visible history is read. */
    for(int i=0;i<256;i++)cache[i]=NAN;
    assert(psionic_gptoss_attention(s,0,fv(q,64),fv(k,64),fv(v,64),fv(sink,1),fv(cache,257),fv(out,65),fv(work,256)).status==COAX_GoStatus_ok);
    for(int lane=0;lane<2;lane++){
        cache[lane*64]=NAN;memcpy(before,cache,sizeof cache);memcpy(before_out,out,sizeof out);
        assert(psionic_gptoss_attention(s,1,fv(q,64),fv(k,64),fv(v,64),fv(sink,1),fv(cache,257),fv(out,65),fv(work,256)).status==COAX_GoStatus_number);
        assert(!memcmp(before,cache,sizeof cache) && !memcmp(before_out,out,sizeof out));cache[lane*64]=0;
    }
    assert(psionic_gptoss_attention(s,1,fv(q,64),fv(k,64),fv(v,64),fv(sink,1),fv(cache,257),fv(out,65),fv(work,256)).status==COAX_GoStatus_ok);
    for(int i=0;i<128;i++)cache[i]=NAN; /* expired slot zero at position two */
    assert(psionic_gptoss_attention(s,2,fv(q,64),fv(k,64),fv(v,64),fv(sink,1),fv(cache,257),fv(out,65),fv(work,256)).status==COAX_GoStatus_ok);
    assert(out[0]==0 && cache[256]==-7);
    float arena[7][512]={{0}},saved[7][512];
    memcpy(saved,arena,sizeof arena);
    for(int a=4;a<7;a++)for(int b=0;b<a;b++){
        coax_v_f32 views[7];int counts[]={64,64,64,1,256,64,256};
        for(int i=0;i<7;i++)views[i]=fv(arena[i],counts[i]);views[a].data=views[b].data;
        coax_t_GaResult r=psionic_gptoss_attention(s,0,views[0],views[1],views[2],views[3],views[4],views[5],views[6]);
        assert(r.status==COAX_GoStatus_alias && !memcmp(arena,saved,sizeof arena));
    }
}
static void limits(void){
    coax_t_GaShape s={1,1,128,128};float q[64]={0},k[64]={0},v[64]={0},sink[1]={0},cache[16385]={0},out[65]={0},work[256];
    cache[16384]=-7;out[64]=-9;
    for(int p=0;p<130;p++){
        v[0]=(float)(p+1);int n=p+1,first=n>128?n-128:0,visible=n-first;
        double expected=(double)visible*(first+1+n)/2/(visible+1);
        coax_t_GaResult r=psionic_gptoss_attention(s,p,fv(q,64),fv(k,64),fv(v,64),fv(sink,1),fv(cache,16385),fv(out,65),fv(work,256));
        assert(r.status==COAX_GoStatus_ok && r.next==p+1 && cache[16384]==-7 && out[64]==-9);close_to(out[0],(float)expected,1e-7);
    }
    s=(coax_t_GaShape){1,1,1,1};v[0]=4;
    assert(psionic_gptoss_attention(s,131071,fv(q,64),fv(k,64),fv(v,64),fv(sink,1),fv(cache,128),fv(out,64),fv(work,256)).next==131072);
    assert(out[0]==2);
    assert(psionic_gptoss_attention(s,131072,fv(q,64),fv(k,64),fv(v,64),fv(sink,1),fv(cache,128),fv(out,64),fv(work,256)).status==COAX_GoStatus_range);
    s=(coax_t_GaShape){64,8,128,128};
    float bq[4096]={0},bk[512]={0},bv[512],bs[64]={0},bc[131072]={0},bo[4096],bw[12800];
    for(int h=0;h<8;h++)for(int d=0;d<64;d++)bv[h*64+d]=(float)(h+1);
    coax_t_GaResult r=psionic_gptoss_attention(s,0,fv(bq,4096),fv(bk,512),fv(bv,512),fv(bs,64),fv(bc,131072),fv(bo,4096),fv(bw,12800));
    assert(r.status==COAX_GoStatus_ok && r.written==4096);
    for(int h=0;h<64;h++)for(int d=0;d<64;d++)assert(bo[h*64+d]==(float)(h/8+1)/2);
    float pairs[32770]={0},activation[16384];pairs[32766]=1;
    assert(psionic_gptoss_swiglu(fv(pairs,32768),fv(activation,16384)).written==16384 && activation[16383]>0.84f);
    assert(psionic_gptoss_swiglu(fv(pairs,32770),fv(activation,16384)).status==COAX_GoStatus_shape);
}
static void math_failures(void){
    float x[64]={0},c[64]={0},out[65],saved[65],work[64];
    for(int i=0;i<65;i++)out[i]=-7;memcpy(saved,out,sizeof out);
    for(int j=0;j<2;j++){
        float *p=j?c:x;p[63]=NAN;
        assert(psionic_gptoss_rotate(1,fv(x,64),fv(c,64),fv(out,65),fv(work,64)).status==COAX_GoStatus_number);
        assert(!memcmp(saved,out,sizeof out));p[63]=0;
    }
    assert(psionic_gptoss_rotate(1,fv(x,64),fv(c,64),fv(x,64),fv(work,64)).status==COAX_GoStatus_alias);
    assert(psionic_gptoss_rotate(1,fv(x,64),fv(c,64),fv(out,65),fv(c,64)).status==COAX_GoStatus_alias);
    assert(psionic_gptoss_rotate(1,fv(x,64),fv(c,64),fv(out,64),fv(out,64)).status==COAX_GoStatus_alias);
    float pairs[]={1,0,2,NAN};
    assert(psionic_gptoss_swiglu(fv(pairs,4),fv(out,65)).status==COAX_GoStatus_number && !memcmp(saved,out,sizeof out));
    assert(psionic_gptoss_swiglu(fv(pairs,4),fv(pairs,4)).status==COAX_GoStatus_alias);
    float a[]={1,2,3,4},v[]={2,-1},bias[]={3,-2},pw[6];
    assert(psionic_gptoss_project(2,2,fv(a,4),fv(v,2),fv(bias,2),fv(out,65),fv(pw,6)).status==COAX_GoStatus_ok);
    assert(out[0]==3 && out[1]==0 && out[2]==-7);memcpy(out,saved,sizeof out);
    float *inputs[]={a,v,bias};for(int j=0;j<3;j++){
        float old=inputs[j][0];inputs[j][0]=INFINITY;
        coax_t_GoResult r=psionic_gptoss_project(2,2,fv(a,4),fv(v,2),fv(bias,2),fv(out,65),fv(pw,6));
        assert(r.status==COAX_GoStatus_arithmetic && r.cause==COAX_PsionicOpStatus_number && !memcmp(out,saved,sizeof out));inputs[j][0]=old;
    }
    assert(psionic_gptoss_project(2,2,fv(a,4),fv(v,2),fv(bias,2),fv(a,4),fv(pw,6)).status==COAX_GoStatus_alias);
    assert(psionic_gptoss_project(2,2,fv(a,4),fv(v,2),fv(bias,2),fv(out,65),fv(out,65)).status==COAX_GoStatus_alias);
}
int main(void){references();attention_failures();limits();math_failures();puts("gpt-oss native: scalar references, ring expiry, 64/8 heads, atomic refusals and aliases pass");}
