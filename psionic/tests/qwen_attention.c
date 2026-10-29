#include <assert.h>
#include <float.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include "qwen-attention-memory.h"
#include "qwen-attention-oracle.h"
_Static_assert(FLT_RADIX==2 && FLT_MANT_DIG==24 && DBL_MANT_DIG==53,"binary32/64 required");
static coax_v_f32 view(float *p,int64_t n) {return (coax_v_f32){p,n};}
static coax_v_u8 bytes(uint8_t *p,int64_t n) {return (coax_v_u8){p,n};}
static void near(const float *a,const float *b,size_t n) {for(size_t i=0;i<n;i++) assert(isfinite(a[i]) && fabsf(a[i]-b[i])<0.000002f);}
int main(void) {
    coax_t_QfShape s={4,2,4,4,0.125f};float p[60],out[65],scratch[16],a[80];
    memcpy(p,prep_in,sizeof prep_in);memcpy(a,att_in,sizeof att_in);
    for(int i=0;i<65;i++)out[i]=-7;
    coax_t_QfResult r=psionic_qwen_prepare(s,view(p,sizeof prep_in/sizeof(float)),view(out,65));
    assert(r.status==COAX_QfStatus_ok && r.written==(int64_t)(sizeof prep_out/sizeof(float)));
    near(out,prep_out,sizeof prep_out/sizeof(float));assert(out[r.written]==-7);
    uint8_t mask[]={1,0,1};s.rotary_dim=0;
    r=psionic_qwen_attend(s,3,view(a,sizeof att_in/sizeof(float)),bytes(mask,3),view(out,16),view(scratch,16));
    assert(r.status==COAX_QfStatus_ok && r.written==16);near(out,att_out,16);
    float old[16];memcpy(old,out,sizeof old);
    r=psionic_qwen_attend(s,3,view(a,sizeof att_in/sizeof(float)),bytes(mask,3),view(out,16),view(out,16));
    assert(r.status==COAX_QfStatus_alias && r.written==0);
    mask[2]=2;r=psionic_qwen_attend(s,3,view(a,sizeof att_in/sizeof(float)),bytes(mask,3),view(out,16),view(scratch,16));
    assert(r.status==COAX_QfStatus_mask);mask[2]=1;
    a[0]=NAN;r=psionic_qwen_attend(s,3,view(a,sizeof att_in/sizeof(float)),bytes(mask,3),view(out,16),view(scratch,16));
    assert(r.status==COAX_QfStatus_number);assert(!memcmp(old,out,sizeof old));
    s.rotary_dim=4;p[0]=INFINITY;r=psionic_qwen_prepare(s,view(p,sizeof prep_in/sizeof(float)),view(out,65));
    assert(r.status==COAX_QfStatus_number && !memcmp(old,out,sizeof old));
    puts("Qwen attention: split-half rotary preparation and gated grouped attention match independent references");
    puts("Qwen attention: alias, mask and non-finite refusals preserve the destination");
}
