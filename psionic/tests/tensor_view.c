/* Independent integer address enumeration and actual native span checks.
   No tensor implementation or framework is called to construct the oracles. */
#include "tensor-view-memory.h"
#include <assert.h>
#include <limits.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#define OK COAX_TvStatus_ok
#define STATUS(x,s) assert((x).status==COAX_TvStatus_##s)
static coax_v_f32 f(float *p,int64_t n){return(coax_v_f32){p,n};}
static coax_v_t_TvStorage o(coax_t_TvStorage *p,int64_t n){return(coax_v_t_TvStorage){p,n};}
static coax_t_TvShape shape(int rank,int64_t a,int64_t b,int64_t c,int64_t d){return(coax_t_TvShape){rank,{{a,b,c,d}}};}
static float bits(uint32_t x){float v;memcpy(&v,&x,4);return v;}
static void enumerated(void){
    coax_t_TvStorage owner=tv_owner(123,4,13).storage;int cases=0;
    for(int d0=0;d0<=4;d0++)for(int d1=0;d1<=4;d1++)for(int s0=-4;s0<=4;s0++)for(int s1=-4;s1<=4;s1++)for(int offset=0;offset<=13;offset++){
        coax_t_TvView view={.identity=123,.epoch=4,.shape=shape(2,d0,d1,0,0),.strides={{s0,s1,0,0}},.offset=offset};
        int addresses[16],n=0,valid=1,unique=1,low=INT_MAX,high=INT_MIN;
        for(int i=0;i<d0;i++)for(int j=0;j<d1;j++){int a=offset+i*s0+j*s1;addresses[n++]=a;if(a<0 || a>=13)valid=0;if(a<low)low=a;if(a>high)high=a;}
        for(int i=0;i<n;i++)for(int j=0;j<i;j++)if(addresses[i]==addresses[j])unique=0;
        coax_t_TvLayout got=tv_layout(owner,view);assert((got.status==OK)==valid);
        if(valid){assert(got.elements==n);if(n){assert(got.low==low && got.high==high);if(got.injective)assert(unique);}
            for(int i=0;i<d0;i++)for(int j=0;j<d1;j++){coax_t_TvIndex index=tv_index(owner,view,(coax_a4_s64){{i,j,0,0}});assert(index.status==OK && index.offset==offset+i*s0+j*s1);}}
        else assert(got.elements==0 && got.low==0 && got.high==0 && !got.contiguous && !got.injective);
        view.writable=1;coax_t_TvLayout write=tv_layout(owner,view);if(write.status==OK)assert(unique && valid);if(valid && !unique)STATUS(write,alias);
        cases++;
    }
    printf("independent address/uniqueness enumeration: %d layouts\n",cases);
    for(int rank=0;rank<=4;rank++){coax_t_TvShape s={.rank=rank};int n=1;for(int axis=0;axis<rank;axis++){s.dims.e[axis]=axis+2;n*=axis+2;}
        coax_t_TvStorage own=tv_owner(1,1,n).storage;coax_t_TvViewResult v=tv_contiguous(own,s,1);STATUS(v,ok);assert(tv_layout(own,v.view).elements==n);
        for(int a=0;a<rank;a++)for(int b=0;b<rank;b++){coax_t_TvViewResult t=tv_transpose(own,v.view,a,b);STATUS(t,ok);coax_t_TvViewResult back=tv_transpose(own,t.view,b,a);STATUS(back,ok);for(int axis=0;axis<4;axis++){assert(back.view.shape.dims.e[axis]==s.dims.e[axis] && back.view.strides.e[axis]==v.view.strides.e[axis]);}}
    }
    coax_t_TvStorage four=tv_owner(8,1,120).storage;
    coax_t_TvView reverse={.identity=8,.epoch=1,.shape=shape(4,2,3,4,5),.strides={{-60,-20,-5,-1}},.offset=119,.writable=1};
    coax_t_TvLayout layout=tv_layout(four,reverse);assert(layout.status==OK && layout.elements==120 && layout.low==0 && layout.high==119 && layout.injective && !layout.contiguous);
    for(int a=0;a<2;a++)for(int b=0;b<3;b++)for(int c=0;c<4;c++)for(int d=0;d<5;d++){
        coax_t_TvIndex index=tv_index(four,reverse,(coax_a4_s64){{a,b,c,d}});assert(index.status==OK && index.offset==119-60*a-20*b-5*c-d);
    }
    coax_t_TvView transposed=tv_transpose(four,reverse,0,3).view;
    for(int a=0;a<5;a++)for(int b=0;b<3;b++)for(int c=0;c<4;c++)for(int d=0;d<2;d++){
        coax_t_TvIndex index=tv_index(four,transposed,(coax_a4_s64){{a,b,c,d}});assert(index.status==OK && index.offset==119-a-20*b-5*c-60*d);
    }
    puts("four-dimensional reverse/transpose: all 240 coordinate addresses match independent equations");
}
static void copies(void){
    coax_t_TvStorage owner=tv_owner(7,11,12).storage;coax_t_TvView v=tv_contiguous(owner,shape(2,3,4,0,0),1).view;
    coax_t_TvView r=tv_slice(owner,v,1,3,2,-2).view;r=tv_transpose(owner,r,0,1).view;
    int addresses[]={3,7,11,1,5,9};float data[12],source[6]={10,20,30,40,50,60},out[8],scratch[8];for(int i=0;i<12;i++)data[i]=(float)i;data[3]=bits(0x80000000);for(int i=0;i<8;i++){out[i]=-7;scratch[i]=-9;}
    coax_t_TvSaved saved=tv_save(owner,r);STATUS(tv_gather(o(&owner,1),r,f(data,12),f(out,8),f(scratch,8)),ok);
    for(int i=0;i<6;i++)assert(!memcmp(&out[i],&data[addresses[i]],4));assert(out[6]==-7 && scratch[6]==-9 && owner.version==0);
    STATUS(tv_scatter(o(&owner,1),r,f(source,6),f(data,12),f(scratch,8)),ok);assert(owner.version==1 && tv_saved_status(owner,saved)==COAX_TvStatus_modified);
    for(int i=0;i<12;i++){int selected=-1;for(int j=0;j<6;j++)if(addresses[j]==i)selected=j;assert(data[i]==(selected<0?(float)i:source[selected]));}
    float before[12],out_before[8],scratch_before[8];memcpy(before,data,sizeof before);memcpy(out_before,out,sizeof out);memcpy(scratch_before,scratch,sizeof scratch);coax_t_TvStorage old=owner;
    uint32_t bad[]={0x7f800000,0xff800000,0x7fc00001,0x7f800001,0xff800001};
    for(size_t i=0;i<sizeof bad/sizeof *bad;i++){
        source[5]=bits(bad[i]);STATUS(tv_scatter(o(&owner,1),r,f(source,6),f(data,12),f(scratch,8)),number);
        assert(!memcmp(data,before,sizeof data) && !memcmp(scratch,scratch_before,sizeof scratch) && !memcmp(&owner,&old,sizeof owner));
        source[5]=60;data[9]=bits(bad[i]);STATUS(tv_gather(o(&owner,1),r,f(data,12),f(out,8),f(scratch,8)),number);
        assert(!memcmp(out,out_before,sizeof out) && !memcmp(scratch,scratch_before,sizeof scratch));data[9]=before[9];
    }
    STATUS(tv_gather(o(&owner,0),r,f(data,12),f(out,8),f(scratch,8)),capacity);
    STATUS(tv_gather(o(&owner,1),r,f(data,11),f(out,8),f(scratch,8)),capacity);
    STATUS(tv_gather(o(&owner,1),r,f(data,12),f(out,5),f(scratch,8)),capacity);
    STATUS(tv_scatter(o(&owner,1),r,f(source,5),f(data,12),f(scratch,8)),capacity);
    STATUS(tv_scatter(o(&owner,1),r,f(source,6),f(data,12),f(scratch,5)),capacity);
    STATUS(tv_scatter(o(&owner,1),r,f(source,6),f(data,11),f(scratch,8)),capacity);
    STATUS(tv_scatter(o(&owner,0),r,f(source,6),f(data,12),f(scratch,8)),capacity);
    r.writable=0;STATUS(tv_scatter(o(&owner,1),r,f(source,6),f(data,12),f(scratch,8)),readonly);r.writable=1;
    /* Entire declared writable tails are included in overlap observations. */
    STATUS(tv_gather(o(&owner,1),r,f(data,12),f(data,8),f(scratch,8)),alias);
    STATUS(tv_gather(o(&owner,1),r,f(data,12),f(out,8),f(out+1,7)),alias);
    STATUS(tv_gather(o(&owner,1),r,f(data,12),f(out,8),f(data+4,8)),alias);
    STATUS(tv_gather(o(&owner,1),r,f(data,12),f((float*)&owner,8),f(scratch,8)),alias);
    STATUS(tv_scatter(o(&owner,1),r,f(data,6),f(data,12),f(scratch,8)),alias);
    STATUS(tv_scatter(o(&owner,1),r,f(source,6),f(data,12),f(data+4,8)),alias);
    STATUS(tv_scatter(o(&owner,1),r,f(source,6),f(data,12),f(source,6)),alias);
    union {coax_t_TvStorage owner;float data[12];} overlap_box;overlap_box.owner=owner;
    STATUS(tv_scatter(o(&overlap_box.owner,1),r,f(source,6),f(overlap_box.data,12),f(scratch,8)),alias);
    STATUS(tv_scatter(o(&owner,1),r,f((float*)&owner,6),f(data,12),f(scratch,8)),alias);
    STATUS(tv_scatter(o(&owner,1),r,f(source,6),f(data,12),f((float*)&owner,8)),alias);
    assert(!memcmp(data,before,sizeof data) && !memcmp(out,out_before,sizeof out) && !memcmp(scratch,scratch_before,sizeof scratch) && !memcmp(&owner,&old,sizeof owner));
    owner.version=INT64_MAX;STATUS(tv_scatter(o(&owner,1),r,f(source,6),f(data,12),f(scratch,8)),limit);owner.version=old.version;
    STATUS(tv_retire(o(&owner,1)),ok);assert(tv_saved_status(owner,saved)==COAX_TvStatus_state);STATUS(tv_gather(o(&owner,1),r,f(data,12),f(out,8),f(scratch,8)),state);
    owner=tv_owner(7,12,12).storage;STATUS(tv_layout(owner,r),state);
    puts("native strided copies: bit preservation, untouched gaps/tails, all nonfinite classes, short spans, actual aliases, version overflow and stale epochs pass");
}
static void empty_and_max(void){
    coax_t_TvStorage owner=tv_owner(1,1,0).storage;coax_t_TvView v=tv_contiguous(owner,shape(2,0,4,0,0),1).view;
    STATUS(tv_gather(o(&owner,1),v,f(NULL,0),f(NULL,0),f(NULL,0)),ok);STATUS(tv_scatter(o(&owner,1),v,f(NULL,0),f(NULL,0),f(NULL,0)),ok);assert(owner.version==0);
    const int n=1048576;float *data=malloc((size_t)n*4),*out=malloc((size_t)n*4),*scratch=malloc((size_t)n*4);assert(data && out && scratch);for(int i=0;i<n;i++)data[i]=(float)i;
    owner=tv_owner(2,1,n).storage;v=tv_contiguous(owner,shape(2,1024,1024,0,0),1).view;v=tv_transpose(owner,v,0,1).view;
    STATUS(tv_gather(o(&owner,1),v,f(data,n),f(out,n),f(scratch,n)),ok);for(int i=0;i<1024;i++)for(int j=0;j<1024;j++)assert(out[i*1024+j]==data[j*1024+i]);
    free(data);free(out);free(scratch);puts("empty null spans and maximum 1,048,576-element transposed materialization pass");
}
int main(void){enumerated();copies();empty_and_max();return 0;}
