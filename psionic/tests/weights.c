/* Independent bytes/IEEE arithmetic, actual files, allocations and aliases. */
#include <assert.h>
#include <stdbool.h>
#include <float.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include "weights-memory.h"
#include "weights-fixtures.h"
_Static_assert(sizeof(float)==4 && FLT_MANT_DIG==24 && FLT_RADIX==2,"binary32");
static coax_string str(const char *s) {return (coax_string){(const char *)s,(int64_t)strlen(s)};}
static coax_v_u8 bytes(const void *p,size_t n) {return (coax_v_u8){(uint8_t *)p,(int64_t)n};}
static coax_v_f32 floats(float *p,int64_t n) {return (coax_v_f32){p,n};}
static coax_v_t_PwTensor tensors(coax_t_PwTensor *p,int64_t n) {return (coax_v_t_PwTensor){p,n};}
static uint32_t bits(float f) {uint32_t b;memcpy(&b,&f,4);return b;}
static void failed(coax_t_PwResult r,int status) {assert(r.status==status && r.written==0 && r.bytes==0);}
static uint8_t *read_original(const char *name,const uint8_t *expected,size_t count) {
    char path[256];snprintf(path,sizeof path,"psionic/fixtures/models/ch04/%s",name);
    FILE *file=fopen(path,"rb");assert(file);uint8_t *out=malloc(count);assert(out);
    assert(fread(out,1,count,file)==count && fgetc(file)==EOF);assert(fclose(file)==0);
    assert(memcmp(out,expected,count)==0);return out;
}
static coax_v_u8 make_file(const char *header,const uint8_t *payload,size_t count) {
    size_t n=strlen(header);uint8_t *p=malloc(8+n+count);assert(p);
    for(int i=0;i<8;i++) p[i]=(uint8_t)((uint64_t)n>>(8*i));
    memcpy(p+8,header,n);memcpy(p+8+n,payload,count);return bytes(p,8+n+count);
}
static void originals(void) {
    uint8_t *cfg=read_original("config.json",CH04_CONFIG_JSON,sizeof CH04_CONFIG_JSON);
    uint8_t *tok=read_original("tokenizer.json",CH04_TOKENIZER_JSON,sizeof CH04_TOKENIZER_JSON);
    uint8_t *one=read_original("model.safetensors",CH04_MODEL_SAFETENSORS,sizeof CH04_MODEL_SAFETENSORS);
    uint8_t *a=read_original("model-00001-of-00002.safetensors",CH04_MODEL_00001_OF_00002_SAFETENSORS,sizeof CH04_MODEL_00001_OF_00002_SAFETENSORS);
    uint8_t *b=read_original("model-00002-of-00002.safetensors",CH04_MODEL_00002_OF_00002_SAFETENSORS,sizeof CH04_MODEL_00002_OF_00002_SAFETENSORS);
    uint8_t *idx=read_original("model.safetensors.index.json",CH04_MODEL_SAFETENSORS_INDEX_JSON,sizeof CH04_MODEL_SAFETENSORS_INDEX_JSON);
    uint8_t *manifest=read_original("manifest.json",CH04_MANIFEST_JSON,sizeof CH04_MANIFEST_JSON);free(manifest);
    coax_t_PwIdentity id={str("openagents-owned-ch04-fixture"),str("owned-v1"),str(CH04_CONFIG_SHA),str(CH04_TOKENIZER_SHA),str(CH04_MODEL_SAFETENSORS_INDEX_JSON_SHA)};
    coax_t_PwShard shards[]={
        {str("model-00001-of-00002.safetensors"),bytes(a,sizeof CH04_MODEL_00001_OF_00002_SAFETENSORS),sizeof CH04_MODEL_00001_OF_00002_SAFETENSORS,str(CH04_MODEL_00001_OF_00002_SAFETENSORS_SHA),false},
        {str("model-00002-of-00002.safetensors"),bytes(b,sizeof CH04_MODEL_00002_OF_00002_SAFETENSORS),sizeof CH04_MODEL_00002_OF_00002_SAFETENSORS,str(CH04_MODEL_00002_OF_00002_SAFETENSORS_SHA),false}};
    coax_t_PwSpec specs[]={
        {str("scale"),{0,{{0,0,0,0}}},COAX_PwDtype_bf16,1},
        {str("linear.bias"),{1,{{2,0,0,0}}},COAX_PwDtype_f32,1},
        {str("linear.weight"),{2,{{2,3,0,0}}},COAX_PwDtype_bf16,0}};
    coax_t_PwTensor out[4]={0},draft[4]={0},saved[4];
    coax_v_t_PwShard sv={shards,2};coax_v_t_PwSpec ev={specs,3};
    coax_v_u8 cv=bytes(cfg,sizeof CH04_CONFIG_JSON),tv=bytes(tok,sizeof CH04_TOKENIZER_JSON),iv=bytes(idx,sizeof CH04_MODEL_SAFETENSORS_INDEX_JSON);
    coax_t_PwResult r=psionic_weights_bundle(id,cv,tv,iv,sv,ev,tensors(out,4),tensors(draft,4));
    assert(r.status==0 && r.written==3 && r.bytes==22 && out[0].shard==1 && out[2].elements==6);
    memcpy(saved,out,sizeof out);
    for(int i=0;i<2;i++) {
        uint8_t *raw=shards[i].data.data;int64_t n=shards[i].data.count;
        raw[n-1]^=1;failed(psionic_weights_bundle(id,cv,tv,iv,sv,ev,tensors(out,4),tensors(draft,4)),COAX_PwStatus_digest);raw[n-1]^=1;
        assert(memcmp(out,saved,sizeof out)==0);
    }
    cfg[0]^=1;failed(psionic_weights_bundle(id,cv,tv,iv,sv,ev,tensors(out,4),tensors(draft,4)),COAX_PwStatus_digest);cfg[0]^=1;
    tok[0]^=1;failed(psionic_weights_bundle(id,cv,tv,iv,sv,ev,tensors(out,4),tensors(draft,4)),COAX_PwStatus_digest);tok[0]^=1;
    idx[0]^=1;failed(psionic_weights_bundle(id,cv,tv,iv,sv,ev,tensors(out,4),tensors(draft,4)),COAX_PwStatus_digest);idx[0]^=1;
    specs[2].dtype=255;failed(psionic_weights_bundle(id,cv,tv,iv,sv,ev,tensors(out,4),tensors(draft,4)),COAX_PwStatus_manifest);specs[2].dtype=0;
    /* Nested readonly views, top-level descriptor arrays and identity strings. */
    coax_string old=shards[0].sha;shards[0].sha=(coax_string){(const char *)out,64};
    failed(psionic_weights_bundle(id,cv,tv,iv,sv,ev,tensors(out,4),tensors(draft,4)),COAX_PwStatus_alias);shards[0].sha=old;
    old=specs[0].name;specs[0].name=(coax_string){(const char *)draft,1};
    failed(psionic_weights_bundle(id,cv,tv,iv,sv,ev,tensors(out,4),tensors(draft,4)),COAX_PwStatus_alias);specs[0].name=old;
    old=id.revision;id.revision=(coax_string){(const char *)out,1};
    failed(psionic_weights_bundle(id,cv,tv,iv,sv,ev,tensors(out,4),tensors(draft,4)),COAX_PwStatus_alias);id.revision=old;
    coax_string *labels[]={&id.origin,&id.config_sha,&id.tokenizer_sha,&id.index_sha,&shards[0].name};
    for(size_t i=0;i<sizeof labels/sizeof *labels;i++) {
        old=*labels[i];*labels[i]=(coax_string){(const char *)out,1};
        failed(psionic_weights_bundle(id,cv,tv,iv,sv,ev,tensors(out,4),tensors(draft,4)),COAX_PwStatus_alias);*labels[i]=old;
    }
    coax_v_u8 *sources[]={&cv,&tv,&iv,&shards[0].data};
    for(size_t i=0;i<sizeof sources/sizeof *sources;i++) {
        coax_v_u8 previous=*sources[i];*sources[i]=bytes(out,(size_t)previous.count);
        failed(psionic_weights_bundle(id,cv,tv,iv,sv,ev,tensors(out,4),tensors(draft,4)),COAX_PwStatus_alias);*sources[i]=previous;
    }
    /* The read-only descriptor arrays themselves are also borrowed storage. */
    void *shared=malloc(sizeof out);assert(shared);memcpy(shared,shards,sizeof shards);
    failed(psionic_weights_bundle(id,cv,tv,iv,(coax_v_t_PwShard){shared,2},ev,tensors(shared,4),tensors(draft,4)),COAX_PwStatus_alias);
    memcpy(shared,specs,sizeof specs);
    failed(psionic_weights_bundle(id,cv,tv,iv,sv,(coax_v_t_PwSpec){shared,3},tensors(shared,4),tensors(draft,4)),COAX_PwStatus_alias);free(shared);
    failed(psionic_weights_bundle(id,cv,tv,iv,sv,ev,tensors(out,4),tensors(out,4)),COAX_PwStatus_alias);
    coax_v_t_PwShard impossible={NULL,17};
    failed(psionic_weights_bundle(id,cv,tv,iv,impossible,ev,tensors(out,4),tensors(draft,4)),COAX_PwStatus_size);
    coax_t_PwShard single={str("model.safetensors"),bytes(one,sizeof CH04_MODEL_SAFETENSORS),sizeof CH04_MODEL_SAFETENSORS,str(CH04_MODEL_SAFETENSORS_SHA),false};
    for(int i=0;i<3;i++) specs[i].shard=0;id.index_sha=str("");
    r=psionic_weights_bundle(id,cv,tv,bytes(NULL,0),(coax_v_t_PwShard){&single,1},ev,tensors(out,4),tensors(draft,4));
    assert(r.status==0 && r.written==3);
    float values[7],stage[7];for(int i=0;i<7;i++) values[i]=stage[i]=-9;
    r=psionic_weights_decode(single.data,str("linear.weight"),0,6,floats(values,7),floats(stage,7),tensors(out,4),tensors(draft,4));
    assert(r.status==0);for(int i=0;i<6;i++) assert(values[i]==i+1 && stage[i]==i+1);assert(values[6]==-9);
    float shortbuf[1]={-8};
    failed(psionic_weights_decode(single.data,str("linear.weight"),0,6,floats(shortbuf,1),floats(stage,7),tensors(out,4),tensors(draft,4)),COAX_PwStatus_range);assert(shortbuf[0]==-8);
    failed(psionic_weights_decode(single.data,str("scale"),0,1,floats(values,7),floats(values+1,6),tensors(out,4),tensors(draft,4)),COAX_PwStatus_alias);
    failed(psionic_weights_decode(single.data,(coax_string){(const char *)values,1},0,1,floats(values,7),floats(stage,7),tensors(out,4),tensors(draft,4)),COAX_PwStatus_alias);
    failed(psionic_weights_decode(bytes(values,sizeof values),str("x"),0,1,floats(values,7),floats(stage,7),tensors(out,4),tensors(draft,4)),COAX_PwStatus_alias);
    failed(psionic_weights_decode(single.data,str("scale"),0,1,floats((float *)out,1),floats(stage,7),tensors(out,4),tensors(draft,4)),COAX_PwStatus_alias);
    failed(psionic_weights_inspect(bytes(out,sizeof out),tensors(out,4),tensors(draft,4)),COAX_PwStatus_alias);
    failed(psionic_weights_inspect(single.data,tensors(out,4),tensors(out+1,3)),COAX_PwStatus_alias);
    coax_t_PwTensor adjacent[6];r=psionic_weights_inspect(single.data,tensors(adjacent,3),tensors(adjacent+3,3));assert(r.status==0);
    for(int64_t n=0;n<single.data.count;n++) {
        memcpy(saved,out,sizeof out);r=psionic_weights_inspect(bytes(one,(size_t)n),tensors(out,4),tensors(draft,4));
        assert(r.status!=0 && r.written==0 && r.bytes==0 && memcmp(saved,out,sizeof out)==0);
    }
    free(cfg);free(tok);free(one);free(a);free(b);free(idx);
}
static void representations(void) {
    uint8_t *payload=malloc(131072);float *out=malloc(65536*4),*scratch=malloc(65536*4);assert(payload && out && scratch);
    for(uint32_t b=0;b<65536;b++) {
        bool finite=(b&0x7f80)!=0x7f80;uint32_t expected=b*65536u;
        assert(pw_bf16_bits((uint16_t)b)==expected);
        uint16_t stored=finite?(uint16_t)b:0;payload[2*b]=(uint8_t)stored;payload[2*b+1]=(uint8_t)(stored>>8);
    }
    coax_v_u8 file=make_file("{\"x\":{\"dtype\":\"BF16\",\"shape\":[65536],\"data_offsets\":[0,131072]}}",payload,131072);
    coax_t_PwTensor inv[1],draft[1];
    coax_t_PwResult r=psionic_weights_decode(file,str("x"),0,65536,floats(out,65536),floats(scratch,65536),tensors(inv,1),tensors(draft,1));assert(r.status==0);
    for(uint32_t b=0;b<65536;b++) {
        if((b&0x7f80)==0x7f80) {assert(bits(out[b])==0);continue;}
        int exponent=(b>>7)&255, fraction=b&127;
        double value=exponent?ldexp(1.0+fraction/128.0,exponent-127):ldexp((double)fraction,-133);
        if(b&0x8000) value=-value;
        assert(bits(out[b])==bits((float)value));
    }
    for(uint32_t b=0;b<65536;b++) if((b&0x7f80)==0x7f80) {
        size_t at=(size_t)pw_header(file).data;file.data[at]=(uint8_t)b;file.data[at+1]=(uint8_t)(b>>8);
        out[0]=-7;scratch[0]=-9;
        failed(psionic_weights_decode(file,str("x"),0,1,floats(out,1),floats(scratch,1),tensors(inv,1),tensors(draft,1)),COAX_PwStatus_number);
        assert(out[0]==-7 && scratch[0]==-9);
    }
    const uint32_t words[]={0,0x80000000,1,0x80000001,0x7f7fffff,0xff7fffff,0x3f800001,0xbf800001};
    uint8_t raw[sizeof words];for(size_t i=0;i<sizeof words/4;i++) for(int k=0;k<4;k++) raw[i*4+k]=(uint8_t)(words[i]>>(8*k));
    coax_v_u8 f32=make_file("{\"x\":{\"dtype\":\"F32\",\"shape\":[8],\"data_offsets\":[0,32]}}",raw,sizeof raw);
    r=psionic_weights_decode(f32,str("x"),0,8,floats(out,8),floats(scratch,8),tensors(inv,1),tensors(draft,1));assert(r.status==0);
    for(int i=0;i<8;i++) assert(bits(out[i])==words[i]);
    uint32_t bad[]={0x7f800000,0xff800000,0x7fc00001,0x7f800001};
    for(size_t b=0;b<4;b++) for(int pos=0;pos<8;pos+=7) {
        size_t start=(size_t)pw_header(f32).data+(size_t)pos*4;
        for(int k=0;k<4;k++) f32.data[start+k]=(uint8_t)(bad[b]>>(8*k));
        for(int i=0;i<8;i++) out[i]=-7,scratch[i]=-9;
        failed(psionic_weights_decode(f32,str("x"),0,8,floats(out,8),floats(scratch,8),tensors(inv,1),tensors(draft,1)),COAX_PwStatus_number);
        for(int i=0;i<8;i++) assert(out[i]==-7 && scratch[i]==-9);
        for(int k=0;k<4;k++) f32.data[start+k]=(uint8_t)(words[pos]>>(8*k));
    }
    free(file.data);free(f32.data);free(payload);free(out);free(scratch);
}
static void identities(void) {
    size_t n=1048576+17;uint8_t *p=malloc(n);assert(p);for(size_t i=0;i<n;i++) p[i]=(uint8_t)(i%251);
    assert(pw_digest(bytes(p,n),str(CH04_CHUNK_SHA)));p[n-1]^=1;assert(!pw_digest(bytes(p,n),str(CH04_CHUNK_SHA)));free(p);
    coax_t_PsionicShape s={0};assert(pw_shape_size(s,255).status==COAX_PwStatus_unsupported);
}
int main(void) {
    originals();representations();identities();
    puts("weights: original single/sharded files, identity/index/inventory, truncations, aliases, 65,536 BF16 encodings and F32 refusal atomicity pass");
}
