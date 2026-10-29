#include "mxfp4-memory.h"
#include "mxfp4-oracle.h"
#include <assert.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static uint32_t bits(float value) { uint32_t word; memcpy(&word, &value, 4); return word; }
static coax_v_u8 bytes(uint8_t *p, int64_t n) { return (coax_v_u8){p,n}; }
static coax_v_f32 floats(float *p, int64_t n) { return (coax_v_f32){p,n}; }

int main(void) {
    uint8_t raw[33], scales[2] = {127,128}; float out[65], before[65];
    uint8_t *packed = raw + 1; /* Original byte views need no float alignment. */
    assert(mx_plan(INT64_MAX,32).status==COAX_MxStatus_limit);
    assert(mx_plan(1,INT64_MAX-31).status==COAX_MxStatus_limit);
    assert(mx_plan(0,32).status==COAX_MxStatus_shape);
    for (unsigned scale=0; scale<253; scale++) {
        scales[0]=(uint8_t)scale;
        for (unsigned byte=0; byte<256; byte++) {
            memset(packed,(int)byte,16);for (int i=0;i<65;i++)out[i]=-7;
            coax_t_MxResult r=psionic_mxfp4_decode(1,32,bytes(packed,16),bytes(scales,1),floats(out,65));
            assert(r.status==COAX_MxStatus_ok && r.written==32 && out[32]==-7);
            for (int i=0;i<16;i++) {
                assert(bits(out[2*i])==mxfp4_expected[scale*16+(byte&15)]);
                assert(bits(out[2*i+1])==mxfp4_expected[scale*16+(byte>>4)]);
            }
        }
    }
    memset(packed,0x91,32);scales[0]=0;scales[1]=252;
    coax_t_MxResult r=psionic_mxfp4_decode(2,32,bytes(packed,32),bytes(scales,2),floats(out,65));
    assert(r.status==COAX_MxStatus_ok && bits(out[0])==0x00200000 &&
        bits(out[1])==0x80200000 && bits(out[32])==mxfp4_expected[252*16+1]);
    for(int i=0;i<65;i++)out[i]=-7;memcpy(before,out,sizeof out);
    for(unsigned scale=253;scale<256;scale++) {
        scales[1]=(uint8_t)scale;
        r=psionic_mxfp4_decode(1,64,bytes(packed,32),bytes(scales,2),floats(out,65));
        assert(r.status==COAX_MxStatus_scale && r.written==0 && !memcmp(out,before,sizeof out));
    }
    scales[1]=127;
    r=psionic_mxfp4_decode(1,64,bytes(packed,31),bytes(scales,2),floats(out,65));
    assert(r.status==COAX_MxStatus_storage && !memcmp(out,before,sizeof out));
    r=psionic_mxfp4_decode(1,64,bytes(packed,32),bytes(scales,1),floats(out,65));
    assert(r.status==COAX_MxStatus_storage);
    r=psionic_mxfp4_decode(1,64,bytes(packed,32),bytes(scales,2),floats(out,63));
    assert(r.status==COAX_MxStatus_capacity && !memcmp(out,before,sizeof out));
    union { float f[65]; uint8_t u[260]; } shared;
    memset(&shared,0x7e,sizeof shared);uint8_t saved[260];memcpy(saved,&shared,sizeof saved);
    r=psionic_mxfp4_decode(1,32,bytes(shared.u+3,16),bytes(scales,1),floats(shared.f,32));
    assert(r.status==COAX_MxStatus_alias && !memcmp(saved,&shared,sizeof saved));
    r=psionic_mxfp4_decode(1,32,bytes(packed,16),bytes(shared.u+127,1),floats(shared.f,32));
    assert(r.status==COAX_MxStatus_alias && !memcmp(saved,&shared,sizeof saved));
    /* An input overlapping only the caller's unused output tail is refused. */
    r=psionic_mxfp4_decode(1,32,bytes(shared.u+128,16),bytes(scales,1),floats(shared.f,65));
    assert(r.status==COAX_MxStatus_alias);
    const int64_t blocks=32768, count=1048576;
    uint8_t *big=malloc((size_t)blocks*16),*scale_bytes=malloc((size_t)blocks);
    float *destination=malloc(((size_t)count+1)*sizeof(float));assert(big&&scale_bytes&&destination);
    memset(big,0x76,(size_t)blocks*16);memset(scale_bytes,127,(size_t)blocks);destination[count]=-7;
    r=psionic_mxfp4_decode(32768,32,bytes(big,blocks*16),bytes(scale_bytes,blocks),floats(destination,count+1));
    assert(r.status==COAX_MxStatus_ok && r.written==count && destination[count]==-7);
    for(int64_t i=0;i<count;i++)assert(destination[i]==(i%2?6.0f:4.0f));
    free(big);free(scale_bytes);free(destination);
    puts("MXFP4 native: 64,768 packed-byte/scale cases, exact bits, aliases and maximum window pass");
}
