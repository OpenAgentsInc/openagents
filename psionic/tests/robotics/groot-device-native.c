/* Firm reason: 2. Independent device/ABI exercise; all shipped math is Coax.
   Fixtures and GPU/CPU results are compared separately by the Torch oracle. */
#include "groot-device.h"
#include "metal-host.h"
#include <assert.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <stdint.h>
#define TRY(x) do { PmStatus s_=(x);if(s_!=PM_OK){fprintf(stderr,"%s: %d %s\n",#x,s_,pm_log());exit(1);} } while(0)
static const char *directory,*backend;static FILE *manifest;static int index_,metal;static PmModule module;static PmKernel kernels[7];static size_t values;
static coax_v_f32 F(float *p,int64_t n){return(coax_v_f32){p,n};}static coax_v_u8 U(uint8_t *p,int64_t n){return(coax_v_u8){p,n};}static coax_v_s64 L(int64_t *p,int64_t n){return(coax_v_s64){p,n};}
static PmArg I(int64_t n){return(PmArg){.kind=PM_ARG_INT,.i=n};}static PmArg S(float n){return(PmArg){.kind=PM_ARG_F32,.f=n};}static PmArg V(PmBuffer p,int64_t n){return(PmArg){.kind=PM_ARG_VIEW,.buffer=p,.count=n};}
static void *memory(size_t bytes){void*p=calloc(bytes?bytes:1,1);assert(p);return p;}
static void save(const char *tag,const void *data,size_t size){char path[1024];snprintf(path,sizeof path,"%s/%03d-%s.bin",directory,index_,tag);FILE*f=fopen(path,"wb");assert(f);assert(fwrite(data,1,size,f)==size);assert(!fclose(f));}
static PmBuffer buffer(const void *data,size_t size){PmBuffer b;TRY(pm_alloc(size?size:4,&b));if(size)memcpy(pm_contents(b),data,size);return b;}
static void run(int kernel,int64_t items,PmArg *a,int count){PmBatch b;TRY(pm_begin(&b));TRY(pm_launch(b,kernels[kernel],items,1,0,a,count));TRY(pm_wait(b,NULL));}
static void zero(const uint8_t*p,int64_t n){for(int64_t i=0;i<n;i++)assert(p[i]==0);}
static void fill(float*x,int64_t n,int salt){for(int64_t i=0;i<n;i++)x[i]=(float)((i*(salt*2+1)+salt)%127-63)/32.0f;}
static void input(const char*tag,const void*data,size_t size){if(manifest)save(tag,data,size);}
static void finish(float*out,int64_t n){save(backend,out,(size_t)n*4);values+=(size_t)n;}
static void norm(int width,int kind){int rows=3,n=rows*width;float*x=memory(n*4),*a=memory(width*4),*b=memory(width*4),*out=memory(n*4);uint8_t*f=memory(rows);fill(x,n,3);for(int i=2*width;i<n;i++)x[i]=2;for(int i=0;i<width;i++){a[i]=0.5f+(float)(i%13)/16;b[i]=(float)(i%7-3)/8;}int na=kind?width:0,nb=kind==1?width:0;float epsilon=kind==1?1e-6f:1e-5f;
  input("x",x,n*4);input("a",a,na*4);input("b",b,nb*4);if(manifest)fprintf(manifest,"{\"index\":%d,\"op\":\"norm\",\"width\":%d,\"kind\":%d,\"rows\":%d,\"epsilon\":%.9g}\n",index_,width,kind,rows,epsilon);
  memset(f,9,rows);for(int i=0;i<rows;i++)gdk_norm(i,width,kind,epsilon,F(x,n),F(a,na),F(b,nb),F(out,n),U(f,rows));zero(f,rows);finish(out,n);
  if(metal){memset(f,9,rows);PmBuffer bx=buffer(x,n*4),ba=buffer(a,na*4),bb=buffer(b,nb*4),bo=buffer(out,n*4),bf=buffer(f,rows);PmArg args[]={I(width),I(kind),S(epsilon),V(bx,n),V(ba,na),V(bb,nb),V(bo,n),V(bf,rows)};run(0,rows,args,8);zero(pm_contents(bf),rows);save("metal",pm_contents(bo),n*4);TRY(pm_free(bx));TRY(pm_free(ba));TRY(pm_free(bb));TRY(pm_free(bo));TRY(pm_free(bf));}
  free(x);free(a);free(b);free(out);free(f);index_++;
}
static void element(int type,int kind,int width){int n=type==2?width*3:4096;float*x=memory(n*4),*a=memory(width*8),*out=memory(n*4);uint8_t*f=memory(n);fill(x,n,11);if(type==1){x[0]=-80;x[1]=80;x[2]=0;x[3]=-0.0f;}fill(a,width*2,5);input("x",x,n*4);input("a",a,type==2?width*8:0);if(manifest)fprintf(manifest,"{\"index\":%d,\"op\":\"%s\",\"kind\":%d,\"width\":%d,\"n\":%d}\n",index_,type==1?"activation":"modulate",kind,width,n);
  memset(f,9,n);for(int i=0;i<n;i++){if(type==1)gdk_activate(i,kind,F(x,n),F(out,n),U(f,n));else gdk_modulate(i,width,kind,F(x,n),F(a,width*2),F(out,n),U(f,n));}zero(f,n);finish(out,n);
  if(metal){memset(f,9,n);PmBuffer bx=buffer(x,n*4),ba=buffer(a,width*8),bo=buffer(out,n*4),bf=buffer(f,n);if(type==1){PmArg args[]={I(kind),V(bx,n),V(bo,n),V(bf,n)};run(2,n,args,4);}else{PmArg args[]={I(width),I(kind),V(bx,n),V(ba,width*2),V(bo,n),V(bf,n)};run(1,n,args,6);}zero(pm_contents(bf),n);save("metal",pm_contents(bo),n*4);TRY(pm_free(bx));TRY(pm_free(ba));TRY(pm_free(bo));TRY(pm_free(bf));}
  free(x);free(a);free(out);free(f);index_++;
}
static void attention(int nq,int nk,int h,int kv,int d,int mode,int empty){int qn=nq*h*d,kn=nk*kv*d,sn=nq*h*nk,on=qn;float*q=memory(qn*4),*k=memory(kn*4),*v=memory(kn*4),*scores=memory(sn*4),*prob=memory(sn*4),*out=memory(on*4);uint8_t*valid=memory(nk),*keep=memory(sn),*sf=memory(sn),*pf=memory(nq*h),*of=memory(on);int64_t*camera=memory(nk*8);fill(q,qn,7);fill(k,kn,13);fill(v,kn,17);for(int i=0;i<nk;i++){valid[i]=(uint8_t)(!empty&&i%3!=1);camera[i]=i/(nk/2?nk/2:1);}int offset=(mode&2)?nk-nq:0,nvalid=(mode&1)?nk:0,ncamera=(mode&4)?nk:0;
  input("x",q,qn*4);input("a",k,kn*4);input("b",v,kn*4);input("mask",valid,nvalid);input("camera",camera,ncamera*8);if(manifest)fprintf(manifest,"{\"index\":%d,\"op\":\"attention\",\"queries\":%d,\"keys\":%d,\"heads\":%d,\"kv_heads\":%d,\"dim\":%d,\"mode\":%d,\"offset\":%d}\n",index_,nq,nk,h,kv,d,mode,offset);
  memset(sf,9,sn);memset(pf,9,nq*h);memset(of,9,on);for(int i=0;i<sn;i++)gdk_scores(i,nq,nk,h,kv,d,mode,offset,F(q,qn),F(k,kn),U(valid,nvalid),L(camera,ncamera),F(scores,sn),U(keep,sn),U(sf,sn));zero(sf,sn);for(int i=0;i<nq*h;i++)gdk_softmax(i,nk,F(scores,sn),U(keep,sn),F(prob,sn),U(pf,nq*h));zero(pf,nq*h);for(int i=0;i<on;i++)gdk_context(i,nq,nk,h,kv,d,F(prob,sn),F(v,kn),F(out,on),U(of,on));zero(of,on);
  float*joined=memory((sn*2+on)*4);memcpy(joined,scores,sn*4);memcpy(joined+sn,prob,sn*4);memcpy(joined+2*sn,out,on*4);finish(joined,sn*2+on);
  if(metal){memset(sf,9,sn);memset(pf,9,nq*h);memset(of,9,on);PmBuffer bq=buffer(q,qn*4),bk=buffer(k,kn*4),bv=buffer(v,kn*4),bm=buffer(valid,nvalid),bc=buffer(camera,ncamera*8),bs=buffer(scores,sn*4),bp=buffer(prob,sn*4),bo=buffer(out,on*4),bkeep=buffer(keep,sn),bsf=buffer(sf,sn),bpf=buffer(pf,nq*h),bof=buffer(of,on);
    PmArg a[]={I(nq),I(nk),I(h),I(kv),I(d),I(mode),I(offset),V(bq,qn),V(bk,kn),V(bm,nvalid),V(bc,ncamera),V(bs,sn),V(bkeep,sn),V(bsf,sn)};run(3,sn,a,14);zero(pm_contents(bsf),sn);PmArg b[]={I(nk),V(bs,sn),V(bkeep,sn),V(bp,sn),V(bpf,nq*h)};run(4,nq*h,b,5);zero(pm_contents(bpf),nq*h);PmArg c[]={I(nq),I(nk),I(h),I(kv),I(d),V(bp,sn),V(bv,kn),V(bo,on),V(bof,on)};run(5,on,c,9);zero(pm_contents(bof),on);
    memcpy(joined,pm_contents(bs),sn*4);memcpy(joined+sn,pm_contents(bp),sn*4);memcpy(joined+2*sn,pm_contents(bo),on*4);save("metal",joined,(sn*2+on)*4);PmBuffer all[]={bq,bk,bv,bm,bc,bs,bp,bo,bkeep,bsf,bpf,bof};for(int i=0;i<12;i++)TRY(pm_free(all[i]));
  }
  free(q);free(k);free(v);free(scores);free(prob);free(out);free(valid);free(keep);free(sf);free(pf);free(of);free(camera);free(joined);index_++;
}
static void split(int width){int n=width*3;float*x=memory(n*12),*q=memory(n*4),*k=memory(n*4),*v=memory(n*4),*joined=memory(n*12);uint8_t*f=memory(n);fill(x,n*3,19);input("x",x,n*12);if(manifest)fprintf(manifest,"{\"index\":%d,\"op\":\"split\",\"width\":%d,\"rows\":3}\n",index_,width);memset(f,9,n);for(int i=0;i<n;i++)gdk_qkv_split(i,width,F(x,n*3),F(q,n),F(k,n),F(v,n),U(f,n));zero(f,n);memcpy(joined,q,n*4);memcpy(joined+n,k,n*4);memcpy(joined+2*n,v,n*4);finish(joined,n*3);
  if(metal){memset(f,9,n);PmBuffer bx=buffer(x,n*12),bq=buffer(q,n*4),bk=buffer(k,n*4),bv=buffer(v,n*4),bf=buffer(f,n);PmArg a[]={I(width),V(bx,n*3),V(bq,n),V(bk,n),V(bv,n),V(bf,n)};run(6,n,a,6);zero(pm_contents(bf),n);memcpy(joined,pm_contents(bq),n*4);memcpy(joined+n,pm_contents(bk),n*4);memcpy(joined+2*n,pm_contents(bv),n*4);save("metal",joined,n*12);TRY(pm_free(bx));TRY(pm_free(bq));TRY(pm_free(bk));TRY(pm_free(bv));TRY(pm_free(bf));}
  free(x);free(q);free(k);free(v);free(joined);free(f);index_++;
}
static void numeric(void){float x[]={1,2},a[]={1,1},b[]={0,0},out[]={7,7};uint8_t f[]={9,9};uint32_t words[]={0x7f800000,0xff800000,0x7fc00001,0x7f800001};for(int i=0;i<4;i++){memcpy(x+1,words+i,4);for(int kind=0;kind<3;kind++){f[0]=9;gdk_norm(0,2,kind,1e-5f,F(x,2),F(a,kind?2:0),F(b,kind==1?2:0),F(out,2),U(f,1));assert(f[0]==1);}for(int kind=0;kind<2;kind++){f[1]=9;gdk_activate(1,kind,F(x,2),F(out,2),U(f,2));assert(f[1]==1);}}x[1]=2;f[0]=9;out[0]=7;gdk_norm(0,2,0,0,F(x,2),F(a,0),F(b,0),F(out,2),U(f,1));assert(f[0]==9&&out[0]==7);}
int main(int argc,char**argv){if(argc!=4)return 2;directory=argv[1];backend=argv[2];metal=!strcmp(backend,"debug");if(metal){PmDeviceInfo info;TRY(pm_open(&info));printf("device %s\n",info.name);FILE*f=fopen(argv[3],"rb");assert(f);fseek(f,0,SEEK_END);long n=ftell(f);rewind(f);char*s=memory(n);assert(fread(s,1,n,f)==(size_t)n);fclose(f);TRY(pm_compile(s,n,"groot-device-f32-v1",&module,NULL));free(s);const char*names[]={"gdk_norm","gdk_modulate","gdk_activate","gdk_scores","gdk_softmax","gdk_context","gdk_qkv_split"};for(int i=0;i<7;i++)TRY(pm_kernel(module,names[i],kernels+i));char path[1024];snprintf(path,sizeof path,"%s/cases.jsonl",directory);manifest=fopen(path,"wx");assert(manifest);}
  int widths[]={1,3,48,64,128,1536,2048};for(int w=0;w<7;w++)for(int kind=0;kind<3;kind++)norm(widths[w],kind);for(int kind=0;kind<2;kind++){element(1,kind,1);element(2,kind,1536);}for(int d=48;d<=128;d=d==48?64:128){for(int mode=0;mode<8;mode++){int q=(mode&4)?8:3,k=(mode&4)?8:13;attention(q,k,16,8,d,mode,0);}attention(3,13,32,32,d,1,1);if(d==128)break;}split(3);split(1024);numeric();if(manifest)assert(!fclose(manifest));if(metal){for(int i=0;i<7;i++)TRY(pm_kernel_free(kernels[i]));TRY(pm_unload(module));}printf("GR00T device %s: %d cases, %zu finite values and numeric refusal passed\n",backend,index_,values);return 0;}
