#include "metal-host.h"
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#define T(x) do{int status_=(x);if(status_){fprintf(stderr,"%d %s\n",status_,pm_log());return 1;}}while(0)
static PmArg I(long long x){return(PmArg){.kind=PM_ARG_INT,.i=x};}static PmArg V(PmBuffer b,int n){return(PmArg){.kind=PM_ARG_VIEW,.buffer=b,.count=n};}
int main(int argc,char**argv){PmDeviceInfo i;T(pm_open(&i));FILE*f=fopen(argv[1],"rb");fseek(f,0,SEEK_END);long n=ftell(f);rewind(f);char*s=malloc(n);fread(s,1,n,f);fclose(f);PmModule m;T(pm_compile(s,n,"time-probe",&m,0));PmKernel k;T(pm_kernel(m,"gak_time",&k));f=fopen(argv[2],"wb");for(int step=0;step<4;step++)for(int kind=0;kind<2;kind++){int w=kind?256:1536;PmBuffer out,flags;T(pm_alloc(w*4,&out));T(pm_alloc(w,&flags));memset(pm_contents(flags),9,w);PmArg a[]={I(kind),I(step),V(out,w),V(flags,w)};PmBatch b;T(pm_begin(&b));T(pm_launch(b,k,w,1,0,a,4));T(pm_wait(b,0));for(int j=0;j<w;j++)if(((unsigned char*)pm_contents(flags))[j])return 1;fwrite(pm_contents(out),4,w,f);T(pm_free(out));T(pm_free(flags));}fclose(f);T(pm_kernel_free(k));T(pm_unload(m));return 0;}
