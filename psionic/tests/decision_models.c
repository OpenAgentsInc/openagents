#include "decision-models.h"
#include <assert.h>
#include <stdio.h>
#include <string.h>
static int equal(coax_string a,const char *b){return a.count==(int64_t)strlen(b) && !memcmp(a.data,b,(size_t)a.count);}
int main(void){
    coax_t_PsTarget a=ps_target(0,128),b=ps_target(1,128);
    assert(a.status==COAX_PsStatus_ok && a.vocab==248320 && a.tensors==760 && a.shards==1 && a.shape.linear.width==4096 && a.shape.state.linear_layers==24);
    assert(equal(a.origin,"baselabs/elephant-1") && a.shard.e[0].bytes==18819722392LL && equal(a.shard.e[0].name,"model.safetensors"));
    assert(b.status==COAX_PsStatus_ok && b.tensors==1184 && b.shards==2 && b.shape.linear.width==5120 && b.shape.state.linear_layers==48 && b.shape.full.attention.heads==24);
    assert(equal(b.origin,"baselabs/mammoth-1") && b.shard.e[0].bytes==49825162976LL && b.shard.e[1].bytes==4888445168LL);
    assert(ps_target(2,1).status==COAX_PsStatus_kind && ps_target(0,0).status==COAX_PsStatus_capacity && ps_target(1,16385).status==COAX_PsStatus_capacity);
    puts("decision targets native: pinned models, shapes and bounds pass");
}
