/* Native ownership/address checks and independent scalar fixture assertions. */
#include "deepseek-hyper-memory.h"
#include "hyper-goldens.h"
#include <assert.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#define LEN(a) ((int64_t)(sizeof(a)/sizeof((a)[0])))
#define V(a) ((coax_v_f32){a,LEN(a)})
typedef struct {const float *input,*previous,*projection,*scale,*base,*mixes,*pre,*post,*comb,*collapsed,*sublayer,*output;} Case;
#define CASE(i) {dhg_chain_##i##_input,dhg_chain_##i##_previous,dhg_chain_##i##_projection,dhg_chain_##i##_scale,dhg_chain_##i##_base,dhg_chain_##i##_mixes,dhg_chain_##i##_pre,dhg_chain_##i##_post,dhg_chain_##i##_comb,dhg_chain_##i##_collapsed,dhg_chain_##i##_sublayer,dhg_chain_##i##_output}
static const Case cases[]={CASE(0),CASE(1),CASE(2),CASE(3),CASE(4),CASE(5)};
static void near(const float *a,const float *b,int64_t n,float tolerance) {for (int64_t i=0;i<n;i++) assert(isfinite(a[i]) && fabsf(a[i]-b[i])<=tolerance);}
static void *allocate(size_t n,size_t width) {void *p=calloc(n,width);assert(p);return p;}
static coax_t_DhWeights weights(int step,float *projection) {
    const Case *c=&cases[step];memcpy(projection,c->projection,288*sizeof(float));coax_t_DhWeights w={.projection={projection,288}};
    memcpy(w.scale.e,c->scale,3*sizeof(float));memcpy(w.base.e,c->base,24*sizeof(float));return w;
}
static void handoffs(void) {
    for (int boundary=0;boundary<=6;boundary++) {
        float *state=allocate(12,sizeof(float));memcpy(state,cases[0].input,12*sizeof(float));coax_t_DhCarry carry=dh_initial();
        for (int step=0;step<6;step++) {
            if (step==boundary) {
                /* Transfer exactly the live stream and temporal pre-mix. Destroy
                 * the sender's storage before resuming on independent memory. */
                float *received=allocate(12,sizeof(float));memcpy(received,state,12*sizeof(float));coax_t_DhCarry received_carry=carry;
                memset(state,0,12*sizeof(float));memset(&carry,0,sizeof(carry));free(state);state=received;carry=received_carry;
            }
            const Case *c=&cases[step];float projection[288],collapsed[4]={0},out[13]={0},stage[13]={0},value[3];coax_t_DhWeights w=weights(step,projection);
            collapsed[3]=-7;out[12]=-13;stage[12]=-9;memcpy(value,c->sublayer,sizeof(value));
            near(state,c->input,12,2e-6f);near(carry.pre.e,c->previous,4,2e-7f);
            coax_t_DhProjection projected=dh_project(3,(coax_v_f32){state,12},w.projection);assert(projected.status==COAX_DsStatus_ok);near(projected.values.e,c->mixes,24,1e-6f);
            coax_t_DhMixResult r=psionic_dh_prepare(3,(coax_v_f32){state,12},carry,w,V(collapsed),V(stage));assert(r.status==COAX_DsStatus_ok);
            near(collapsed,c->collapsed,3,1e-6f);near(r.mix.pre.e,c->pre,4,2e-7f);near(r.mix.post.e,c->post,4,3e-7f);near(r.mix.comb.e,c->comb,16,2e-7f);
            if (step==1) {
                float lost_carry[3],scratch[3];coax_t_DhMixResult wrong=psionic_dh_prepare(3,(coax_v_f32){state,12},dh_initial(),w,V(lost_carry),V(scratch));assert(wrong.status==COAX_DsStatus_ok);
                _Bool differs=0;for (int j=0;j<3;j++) differs|=fabsf(lost_carry[j]-collapsed[j])>1e-4f;assert(differs);
            }
            coax_t_DhFinish f=psionic_dh_finish(3,(coax_v_f32){state,12},V(value),r.mix,V(out),V(stage));assert(f.status==COAX_DsStatus_ok && f.written==12);
            near(out,c->output,12,2e-6f);assert(memcmp(f.carry.pre.e,r.mix.pre.e,4*sizeof(float))==0);
            assert(collapsed[3]==-7 && out[12]==-13 && stage[12]==-9);near(state,c->input,12,2e-6f);
            memcpy(state,out,12*sizeof(float));carry=f.carry;
        }
        if (boundary==6) {
            float received[12];coax_t_DhCarry received_carry=carry;memcpy(received,state,sizeof(received));memset(state,0,sizeof(received));memset(&carry,0,sizeof(carry));
            near(received,cases[5].output,12,2e-6f);near(received_carry.pre.e,cases[5].pre,4,2e-7f);
            memcpy(state,received,sizeof(received));carry=received_carry;
        }
        float collapsed[3],scratch[3];coax_t_DsResult head=psionic_dh_collapse(3,(coax_v_f32){state,12},carry,V(collapsed),V(scratch));assert(head.status==COAX_DsStatus_ok && head.written==3);near(collapsed,dhg_final_collapsed,3,2e-6f);
        free(state);
    }
    puts("Hyper: six subblocks, seven stage boundaries, previous-mix negative control, both matrix axes and untouched tails pass");
}
static void refusals(void) {
    float x[12],projection[288],value[3],out[12],stage[12];memcpy(x,cases[0].input,sizeof(x));memcpy(value,cases[0].sublayer,sizeof(value));
    coax_t_DhWeights w=weights(0,projection);coax_t_DhCarry previous=dh_initial();coax_t_DhMix mix={0};memcpy(mix.pre.e,cases[0].pre,4*sizeof(float));memcpy(mix.post.e,cases[0].post,4*sizeof(float));memcpy(mix.comb.e,cases[0].comb,16*sizeof(float));
    for (int i=0;i<12;i++) out[i]=7;
    for (int field=0;field<6;field++) {
        float saved;float *bad=field==0?&x[0]:field==1?&x[11]:field==2?&projection[287]:field==3?&w.scale.e[2]:field==4?&w.base.e[23]:&previous.pre.e[3];saved=*bad;*bad=field%2?INFINITY:NAN;
        assert(psionic_dh_prepare(3,V(x),previous,w,V(out),V(stage)).status==COAX_DsStatus_number);*bad=saved;for (int i=0;i<12;i++) assert(out[i]==7);
    }
    for (int field=0;field<3;field++) {
        float *bad=field==0?&x[0]:field==1?&x[11]:&previous.pre.e[3];float saved=*bad;*bad=NAN;
        assert(psionic_dh_collapse(3,V(x),previous,V(out),V(stage)).status==COAX_DsStatus_number);*bad=saved;
    }
    for (int field=0;field<6;field++) {
        float *bad=field==0?&x[0]:field==1?&x[11]:field==2?&value[2]:field==3?&mix.pre.e[3]:field==4?&mix.post.e[3]:&mix.comb.e[15];float saved=*bad;*bad=field%2?INFINITY:NAN;
        coax_t_DhFinish r=psionic_dh_finish(3,V(x),V(value),mix,V(out),V(stage));assert(r.status==COAX_DsStatus_number && r.written==0);*bad=saved;for (int i=0;i<12;i++) assert(out[i]==7);
    }
    assert(psionic_dh_prepare(3,V(x),previous,w,V(out),V(out)).status==COAX_DsStatus_alias);
    assert(psionic_dh_prepare(3,V(x),previous,w,(coax_v_f32){x,3},V(stage)).status==COAX_DsStatus_alias);
    assert(psionic_dh_prepare(3,V(x),previous,w,(coax_v_f32){projection,3},V(stage)).status==COAX_DsStatus_alias);
    assert(psionic_dh_prepare(3,V(x),previous,w,V(out),(coax_v_f32){projection,3}).status==COAX_DsStatus_alias);
    assert(psionic_dh_finish(3,V(x),V(value),mix,V(out),V(out)).status==COAX_DsStatus_alias);
    assert(psionic_dh_collapse(3,V(x),previous,V(out),V(out)).status==COAX_DsStatus_alias);
    assert(psionic_dh_collapse(3,V(x),previous,V(x),V(stage)).status==COAX_DsStatus_alias);
    assert(psionic_dh_finish(3,V(x),V(value),mix,V(x),V(stage)).status==COAX_DsStatus_alias);
    assert(psionic_dh_finish(3,V(x),V(value),mix,V(out),V(x)).status==COAX_DsStatus_alias);
    assert(psionic_dh_finish(3,V(x),V(value),mix,V(out),(coax_v_f32){value,3}).status==COAX_DsStatus_capacity);
    float large_value[12]={0};assert(psionic_dh_finish(3,V(x),(coax_v_f32){large_value,3},mix,V(out),V(large_value)).status==COAX_DsStatus_alias);
    for (int i=0;i<12;i++) assert(out[i]==7);
    puts("Hyper: first/final NaN/Inf, all mix fields, nested aliases and atomic refusals pass");
}
static void dimensions(int64_t width) {
    int64_t n=4*width;float *x=allocate((size_t)n,sizeof(float)),*w=allocate((size_t)(24*n),sizeof(float));
    for (int64_t j=0;j<n;j++) x[j]=1;
    for (int64_t i=0;i<24;i++) for (int64_t j=0;j<n;j++) w[i*n+j]=(float)(i+1)/65536;
    coax_t_DhProjection r=dh_project(width,(coax_v_f32){x,n},(coax_v_f32){w,24*n});assert(r.status==COAX_DsStatus_ok);
    for (int64_t i=0;i<24;i++) assert(r.values.e[i]==(float)(n*(i+1))/65536);
    free(x);free(w);
}
int main(void) {handoffs();refusals();dimensions(5120);dimensions(8192);puts("Hyper: original width 5120 and maximum width 8192 complete projections pass");return 0;}
