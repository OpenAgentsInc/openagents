/* Actual generated cooperative kernels on CUDA. All arithmetic under test
   is Coax; reference() from the independent native test is an oracle only.
   This harness allocates/copies/launches and observes guards and exact words.
   No optimizer, resident step, throughput or training quality is measured. */
#define main gd_cpu_oracle_main
#include "training-dense-native.c"
#undef main
#include "cuda-host.h"
#define TRY(x) do {PcStatus s_=(x);if(s_!=PC_OK){fprintf(stderr,"%s: %s\n",#x,pc_log());exit(1);}}while(0)
static PcKernel kernels[3];
static const char *names[]={"gdk_forward","gdk_dx","gdk_dw"};
static PcArg integer(int64_t x){return(PcArg){PC_ARG_INT,x,0,0,0};}
static PcArg device_view(PcBuffer p,int64_t n){return(PcArg){PC_ARG_VIEW,0,0,p+4,n};}
static void device_fixture(int m,int n,int k,int rounded){
    int64_t count[]={m*k,k*n,m*n,m*n,m*k,k*n};Buffer host[6];PcBuffer dev[6];
    for(int i=0;i<6;i++){host[i]=buffer(count[i]);TRY(pc_alloc((size_t)(count[i]+2)*4,dev+i));}
    for(int i=0;i<3;i++)for(int64_t j=0;j<count[i];j++)host[i].v[j]=rounded?(float)(j%7-3)*0.1f:datum(j,1+4*i);
    coax_t_GdPlan plan=gd_plan(m,n,k,count[0],count[1],count[2],count[3],count[4],count[5],0);REQUIRE(plan.status==COAX_GdStatus_ok);
    for(int i=0;i<3;i++)REQUIRE(gd_numbers(view(host[i])));
    for(int i=0;i<6;i++)TRY(pc_upload(dev[i],host[i].allocation,(size_t)(count[i]+2)*4,NULL));
    int left[]={0,2,0},right[]={1,1,2};int64_t groups[]={plan.forward,plan.dx,plan.dw};
    for(int i=0;i<3;i++){
        PcArg args[]={integer(m),integer(n),integer(k),device_view(dev[left[i]],count[left[i]]),device_view(dev[right[i]],count[right[i]]),device_view(dev[3+i],count[3+i])};
        TRY(pc_launch_warps(kernels[i],groups[i]*4,128,args,6,NULL));
    }
    TRY(pc_sync());
    float *expected[3];for(int i=0;i<3;i++){expected[i]=malloc((size_t)count[3+i]*4);REQUIRE(expected[i]);}
    reference(m,n,k,host[0].v,host[1].v,host[2].v,expected[0],expected[1],expected[2]);
    for(int i=0;i<6;i++){
        Buffer got=buffer(count[i]);TRY(pc_download(got.allocation,dev[i],(size_t)(count[i]+2)*4,NULL));guards(got);
        for(int64_t j=0;j<count[i];j++)REQUIRE(same(got.v[j],i<3?host[i].v[j]:expected[i-3][j]));
        release(got);release(host[i]);TRY(pc_free(dev[i]));
    }
    for(int i=0;i<3;i++)free(expected[i]);
    printf("CUDA exact forward+dX+dW %dx%dx%d%s; all guards and input words unchanged\n",m,n,k,rounded?" rounded-products":"");
}
int main(int argc,char **argv){
    REQUIRE(argc==2);PcDeviceInfo info;TRY(pc_open(&info));REQUIRE(gdt_admit(info.major,info.minor,info.max_threads,info.max_shared)==0);
    printf("device: %s sm_%d%d; max threads %d shared %d; Coax capability admission passed\n",info.name,info.major,info.minor,info.max_threads,info.max_shared);
    FILE *f=fopen(argv[1],"rb");REQUIRE(f);REQUIRE(fseek(f,0,SEEK_END)==0);long size=ftell(f);REQUIRE(size>0);rewind(f);char *source=malloc((size_t)size+1);REQUIRE(source);REQUIRE(fread(source,1,(size_t)size,f)==(size_t)size);source[size]=0;fclose(f);
    PcModule module;double seconds;TRY(pc_compile(source,(size_t)size,"training-dense.cu",&module,&seconds));free(source);printf("NVRTC compile seconds: %.9g\n",seconds);
    for(int i=0;i<3;i++)TRY(pc_kernel(module,names[i],kernels+i));
    const int shapes[][3]={{1,1,1},{31,33,7},{32,32,32},{33,31,35},{37,45,70},{128,1,128},{64,65,64}};
    for(size_t i=0;i<sizeof shapes/sizeof *shapes;i++)device_fixture(shapes[i][0],shapes[i][1],shapes[i][2],0);
    device_fixture(33,35,34,1);TRY(pc_unload(module));
    puts("CUDA training dense: PASS8fixtures; no resident step, training, throughput or BF16 qualification");return 0;
}
