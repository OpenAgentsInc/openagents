/* Independent native ABI/format/lifetime checks, authored before the C binding. */
#include "interchange.h"
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
static int deleted;
static void done(DLManagedTensorVersioned *t){(void)t;deleted++;}
static void legacy_done(DLManagedTensor *t){(void)t;deleted++;}
static PsTensorV1 tensor(const void *p,int64_t n){return (PsTensorV1){.abi_version=1,.rank=1,.dims={n},.data=p,.byte_count=(uint64_t)n*4};}
static void weights(void){
    float f[2]={-0.0f,1.5f};uint16_t b[2]={0x3f80,0xc000};
    PsParameterV1 p[2]={{.name="z.weight",.name_bytes=8,.tensor=tensor(f,2)}, {.name="a.bias",.name_bytes=6,.tensor={.abi_version=1,.dtype=PS_BF16,.rank=1,.dims={2},.data=b,.byte_count=4}}};
    unsigned char out[2048],before[2048];memset(out,0xa5,sizeof out);memcpy(before,out,sizeof out);
    PsResultV1 n=psionic_weights_size_v1(p,2);assert(!n.status && n.written<sizeof out);
    assert(psionic_weights_write_v1(p,2,out,n.written-1).status==PS_CAPACITY && !memcmp(out,before,sizeof out));
    PsResultV1 r=psionic_weights_write_v1(p,2,out,sizeof out);assert(!r.status && r.written==n.written && out[r.written]==0xa5);
    PsParameterV1 reversed[2]={p[1],p[0]};unsigned char canonical[2048];PsResultV1 same=psionic_weights_write_v1(reversed,2,canonical,sizeof canonical);assert(!same.status && same.written==n.written && !memcmp(out,canonical,(size_t)n.written));
    uint64_t header=0;for(int i=0;i<8;i++)header|=(uint64_t)out[i]<<(8*i);assert(header%8==0 && out[8]=='{' && !memcmp(out+8+header,b,4) && !memcmp(out+12+header,f,8));
    PsParameterInfoV1 info[2];memset(info,0x7c,sizeof info);r=psionic_weights_inspect_v1(out,r.written,info,2);assert(!r.status && r.written==2 && !strcmp(info[0].name,"a.bias") && info[0].dtype==PS_BF16 && info[1].data_offset==12+header);
    for(uint64_t cut=0;cut<n.written;cut++){PsParameterInfoV1 bad[2];memset(bad,0x7c,sizeof bad);PsParameterInfoV1 before_bad[2];memcpy(before_bad,bad,sizeof bad);assert(psionic_weights_inspect_v1(out,cut,bad,2).status && !memcmp(bad,before_bad,sizeof bad));}
    PsParameterInfoV1 saved[2];memcpy(saved,info,sizeof saved);assert(psionic_weights_inspect_v1(out,n.written,info,1).status==PS_CAPACITY && !memcmp(saved,info,sizeof info));
    p[1].name="z.weight";p[1].name_bytes=8;assert(psionic_weights_size_v1(p,2).status==PS_NAME);p[1].name="bad\"";p[1].name_bytes=4;assert(psionic_weights_size_v1(p,2).status==PS_NAME);
    uint32_t bads[]={0x7f800000,0x7fc00000,0x7f800001,0xff800000};p[0].tensor=tensor(bads,1);for(int i=0;i<4;i++){p[0].tensor.data=bads+i;memcpy(out,before,sizeof out);assert(psionic_weights_write_v1(p,1,out,sizeof out).status==PS_NUMBER && !memcmp(out,before,sizeof out));}
    uint32_t subnormal=1;p[0].tensor=tensor(&subnormal,1);r=psionic_weights_write_v1(p,1,out,sizeof out);assert(!r.status && !memcmp(out+r.written-4,&subnormal,4));
    p[0].tensor=tensor(out,1);assert(psionic_weights_write_v1(p,1,out,sizeof out).status==PS_ALIAS);
    p[0].tensor=tensor(f,2);p[0].tensor.abi_version=2;assert(psionic_weights_size_v1(p,1).status==PS_VERSION);
    p[0].tensor.abi_version=1;p[0].tensor.rank=5;assert(psionic_weights_size_v1(p,1).status==PS_SHAPE);
    assert(psionic_weights_size_v1(NULL,0).status && psionic_weights_inspect_v1("bad",3,info,2).status);
}
static void carriers(void){
    float data[6]={1,2,3,4,5,6};int64_t shape[2]={2,3},strides[2]={3,1};
    DLManagedTensorVersioned t={.version={1,0},.deleter=done,.dl_tensor={.data=data,.device={kDLCPU,0},.ndim=2,.dtype={kDLFloat,32,1},.shape=shape,.strides=strides}};
    PsTensorHandleV1 *h=NULL,*other=NULL;assert(!psionic_dlpack_import_v1(&t,PS_DLPACK_VERSIONED,&h));
    PsTensorV1 view;assert(!psionic_tensor_view_v1(h,&view) && view.data==data && view.dims[0]==2 && view.flags==PS_READONLY);
    shape[0]=6;assert(!psionic_tensor_view_v1(h,&view) && view.dims[0]==2);shape[0]=2;
    assert(!psionic_tensor_retain_v1(h,&other));DLManagedTensorVersioned *exported=NULL;assert(!psionic_tensor_dlpack_v1(h,&exported));assert(exported->version.major==1 && exported->flags==DLPACK_FLAG_BITMASK_READ_ONLY && exported->dl_tensor.strides[0]==3);
    assert(!psionic_tensor_release_v1(&h) && !h && !psionic_tensor_release_v1(&other) && !deleted);assert(((float*)exported->dl_tensor.data)[5]==6);psionic_dlpack_dispose_v1(exported);exported=NULL;assert(deleted==1 && !psionic_tensor_live_v1());
    int start=deleted;t.version.major=2;t.dl_tensor.shape=(int64_t*)1;assert(psionic_dlpack_import_v1(&t,1,&h)==PS_VERSION && deleted==start+1);t.version.major=1;
    t.dl_tensor.device.device_type=kDLCUDA;assert(psionic_dlpack_import_v1(&t,1,&h)==PS_DTYPE && deleted==start+2);t.dl_tensor.device.device_type=kDLCPU;t.dl_tensor.shape=shape;
    strides[0]=1;assert(psionic_dlpack_import_v1(&t,1,&h)==PS_SHAPE);strides[0]=3;
    t.dl_tensor.byte_offset=1;assert(psionic_dlpack_import_v1(&t,1,&h)==PS_ARGUMENT);t.dl_tensor.byte_offset=0;
    t.flags=4;assert(psionic_dlpack_import_v1(&t,1,&h)==PS_ARGUMENT);t.flags=0;
    t.version.minor=2;t.dl_tensor.strides=NULL;assert(psionic_dlpack_import_v1(&t,1,&h)==PS_SHAPE);t.version.minor=0;t.dl_tensor.strides=strides;
    t.dl_tensor.data=(void*)(uintptr_t)(UINTPTR_MAX-3);t.dl_tensor.byte_offset=4;assert(psionic_dlpack_import_v1(&t,1,&h)==PS_ARGUMENT);t.dl_tensor.data=data;t.dl_tensor.byte_offset=0;
    int count=deleted;assert(count==start+7);assert(psionic_dlpack_import_v1(&t,9,&h)==PS_ARGUMENT && deleted==count);
    DLManagedTensor legacy={.dl_tensor=t.dl_tensor,.deleter=legacy_done};assert(!psionic_dlpack_import_v1(&legacy,0,&h));assert(psionic_dlpack_import_v1(&t,1,&h)==PS_ARGUMENT && deleted==count);assert(psionic_dlpack_import_v1(&t,1,NULL)==PS_ARGUMENT && deleted==count);assert(!psionic_tensor_release_v1(&h) && deleted==count+1);
    PsTensorV1 d=tensor(data,6);assert(!psionic_tensor_copy_v1(&d,&h));assert(!psionic_tensor_view_v1(h,&view) && view.data!=data && !memcmp(view.data,data,24));assert(!psionic_tensor_release_v1(&h));
    d=tensor(NULL,0);assert(!psionic_tensor_copy_v1(&d,&h));assert(!psionic_tensor_view_v1(h,&view) && !view.data && !view.byte_count);assert(!psionic_tensor_release_v1(&h));
    d=tensor(data,1);d.rank=0;d.dims[0]=0;assert(!psionic_tensor_copy_v1(&d,&h));assert(!psionic_tensor_dlpack_v1(h,&exported));assert(exported->dl_tensor.ndim==0);assert(!psionic_tensor_release_v1(&h));psionic_dlpack_dispose_v1(exported);
    uint32_t nan=0x7f800001;d=tensor(&nan,1);assert(!psionic_tensor_copy_v1(&d,&h));assert(!psionic_tensor_view_v1(h,&view) && !memcmp(view.data,&nan,4));assert(!psionic_tensor_release_v1(&h));
    assert(!psionic_tensor_live_v1());
}
int main(void){assert(psionic_interchange_abi_version()==1);weights();carriers();puts("interchange: canonical weights, ABI refusals, zero-copy carriers and lease lifetimes pass");}
