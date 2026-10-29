#include "rank5-memory.h"
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

int main(void) {
    const char *header="{\"vision\":{\"dtype\":\"BF16\",\"shape\":[1,2,1,1,1],\"data_offsets\":[0,4]},\"scalar\":{\"dtype\":\"F32\",\"shape\":[],\"data_offsets\":[4,8]}}";
    size_t n=strlen(header);unsigned char bytes[512]={0};
    for(int i=0;i<8;i++)bytes[i]=(unsigned char)((uint64_t)n>>(8*i));
    memcpy(bytes+8,header,n);coax_t_P5Tensor out[3],draft[3]; memset(out,0,sizeof(out)); memset(draft,0,sizeof(draft));
    coax_v_u8 file={bytes,(int64_t)n+16};coax_v_t_P5Tensor a={out,3},b={draft,3};
    coax_t_PwResult r=psionic_rank5_inspect(file,a,b);
    assert(r.status==COAX_PwStatus_ok && r.written==2 && r.bytes==8);
    assert(out[0].shape.rank==5 && out[0].shape.dims.e[1]==2 && out[0].shape.dims.e[4]==1 && out[0].elements==2);
    assert(out[1].shape.rank==0 && out[1].dtype==COAX_PwDtype_f32 && out[1].elements==1);
    out[0].elements=-7;
    r=psionic_rank5_inspect(file,a,a);assert(r.status==COAX_PwStatus_alias && out[0].elements==-7);
    r=psionic_rank5_inspect(file,(coax_v_t_P5Tensor){out,1},b);assert(r.status==COAX_PwStatus_capacity && out[0].elements==-7);
    bytes[8]=' ';r=psionic_rank5_inspect(file,a,b);assert(r.status!=COAX_PwStatus_ok && out[0].elements==-7);
    puts("rank-five inventory native: shapes, complete payload and atomic refusals pass");
}
