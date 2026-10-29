/* Fixture I/O/assertions only; arithmetic and admission come from Coax. */
#include "deepseek-v2-math-memory.h"
#include "mla-goldens.h"
#include "v2-rope-goldens.h"
#include <assert.h>
#include <math.h>
#include <stdio.h>
#include <string.h>
#define LEN(a) ((int64_t)(sizeof(a)/sizeof((a)[0])))
#define V(a) {a,LEN(a)}
static float bits(uint32_t b) {float x;memcpy(&x,&b,4);return x;}
static void latent(void) {
    const float *expected[2][4]={{mlg_compressed_outputs0,mlg_compressed_outputs1,mlg_compressed_outputs2,mlg_compressed_outputs3},
        {mlg_expanded_outputs0,mlg_expanded_outputs1,mlg_expanded_outputs2,mlg_expanded_outputs3}};
    const float *rows[2][4]={{mlg_compressed_rows0,mlg_compressed_rows1,mlg_compressed_rows2,mlg_compressed_rows3},
        {mlg_expanded_rows0,mlg_expanded_rows1,mlg_expanded_rows2,mlg_expanded_rows3}};
    const double *refs[2][4]={{mlg_compressed_scores0,mlg_compressed_scores1,mlg_compressed_scores2,mlg_compressed_scores3},
        {mlg_expanded_scores0,mlg_expanded_scores1,mlg_expanded_scores2,mlg_expanded_scores3}};
    coax_t_MlShape s={.heads=2,.nope=2,.rope=2,.value=2,.latent=3,.capacity=5};
    float wq[32],z[12],pe[8];uint8_t w[48];memcpy(w,mlg_weights,sizeof(w));memcpy(wq,mlg_queries,sizeof(wq));memcpy(z,mlg_latents,sizeof(z));memcpy(pe,mlg_pe,sizeof(pe));
    float actual[2][4][4];
    for (uint8_t mode=0;mode<2;mode++) {
        coax_t_MlPlan plan=ml_plan(s,mode);assert(plan.status==COAX_DsStatus_ok && plan.row==(mode?10:5));
        float history[50]={0},out[5],row[11],work[26];double scores[10];
        for (int64_t t=0;t<4;t++) {
            for (size_t i=0;i<5;i++) out[i]=-7;
            for (size_t i=0;i<11;i++) row[i]=-9;
            float before[50];memcpy(before,history,sizeof(history));
            coax_t_MlResult r=psionic_ml_attention(s,mode,(coax_v_u8)V(w),(coax_v_f32){wq+8*t,8},(coax_v_f32){z+3*t,3},
                (coax_v_f32){pe+2*t,2},(coax_v_f32){history,t*plan.row},t,0.375,(coax_v_f32)V(out),(coax_v_f32)V(row),
                (coax_v_f32){work,plan.floats},(coax_v_f64){scores,2*(t+1)});
            assert(r.status==COAX_DsStatus_ok && r.written==4 && r.row_written==plan.row);
            for (int i=0;i<4;i++) {assert(fabsf(out[i]-expected[mode][t][i])<2e-7f);actual[mode][t][i]=out[i];}
            assert(out[4]==-7);assert(memcmp(history,before,sizeof(history))==0);
            for (int64_t i=0;i<plan.row;i++) assert(row[i]==rows[mode][t][i]);
            for (int64_t i=plan.row;i<11;i++) assert(row[i]==-9);
            for (int i=0;i<2*(t+1);i++) assert(fabs(scores[i]-refs[mode][t][i])<1e-13);
            /* Same immutable prefix, a second call gives the same result/row. */
            float replay[4],fresh[10];
            r=psionic_ml_attention(s,mode,(coax_v_u8)V(w),(coax_v_f32){wq+8*t,8},(coax_v_f32){z+3*t,3},
                (coax_v_f32){pe+2*t,2},(coax_v_f32){history,t*plan.row},t,0.375,(coax_v_f32)V(replay),
                (coax_v_f32){fresh,plan.row},(coax_v_f32){work,plan.floats},(coax_v_f64){scores,2*(t+1)});
            assert(r.status==COAX_DsStatus_ok && memcmp(replay,out,sizeof(replay))==0 && memcmp(fresh,row,(size_t)plan.row*sizeof(float))==0);
            memcpy(history+t*plan.row,row,(size_t)plan.row*sizeof(float));
        }
    }
    double maximum=0;for (int t=0;t<4;t++) for (int j=0;j<4;j++) {double d=fabs((double)actual[0][t][j]-actual[1][t][j]);if (d>maximum) maximum=d;}
    assert(maximum<2e-7);printf("MLA both cache forms: 4 tokens, replay exact, max difference %.9g\n",maximum);
}
static void malformed(void) {
    coax_t_MlShape s={.heads=2,.nope=2,.rope=2,.value=2,.latent=3,.capacity=5};
    uint8_t w[48];float q[8],z[3],pe[2],history[5];
    memcpy(w,mlg_weights,sizeof(w));memcpy(q,mlg_queries,sizeof(q));memcpy(z,mlg_latents,sizeof(z));memcpy(pe,mlg_pe,sizeof(pe));memcpy(history,mlg_compressed_rows0,sizeof(history));
    float out[5]={7,7,7,7,7},row[5]={9,9,9,9,9},work[21];double scores[4];
    float *inputs[]={q,z,pe,history};int counts[]={8,3,2,5};uint32_t bad[]={0x7f800000,0xff800000,0x7fc12345,0xffc12345};
    for (int input=0;input<4;input++) for (int side=0;side<2;side++) for (int b=0;b<4;b++) {
        int at=side?counts[input]-1:0;float prior=inputs[input][at];inputs[input][at]=bits(bad[b]);
        coax_t_MlResult r=psionic_ml_attention(s,COAX_MlMode_compressed,(coax_v_u8)V(w),(coax_v_f32)V(q),
            (coax_v_f32)V(z),(coax_v_f32)V(pe),(coax_v_f32)V(history),1,0.375,(coax_v_f32)V(out),(coax_v_f32)V(row),(coax_v_f32)V(work),(coax_v_f64)V(scores));
        assert(r.status==COAX_DsStatus_number && r.written==0 && r.row_written==0);inputs[input][at]=prior;
    }
    assert(psionic_ml_attention(s,0,(coax_v_u8)V(w),(coax_v_f32)V(q),(coax_v_f32)V(z),(coax_v_f32)V(pe),(coax_v_f32)V(history),1,NAN,
        (coax_v_f32)V(out),(coax_v_f32)V(row),(coax_v_f32)V(work),(coax_v_f64)V(scores)).status==COAX_DsStatus_number);
    assert(psionic_ml_attention(s,0,(coax_v_u8)V(w),(coax_v_f32)V(q),(coax_v_f32)V(z),(coax_v_f32)V(pe),(coax_v_f32)V(history),1,INFINITY,
        (coax_v_f32)V(out),(coax_v_f32)V(row),(coax_v_f32)V(work),(coax_v_f64)V(scores)).status==COAX_DsStatus_number);
    assert(psionic_ml_attention(s,0,(coax_v_u8)V(w),(coax_v_f32)V(q),(coax_v_f32)V(z),(coax_v_f32)V(pe),(coax_v_f32)V(history),1,0.375,
        (coax_v_f32)V(out),(coax_v_f32){out,5},(coax_v_f32)V(work),(coax_v_f64)V(scores)).status==COAX_DsStatus_alias);
    assert(psionic_ml_attention(s,0,(coax_v_u8)V(w),(coax_v_f32)V(q),(coax_v_f32)V(z),(coax_v_f32)V(pe),(coax_v_f32)V(history),1,0.375,
        (coax_v_f32){q,4},(coax_v_f32)V(row),(coax_v_f32)V(work),(coax_v_f64)V(scores)).status==COAX_DsStatus_alias);
    for (int i=0;i<5;i++) {assert(out[i]==7);assert(row[i]==9);}
}
static void rotary(void) {
    const float *coeff[]={dvr_coeff0,dvr_coeff1,dvr_coeff2,dvr_coeff3,dvr_coeff4};
    const int widths[]={2,8,64,64,128},positions[]={0,1,4096,163839,163839};float out[129],stage[128];
    for (int c=0;c<5;c++) {
        for (int i=0;i<129;i++) out[i]=-7;
        coax_t_DsResult r=psionic_dv_yarn(widths[c],positions[c],(coax_v_f32)V(out),(coax_v_f32)V(stage));assert(r.status==COAX_DsStatus_ok && r.written==widths[c]);
        for (int i=0;i<widths[c];i++) assert(fabsf(out[i]-coeff[c][i])<=2e-7f);
        for (int i=widths[c];i<129;i++) assert(out[i]==-7);
    }
    float q[22],k[8],qo[23],ko[9],work[30];memcpy(q,dvr_query,sizeof(q));memcpy(k,dvr_key,sizeof(k));
    qo[22]=-7;ko[8]=-9;
    coax_t_DvRotateResult r=psionic_dv_rotary(2,3,8,(coax_v_f32)V(q),(coax_v_f32)V(k),731,(coax_v_f32)V(qo),(coax_v_f32)V(ko),(coax_v_f32)V(work));
    assert(r.status==COAX_DsStatus_ok && r.query_written==22 && r.key_written==8 && qo[22]==-7 && ko[8]==-9);
    for (int i=0;i<22;i++) assert(fabsf(qo[i]-dvr_expected_query[i])<5e-7f);
    for (int i=0;i<8;i++) assert(fabsf(ko[i]-dvr_expected_key[i])<5e-7f);
    for (int side=0;side<2;side++) {
        float *a=side?k:q;int at=side?7:21;float prior=a[at];a[at]=bits(0x7fc00001);
        assert(psionic_dv_rotary(2,3,8,(coax_v_f32)V(q),(coax_v_f32)V(k),731,(coax_v_f32)V(qo),(coax_v_f32)V(ko),(coax_v_f32)V(work)).status==COAX_DsStatus_number);a[at]=prior;
    }
    assert(psionic_dv_yarn(8,1,(coax_v_f32)V(out),(coax_v_f32){out,128}).status==COAX_DsStatus_alias);
    assert(psionic_dv_rotary(2,3,8,(coax_v_f32)V(q),(coax_v_f32)V(k),731,(coax_v_f32)V(qo),(coax_v_f32){qo,8},(coax_v_f32)V(work)).status==COAX_DsStatus_alias);
    puts("V2 YaRN: original widths, long/final positions, pair permutation and atomic refusals pass");
}
int main(void) {latent();malformed();rotary();puts("V2 attention math native pass");return 0;}
