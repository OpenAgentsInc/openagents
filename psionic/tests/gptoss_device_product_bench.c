/* Opt-in owned original-shaped product microbenchmark; no model required. */
#define _POSIX_C_SOURCE 200809L
#include "cuda-host.h"
#include <stdio.h>
#include <stdlib.h>
#include <assert.h>
#define OK(call) do{PcStatus z=(call);if(z){fprintf(stderr,"%s: %d %s\n",#call,z,pc_log());exit(2);}}while(0)
#define I(value) (PcArg){.kind=PC_ARG_INT,.i=(value)}
#define V(buffer,length) (PcArg){.kind=PC_ARG_VIEW,.data=(buffer),.count=(length)}
int main(int argc,char **argv){assert(argc==5);int rt=atoi(argv[2]),tt=atoi(argv[3]),block=atoi(argv[4]);PcDeviceInfo info;OK(pc_open(&info));FILE *f=fopen(argv[1],"rb");assert(f);fseek(f,0,SEEK_END);long bytes=ftell(f);rewind(f);char *source=malloc((size_t)bytes);assert(source && fread(source,1,(size_t)bytes,f)==(size_t)bytes);fclose(f);PcModule module;double startup;OK(pc_compile(source,(size_t)bytes,argv[1],&module,&startup));free(source);PcKernel prefix,product;OK(pc_kernel(module,"gk_group_prefix",&prefix));OK(pc_kernel(module,"gk_batch_mx_part",&product));PcStream stream;OK(pc_stream(&stream));
    const int rows=5760,cols=2880,n=128,e=32,z=4,lanes=16;assert(rt>0 && rt<=rows && rows%rt==0 && tt>0 && tt<=128 && block>=1 && block<=1024);int64_t words=(int64_t)e*rows*(cols/32)*2,scales=(int64_t)e*rows*(cols/32),floats=(int64_t)n*cols,partials=(int64_t)n*z*lanes*rows;
    PcBuffer w,s,x,p,map,counts;OK(pc_alloc((size_t)words*8,&w));OK(pc_alloc((size_t)scales,&s));OK(pc_alloc((size_t)floats*4,&x));OK(pc_alloc((size_t)partials*4,&p));OK(pc_alloc(e*n*8,&map));OK(pc_alloc((2*e+1)*8,&counts));OK(pc_fill(w,0x22222222,(size_t)words*2,stream));OK(pc_fill(s,0x7f7f7f7f,(size_t)scales/4,stream));OK(pc_fill(x,0x3f800000,(size_t)floats,stream));
    int64_t mapping[4096]={0},count[65]={0};for(int expert=0;expert<32;expert++){count[expert]=16;for(int j=0;j<16;j++)mapping[expert*128+j]=(expert/4+8*j)*4+expert%4;}OK(pc_upload(map,mapping,sizeof mapping,stream));OK(pc_upload(counts,count,sizeof count,stream));PcArg pa[]={I(e),V(counts,65)};OK(pc_launch(prefix,1,256,pa,2,stream));
    PcArg a[]={I(rows),I(cols),I(n),I(e),I(z),I(lanes),I(0),V(w,words),V(s,scales),V(map,e*n),V(counts,65),V(x,floats),V(p,partials)};int64_t items=(int64_t)((n*z+tt-1)/tt+e)*(rows/rt)*lanes;OK(pc_launch(product,items,block,a,13,stream));OK(pc_sync_stream(stream));
    for(int slot=0;slot<512;slot+=73){for(int lane=0;lane<16;lane+=15){float value;int64_t offset=((int64_t)slot*16+lane)*rows+5759;OK(pc_download(&value,p+(uint64_t)offset*4,4,stream));OK(pc_sync_stream(stream));assert(value==(lane<10?192.0f:160.0f));}}
    PcEvent begin,end;OK(pc_event(&begin));OK(pc_event(&end));for(int repeat=0;repeat<5;repeat++){OK(pc_record(begin,stream));for(int k=0;k<20;k++)OK(pc_launch(product,items,block,a,13,stream));OK(pc_record(end,stream));OK(pc_sync_stream(stream));double secs;OK(pc_event_seconds(begin,end,&secs));printf("row_tile=%d token_tile=%d block=%d repeat=%d up_product_ms=%.6f\n",rt,tt,block,repeat,secs*1000/20);fflush(stdout);}OK(pc_destroy_event(begin));OK(pc_destroy_event(end));pc_free(w);pc_free(s);pc_free(x);pc_free(p);pc_free(map);pc_free(counts);pc_destroy_stream(stream);pc_unload(module);return 0;}
