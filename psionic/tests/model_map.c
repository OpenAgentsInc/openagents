#include "model-map-memory.h"
#include <assert.h>
#include <stdio.h>
#include <string.h>
static coax_string str(const char *s){return (coax_string){s,(int64_t)strlen(s)};}
int main(void){
    coax_t_PdShape delta={1,1,1,1,1};coax_t_QfShape att={1,1,2,2,1};
    coax_t_QeShape shape={.linear={2,1,delta,COAX_QmDtype_bf16},.full={2,1,1,att,COAX_QmDtype_bf16},.state={3,1,1,delta,att}};
    const char *linear[]={"input_layernorm.weight","post_attention_layernorm.weight","linear_attn.conv1d.weight","linear_attn.A_log","linear_attn.dt_bias","linear_attn.norm.weight","linear_attn.in_proj_qkv.weight","linear_attn.in_proj_z.weight","linear_attn.in_proj_a.weight","linear_attn.in_proj_b.weight","linear_attn.out_proj.weight","mlp.gate_proj.weight","mlp.up_proj.weight","mlp.down_proj.weight"};
    const char *full[]={"input_layernorm.weight","post_attention_layernorm.weight","self_attn.q_norm.weight","self_attn.k_norm.weight","self_attn.q_proj.weight","self_attn.k_proj.weight","self_attn.v_proj.weight","self_attn.o_proj.weight","mlp.gate_proj.weight","mlp.up_proj.weight","mlp.down_proj.weight"};
    int64_t lr[]={1,1,3,1,1,1,2,2,2,2,2,2,2,2},fr[]={1,1,1,1,2,2,2,2,2,2,2};
    int64_t ld[][3]={{2,0,0},{2,0,0},{3,1,4},{1,0,0},{1,0,0},{1,0,0},{3,2,0},{1,2,0},{1,2,0},{1,2,0},{2,1,0},{1,2,0},{1,2,0},{2,1,0}};
    int64_t fd[][3]={{2,0,0},{2,0,0},{2,0,0},{2,0,0},{4,2,0},{2,2,0},{2,2,0},{2,2,0},{1,2,0},{1,2,0},{2,1,0}};
    coax_t_P5Tensor inventory[53];memset(inventory,0,sizeof(inventory));char names[53][128];unsigned char bytes[302]={0};int entry=0;int64_t at=0;
    for(int layer=0;layer<4;layer++){
        int n=layer==3?11:14;
        for(int i=0;i<n;i++){
            const char *suffix=layer==3?full[i]:linear[i];int64_t rank=layer==3?fr[i]:lr[i];int64_t *dims=layer==3?fd[i]:ld[i];
            assert(snprintf(names[entry],128,"model.language_model.layers.%d.%s",layer,suffix)>0);inventory[entry].name=str(names[entry]);inventory[entry].shape.rank=rank;
            int64_t size=1;for(int j=0;j<rank;j++){inventory[entry].shape.dims.e[j]=dims[j];size*=dims[j];}
            inventory[entry].elements=size;inventory[entry].begin=at;at+=2*size;inventory[entry].end=at;entry++;
        }
    }
    assert(entry==53 && at==302);coax_t_PaFile file={{bytes,302}};coax_t_QeLayer layers[5];memset(layers,0,sizeof(layers));float params[66],stage[65],work[27];
    for(int i=0;i<66;i++)params[i]=-7;
    coax_t_PmaResult r=psionic_map_model(shape,(coax_v_t_P5Tensor){inventory,53},(coax_v_t_PaFile){&file,1},(coax_v_t_QeLayer){layers,5},(coax_v_f32){params,66},(coax_v_f32){stage,65},(coax_v_f32){work,27});
    assert(r.status==COAX_PwStatus_ok && r.written==4 && r.parameters==65 && params[65]==-7);
    params[0]=5;params[57]=7;assert(layers[0].linear.input_norm.data[0]==5 && layers[3].full.input_norm.data[0]==7);
    params[0]=-7;layers[0].kind=9;inventory[52].name=str("missing");
    r=psionic_map_model(shape,(coax_v_t_P5Tensor){inventory,53},(coax_v_t_PaFile){&file,1},(coax_v_t_QeLayer){layers,5},(coax_v_f32){params,66},(coax_v_f32){stage,65},(coax_v_f32){work,27});
    assert(r.status==COAX_PwStatus_manifest && layers[0].kind==9 && params[0]==-7);
    inventory[52].name=str(names[52]);
    r=psionic_map_model(shape,(coax_v_t_P5Tensor){inventory,53},(coax_v_t_PaFile){&file,1},(coax_v_t_QeLayer){layers,5},(coax_v_f32){params,66},(coax_v_f32){params,65},(coax_v_f32){work,27});
    assert(r.status==COAX_PwStatus_alias && layers[0].kind==9 && params[0]==-7);
    puts("model mapping native: complete publication and late-failure isolation pass");
}
