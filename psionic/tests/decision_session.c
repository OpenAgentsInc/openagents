#include "decision-session-memory.h"
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
int main(void){
    unsigned char bytes[8]={0};coax_v_u8 v={bytes,8},empty={NULL,0};coax_t_PaFile file={v};
    coax_t_P5Spec *specs=calloc(760,sizeof(*specs));assert(specs);coax_t_PsWorkspace memory;memset(&memory,0,sizeof(memory));
    coax_t_PsSession s=psionic_session_open(2,1,v,v,v,empty,(coax_v_t_PaFile){&file,1},(coax_v_t_P5Spec){specs,760},v,memory);
    assert(s.status==COAX_PsOpenStatus_shape && s.model.vocab==0);
    s=psionic_session_open(0,1,v,v,v,empty,(coax_v_t_PaFile){&file,1},(coax_v_t_P5Spec){specs,1},v,memory);
    assert(s.status==COAX_PsOpenStatus_shape && s.model.vocab==0);
    s=psionic_session_open(0,1,v,v,v,empty,(coax_v_t_PaFile){&file,1},(coax_v_t_P5Spec){specs,760},v,memory);
    assert(s.status==COAX_PsOpenStatus_digest && s.model.vocab==0);
    float shared[4]={-7,-7,-7,-7};memory.norm=(coax_v_f32){shared,4};
    s=psionic_session_open(0,1,v,v,v,empty,(coax_v_t_PaFile){&file,1},(coax_v_t_P5Spec){specs,760},(coax_v_u8){(uint8_t *)shared,sizeof(shared)},memory);
    assert(s.status==COAX_PsOpenStatus_alias && shared[0]==-7);
    coax_t_P5Spec *mammoth=calloc(1184,sizeof(*mammoth));assert(mammoth);coax_t_PaFile two[2]={file,file};memory.norm=(coax_v_f32){NULL,0};
    s=psionic_session_open(1,1,v,v,v,v,(coax_v_t_PaFile){&file,1},(coax_v_t_P5Spec){mammoth,1184},v,memory);
    assert(s.status==COAX_PsOpenStatus_shape && s.model.vocab==0);
    s=psionic_session_open(1,1,v,v,v,v,(coax_v_t_PaFile){two,2},(coax_v_t_P5Spec){mammoth,1184},v,memory);
    assert(s.status==COAX_PsOpenStatus_digest && s.model.vocab==0);
    free(mammoth);free(specs);puts("session native: pinned targets, nested views and refusals pass");
}
