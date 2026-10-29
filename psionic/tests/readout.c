#include "readout-memory.h"
#include <assert.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>

int main(void) {
    const int64_t vocab=248320;
    float *logits=calloc((size_t)vocab,sizeof(*logits)); assert(logits);
    int64_t slots[256];
    for (int64_t i=0;i<256;i++) {
        slots[i]=qr_pinned_slot(i); assert(slots[i]>=0 && slots[i]<vocab);
        for (int64_t j=0;j<i;j++) assert(slots[i]!=slots[j]);
    }
    assert(slots[0]==32 && slots[1]==33 && qr_pinned_slot(-1)==-1 && qr_pinned_slot(256)==-1);
    double out[3]={-7,-7,-7},work[3]={-9,-9,-9};
    coax_v_f32 x={logits,vocab}; coax_v_s64 ids={slots,2}; coax_v_f64 y={out,3},w={work,3};
    coax_t_QrResult r=psionic_readout(COAX_QrKind_choice,1,x,ids,y,w);
    assert(r.status==COAX_QrStatus_ok && r.written==2 && r.best==0);
    assert(fabs(r.mass-2.0/(double)vocab)<1e-18 && r.confidence==0);
    assert(out[0]==.5 && out[1]==.5 && out[2]==-7 && work[2]==-9);
    logits[slots[0]]=2; logits[slots[1]]=1;
    r=psionic_readout(COAX_QrKind_choice,2,x,ids,y,w);
    double p=exp(1.0)/(exp(1.0)+exp(.5));
    assert(r.status==COAX_QrStatus_ok && r.best==0 && fabs(out[0]-p)<1e-14);
    assert(fabs(r.mass-(exp(2.0)+exp(1.0))/(exp(2.0)+exp(1.0)+vocab-2))<1e-14);
    assert(fabs(r.confidence-(2*p-1))<1e-14);
    out[0]=out[1]=-7; logits[vocab-1]=NAN;
    r=psionic_readout(COAX_QrKind_choice,1,x,ids,y,w);
    assert(r.status==COAX_QrStatus_number && out[0]==-7 && out[1]==-7);
    logits[vocab-1]=0;
    r=psionic_readout(COAX_QrKind_choice,1,x,ids,y,(coax_v_f64){out,2});
    assert(r.status==COAX_QrStatus_alias && out[0]==-7 && out[1]==-7);
    r=psionic_readout(COAX_QrKind_choice,1,x,ids,(coax_v_f64){(double *)slots,2},w);
    assert(r.status==COAX_QrStatus_alias && slots[0]==32 && slots[1]==33);
    r=psionic_readout(COAX_QrKind_choice,1,x,ids,y,(coax_v_f64){(double *)logits,2});
    assert(r.status==COAX_QrStatus_alias && out[0]==-7);
    slots[1]=slots[0]; r=psionic_readout(COAX_QrKind_noul,1,x,ids,y,w);
    assert(r.status==COAX_QrStatus_index && out[0]==-7 && out[1]==-7);
    slots[1]=33; r=psionic_readout(COAX_QrKind_noul,1,x,ids,(coax_v_f64){out,1},w);
    assert(r.status==COAX_QrStatus_capacity && out[0]==-7);
    free(logits); puts("readout native: full vocabulary, original slots, statistics and refusal checks pass");
}
