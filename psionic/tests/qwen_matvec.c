#include <assert.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include "qwen-matvec-memory.h"
static coax_v_f32 fv(float *p,int64_t n) {return (coax_v_f32){p,n};}
static coax_v_u8 bv(uint8_t *p,int64_t n) {return (coax_v_u8){p,n};}
int main(void) {
    uint8_t bytes[]={0xee,0x80,0x3f,0x00,0x40,0x40,0x40,0x80,0x40};
    float x[]={1,2},out[]={-7,-7,-7},work[]={-9,-9,-9};
    coax_t_QmResult r=psionic_qwen_project(COAX_QmDtype_bf16,2,2,bv(bytes+1,8),fv(x,2),fv(out,3),fv(work,3));
    assert(r.status==COAX_QmStatus_ok && r.written==2 && out[0]==5 && out[1]==11 && out[2]==-7 && work[2]==-9);
    float before[3];memcpy(before,out,sizeof before);
    r=psionic_qwen_project(COAX_QmDtype_bf16,2,2,bv(bytes+1,8),fv(x,2),fv(out,3),fv(out,3));
    assert(r.status==COAX_QmStatus_alias && r.written==0);
    x[1]=NAN;r=psionic_qwen_project(COAX_QmDtype_bf16,2,2,bv(bytes+1,8),fv(x,2),fv(out,3),fv(work,3));
    assert(r.status==COAX_QmStatus_number && r.written==0);x[1]=2;
    bytes[7]=0x80;bytes[8]=0x7f;r=psionic_qwen_project(COAX_QmDtype_bf16,2,2,bv(bytes+1,8),fv(x,2),fv(out,3),fv(work,3));
    assert(r.status==COAX_QmStatus_number && r.written==0 && !memcmp(before,out,sizeof before));
    r=psionic_qwen_project(2,2,2,bv(bytes+1,8),fv(x,2),fv(out,3),fv(work,3));
    assert(r.status==COAX_QmStatus_shape);
    puts("Qwen matvec: unaligned original BF16 rows, finite and alias refusals pass");
}
