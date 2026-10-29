/* Opt-in A/B harness: one authenticated model, two Coax-generated CUDA
   modules, alternating trials. Private session access is test-only so the
   production ABI needs no runtime backend-switch or benchmark policy. An
   optional baseline shared Coax translation preserves the actual old launch
   schedule when a candidate changes scheduling; C only chooses the test variant. */
#define _POSIX_C_SOURCE 200809L
#include "gptoss-device.h"
#include <dlfcn.h>
typedef coax_t_GDProgram (*SpeedProgram)(coax_t_GShape,coax_t_GDPlan,coax_v_t_GDParam,int64_t,int64_t,_Bool,int64_t,coax_v_t_GDOp,coax_v_t_GDArg);
static SpeedProgram speed_program;
static coax_t_GDProgram speed_dispatch(coax_t_GShape shape,coax_t_GDPlan plan,coax_v_t_GDParam params,int64_t position,int64_t token,_Bool choose,int64_t limit,coax_v_t_GDOp ops,coax_v_t_GDArg args){return speed_program(shape,plan,params,position,token,choose,limit,ops,args);}
#define gd_program speed_dispatch
#include "gptoss-device.c"
#undef gd_program
#include <assert.h>
#include <fcntl.h>
#include <sys/mman.h>
#include <sys/stat.h>
#include <time.h>
#include <unistd.h>
#define OK(call) do{PcStatus z=(call);if(z){fprintf(stderr,"%s: %d %s\n",#call,z,pc_log());exit(2);}}while(0)
static double now(void){struct timespec t;clock_gettime(CLOCK_MONOTONIC,&t);return t.tv_sec+t.tv_nsec*1e-9;}
static coax_v_u8 bv(void *p,int64_t n){return(coax_v_u8){p,n};}
static coax_v_s64 iv(int64_t *p,int64_t n){return(coax_v_s64){p,n};}
static void *map(const char *path,size_t *n){int fd=open(path,O_RDONLY);struct stat st;assert(fd>=0 && !fstat(fd,&st) && st.st_size>0);*n=(size_t)st.st_size;void *p=mmap(NULL,*n,PROT_READ,MAP_PRIVATE,fd,0);close(fd);assert(p!=MAP_FAILED);return p;}
static coax_string text(const char *s){return(coax_string){s,(int64_t)strlen(s)};}
int main(int argc,char **argv){
    assert(argc==5 || (argc==6 && !strcmp(argv[5],"--profile")) || (argc==7 && !strcmp(argv[5],"--baseline-plan")));
    SpeedProgram programs[2]={gd_program,gd_program};void *baseline_library=NULL;
    if(argc==7){baseline_library=dlopen(argv[6],RTLD_NOW|RTLD_LOCAL);assert(baseline_library);*(void**)&programs[0]=dlsym(baseline_library,"gd_program");assert(programs[0]);}speed_program=programs[0];size_t bytes[3],sizes[2];void *maps[3];const char *sources[2]={map(argv[2],&sizes[0]),map(argv[3],&sizes[1])};coax_t_GArtifact pins[10];coax_t_GSpec specs[543];coax_t_GProfile profile=g_profile(0,2048,(coax_v_t_GArtifact){pins,10},(coax_v_t_GSpec){specs,543});assert(!profile.status);coax_t_GFile files[3];
    for(int i=0;i<3;i++){char path[4096];assert(snprintf(path,sizeof path,"%s/%.*s",argv[1],(int)pins[i].name.count,pins[i].name.data)>0);maps[i]=map(path,&bytes[i]);files[i]=(coax_t_GFile){pins[i].name,bv(maps[i],(int64_t)bytes[i])};}
    PgSession *s=NULL;coax_t_GResult why;double start=now();OK(pg_open(profile,(coax_v_t_GFile){files,3},sources[0],sizes[0],17179869184,&s,&why));assert(!why.status && s);printf("authenticated once: %.3f seconds\n",now()-start);fflush(stdout);
#ifdef PSIONIC_DEVICE_TEST
    if(argc==6){int64_t ids[1024],control[8];for(int i=0;i<1024;i++)ids[i]=1000+(i*7919)%190000;OK(pg_prefill(s,ids,1024,0,0,128,control));assert(!control[0]);pg_profile(s,1);OK(pg_step(s,ids[0],1024,1,512,1,control));assert(!control[0]);pg_profile(s,0);pg_close(s);return 0;}
#endif
    PcModule modules[2]={s->module,NULL};PcKernel *kernels[2]={s->kernels,calloc((size_t)gd_kernel_count(),sizeof(PcKernel))};assert(kernels[1]);double compilation;OK(pc_compile(sources[1],sizes[1],"candidate.cu",&modules[1],&compilation));
    for(int64_t i=0;i<gd_kernel_count();i++){coax_string name=gd_kernel_name(i);char label[128];assert(name.count>0 && name.count<127);memcpy(label,name.data,(size_t)name.count);label[name.count]=0;OK(pc_kernel(modules[1],label,&kernels[1][i]));}
    size_t cache_bytes=(size_t)s->plan.counts.e[16]*4,logit_bytes=(size_t)s->shape.vocab*4,trace_bytes=(size_t)s->plan.counts.e[19]*4;unsigned char *cache[2]={malloc(cache_bytes),malloc(cache_bytes)},*logits[2]={malloc(logit_bytes),malloc(logit_bytes)},*trace[2]={malloc(trace_bytes),malloc(trace_bytes)};int64_t route[2][96],control[8],corpus[1024];for(int i=0;i<1024;i++)corpus[i]=1000+(i*7919)%190000;
    for(int repeat=-1;repeat<5;repeat++){for(int order=0;order<2;order++){int v=(repeat+order+2)%2;s->module=modules[v];s->kernels=kernels[v];speed_program=programs[v];OK(pg_reset(s));start=now();OK(pg_prefill(s,corpus,1024,0,0,128,control));assert(!control[0]);double pref=now()-start;start=now();for(int i=0;i<128;i++)OK(pg_step(s,corpus[i],1024+i,1,512,1,control));assert(!control[0]);double dec=now()-start;
            OK(pg_read(s,16,cache[v],cache_bytes));OK(pg_read(s,18,logits[v],logit_bytes));OK(pg_read(s,19,trace[v],trace_bytes));OK(pg_read(s,20,route[v],sizeof route[v]));printf("paired repeat=%d variant=%d input=1024 forced_output=128 prefill=%.6f decode=%.6f tok_s=%.3f\n",repeat,v,pref,dec,128/dec);fflush(stdout);
        }assert(!memcmp(cache[0],cache[1],cache_bytes) && !memcmp(logits[0],logits[1],logit_bytes) && !memcmp(trace[0],trace[1],trace_bytes) && !memcmp(route[0],route[1],sizeof route[0]));}
    puts("all paired full caches, logits, traces and expert IDs: bitwise equal");
    size_t nt;void *table_data=map(argv[4],&nt);coax_v_u8 table=bv(table_data,(int64_t)nt);assert(hs_admit(table));uint8_t sys[1024],sys_work[1024];coax_t_PtResult system=hm_system_checked(0,bv(sys,1024),bv(sys_work,1024),0);assert(!system.status);coax_t_HmMessage messages[2]={{.role=COAX_HmRole_system,.body={(const char*)sys,system.written}},{.role=COAX_HmRole_user,.body=text("List the integers from 1 to 100, separated by commas.")}};int64_t prefix[16384],stage[16384],symbols[1024],generated[2][128],count[2];coax_t_HmResult p=hm_render_checked(table,(coax_v_t_HmMessage){messages,2},1,iv(prefix,16384),iv(stage,16384),iv(symbols,1024),0);assert(!p.status);
    for(int repeat=-1;repeat<5;repeat++){for(int order=0;order<2;order++){int v=(repeat+order+2)%2;s->module=modules[v];s->kernels=kernels[v];speed_program=programs[v];OK(pg_reset(s));start=now();OK(pg_prefill(s,prefix,p.written,0,1,128,control));assert(!control[0]);double pref=now()-start;count[v]=0;generated[v][count[v]++]=control[1];start=now();while(!control[4]){OK(pg_step(s,-1,control[2],1,128,1,control));assert(!control[0] && count[v]<128);generated[v][count[v]++]=control[1];}double dec=now()-start;printf("greedy repeat=%d variant=%d input=%lld output=%lld prefill=%.6f decode=%.6f tok_s=%.3f\n",repeat,v,(long long)p.written,(long long)count[v],pref,dec,(count[v]-1)/dec);fflush(stdout);
        }assert(count[0]==count[1] && !memcmp(generated[0],generated[1],(size_t)count[0]*8));}
    puts("all greedy generated IDs and stops: bitwise equal");s->module=modules[0];s->kernels=kernels[0];pg_close(s);pc_unload(modules[1]);free(kernels[1]);for(int v=0;v<2;v++){free(cache[v]);free(logits[v]);free(trace[v]);munmap((void*)sources[v],sizes[v]);}for(int i=0;i<3;i++)munmap(maps[i],bytes[i]);munmap(table_data,nt);if(baseline_library)dlclose(baseline_library);return 0;
}
