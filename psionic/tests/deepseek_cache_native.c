#include "deepseek-cache-memory.h"
#include "cache-goldens.h"
#include <assert.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#define V16(p,n) ((coax_v_u16){(uint16_t *)(p),(n)})
#define V8(p,n) ((coax_v_u8){(uint8_t *)(p),(n)})
static void exhaustive(const char *path) {
    FILE *file=fopen(path,"rb");assert(file);
    for(uint8_t kind=0;kind<3;kind++) {
        int64_t group=kind==1?16:32;coax_t_DkPlan p=dk_plan(kind,1,group);
        for(uint32_t bits=0;bits<65536;bits++) {
            uint8_t expected[5];assert(fread(expected,1,5,file)==5);
            uint16_t input[32],output[33],work[32];uint8_t data[33],scales[2],stage[33];
            for(int i=0;i<32;i++) input[i]=(uint16_t)bits;
            for(int i=0;i<33;i++){output[i]=0x1234;data[i]=77;}scales[0]=77;scales[1]=88;
            coax_t_DkPacked packed={V8(data,p.data),V8(scales,p.scales)};
            coax_t_DkResult r=psionic_dk_pack(kind,1,group,V16(input,group),packed,V8(stage,p.bytes));
            uint8_t status=expected[0]==1?COAX_DkStatus_number:expected[0]==2?COAX_DkStatus_arithmetic:COAX_DkStatus_ok;
            if(r.status!=status) {fprintf(stderr,"kind %u input %04x status %u expected %u\n",kind,bits,r.status,status);abort();}
            if(status) {
                for(int64_t i=0;i<p.data;i++)assert(data[i]==77);assert(scales[0]==77);
                for(int64_t i=0;i<group;i++)input[i]=0;input[bits%(uint32_t)group]=(uint16_t)bits;
                assert(psionic_dk_pack(kind,1,group,V16(input,group),packed,V8(stage,p.bytes)).status==status);
                for(int64_t i=0;i<p.data;i++)assert(data[i]==77);assert(scales[0]==77);continue;
            }
            assert(r.written==group && scales[0]==expected[1] && scales[1]==88);
            uint8_t code=expected[2],byte=kind==0?code:code|(code<<4);
            for(int64_t i=0;i<p.data;i++)assert(data[i]==byte);
            r=psionic_dk_unpack(kind,1,group,packed,V16(output,group+1),V16(work,group));assert(!r.status && r.written==group);
            uint16_t out=(uint16_t)(expected[3]|expected[4]<<8);
            for(int64_t i=0;i<group;i++)assert(output[i]==out);assert(output[group]==0x1234);
            // The same maximum in one lane must retain the same scale/grid.
            for(int64_t i=0;i<group;i++)input[i]=0;
            int64_t lane=bits%(uint32_t)group;input[lane]=(uint16_t)bits;
            r=psionic_dk_pack(kind,1,group,V16(input,group),packed,V8(stage,p.bytes));assert(!r.status && scales[0]==expected[1]);
            r=psionic_dk_unpack(kind,1,group,packed,V16(output,group+1),V16(work,group));assert(!r.status);
            for(int64_t i=0;i<group;i++)assert(output[i]==(i==lane?out:0));
        }
    }assert(fgetc(file)==EOF);fclose(file);
    puts("Cache: every BF16 pattern in uniform and sparse groups matches independent source equations, all three profiles");
}
static void mixed(void) {
    const uint16_t *inputs[]={dkg_0_input,dkg_1_input,dkg_2_input},*outputs[]={dkg_0_output,dkg_1_output,dkg_2_output};
    const uint8_t *bytes[]={dkg_0_data,dkg_1_data,dkg_2_data},*scale[]={dkg_0_scales,dkg_1_scales,dkg_2_scales};
    for(uint8_t kind=0;kind<3;kind++) {
        int64_t width=kind==2?128:512;coax_t_DkPlan p=dk_plan(kind,3,width);uint8_t data[1537],scales[97],stage[1632];uint16_t out[1537],work[1536];
        memset(stage,0xa5,sizeof stage);data[p.data]=66;scales[p.scales]=77;out[p.values]=88;
        coax_t_DkPacked packed={V8(data,p.data),V8(scales,p.scales)};
        assert(!psionic_dk_pack(kind,3,width,V16(inputs[kind],p.values),packed,V8(stage,p.bytes)).status);
        assert(!memcmp(data,bytes[kind],(size_t)p.data)&&!memcmp(scales,scale[kind],(size_t)p.scales));
        assert(!psionic_dk_unpack(kind,3,width,packed,V16(out,1537),V16(work,1536)).status);
        assert(!memcmp(out,outputs[kind],(size_t)p.values*2)&&data[p.data]==66&&scales[p.scales]==77&&out[p.values]==88);
        // Rows packed alone and in chunks carry exactly the same code/scale bytes.
        coax_t_DkPlan one=dk_plan(kind,1,width);
        for(int64_t row=0;row<3;row++) {
            uint8_t row_data[512],row_scales[32];coax_t_DkPacked part={V8(row_data,one.data),V8(row_scales,one.scales)};
            assert(!psionic_dk_pack(kind,1,width,V16(inputs[kind]+row*width,width),part,V8(stage,one.bytes)).status);
            assert(!memcmp(row_data,data+row*one.data,(size_t)one.data)&&!memcmp(row_scales,scales+row*one.scales,(size_t)one.scales));
        }
    }puts("Cache: mixed original 512/128-wide rows, low-first bytes, dirty scratch, tails and chunk-independent packing pass");
}
static void anchored(const char *path) {
    FILE *file=fopen(path,"rb");assert(file);
    for(uint8_t kind=0;kind<3;kind++) {
        int64_t group=kind==1?16:32;coax_t_DkPlan p=dk_plan(kind,1,group);
        for(uint32_t bits=0;bits<65536;bits++) {
            uint8_t expected[8];assert(fread(expected,1,8,file)==8);
            uint16_t input[32]={0},out[32],work[32];uint8_t data[32],scales[2],stage[33];
            input[0]=kind==0?0x43e0:0x40c0;input[1]=(uint16_t)bits;
            coax_t_DkPacked packed={V8(data,p.data),V8(scales,p.scales)};
            uint8_t status=expected[0]==1?COAX_DkStatus_number:expected[0]==2?COAX_DkStatus_arithmetic:COAX_DkStatus_ok;
            coax_t_DkResult r=psionic_dk_pack(kind,1,group,V16(input,group),packed,V8(stage,p.bytes));
            assert(r.status==status);if(status)continue;
            assert(scales[0]==expected[1]);
            assert((kind==0?data[0]:(data[0]&15))==expected[2]);assert((kind==0?data[1]:(data[0]>>4))==expected[3]);
            assert(!psionic_dk_unpack(kind,1,group,packed,V16(out,group),V16(work,group)).status);
            assert(out[0]==(uint16_t)(expected[4]|expected[5]<<8)&&out[1]==(uint16_t)(expected[6]|expected[7]<<8));
            for(int64_t i=2;i<group;i++)assert(out[i]==0);
        }
    }assert(fgetc(file)==EOF);fclose(file);
    puts("Cache: every BF16 pattern with a fixed maximum anchor matches independent codes, scales and reconstructed bits");
}
static void lanes(const char *path) {
    FILE *file=fopen(path,"rb");assert(file);
    for(uint8_t kind=0;kind<3;kind++)for(int scale=0;scale<256;scale++)for(int code=0;code<(kind==0?256:16);code++) {
        uint8_t expected[3];assert(fread(expected,1,3,file)==3);
        uint8_t status=expected[0]==1?COAX_DkStatus_number:expected[0]==2?COAX_DkStatus_arithmetic:COAX_DkStatus_ok;
        coax_t_DkScalar r=dk_lane(kind,(uint8_t)code,(uint8_t)scale);
        assert(r.status==status);if(!status)assert(r.bits==(uint16_t)(expected[1]|expected[2]<<8));
    }assert(fgetc(file)==EOF);fclose(file);
    // All finite BF16 neighboring midpoints and their immediately adjacent F32s.
    for(uint32_t sign=0;sign<2;sign++)for(uint32_t lo=0;lo<0x7f7f;lo++)for(int delta=-1;delta<=1;delta++) {
        uint32_t b=(sign<<31)+(lo<<16)+0x8000+(uint32_t)delta;
        uint16_t expected=(uint16_t)((sign<<15)+(delta<0?lo:delta>0?lo+1:lo+(lo&1)));
        coax_t_DkScalar r=dk_bf16_bits(b);assert(!r.status&&r.bits==expected);
    }
    puts("Cache: every encoded scalar/scale pair and every finite neighboring BF16 rounding boundary is exact");
}
static void budgets(void) {
    for(size_t i=0;i<sizeof(dkg_budgets)/sizeof(dkg_budgets[0]);i++){
        DkgBudget e=dkg_budgets[i];coax_t_DkBudget r=dk_flash_cache_budget(e.context,e.chunk,e.drafts,e.cache,e.step);
        assert(!r.status&&r.cache==e.cache&&r.step==e.step&&r.total==e.cache+e.step);
        assert(dk_flash_cache_budget(e.context,e.chunk,e.drafts,e.cache-1,e.step).status==COAX_DkStatus_budget);
        assert(dk_flash_cache_budget(e.context,e.chunk,e.drafts,e.cache,e.step-1).status==COAX_DkStatus_budget);
    }
    puts("Cache: pinned source ownership and chunk work budgets match an independent per-layer inventory");
}
static void refusals(void) {
    uint16_t input[64]={0},out[65],work[64];uint8_t data[68],scales[5],stage[68];
    for(int i=0;i<65;i++){out[i]=0x1234;data[i]=77;}for(int i=0;i<5;i++)scales[i]=88;
    coax_t_DkPacked p={V8(data,64),V8(scales,2)};
    assert(psionic_dk_pack(0,2,32,V16(input,64),(coax_t_DkPacked){V8(input,64),V8(scales,2)},V8(stage,66)).status==COAX_DkStatus_alias);
    assert(psionic_dk_pack(0,2,32,V16(input,64),(coax_t_DkPacked){V8(data,64),V8(data+63,2)},V8(stage,66)).status==COAX_DkStatus_alias);
    assert(psionic_dk_pack(0,2,32,V16(input,64),p,V8(data,66)).status==COAX_DkStatus_alias);
    assert(psionic_dk_pack(0,2,32,V16(input,64),p,V8(input,66)).status==COAX_DkStatus_alias);
    input[63]=0x7fc0;assert(psionic_dk_pack(0,2,32,V16(input,64),p,V8(stage,66)).status==COAX_DkStatus_number);
    for(int i=0;i<65;i++)assert(data[i]==77);for(int i=0;i<5;i++)assert(scales[i]==88);
    input[63]=0x7f7f;assert(psionic_dk_pack(0,2,32,V16(input,64),p,V8(stage,66)).status==COAX_DkStatus_arithmetic);
    for(int i=0;i<65;i++)assert(data[i]==77);
    input[63]=0;assert(!psionic_dk_pack(0,2,32,V16(input,64),p,V8(stage,66)).status);
    assert(psionic_dk_unpack(0,2,32,p,V16(out,63),V16(work,64)).status==COAX_DkStatus_capacity);
    assert(psionic_dk_unpack(0,2,32,p,V16(input,64),V16(input,64)).status==COAX_DkStatus_alias);
    // Real, aligned mixed-width overlap with truthful allocation extents.
    p.data=V8(input,64);p.scales=V8(scales,2);
    assert(psionic_dk_unpack(0,2,32,p,V16(input,64),V16(work,64)).status==COAX_DkStatus_alias);
    p.data=V8(data,64);data[63]=255;
    assert(psionic_dk_unpack(0,2,32,p,V16(out,65),V16(work,64)).status==COAX_DkStatus_number);
    for(int i=0;i<65;i++)assert(out[i]==0x1234);
    data[63]=0;scales[1]=255;
    assert(psionic_dk_unpack(0,2,32,p,V16(out,65),V16(work,64)).status==COAX_DkStatus_number);
    assert(dk_bf16(NAN).status==COAX_DkStatus_number&&dk_bf16(INFINITY).status==COAX_DkStatus_number);
    puts("Cache: native aliases, non-finite inputs, late bad codes/scales and overflow preserve public outputs");
}
int main(int argc,char **argv) {assert(argc==4);exhaustive(argv[1]);anchored(argv[2]);lanes(argv[3]);mixed();budgets();refusals();return 0;}
