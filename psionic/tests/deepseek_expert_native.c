/* Independent bit vectors, numeric fixtures and real memory/alias assertions. */
#include "deepseek-expert-memory.h"
#include "expert-goldens.h"
#include <assert.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#define LEN(a) ((int64_t)(sizeof(a)/sizeof((a)[0])))
#define F(a) ((coax_v_f32){a,LEN(a)})
#define D(a) ((coax_v_f64){a,LEN(a)})
#define B(a) ((coax_v_u8){a,LEN(a)})
#define I(a) ((coax_v_s64){a,LEN(a)})
#define A(a) ((coax_v_t_PsionicAccumulator){a,LEN(a)})
static void near(const float *a,const float *b,int64_t n) {for (int64_t j=0;j<n;j++) assert(isfinite(a[j]) && fabsf(a[j]-b[j])<=2e-5f+1e-6f*fabsf(b[j]));}
static uint64_t bits(double x) {uint64_t b;memcpy(&b,&x,8);return b;}
static uint64_t little(const uint8_t *p) {uint64_t v=0;for (int i=0;i<8;i++) v|=(uint64_t)p[i]<<(8*i);return v;}
static void bit_vectors(const char *path) {
    FILE *f=fopen(path,"rb");assert(f);uint8_t *data=malloc(557064);assert(data && fread(data,1,557064,f)==557064 && fgetc(f)==EOF);fclose(f);assert(memcmp(data,"DEXBITS1",8)==0);
    for (int scale=0;scale<256;scale++) for (int code=0;code<16;code++) {
        coax_t_DeScalar r=de_fp4((uint8_t)code,(uint8_t)scale);assert(r.status==(scale==255?COAX_DsStatus_number:COAX_DsStatus_ok));if (scale!=255) assert(bits(r.value)==little(data+8+8*(16*scale+code)));
    }
    for (int scale=0;scale<256;scale++) for (int code=0;code<256;code++) {
        _Bool invalid=scale==255 || (code&127)==127;coax_t_DeScalar r=de_fp8((uint8_t)code,(uint8_t)scale);assert(r.status==(invalid?COAX_DsStatus_number:COAX_DsStatus_ok));if (!invalid) assert(bits(r.value)==little(data+8+32768+8*(256*scale+code)));
    }
    free(data);puts("Experts: every FP4/FP8 code x every UE8M0 scale is bit-exact, including signed zero and NaN refusal");
    for (int64_t i=0;i<LEN(deg_round_input);i++) {float x;memcpy(&x,&deg_round_input[i],4);coax_t_DeCode r=de_fp8_round(x);assert(r.status==COAX_DsStatus_ok && r.code==deg_round_codes[i]);}
    for (int64_t i=0;i<LEN(deg_scale_input);i++) {float x[32]={0};memcpy(&x[0],&deg_scale_input[i],4);coax_t_DeCode r=de_scale(F(x));assert(r.status==COAX_DsStatus_ok && r.code==deg_scale_codes[i]);}
}
static void complete(void) {
    const float *expected[]={deg_0_output,deg_1_output,deg_2_output,deg_3_output,deg_4_output};
    for (int which=0;which<5;which++) {
        _Alignas(double) uint8_t bytes[8192];memset(bytes,165,sizeof(bytes));coax_t_DeExpert e=de_test_fixture((coax_t_DeFixture)which,B(bytes));coax_t_DePlan p=de_plan(e);assert(p.status==COAX_DsStatus_ok);
        float input[64],out[65],storage[512];double wide[64];memcpy(input,deg_input,sizeof(input));for (int i=0;i<65;i++) out[i]=-7;
        coax_t_DeWork work={.floats=F(storage),.wide=D(wide)};float route=which==4?.371f:which==3?1:.137f;
        coax_t_DeResult r=psionic_de_expert(e,(coax_v_f32){input,p.width},route,10,F(out),work);assert(r.status==COAX_DsStatus_ok && r.written==p.width);near(out,expected[which],p.width);assert(out[p.width]==-7);
        /* The final down-projection is late enough to test whole-expert commit. */
        uint8_t saved=e.down.scales.data[e.down.scales.count-1];e.down.scales.data[e.down.scales.count-1]=255;float previous[65];memcpy(previous,out,sizeof(out));
        r=psionic_de_expert(e,(coax_v_f32){input,p.width},route,10,F(out),work);assert(r.status==COAX_DsStatus_number && r.written==0 && memcmp(out,previous,sizeof(out))==0);e.down.scales.data[e.down.scales.count-1]=saved;
        assert(psionic_de_expert(e,(coax_v_f32){input,p.width},route,10,(coax_v_f32){storage,p.width},work).status==COAX_DsStatus_alias);
        assert(psionic_de_expert(e,(coax_v_f32){input,p.width},route,10,(coax_v_f32){(float *)bytes,p.width},work).status==COAX_DsStatus_alias);
    }
    puts("Experts: complete FP4 and shared FP8 FFNs, both unequal shapes, nested aliases and late down failure pass");
}
static void mixtures(void) {
    uint8_t b0[8192],b1[8192],b2[8192],bs[8192];coax_t_DeExpert e[]={de_test_fixture(0,B(b0)),de_test_fixture(1,B(b1)),de_test_fixture(2,B(b2))},shared=de_test_fixture(3,B(bs));
    float input[64],out[65],storage[352],weights[]={.137f,.719f};int64_t ids[]={2,0};double wide[64];coax_t_PsionicAccumulator acc[64];memcpy(input,deg_input,sizeof(input));coax_t_DeWork work={.floats=F(storage),.wide=D(wide)};out[64]=-7;
    for (int i=0;i<64;i++) acc[i]=(coax_t_PsionicAccumulator){.sum=17,.correction=19};
    coax_t_DeResult r=psionic_de_mixture((coax_v_t_DeExpert){e,3},shared,F(input),I(ids),F(weights),10,(coax_v_f32){out,64},work,A(acc));assert(r.status==COAX_DsStatus_ok && r.written==64);near(out,deg_combined,64);assert(out[64]==-7);
    /* An unselected expert's coefficient values must not affect the result. */
    uint8_t saved=e[1].gate.scales.data[0];e[1].gate.scales.data[0]=255;
    r=psionic_de_mixture((coax_v_t_DeExpert){e,3},shared,F(input),I(ids),F(weights),10,(coax_v_f32){out,64},work,A(acc));assert(r.status==COAX_DsStatus_ok);near(out,deg_combined,64);
    float previous[65];memcpy(previous,out,sizeof(out));ids[0]=1;r=psionic_de_mixture((coax_v_t_DeExpert){e,3},shared,F(input),I(ids),F(weights),10,F(out),work,A(acc));assert(r.status==COAX_DsStatus_number && memcmp(out,previous,sizeof(out))==0);ids[0]=2;e[1].gate.scales.data[0]=saved;
    saved=shared.down.scales.data[shared.down.scales.count-1];shared.down.scales.data[shared.down.scales.count-1]=255;r=psionic_de_mixture((coax_v_t_DeExpert){e,3},shared,F(input),I(ids),F(weights),10,F(out),work,A(acc));assert(r.status==COAX_DsStatus_number && memcmp(out,previous,sizeof(out))==0);shared.down.scales.data[shared.down.scales.count-1]=saved;
    float alias_weights[64]={.137f,.719f};assert(psionic_de_mixture((coax_v_t_DeExpert){e,3},shared,F(input),I(ids),(coax_v_f32){alias_weights,2},10,F(alias_weights),work,A(acc)).status==COAX_DsStatus_alias);
    /* Full original expert count; only six selected experts execute. */
    coax_t_DeExpert all[384];for (int i=0;i<384;i++) all[i]=e[i%3];int64_t selected[]={1,7,64,128,255,383};float ws[]={.1f,.2f,.3f,.4f,.5f,.6f};
    r=psionic_de_mixture((coax_v_t_DeExpert){all,384},shared,F(input),I(selected),F(ws),10,F(out),work,A(acc));assert(r.status==COAX_DsStatus_ok && r.written==64 && out[64]==-7);
    float replay[65];memcpy(replay,out,sizeof(out));r=psionic_de_mixture((coax_v_t_DeExpert){all,384},shared,F(input),I(selected),F(ws),10,F(out),work,A(acc));assert(r.status==COAX_DsStatus_ok && memcmp(out,replay,sizeof(out))==0);
    puts("Experts: routed+shared parity, 384/6 shape, dirty work replay, selected-only values and atomic shared failure pass");
}
static void malformed(void) {
    float x[64]={0},out[65],stage[64];double q[65],qs[64];for (int i=0;i<65;i++) {out[i]=7;q[i]=9;}x[63]=NAN;
    assert(psionic_de_quantize(F(x),D(q),D(qs)).status==COAX_DsStatus_number);for (int i=0;i<65;i++) assert(q[i]==9);x[63]=0;
    assert(de_fp8_round(NAN).status==COAX_DsStatus_number && de_fp8_round(INFINITY).status==COAX_DsStatus_number);
    assert(de_activation(NAN,1,1,10).status==COAX_DsStatus_number && de_activation(1,INFINITY,1,10).status==COAX_DsStatus_number && de_activation(1,1,NAN,10).status==COAX_DsStatus_number && de_activation(1,1,1,INFINITY).status==COAX_DsStatus_number);
    float gate[2]={1,2},up[2]={2,INFINITY};assert(psionic_de_activate(F(gate),F(up),1,10,F(out),F(stage)).status==COAX_DsStatus_number);for (int i=0;i<65;i++) assert(out[i]==7);
    uint8_t bytes[8192];coax_t_DeExpert e=de_test_fixture(3,B(bytes));uint8_t saved=e.gate.data.data[e.gate.data.count-1];e.gate.data.data[e.gate.data.count-1]=127;
    assert(psionic_de_project(e.gate,F(x),F(out),D(qs),F(stage)).status==COAX_DsStatus_number);for (int i=0;i<65;i++) assert(out[i]==7);e.gate.data.data[e.gate.data.count-1]=saved;
    assert(psionic_de_project(e.gate,F(x),F(x),D(qs),F(stage)).status==COAX_DsStatus_alias);
    assert(psionic_de_project(e.gate,F(x),F(out),D(qs),F(out)).status==COAX_DsStatus_alias);
    assert(psionic_de_quantize(F(x),D(q),D(q)).status==COAX_DsStatus_alias);
    assert(psionic_de_decode(e.gate,D(q),D(q)).status==COAX_DsStatus_capacity);
    double decoded[2048];assert(psionic_de_decode(e.gate,D(decoded),D(decoded)).status==COAX_DsStatus_alias);
    puts("Experts: NaN/Inf, invalid FP8 payloads, activation atomicity and mixed-width aliases pass");
}
int main(int argc,char **argv) {assert(argc==2);bit_vectors(argv[1]);complete();mixtures();malformed();return 0;}
