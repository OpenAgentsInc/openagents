#include "layer-map-memory.h"
#include <assert.h>
#include <stdio.h>
#include <string.h>
static coax_string str(const char *s){return (coax_string){s,(int64_t)strlen(s)};}
int main(void){
    coax_t_PdShape delta={1,1,1,1,1};coax_t_QfShape att={1,1,2,2,1};
    coax_t_QeShape shape={.linear={2,1,delta,COAX_QmDtype_bf16},.full={2,1,1,att,COAX_QmDtype_bf16},.state={3,1,1,delta,att}};
    const char *names[]={"model.language_model.layers.0.input_layernorm.weight","model.language_model.layers.0.post_attention_layernorm.weight","model.language_model.layers.0.linear_attn.conv1d.weight","model.language_model.layers.0.linear_attn.A_log","model.language_model.layers.0.linear_attn.dt_bias","model.language_model.layers.0.linear_attn.norm.weight","model.language_model.layers.0.linear_attn.in_proj_qkv.weight","model.language_model.layers.0.linear_attn.in_proj_z.weight","model.language_model.layers.0.linear_attn.in_proj_a.weight","model.language_model.layers.0.linear_attn.in_proj_b.weight","model.language_model.layers.0.linear_attn.out_proj.weight","model.language_model.layers.0.mlp.gate_proj.weight","model.language_model.layers.0.mlp.up_proj.weight","model.language_model.layers.0.mlp.down_proj.weight"};
    int64_t ranks[]={1,1,3,1,1,1,2,2,2,2,2,2,2,2};
    int64_t dims[][3]={{2,0,0},{2,0,0},{3,1,4},{1,0,0},{1,0,0},{1,0,0},{3,2,0},{1,2,0},{1,2,0},{1,2,0},{2,1,0},{1,2,0},{1,2,0},{2,1,0}};
    unsigned char bytes[78]={0};coax_t_P5Tensor inventory[14];memset(inventory,0,sizeof(inventory));int64_t at=0;
    for(int i=0;i<14;i++){
        int64_t size=1;inventory[i].name=str(names[i]);inventory[i].shape.rank=ranks[i];
        for(int j=0;j<ranks[i];j++){inventory[i].shape.dims.e[j]=dims[i][j];size*=dims[i][j];}
        inventory[i].elements=size;inventory[i].begin=at;at+=2*size;inventory[i].end=at;
    }
    assert(at==78);coax_t_PaFile file={.data={bytes,78}};coax_v_t_PaFile files={&file,1};coax_v_t_P5Tensor iv={inventory,14};
    float out[20],work[20];for(int i=0;i<20;i++){out[i]=-7;work[i]=-9;}
    coax_t_PmResult r=psionic_map_layer(shape,0,iv,files,(coax_v_f32){out,20},(coax_v_f32){work,20});
    assert(r.status==COAX_PwStatus_ok && r.written==19 && r.layer.linear.conv.count==12 && r.layer.linear.qkv.count==12 && out[19]==-7);
    for(int i=0;i<20;i++)out[i]=-7;
    r=psionic_map_layer(shape,0,iv,files,(coax_v_f32){out,20},(coax_v_f32){out,20});assert(r.status==COAX_PwStatus_alias && out[0]==-7);
    coax_string old=inventory[0].name;inventory[0].name=(coax_string){(const char *)out,1};
    r=psionic_map_layer(shape,0,iv,files,(coax_v_f32){out,20},(coax_v_f32){work,20});assert(r.status==COAX_PwStatus_alias && out[0]==-7);inventory[0].name=old;
    coax_t_PaFile other={.data={(uint8_t *)out,78}};
    r=psionic_map_layer(shape,0,iv,(coax_v_t_PaFile){&other,1},(coax_v_f32){out,20},(coax_v_f32){work,20});assert(r.status==COAX_PwStatus_alias && out[0]==-7);
    puts("layer mapping native: original views, parameter staging and aliases pass");
}
