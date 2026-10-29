#include "deepseek-router-memory.h"
#include "router-goldens.h"
#include <assert.h>
#include <math.h>
#include <stdio.h>
#include <string.h>
#define LEN(a) ((int64_t)(sizeof(a)/sizeof((a)[0])))
#define V(a) {a,LEN(a)}
static void close_value(double actual,double expected) {
    assert(isfinite(actual));assert(fabs(actual-expected)<=1e-12*fabs(expected)+1e-300);
}
static float from_bits(uint32_t bits) {float x;memcpy(&x,&bits,sizeof(x));return x;}
static void oracle(void) {
    for (size_t n=0;n<sizeof(DR_GOLDENS)/sizeof(DR_GOLDENS[0]);n++) {
        const DrGolden *g=DR_GOLDENS+n;float x[384],bias[384];
        memcpy(x,g->x,(size_t)g->n*sizeof(float));memcpy(bias,g->bias,(size_t)(g->profile?g->n:0)*sizeof(float));
        double scores[385],ranked[385];for (size_t i=0;i<385;i++) {scores[i]=-7;ranked[i]=-9;}
        coax_t_DrConfig c={.profile=(uint8_t)g->profile,.k=g->k,.groups=g->groups,.selected_groups=g->keep,.scale=g->scale};
        coax_t_DrRoute r=psionic_dr_route(c,(coax_v_f32){x,g->n},(coax_v_f32){bias,g->profile?g->n:0},(coax_v_f64)V(scores),(coax_v_f64)V(ranked));
        assert(r.status==COAX_DrStatus_ok && r.count==g->k && r.experts==g->n && r.profile==g->profile && r.group_count==g->keep);
        for (int i=0;i<g->n;i++) {close_value(scores[i],g->scores[i]);close_value(ranked[i],g->ranked[i]);}
        for (int i=g->n;i<385;i++) assert(scores[i]==-7 && ranked[i]==-9);
        for (int i=0;i<g->k;i++) {assert(r.ids.e[i]==g->ids[i]);close_value(r.weights.e[i],g->weights[i]);}
        for (int i=0;i<g->keep;i++) assert(r.groups.e[i]==g->group_ids[i]);
        assert(r.has_margin==(g->excluded>=0) && r.excluded==g->excluded);
        close_value(r.cutoff,g->cutoff);close_value(r.runner,g->runner);assert(fabs(r.margin-g->margin)<=g->margin_bound);
        assert(r.has_group_margin==(g->group_excluded>=0) && r.group_excluded==g->group_excluded);
        assert(fabs(r.group_margin-g->group_margin)<=g->group_margin_bound);
        /* Every numeric comparison keeps both complete routing/margin reports. */
        coax_t_DrRoute again=psionic_dr_route(c,(coax_v_f32){x,g->n},(coax_v_f32){bias,g->profile?g->n:0},(coax_v_f64)V(scores),(coax_v_f64)V(ranked));
        coax_t_DrComparison compare=dr_compare(r,again);
        assert(compare.status==COAX_DrStatus_ok && compare.identical_ids && compare.changed==0 && compare.weight_l1==0);
        assert(compare.reference.margin==r.margin && compare.candidate.margin==again.margin);
        printf("router oracle %zu: experts=%d margin=%.17g group_margin=%.17g exact_ids=1\n",n,g->n,r.margin,r.group_margin);
    }
}
static void refusal(void) {
    float x[384]={0},bias[384]={0};double scores[384],ranked[384];
    coax_t_DrConfig c={.profile=COAX_DrProfile_v41,.k=6,.groups=1,.selected_groups=1,.scale=1.5};
    for (size_t i=0;i<384;i++) {scores[i]=-7;ranked[i]=-9;}
    uint32_t bad[]={0x7f800000,0xff800000,0x7fc12345,0xffc12345};
    for (size_t k=0;k<sizeof(bad)/sizeof(bad[0]);k++) for (int side=0;side<2;side++) for (int pos=0;pos<384;pos+=383) {
        float *input=side?bias:x;input[pos]=from_bits(bad[k]);
        coax_t_DrRoute r=psionic_dr_route(c,(coax_v_f32)V(x),(coax_v_f32)V(bias),(coax_v_f64)V(scores),(coax_v_f64)V(ranked));
        assert(r.status==COAX_DrStatus_number && r.count==0);input[pos]=0;
    }
    c.scale=NAN;assert(psionic_dr_route(c,(coax_v_f32)V(x),(coax_v_f32)V(bias),(coax_v_f64)V(scores),(coax_v_f64)V(ranked)).status==COAX_DrStatus_number);
    c.scale=INFINITY;assert(psionic_dr_route(c,(coax_v_f32)V(x),(coax_v_f32)V(bias),(coax_v_f64)V(scores),(coax_v_f64)V(ranked)).status==COAX_DrStatus_number);c.scale=1.5;
    assert(psionic_dr_route(c,(coax_v_f32)V(x),(coax_v_f32)V(bias),(coax_v_f64){scores,383},(coax_v_f64)V(ranked)).status==COAX_DrStatus_capacity);
    assert(psionic_dr_route(c,(coax_v_f32)V(x),(coax_v_f32)V(bias),(coax_v_f64)V(scores),(coax_v_f64)V(scores)).status==COAX_DrStatus_alias);
    union {double d[384];float f[768];} alias={0};
    assert(psionic_dr_route(c,(coax_v_f32){alias.f,384},(coax_v_f32)V(bias),(coax_v_f64){alias.d,384},(coax_v_f64)V(ranked)).status==COAX_DrStatus_alias);
    for (int i=0;i<384;i++) assert(scores[i]==-7 && ranked[i]==-9);
    for (int i=0;i<384;i++) {x[i]=-1000;}c.profile=COAX_DrProfile_v3;
    assert(psionic_dr_route(c,(coax_v_f32)V(x),(coax_v_f32)V(bias),(coax_v_f64)V(scores),(coax_v_f64)V(ranked)).status==COAX_DrStatus_arithmetic);
    for (int i=0;i<384;i++) assert(scores[i]==-7 && ranked[i]==-9);
}
int main(void) {oracle();refusal();puts("DeepSeek router: independent vectors, margins, atomic failures and aliases pass");return 0;}
