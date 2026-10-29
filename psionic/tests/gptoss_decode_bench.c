/* Opt-in, owned original-shaped decode products. Alternate two Coax CUDA
   modules on one device; flush more than the 4080 L2 before each timed launch.
   No full model, upstream runtime or hand-written arithmetic kernel. */
#define _POSIX_C_SOURCE 200809L
#include "cuda-host.h"
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#define OK(call) do{PcStatus z=(call);if(z){fprintf(stderr,"%s: %d %s\n",#call,z,pc_log());exit(2);}}while(0)
#define I(value) (PcArg){.kind=PC_ARG_INT,.i=(value)}
#define V(buffer,length) (PcArg){.kind=PC_ARG_VIEW,.data=(buffer),.count=(length)}
static PcModule module(const char *name){FILE *f=fopen(name,"rb");assert(f);assert(!fseek(f,0,SEEK_END));long n=ftell(f);rewind(f);char *src=malloc((size_t)n);assert(src && fread(src,1,(size_t)n,f)==(size_t)n);fclose(f);PcModule m;double seconds;OK(pc_compile(src,(size_t)n,name,&m,&seconds));free(src);return m;}
static void run(PcModule modules[2],int bf,int rows,int cols,int stride,int block,PcStream stream,PcBuffer flush){
    const int experts=32,selected=4,lanes=16;int slots=bf?1:selected;int64_t words=(int64_t)(bf?1:experts)*rows*(cols/(bf?4:32))*(bf?1:2),ns=(int64_t)experts*rows*(cols/32),nx=(stride?selected:1)*cols,np=(int64_t)slots*lanes*rows;
    PcBuffer w,s,x,p,ids;OK(pc_alloc((size_t)words*8,&w));OK(pc_alloc((size_t)ns,&s));OK(pc_alloc((size_t)nx*4,&x));OK(pc_alloc((size_t)np*4,&p));OK(pc_alloc(selected*8,&ids));
    OK(pc_fill(w,bf?0x3f803f80:0x22222222,(size_t)words*2,stream));OK(pc_fill(s,0x7f7f7f7f,(size_t)ns/4,stream));OK(pc_fill(x,0x3f800000,(size_t)nx,stream));int64_t chosen[4]={0,7,19,31};OK(pc_upload(ids,chosen,sizeof chosen,stream));
    PcKernel kernels[2],clear[2];for(int v=0;v<2;v++){OK(pc_kernel(modules[v],bf?"gk_bf_part":"gk_mx_part",&kernels[v]));OK(pc_kernel(modules[v],"gk_clear",&clear[v]));}
    PcArg mx[]={I(rows),I(cols),I(lanes),I(stride),V(w,words),V(s,ns),V(ids,selected),V(x,nx),V(p,np)};
    PcArg dense[]={I(rows),I(cols),I(lanes),V(w,words),V(x,nx),V(p,np)};PcArg wipe[]={V(flush,83886080)};PcArg *args=bf?dense:mx;int nargs=bf?6:9;
    PcEvent begin,end;OK(pc_event(&begin));OK(pc_event(&end));
    for(int repeat=-1;repeat<5;repeat++){double seconds[2]={0,0};for(int sample=0;sample<10;sample++){for(int order=0;order<2;order++){int v=(sample+order)%2;OK(pc_launch(clear[v],83886080,256,wipe,1,stream));OK(pc_record(begin,stream));OK(pc_launch(kernels[v],np,block,args,nargs,stream));OK(pc_record(end,stream));OK(pc_sync_stream(stream));double elapsed;OK(pc_event_seconds(begin,end,&elapsed));seconds[v]+=elapsed;}}
        for(int v=0;v<2;v++)if(repeat>=0)printf("decode_product kind=%s rows=%d cols=%d block=%d variant=%d repeat=%d milliseconds=%.6f\n",bf?"bf16":"mxfp4",rows,cols,block,v,repeat,seconds[v]*100);fflush(stdout);
    }
    // Constant coefficients/inputs give exact independent per-lane answers.
    for(int v=0;v<2;v++){OK(pc_launch(kernels[v],np,block,args,nargs,stream));for(int slot=0;slot<slots;slot++){for(int lane=0;lane<lanes;lane++){float got;size_t offset=((size_t)slot*lanes+lane)*rows+rows-1;OK(pc_download(&got,p+offset*4,4,stream));OK(pc_sync_stream(stream));int groups=cols/(bf?4:32);float expected=(float)(((groups+lanes-1-lane)/lanes)*(bf?4:32));assert(got==expected);}}}
    OK(pc_destroy_event(begin));OK(pc_destroy_event(end));pc_free(w);pc_free(s);pc_free(x);pc_free(p);pc_free(ids);
}
int main(int argc,char **argv){assert(argc==4);int block=atoi(argv[3]);assert(block>=32 && block<=1024);PcDeviceInfo info;OK(pc_open(&info));PcModule modules[2]={module(argv[1]),module(argv[2])};PcStream stream;OK(pc_stream(&stream));PcBuffer flush;OK(pc_alloc(83886080,&flush));run(modules,0,5760,2880,0,block,stream,flush);run(modules,0,2880,2880,2880,block,stream,flush);run(modules,1,201088,2880,0,block,stream,flush);pc_free(flush);pc_destroy_stream(stream);pc_unload(modules[0]);pc_unload(modules[1]);return 0;}
