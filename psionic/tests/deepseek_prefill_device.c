/* Coax-generated CUDA products against identical Coax C arithmetic.
 * Host effects and independent fixture preparation only; no shipped kernels. */
#include "cuda-host.h"
#include "kernels.h"
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#define TRY(x) do{PcStatus code_=(x);if(code_){fprintf(stderr,"%s: %s\n",#x,pc_log());exit(1);}}while(0)
#define B(p,n) ((coax_v_u8){(uint8_t *)(p),(n)})
#define F(p,n) ((coax_v_f32){(float *)(p),(n)})
#define D(p,n) ((coax_v_f64){(double *)(p),(n)})
#define ARG_I(n) ((PcArg){.kind=PC_ARG_INT,.i=(n)})
#define ARG_V(p,n) ((PcArg){.kind=PC_ARG_VIEW,.data=(p),.count=(n)})
static PcKernel product,reduce,publish;
static void run(int kind,int64_t rows,int64_t columns,int64_t batch,const uint8_t *source,const uint8_t *scale_source,int fail){
    int64_t codes=rows*columns/(kind?1:2),scales=(kind?(rows+31)/32:rows)*(columns/32),xn=batch*columns,on=batch*rows;
    uint8_t *w=malloc((size_t)codes),*s=malloc((size_t)scales),*cpu_flags=calloc((size_t)on,1),*gpu_flags=malloc((size_t)on);double *x=malloc((size_t)xn*8);float *cpu=malloc((size_t)on*4),*gpu=malloc((size_t)on*4),*sentinel=malloc((size_t)on*4);assert(w&&s&&cpu_flags&&gpu_flags&&x&&cpu&&gpu&&sentinel);
    for(int64_t i=0;i<codes;i++)w[i]=source?source[i]:(uint8_t)(kind?((i*13)%127)+((i%3==0)?128:0):(i*37+5)%256);
    for(int64_t i=0;i<scales;i++)s[i]=scale_source?scale_source[i]:(uint8_t)(120+i%9);
    for(int64_t i=0;i<xn;i++)x[i]=(double)((i*17+i/columns*3)%31-15)*.125;
    if(fail==1)s[scales-1]=255;
    if(fail==2){for(int64_t i=0;i<codes;i++)w[i]=kind?0x7e:0x77;for(int64_t i=0;i<scales;i++)s[i]=254;for(int64_t i=0;i<xn;i++)x[i]=3.4028234663852886e38;}
    for(int64_t i=0;i<on;i++)cpu[i]=gpu[i]=sentinel[i]=77;
    for(int64_t item=0;item<rows*((batch+7)/8);item++)dpkg_project(item,kind,rows,columns,batch,B(w,codes),B(s,scales),D(x,xn),F(cpu,on),B(cpu_flags,on));
    PcBuffer dw,ds,dx,stage,flags,summary,out;TRY(pc_alloc((size_t)codes,&dw));TRY(pc_alloc((size_t)scales,&ds));TRY(pc_alloc((size_t)xn*8,&dx));TRY(pc_alloc((size_t)on*4,&stage));TRY(pc_alloc((size_t)on,&flags));TRY(pc_alloc(1,&summary));TRY(pc_alloc((size_t)on*4,&out));
    TRY(pc_upload(dw,w,(size_t)codes,NULL));TRY(pc_upload(ds,s,(size_t)scales,NULL));TRY(pc_upload(dx,x,(size_t)xn*8,NULL));TRY(pc_upload(stage,sentinel,(size_t)on*4,NULL));TRY(pc_upload(out,sentinel,(size_t)on*4,NULL));
    PcArg args[]={ARG_I(kind),ARG_I(rows),ARG_I(columns),ARG_I(batch),ARG_V(dw,codes),ARG_V(ds,scales),ARG_V(dx,xn),ARG_V(stage,on),ARG_V(flags,on)};
    TRY(pc_launch(product,rows*((batch+7)/8),128,args,9,NULL));PcArg ra[]={ARG_V(flags,on),ARG_V(summary,1)};TRY(pc_launch(reduce,1,1,ra,2,NULL));PcArg pa[]={ARG_V(summary,1),ARG_V(stage,on),ARG_V(out,on)};TRY(pc_launch(publish,on,128,pa,3,NULL));TRY(pc_download(gpu,out,(size_t)on*4,NULL));TRY(pc_download(gpu_flags,flags,(size_t)on,NULL));TRY(pc_sync());
    assert(!memcmp(cpu_flags,gpu_flags,(size_t)on));if(fail)assert(!memcmp(gpu,sentinel,(size_t)on*4));else if(memcmp(cpu,gpu,(size_t)on*4)){for(int64_t i=0;i<on;i++)if(memcmp(cpu+i,gpu+i,4)){fprintf(stderr,"mismatch %lld: %.9g %.9g\n",(long long)i,cpu[i],gpu[i]);break;}abort();}
    printf("CUDA grouped kind %d, %lldx%lld, batch %lld, failure %d: %lld exact output/flag words; publication atomic\n",kind,(long long)rows,(long long)columns,(long long)batch,fail,(long long)on);
    TRY(pc_free(dw));TRY(pc_free(ds));TRY(pc_free(dx));TRY(pc_free(stage));TRY(pc_free(flags));TRY(pc_free(summary));TRY(pc_free(out));free(w);free(s);free(cpu_flags);free(gpu_flags);free(x);free(cpu);free(gpu);free(sentinel);
}
int main(int argc,char **argv){
    assert(argc==2||argc==3);FILE *f=fopen(argv[1],"rb");assert(f);fseek(f,0,SEEK_END);long bytes=ftell(f);rewind(f);char *src=malloc((size_t)bytes);assert(src&&fread(src,1,(size_t)bytes,f)==(size_t)bytes);fclose(f);PcDeviceInfo info;TRY(pc_open(&info));printf("CUDA: %s, driver %d, %zu free bytes\n",info.name,info.driver_version,info.free_bytes);PcModule module;double compile_seconds;TRY(pc_compile(src,(size_t)bytes,"deepseek-prefill.cu",&module,&compile_seconds));free(src);TRY(pc_kernel(module,"dpkg_project",&product));TRY(pc_kernel(module,"dpkg_flags",&reduce));TRY(pc_kernel(module,"dpkg_publish",&publish));
    for(int kind=0;kind<2;kind++){run(kind,64,96,1,NULL,NULL,0);run(kind,64,96,9,NULL,NULL,0);run(kind,64,96,17,NULL,NULL,1);run(kind,32,32,2,NULL,NULL,2);}
    if(argc==3){FILE *p=fopen(argv[2],"rb");assert(p);uint8_t *packet=malloc(18800640);assert(packet&&fread(packet,1,18800640,p)==18800640&&fgetc(p)==EOF);fclose(p);for(int matrix=0;matrix<3;matrix++){int64_t rows=matrix==2?5120:2304,cols=matrix==2?2304:5120;const uint8_t *data=packet+matrix*6266880,*scales=data+5898240;run(0,rows,cols,9,data,scales,0);}free(packet);}
    TRY(pc_unload(module));printf("CUDA grouped: generated source compiled in %.6f s; all requested exact checks passed\n",compile_seconds);return 0;
}
