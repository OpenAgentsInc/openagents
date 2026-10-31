/* Firm reason2: owned ABI/GPU fixtures and canaries; source arithmetic is Coax.
   Independent Torch reads saved inputs, never candidate outputs as references. */
#include "kernels.h"
#include "metal-host.h"
#include <assert.h>
#include <float.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#define TRY(e) do{PmStatus s_=(e);if(s_!=PM_OK){fprintf(stderr,"%s: %d: %s\n",#e,s_,pm_log());exit(1);}}while(0)
static const char *directory,*backend;static int gpu,index_;static FILE *manifest;static PmModule module;static PmKernel kernels[5];static size_t values;
static coax_v_f32 F(float*p,int64_t n){return(coax_v_f32){p,n};}static coax_v_u8 U(uint8_t*p,int64_t n){return(coax_v_u8){p,n};}static coax_v_u16 W(uint16_t*p,int64_t n){return(coax_v_u16){p,n};}static coax_v_s64 L(int64_t*p,int64_t n){return(coax_v_s64){p,n};}
static PmArg I(int64_t n){return(PmArg){.kind=PM_ARG_INT,.i=n};}static PmArg V(PmBuffer b,int64_t n){return(PmArg){.kind=PM_ARG_VIEW,.buffer=b,.count=n};}
static void *memory(size_t n){void*p=calloc(n?n:1,1);assert(p);return p;}
static PmBuffer buffer(const void*p,size_t n){PmBuffer b;TRY(pm_alloc(n?n:4,&b));if(n)memcpy(pm_contents(b),p,n);return b;}
static void save(const char*tag,const void*p,size_t n){char path[1024];snprintf(path,sizeof path,"%s/%03d-%s.bin",directory,index_,tag);FILE*f=fopen(path,"wb");assert(f&&fwrite(p,1,n,f)==n&&!fclose(f));}
static void input(const char*tag,const void*p,size_t n){if(manifest)save(tag,p,n);}
static void fill(float*p,int n,int salt){for(int i=0;i<n;i++)p[i]=(float)((i*(salt*2+1)+salt)%127-63)/32.f;}
static void poison(float*out,uint8_t*flags,int n){for(int i=0;i<=n;i++){out[i]=7;flags[i]=9;}}
static void checked(const float*out,const uint8_t*flags,int n){for(int i=0;i<n;i++)assert(flags[i]==0&&isfinite(out[i]));assert(out[n]==7&&flags[n]==9);}
static void run(int kernel,int64_t n,PmArg*a,int argc){PmBatch b;TRY(pm_begin(&b));TRY(pm_launch(b,kernels[kernel],n,1,0,a,argc));TRY(pm_wait(b,NULL));}
static void finish(float*out,int n){save(backend,out,(size_t)n*4);values+=(size_t)n;}
static void embedding(int tokens,int width,int vocab,int inject){
  int n=tokens*width,en=vocab*width,vn=128*width;int64_t*ids=memory(tokens*8),*map=memory(tokens*8);uint16_t*e=memory(en*2);float*x=memory(n*4),*visual=memory(vn*4),*out=memory((n+1)*4);uint8_t*flags=memory(n+1);
  for(int i=0;i<en;i++){float f=(float)(i%127-63)/32.f;uint32_t bits;memcpy(&bits,&f,4);e[i]=(uint16_t)(bits>>16);}
  fill(x,n,7);fill(visual,vn,11);for(int t=0;t<tokens;t++){ids[t]=(t*37+17)%vocab;map[t]=t%3==0?-1:(t*7)%128;}if(tokens>2){map[1]=0;map[tokens-1]=127;}
  input("ids",ids,tokens*8);input("map",map,tokens*8);input("embedding",e,en*2);input("visual",visual,vn*4);input("x",x,n*4);
  if(manifest)fprintf(manifest,"{\"index\":%d,\"op\":\"%s\",\"tokens\":%d,\"width\":%d,\"vocab\":%d}\n",index_,inject?"inject":"embed",tokens,width,vocab);
  poison(out,flags,n);for(int i=0;i<n;i++){if(inject)gtk_inject(i,width,F(x,n),L(map,tokens),F(visual,vn),F(out,n+1),U(flags,n+1));else gtk_embed(i,width,vocab,L(ids,tokens),W(e,en),L(map,tokens),F(visual,vn),F(out,n+1),U(flags,n+1));}checked(out,flags,n);finish(out,n);
  if(gpu){poison(out,flags,n);PmBuffer bi=buffer(ids,tokens*8),bm=buffer(map,tokens*8),be=buffer(e,en*2),bv=buffer(visual,vn*4),bx=buffer(x,n*4),bo=buffer(out,(n+1)*4),bf=buffer(flags,n+1);
    if(inject){PmArg a[]={I(width),V(bx,n),V(bm,tokens),V(bv,vn),V(bo,n+1),V(bf,n+1)};run(2,n+3,a,6);}else{PmArg a[]={I(width),I(vocab),V(bi,tokens),V(be,en),V(bm,tokens),V(bv,vn),V(bo,n+1),V(bf,n+1)};run(0,n+3,a,8);}
    checked(pm_contents(bo),pm_contents(bf),n);save("metal",pm_contents(bo),n*4);PmBuffer all[]={bi,bm,be,bv,bx,bo,bf};for(int i=0;i<7;i++)TRY(pm_free(all[i]));}
  free(ids);free(map);free(e);free(x);free(visual);free(out);free(flags);index_++;
}
static void rotate(int tokens,int heads,int dim){
  int n=tokens*heads*dim,rn=tokens*dim;float*x=memory(n*4),*rope=memory(rn*4),*out=memory((n+1)*4);uint8_t*flags=memory(n+1);fill(x,n,13);
  for(int t=0;t<tokens;t++)for(int j=0;j<dim/2;j++){float phase=(float)(t%17+j%23)/64.f;rope[t*dim+2*j]=cosf(phase);rope[t*dim+2*j+1]=sinf(phase);}
  input("x",x,n*4);input("rope",rope,rn*4);if(manifest)fprintf(manifest,"{\"index\":%d,\"op\":\"rotate\",\"tokens\":%d,\"heads\":%d,\"dim\":%d}\n",index_,tokens,heads,dim);
  poison(out,flags,n);for(int i=0;i<n;i++)gtk_rotate(i,heads,dim,F(x,n),F(rope,rn),F(out,n+1),U(flags,n+1));checked(out,flags,n);finish(out,n);
  if(gpu){poison(out,flags,n);PmBuffer bx=buffer(x,n*4),br=buffer(rope,rn*4),bo=buffer(out,(n+1)*4),bf=buffer(flags,n+1);PmArg a[]={I(heads),I(dim),V(bx,n),V(br,rn),V(bo,n+1),V(bf,n+1)};run(1,n+3,a,6);checked(pm_contents(bo),pm_contents(bf),n);save("metal",pm_contents(bo),n*4);TRY(pm_free(bx));TRY(pm_free(br));TRY(pm_free(bo));TRY(pm_free(bf));}
  free(x);free(rope);free(out);free(flags);index_++;
}
static void element(int n,int add){
  float*a=memory(n*4),*b=memory(n*4),*out=memory((n+1)*4);uint8_t*flags=memory(n+1);fill(a,n,17);fill(b,n,19);input("x",a,n*4);input("b",b,n*4);if(manifest)fprintf(manifest,"{\"index\":%d,\"op\":\"%s\",\"n\":%d}\n",index_,add?"add":"multiply",n);
  poison(out,flags,n);for(int i=0;i<n;i++){if(add)gtk_add(i,F(a,n),F(b,n),F(out,n+1),U(flags,n+1));else gtk_multiply(i,F(a,n),F(b,n),F(out,n+1),U(flags,n+1));}checked(out,flags,n);finish(out,n);
  if(gpu){poison(out,flags,n);PmBuffer ba=buffer(a,n*4),bb=buffer(b,n*4),bo=buffer(out,(n+1)*4),bf=buffer(flags,n+1);PmArg args[]={V(ba,n),V(bb,n),V(bo,n+1),V(bf,n+1)};run(add?4:3,n+3,args,4);checked(pm_contents(bo),pm_contents(bf),n);save("metal",pm_contents(bo),n*4);TRY(pm_free(ba));TRY(pm_free(bb));TRY(pm_free(bo));TRY(pm_free(bf));}
  free(a);free(b);free(out);free(flags);index_++;
}
static void numeric_and_metadata(void){
  uint32_t bads[]={0x7f800000,0xff800000,0x7fc00001,0x7f800001};
  for(unsigned bad=0;bad<4;bad++){
    float a[2],b[]={0,FLT_MAX},out[3],rope[]={1,0},visual[128]={0};uint8_t flags[3];int64_t ids[]={0,0},map[]={-1,0};uint16_t embedding[]={0x7f81};memcpy(a,bads+bad,4);a[1]=FLT_MAX;visual[0]=a[0];
    for(int op=0;op<5;op++){
      poison(out,flags,2);
      for(int i=0;i<2;i++){if(op==0)gtk_embed(i,1,1,L(ids,2),W(embedding,1),L(map,2),F(visual,128),F(out,3),U(flags,3));if(op==1)gtk_rotate(i,1,2,F(a,2),F(rope,2),F(out,3),U(flags,3));if(op==2)gtk_inject(i,1,F(a,2),L(map,2),F(visual,128),F(out,3),U(flags,3));if(op==3)gtk_multiply(i,F(a,2),F(b,2),F(out,3),U(flags,3));if(op==4)gtk_add(i,F(a,2),F(b,2),F(out,3),U(flags,3));}
      for(int i=0;i<2;i++)assert(flags[i]==1&&out[i]==7);assert(out[2]==7&&flags[2]==9);
      if(gpu){poison(out,flags,2);PmBuffer ba=buffer(a,8),bb=buffer(b,8),br=buffer(rope,8),bv=buffer(visual,512),bi=buffer(ids,16),bm=buffer(map,16),be=buffer(embedding,2),bo=buffer(out,12),bf=buffer(flags,3);
        if(op==0){PmArg z[]={I(1),I(1),V(bi,2),V(be,1),V(bm,2),V(bv,128),V(bo,3),V(bf,3)};run(op,5,z,8);}else if(op==1){PmArg z[]={I(1),I(2),V(ba,2),V(br,2),V(bo,3),V(bf,3)};run(op,5,z,6);}else if(op==2){PmArg z[]={I(1),V(ba,2),V(bm,2),V(bv,128),V(bo,3),V(bf,3)};run(op,5,z,6);}else{PmArg z[]={V(ba,2),V(bb,2),V(bo,3),V(bf,3)};run(op,5,z,4);}
        const float*o=pm_contents(bo);const uint8_t*f=pm_contents(bf);for(int i=0;i<2;i++)assert(f[i]==1&&o[i]==7);assert(o[2]==7&&f[2]==9);PmBuffer all[]={ba,bb,br,bv,bi,bm,be,bo,bf};for(int i=0;i<9;i++)TRY(pm_free(all[i]));}
    }
  }
  /* Malformed metadata is distinct from numerical failure: flags stay9. */
  for(int op=0;op<5;op++){
    float a[]={1,2},v[128]={0},out[3];uint8_t flags[3];uint16_t e[]={0x3f80};int64_t ids[]={1,1},map[]={128,-2};poison(out,flags,2);
    for(int i=0;i<2;i++){if(op==0)gtk_embed(i,1,1,L(ids,2),W(e,1),L(map,2),F(v,128),F(out,3),U(flags,3));if(op==1)gtk_rotate(i,0,2,F(a,2),F(a,2),F(out,3),U(flags,3));if(op==2)gtk_inject(i,1,F(a,2),L(map,2),F(v,128),F(out,3),U(flags,3));if(op==3)gtk_multiply(i,F(a,2),F(a,1),F(out,3),U(flags,3));if(op==4)gtk_add(i,F(a,2),F(a,1),F(out,3),U(flags,3));}
    for(int i=0;i<3;i++)assert(out[i]==7&&flags[i]==9);
    if(gpu){PmBuffer ba=buffer(a,8),bv=buffer(v,512),bi=buffer(ids,16),bm=buffer(map,16),be=buffer(e,2),bo=buffer(out,12),bf=buffer(flags,3);
      if(op==0){PmArg z[]={I(1),I(1),V(bi,2),V(be,1),V(bm,2),V(bv,128),V(bo,3),V(bf,3)};run(op,2,z,8);}else if(op==1){PmArg z[]={I(0),I(2),V(ba,2),V(ba,2),V(bo,3),V(bf,3)};run(op,2,z,6);}else if(op==2){PmArg z[]={I(1),V(ba,2),V(bm,2),V(bv,128),V(bo,3),V(bf,3)};run(op,2,z,6);}else{PmArg z[]={V(ba,2),V(ba,1),V(bo,3),V(bf,3)};run(op,2,z,4);}
      const float*o=pm_contents(bo);const uint8_t*f=pm_contents(bf);for(int i=0;i<3;i++)assert(o[i]==7&&f[i]==9);PmBuffer all[]={ba,bv,bi,bm,be,bo,bf};for(int i=0;i<7;i++)TRY(pm_free(all[i]));}
  }
  /* Raw C ABI negative extents must be rejected before bounded products. */
  {int64_t ids[2]={0,0},map[2]={-1,-1};uint16_t e[2]={0x3f80,0x4000};float a[2]={1,2},v[256]={0},out[3];uint8_t flags[3];poison(out,flags,2);
   gtk_embed(0,2,1,L(ids,INT64_MIN),W(e,2),L(map,INT64_MIN),F(v,256),F(out,3),U(flags,3));
   gtk_inject(0,2,F(a,2),L(map,INT64_MIN),F(v,256),F(out,3),U(flags,3));
   for(int i=0;i<3;i++)assert(out[i]==7&&flags[i]==9);}
}
int main(int argc,char**argv){
  assert(argc==4);directory=argv[1];backend=argv[2];gpu=!strcmp(backend,"debug");
  if(gpu){PmDeviceInfo info;TRY(pm_open(&info));printf("Metal device: %s\n",info.name);FILE*f=fopen(argv[3],"rb");assert(f&&!fseek(f,0,SEEK_END));long n=ftell(f);rewind(f);char*source=memory((size_t)n);assert(fread(source,1,(size_t)n,f)==(size_t)n&&!fclose(f));TRY(pm_compile(source,(size_t)n,"groot-text-f32-v1",&module,NULL));free(source);const char*names[]={"gtk_embed","gtk_rotate","gtk_inject","gtk_multiply","gtk_add"};for(int i=0;i<5;i++)TRY(pm_kernel(module,names[i],kernels+i));char path[1024];snprintf(path,sizeof path,"%s/cases.jsonl",directory);manifest=fopen(path,"wx");assert(manifest);}
  const int shape[][3]={{1,1,1},{7,3,17},{140,2048,257},{512,128,17},{3,1,151936}};
  for(unsigned i=0;i<sizeof shape/sizeof *shape;i++)for(int inject=0;inject<2;inject++)embedding(shape[i][0],shape[i][1],shape[i][2],inject);
  rotate(1,1,2);rotate(7,3,6);rotate(140,16,128);rotate(140,8,128);rotate(512,16,128);
  for(int add=0;add<2;add++){element(1,add);element(140*2048,add);element(512*6144,add);}
  numeric_and_metadata();if(manifest)assert(!fclose(manifest));if(gpu){for(int i=0;i<5;i++)TRY(pm_kernel_free(kernels[i]));TRY(pm_unload(module));}
  printf("GR00T text kernels %s: %d cases,%zu finite values plus numeric/metadata poison passed\n",backend,index_,values);return 0;
}
