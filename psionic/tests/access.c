#include "access-memory.h"
#include <assert.h>
#include <stdio.h>
#include <string.h>
static coax_string str(const char *s) {return (coax_string){s,(int64_t)strlen(s)};}
int main(void) {
    unsigned char bytes[9]={99,0x80,0x3f,0,0x40,0x40,0x40,99,99};
    coax_t_P5Shape shape={1,{{3,0,0,0,0}}};coax_t_P5Tensor tensor;memset(&tensor,0,sizeof(tensor));
    tensor.name=str("weights");tensor.shape=shape;tensor.dtype=COAX_PwDtype_bf16;tensor.elements=3;tensor.begin=1;tensor.end=7;
    coax_t_PaFile file={.data={bytes,9}};
    coax_t_PaView view=pa_view(str("weights"),shape,COAX_PwDtype_bf16,(coax_v_t_P5Tensor){&tensor,1},(coax_v_t_PaFile){&file,1});
    assert(view.status==COAX_PwStatus_ok && view.data.data==bytes+1 && view.data.count==6);
    float out[4]={-7,-7,-7,-7},work[4]={-9,-9,-9,-9};
    coax_t_PwResult r=psionic_access_decode(view,(coax_v_f32){out,4},(coax_v_f32){work,4});
    assert(r.status==COAX_PwStatus_ok && r.written==3 && out[0]==1 && out[1]==2 && out[2]==3 && out[3]==-7 && work[3]==-9);
    out[0]=out[1]=out[2]=-7;bytes[5]=0x80;bytes[6]=0x7f;
    r=psionic_access_decode(view,(coax_v_f32){out,4},(coax_v_f32){work,4});assert(r.status==COAX_PwStatus_number && out[0]==-7 && out[2]==-7);
    r=psionic_access_decode(view,(coax_v_f32){out,4},(coax_v_f32){out,4});assert(r.status==COAX_PwStatus_alias && out[0]==-7);
    tensor.end=10;view=pa_view(str("weights"),shape,COAX_PwDtype_bf16,(coax_v_t_P5Tensor){&tensor,1},(coax_v_t_PaFile){&file,1});assert(view.status==COAX_PwStatus_offsets);
    puts("original tensor access native: unaligned bytes, decoding and refusal pass");
}
