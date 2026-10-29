/* Owned softmax-only slice: same input and module, original sequential row
   versus exact denominator plus parallel divisions. No handwritten kernels. */
#define _POSIX_C_SOURCE 200809L
#include "cuda-host.h"
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#define OK(call) do{PcStatus z=(call);if(z){fprintf(stderr,"%s: %d %s\n",#call,z,pc_log());exit(2);}}while(0)
#define I(value) (PcArg){.kind=PC_ARG_INT,.i=(value)}
#define V(buffer,length) (PcArg){.kind=PC_ARG_VIEW,.data=(buffer),.count=(length)}
int main(int argc,char **argv){
    assert(argc==2);FILE *f=fopen(argv[1],"rb");assert(f);fseek(f,0,SEEK_END);long bytes=ftell(f);rewind(f);char *source=malloc((size_t)bytes);assert(source && fread(source,1,(size_t)bytes,f)==(size_t)bytes);fclose(f);PcDeviceInfo info;OK(pc_open(&info));PcModule module;double startup;OK(pc_compile(source,(size_t)bytes,argv[1],&module,&startup));free(source);
    PcKernel copy,full,prepare,divide;OK(pc_kernel(module,"gk_copy",&copy));OK(pc_kernel(module,"gk_softmax",&full));OK(pc_kernel(module,"gk_softmax_prepare",&prepare));OK(pc_kernel(module,"gk_softmax_divide",&divide));PcStream stream;OK(pc_stream(&stream));PcBuffer input,scores,sinks,den,flags;const int n=64*2048;OK(pc_alloc((size_t)n*4,&input));OK(pc_alloc((size_t)n*4,&scores));OK(pc_alloc(128,&sinks));OK(pc_alloc(64*8,&den));OK(pc_alloc(64,&flags));float *host=malloc((size_t)n*4),*out[2]={malloc((size_t)n*4),malloc((size_t)n*4)};assert(host && out[0] && out[1]);for(int i=0;i<n;i++)host[i]=(float)(i%31-15)*0.25f;OK(pc_upload(input,host,(size_t)n*4,stream));OK(pc_fill(sinks,0,32,stream));PcEvent begin,end;OK(pc_event(&begin));OK(pc_event(&end));
    for(int c=0;c<2;c++){int visible=c?1025:128;PcArg copying[]={V(input,n),V(scores,n)},old[]={I(visible),I(2048),V(sinks,64),V(scores,n),V(flags,64)},prep[]={I(visible),I(2048),V(sinks,64),V(scores,n),V(den,64),V(flags,64)},norm[]={I(visible),I(2048),V(den,64),V(scores,n)};
        for(int repeat=-1;repeat<5;repeat++){double total[2]={0,0};for(int sample=0;sample<10;sample++){for(int order=0;order<2;order++){int v=(sample+order)%2;OK(pc_launch(copy,n,256,copying,2,stream));OK(pc_record(begin,stream));if(v==0){OK(pc_launch(full,64,256,old,5,stream));}else{OK(pc_launch(prepare,64,256,prep,6,stream));OK(pc_launch(divide,64*visible,256,norm,4,stream));}OK(pc_record(end,stream));OK(pc_sync_stream(stream));double seconds;OK(pc_event_seconds(begin,end,&seconds));total[v]+=seconds;OK(pc_download(out[v],scores,(size_t)n*4,stream));OK(pc_sync_stream(stream));}assert(!memcmp(out[0],out[1],(size_t)n*4));}
            if(repeat>=0)for(int v=0;v<2;v++)printf("softmax visible=%d variant=%d repeat=%d milliseconds=%.6f\n",visible,v,repeat,total[v]*100);fflush(stdout);
        }
    }
    puts("all active scores and untouched tails: bitwise equal");pc_destroy_event(begin);pc_destroy_event(end);pc_free(input);pc_free(scores);pc_free(sinks);pc_free(den);pc_free(flags);pc_destroy_stream(stream);pc_unload(module);free(host);free(out[0]);free(out[1]);return 0;
}
