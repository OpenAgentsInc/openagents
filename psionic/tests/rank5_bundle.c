#include "rank5-bundle-memory.h"
#include <assert.h>
#include <stdio.h>
#include <string.h>
static coax_string str(const char *s) {return (coax_string){s,(int64_t)strlen(s)};}
int main(void) {
    const char *head="{\"vision\":{\"dtype\":\"BF16\",\"shape\":[1,1,1,1,1],\"data_offsets\":[0,2]},\"bias\":{\"dtype\":\"F32\",\"shape\":[],\"data_offsets\":[2,6]}}";
    const char *sha="28abc5041aa051b548e3e2bfb6c2fa726172d96c78a684b0a72d60eec00dc6e4";
    const char *json_sha="44136fa355b3678a1146ad16f7e8649e94fb4fc21fe77e8310c060f61caaff8a";
    unsigned char file[512]={0};size_t n=strlen(head);for(int i=0;i<8;i++)file[i]=(unsigned char)((uint64_t)n>>(8*i));memcpy(file+8,head,n);
    unsigned char config[]="{}";coax_v_u8 text={config,2},none={NULL,0};
    coax_t_PwIdentity id={str("owned"),str("v2"),str(json_sha),str(json_sha),str("")};
    coax_t_PwShard shard={str("model.safetensors"),{file,(int64_t)n+14},(int64_t)n+14,str(sha)};
    coax_t_P5Spec specs[2];memset(specs,0,sizeof(specs));specs[0].name=str("vision");specs[0].shape.rank=5;
    for(int i=0;i<5;i++) {specs[0].shape.dims.e[i]=1;}
    specs[1].name=str("bias");specs[1].dtype=COAX_PwDtype_f32;
    coax_t_P5Tensor out[2],draft[2];memset(out,0,sizeof(out));memset(draft,0,sizeof(draft));
    coax_v_t_PwShard shards={&shard,1};coax_v_t_P5Spec expected={specs,2};coax_v_t_P5Tensor a={out,2},b={draft,2};
    coax_t_PwResult r=psionic_rank5_bundle(id,text,text,none,shards,expected,a,b);
    assert(r.status==COAX_PwStatus_ok && r.written==2 && r.bytes==6 && out[0].shape.rank==5);
    file[n+13]^=1;r=psionic_rank5_bundle(id,text,text,none,shards,expected,a,b);
    assert(r.status==COAX_PwStatus_digest && out[0].shape.rank==5);file[n+13]^=1;
    specs[0].shape.dims.e[4]=2;r=psionic_rank5_bundle(id,text,text,none,shards,expected,a,b);
    assert(r.status==COAX_PwStatus_manifest && out[0].shape.rank==5);specs[0].shape.dims.e[4]=1;
    r=psionic_rank5_bundle(id,text,text,none,shards,expected,a,a);assert(r.status==COAX_PwStatus_alias);
    coax_string original=specs[0].name;specs[0].name=(coax_string){(char *)out,1};
    r=psionic_rank5_bundle(id,text,text,none,shards,expected,a,b);assert(r.status==COAX_PwStatus_alias);specs[0].name=original;
    puts("rank-five bundle native: digest, manifest and alias checks pass");
}
