/* Independent native ownership, byte-format and numerical oracles. Including
   the adapter permits address-alias checks without exposing native pointers. */
#include "tensor-device-runtime.c"
#include "tensor-device-oracle.h"
#include <math.h>
#include <stdio.h>
#include <assert.h>
#define CHECK(x) do{if(!(x)){fprintf(stderr,"FAIL line %d: %s\n",__LINE__,#x);abort();}}while(0)
#define SUCCESS(x) do{coax_t_TdStatus z_=(x);if(z_){fprintf(stderr,"status %d line %d: %s\n",z_,__LINE__,#x);abort();}}while(0)
static int groups;
static coax_t_TvShape shape(int64_t n){return(coax_t_TvShape){.rank=1,.dims={.e={n,0,0,0}}};}
static uint32_t bits(float f){uint32_t u;memcpy(&u,&f,4);return u;}
static float value(uint32_t u){float f;memcpy(&f,&u,4);return f;}
static uint32_t decode(uint16_t w,int dtype){if(dtype==2)return(uint32_t)w<<16;int exponent=(w>>10)&31,fraction=w&1023;float f=exponent?ldexpf(1.0f+fraction/1024.0f,exponent-15):ldexpf((float)fraction,-24);if(w&32768)f=-f;return bits(f);}
static void drop(PtdTensor **t){SUCCESS(ptd_release(t));CHECK(!*t);}
static void empty(PtdSession *s){coax_t_TdSessionState a=ptd_stats(s);CHECK(a.bytes==0&&a.allocations==0&&a.pending==0);}
static void randoms(PtdSession *s){
    const uint8_t *expected[]={oracle_random_0,oracle_random_1,oracle_random_2};
    for(int dtype=0;dtype<3;dtype++){
        PtdTensor *t=NULL,*a=NULL,*b=NULL;coax_t_TdRng rng={.seed=123,.stream=7,.next=17};int width=dtype?2:4;uint8_t out[1028],chunk[1028];
        SUCCESS(ptd_random(s,shape(257),dtype,rng,&t));SUCCESS(ptd_export(t,out,sizeof out));CHECK(!memcmp(out,expected[dtype],257*width));
        SUCCESS(ptd_random(s,shape(52),dtype,rng,&a));rng.next+=52;SUCCESS(ptd_random(s,shape(205),dtype,rng,&b));SUCCESS(ptd_export(a,chunk,52*width));SUCCESS(ptd_export(b,chunk+52*width,205*width));CHECK(!memcmp(out,chunk,257*width));
        drop(&t);drop(&a);drop(&b);rng.next=4294967295LL;CHECK(ptd_random(s,shape(2),dtype,rng,&t)==ERR(counter)&&!t);empty(s);
    }
    groups++;
}
static void encodings(PtdSession *s,PtdSession *cpu){
    for(int dtype=1;dtype<3;dtype++){
        int n=dtype==1?63488:65280;uint16_t *raw=malloc(n*2),*again=malloc(n*2);uint32_t *wide=malloc(n*4);CHECK(raw&&again&&wide);int k=0;
        for(int u=0;u<65536;u++){if(dtype==1?((u&0x7c00)!=0x7c00):((u&0x7f80)!=0x7f80))raw[k++]=(uint16_t)u;}
        CHECK(k==n);
        PtdTensor *a=NULL,*b=NULL,*c=NULL;SUCCESS(ptd_from_bytes(s,shape(n),dtype,(uint8_t*)raw,n*2,dtype,&a));SUCCESS(ptd_export(a,(uint8_t*)again,n*2));CHECK(!memcmp(raw,again,n*2));
        SUCCESS(ptd_to(a,s,F32,&b));SUCCESS(ptd_export(b,(uint8_t*)wide,n*4));for(int i=0;i<n;i++)CHECK(wide[i]==decode(raw[i],dtype));
        SUCCESS(ptd_to(b,cpu,dtype,&c));SUCCESS(ptd_export(c,(uint8_t*)again,n*2));CHECK(!memcmp(raw,again,n*2));
        drop(&a);drop(&b);drop(&c);free(raw);free(again);free(wide);empty(s);empty(cpu);
    }
    groups++;
}
static void rounding(PtdSession *s){
    const uint32_t *input[]={oracle_round_input_1,oracle_round_input_2};
    const uint16_t *expected[]={oracle_round_output_1,oracle_round_output_2};
    const size_t counts[]={sizeof oracle_round_input_1/4,sizeof oracle_round_input_2/4};
    for(int d=0;d<2;d++){
        PtdTensor *a=NULL;size_t n=counts[d];uint16_t *out=malloc(n*2);CHECK(out);
        SUCCESS(ptd_from_bytes(s,shape((int64_t)n),F32,(const uint8_t*)input[d],(int64_t)n*4,d+1,&a));
        SUCCESS(ptd_export(a,(uint8_t*)out,(int64_t)n*2));CHECK(!memcmp(out,expected[d],n*2));free(out);drop(&a);empty(s);
    }
    groups++;
}
static void programs(PtdSession *s,PtdSession *cpu){
    float av[21],bv[21];for(int i=0;i<21;i++){av[i]=(i-10)/8.0f;bv[i]=(i%7)/4.0f;}
    for(int ad=0;ad<3;ad++)for(int bd=0;bd<3;bd++){
        PtdTensor *a=NULL,*b=NULL,*o=NULL,*read=NULL;SUCCESS(ptd_from_f32(s,(coax_t_TvShape){.rank=2,.dims={.e={3,7,0,0}}},ad,av,21,&a));SUCCESS(ptd_from_f32(s,(coax_t_TvShape){.rank=2,.dims={.e={3,7,0,0}}},bd,bv,21,&b));
        coax_t_TdDropout r={.seed=123,.stream=7,.first=0,.keep=32768,.training=true};SUCCESS(ptd_program(s,a,b,r,&o));CHECK(ptd_info(o).state.pending);CHECK(ptd_info(a).state.readers==1);
        uint8_t sentinel[84];memset(sentinel,0x55,sizeof sentinel);CHECK(ptd_export(o,sentinel,sizeof sentinel)==ERR(busy)&&sentinel[0]==0x55);CHECK(ptd_to(o,cpu,F32,&read)==ERR(busy)&&!read);CHECK(ptd_release(&o)==ERR(busy)&&o);CHECK(ptd_release(&a)==ERR(busy)&&a);CHECK(ptd_close(&s)==ERR(busy));
        SUCCESS(ptd_wait(o));SUCCESS(ptd_wait(o));CHECK(ptd_info(a).state.readers==0&&ptd_info(b).state.readers==0);CHECK(ptd_info(o).state.dtype==(ad==bd?ad:F32));
        SUCCESS(ptd_to(o,cpu,F32,&read));float out[21];SUCCESS(ptd_export(read,(uint8_t*)out,sizeof out));for(int i=0;i<21;i++){float want=fmaxf(0,av[i]+bv[i]);if((oracle_words[i]>>16)>=32768)want=0;else want*=2;CHECK(bits(out[i])==bits(want));}
        drop(&read);drop(&a);drop(&b);drop(&o);empty(s);empty(cpu);
    }
    groups++;
}
static void views(PtdSession *s,PtdSession *cpu){
    float data[]={1,2,3,4,5,6};PtdTensor *a=NULL,*v=NULL,*copy=NULL,*o=NULL,*c=NULL;SUCCESS(ptd_from_f32(s,shape(6),F32,data,6,&a));PtdInfo info=ptd_info(a);
    coax_t_TvView reverse=info.view;reverse.offset=5;reverse.strides.e[0]=-1;SUCCESS(ptd_view(a,reverse,&v));CHECK(ptd_info(v).state.refs==2);drop(&a);CHECK(ptd_info(v).state.refs==1);
    SUCCESS(ptd_to(v,s,F32,&copy));CHECK(ptd_info(copy).state.owner.identity!=ptd_info(v).state.owner.identity);drop(&v);float got[6];SUCCESS(ptd_export(copy,(uint8_t*)got,sizeof got));for(int i=0;i<6;i++)CHECK(got[i]==6-i);
    coax_t_TdDropout r={.training=false};SUCCESS(ptd_program(s,copy,copy,r,&o));CHECK(ptd_info(copy).state.readers==2);SUCCESS(ptd_wait(o));CHECK(ptd_info(copy).state.readers==0);SUCCESS(ptd_to(o,cpu,F32,&c));SUCCESS(ptd_export(c,(uint8_t*)got,sizeof got));for(int i=0;i<6;i++)CHECK(got[i]==2*(6-i));drop(&c);drop(&o);
    SUCCESS(ptd_program(s,copy,copy,r,&o));CHECK(ptd_cancel(o)==ERR(cancelled));CHECK(ptd_info(copy).state.readers==0);CHECK(ptd_export(o,(uint8_t*)got,sizeof got)==ERR(cancelled));drop(&o);drop(&copy);
    SUCCESS(ptd_from_f32(s,(coax_t_TvShape){.rank=0},F32,data,1,&a));info=ptd_info(a);coax_t_TvView broad=info.view;broad.shape=shape(6);broad.strides.e[0]=0;SUCCESS(ptd_view(a,broad,&v));SUCCESS(ptd_to(v,cpu,F32,&c));SUCCESS(ptd_export(c,(uint8_t*)got,sizeof got));for(int i=0;i<6;i++)CHECK(got[i]==1);drop(&c);drop(&v);drop(&a);empty(s);empty(cpu);groups++;
}
static void transfers(PtdSession *s,PtdSession *cpu){
    float data[]={1,2,3,4,5,6};PtdTensor *a=NULL,*v=NULL,*t=NULL,*back=NULL,*o=NULL,*p=NULL;
    SUCCESS(ptd_from_f32(cpu,(coax_t_TvShape){.rank=2,.dims={.e={2,3,0,0}}},F32,data,6,&a));
    coax_t_TvView view=ptd_info(a).view;view.shape.dims.e[0]=3;view.shape.dims.e[1]=2;view.strides.e[0]=1;view.strides.e[1]=3;
    SUCCESS(ptd_view(a,view,&v));drop(&a);SUCCESS(ptd_to(v,s,2,&t));drop(&v);
    SUCCESS(ptd_to(t,cpu,F32,&back));float got[6],want[]={1,4,2,5,3,6};SUCCESS(ptd_export(back,(uint8_t*)got,sizeof got));CHECK(!memcmp(got,want,sizeof got));drop(&back);
    coax_t_TdDropout r={.training=false};SUCCESS(ptd_program(s,t,t,r,&o));SUCCESS(ptd_program(s,t,t,r,&p));CHECK(ptd_info(t).state.readers==4);
    SUCCESS(ptd_wait(p));CHECK(ptd_info(t).state.readers==2&&ptd_release(&t)==ERR(busy));SUCCESS(ptd_wait(o));CHECK(ptd_info(t).state.readers==0);drop(&o);drop(&p);drop(&t);empty(s);empty(cpu);groups++;
}
static void refusals(PtdSession *s,PtdSession *cpu){
    PtdTensor *a=NULL,*o=NULL;uint32_t bad[]={0x7f800000,0x7f800001,0xff800000,0x7fc01234};
    for(int i=0;i<4;i++){CHECK(ptd_from_bytes(s,shape(1),F32,(uint8_t*)&bad[i],4,F32,&a)==ERR(number)&&!a);empty(s);}
    float too=65520.0f;CHECK(ptd_from_f32(s,shape(1),1,&too,1,&a)==ERR(arithmetic)&&!a);empty(s);
    CHECK(ptd_from_f32(s,shape(1),255,&too,1,&a)==ERR(dtype)&&!a);CHECK(ptd_from_f32(s,shape(1),F32,NULL,1,&a)==ERR(capacity));
    float huge=value(0x7f7fffff);SUCCESS(ptd_from_f32(s,shape(1),F32,&huge,1,&a));SUCCESS(ptd_program(s,a,a,(coax_t_TdDropout){.training=false},&o));CHECK(ptd_wait(o)==ERR(arithmetic));drop(&o);drop(&a);empty(s);
    float v=49984;SUCCESS(ptd_from_f32(s,shape(1),1,&v,1,&a));coax_t_TdDropout r={.training=false};SUCCESS(ptd_program(s,a,a,r,&o));CHECK(ptd_wait(o)==ERR(arithmetic));CHECK(!ptd_info(a).state.readers&&!ptd_stats(s).poisoned);uint8_t out[4]={1,2,3,4};CHECK(ptd_export(o,out,4)==ERR(arithmetic)&&out[0]==1);drop(&o);
    CHECK(ptd_export(a,out,1)==ERR(capacity)&&out[0]==1);CHECK(ptd_export(a,(uint8_t*)a,2)==ERR(alias));if(host_pointer(s,a->root->data))CHECK(ptd_export(a,host_pointer(s,a->root->data),2)==ERR(alias));coax_t_TvView invalid=ptd_info(a).view;invalid.identity+=1;CHECK(ptd_view(a,invalid,&o)==ERR(state)&&!o);
    PtdTensor *foreign=NULL;SUCCESS(ptd_from_f32(cpu,shape(1),1,&v,1,&foreign));CHECK(ptd_program(s,a,foreign,r,&o)==ERR(device)&&!o);drop(&foreign);drop(&a);empty(s);empty(cpu);
    PtdSession *small=NULL;SUCCESS(ptd_open(CPU,127,NULL,0,&small));CHECK(ptd_from_f32(small,shape(0),F32,NULL,0,&a)==ERR(budget));SUCCESS(ptd_close(&small));
    SUCCESS(ptd_from_f32(s,shape(0),F32,NULL,0,&a));SUCCESS(ptd_export(a,NULL,0));SUCCESS(ptd_program(s,a,a,r,&o));SUCCESS(ptd_wait(o));drop(&o);drop(&a);empty(s);groups++;
}
static void subnormals(PtdSession *s){
    const uint32_t input[]={1,0x00010000,0x007fffff,0x00800000};uint32_t got[4];
    PtdTensor *a=NULL,*o=NULL;SUCCESS(ptd_from_bytes(s,shape(4),F32,(const uint8_t*)input,16,F32,&a));
    SUCCESS(ptd_program(s,a,a,(coax_t_TdDropout){.training=false},&o));SUCCESS(ptd_wait(o));SUCCESS(ptd_export(o,(uint8_t*)got,16));
    int flushed=0;for(int i=0;i<4;i++){uint32_t exact=bits(value(input[i])*2);CHECK(got[i]==exact||(input[i]<0x00800000&&got[i]==0));if(got[i]!=exact)flushed++;}
    printf("f32 subnormal add: %d/4 flushed (cast bits remain exact)\n",flushed);drop(&o);drop(&a);empty(s);groups++;
}
static void numerical(PtdSession *s,PtdSession *cpu){
    float av[21],bv[21];for(int i=0;i<21;i++){av[i]=(i-9)*0.0137f;bv[i]=(i%7)*0.0271f;}
    double maximum=0;for(int dtype=0;dtype<3;dtype++){
        PtdTensor *a=NULL,*b=NULL,*o=NULL,*ca=NULL,*cb=NULL,*co=NULL,*read=NULL;SUCCESS(ptd_from_f32(s,shape(21),dtype,av,21,&a));SUCCESS(ptd_from_f32(s,shape(21),dtype,bv,21,&b));SUCCESS(ptd_to(a,cpu,F32,&ca));SUCCESS(ptd_to(b,cpu,F32,&cb));float aa[21],bb[21];SUCCESS(ptd_export(ca,(uint8_t*)aa,sizeof aa));SUCCESS(ptd_export(cb,(uint8_t*)bb,sizeof bb));
        coax_t_TdDropout r={.seed=123,.stream=7,.keep=21845,.training=true};SUCCESS(ptd_program(s,a,b,r,&o));SUCCESS(ptd_wait(o));SUCCESS(ptd_to(o,cpu,F32,&read));float got[21];SUCCESS(ptd_export(read,(uint8_t*)got,sizeof got));double bound=dtype==0?2e-6:dtype==1?0x1p-9:0x1p-6;double per_type=0;
        for(int i=0;i<21;i++){double want=fmax(0,(double)aa[i]+bb[i]);if((oracle_words[i]>>16)>=21845)want=0;else want*=65536.0/21845;double err=fabs(got[i]-want)/fmax(1,fabs(want));CHECK(err<=bound);if(err>maximum)maximum=err;if(err>per_type)per_type=err;}
        printf("dtype %d non-power dropout scaled error %.9g\n",dtype,per_type);
        drop(&a);drop(&b);drop(&o);drop(&ca);drop(&cb);drop(&read);CHECK(!co);empty(s);empty(cpu);
    }printf("non-power dropout maximum scaled error %.9g\n",maximum);groups++;
}
int main(int argc,char **argv){
    CHECK(argc>=2);int device=atoi(argv[1]);char *source=NULL;size_t length=0;if(device){CHECK(argc==3);FILE *f=fopen(argv[2],"rb");CHECK(f);fseek(f,0,SEEK_END);length=(size_t)ftell(f);rewind(f);source=malloc(length+1);CHECK(source&&fread(source,1,length,f)==length);source[length]=0;fclose(f);}
    PtdSession *s=NULL,*cpu=NULL;coax_t_TdStatus z=ptd_open(device,64*1024*1024,source,length,&s);if(z){fprintf(stderr,"open status %d CUDA %s\n",z,pc_log());
#ifdef __APPLE__
        fprintf(stderr,"Metal %s\n",pm_log());
#endif
        return 1;}
    SUCCESS(ptd_open(CPU,64*1024*1024,NULL,0,&cpu));randoms(s);encodings(s,cpu);rounding(s);programs(s,cpu);views(s,cpu);transfers(s,cpu);refusals(s,cpu);numerical(s,cpu);subnormals(s);SUCCESS(ptd_close(&s));SUCCESS(ptd_close(&cpu));free(source);printf("tensor-device backend %d: %d native groups pass\n",device,groups);return 0;
}
