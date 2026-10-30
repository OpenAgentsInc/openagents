/* Opaque runtime plus deliberate private-address alias/fault-boundary probes. */
#include "tensor-ir-runtime.c"
#include <math.h>
#include "tensor-ir-oracle.h"
static void diagnostics(void){
#ifdef __APPLE__
    fprintf(stderr,"Metal: %s\n",pm_log());
#endif
}
#define CHECK(x) do{if(!(x)){fprintf(stderr,"FAIL %d: %s\n",__LINE__,#x);abort();}}while(0)
#define OK(x) do{coax_t_TiStatus z_=(x);if(z_){fprintf(stderr,"status %d at %d: %s; CUDA %s\n",z_,__LINE__,#x,pc_log());diagnostics();abort();}}while(0)
static coax_t_TiTarget target(int device){coax_t_TiTarget t={.device=(uint8_t)device,.budget=67108864};memcpy(t.compiler.e,qualified_compiler,sizeof qualified_compiler);return t;}
static double maximum;
static void compare(const float *out,const float *expected,int64_t n){for(int64_t i=0;i<n;i++){double e=fabs((double)out[i]-expected[i])/fmax(1,fabs(expected[i]));if(e>maximum)maximum=e;CHECK(e<=0.0001);}}
static void execute_cases(int device,const char *module,size_t bytes){
    const int cases=(int)(sizeof oracles/sizeof *oracles);
    for(int which=0;which<cases;which++){
        const TiOracle *o=&oracles[which];coax_t_TiProgram graph=oracle_program(which);Pti *reference=NULL,*planned=NULL;
        OK(pti_open(graph,target(0),true,NULL,0,&reference));OK(pti_open(graph,target(device),false,module,bytes,&planned));coax_t_TiPlan info=pti_info(planned);
        CHECK(info.status==TI(ok)&&info.output_elements==o->outputs);CHECK(pti_matches(planned,graph,info.target));
        float *cpu=malloc((size_t)(o->outputs+3)*4),*actual=malloc((size_t)(o->outputs+3)*4);CHECK(cpu&&actual);
        for(int64_t i=0;i<o->outputs+3;i++)cpu[i]=actual[i]=-7;
        OK(pti_run(reference,o->input,o->inputs,cpu,o->outputs+3));compare(cpu,o->expected,o->outputs);
        uint64_t allocations=pti_stats(planned).allocations;CHECK(allocations==5&&pti_stats(planned).buffer_bytes==info.bytes);
        for(int repeat=0;repeat<3;repeat++){OK(pti_run(planned,o->input,o->inputs,actual,o->outputs+3));compare(actual,o->expected,o->outputs);compare(actual,cpu,o->outputs);CHECK(pti_stats(planned).allocations==allocations&&actual[o->outputs]==-7);}
        CHECK(pti_stats(planned).runs==3);if(which==0){CHECK(info.workspace==16&&info.naive==32);printf("feed-forward measured workspace %lld bytes (fresh %lld); five buffers %llu bytes\n",(long long)pti_stats(planned).workspace_bytes,(long long)info.naive*4,(unsigned long long)pti_stats(planned).buffer_bytes);}
        if(which==cases-1)printf("larger block: %lld scalar work, %lld workspace floats, %lld fresh floats\n",(long long)info.work,(long long)info.workspace,(long long)info.naive);
        // Exact descriptor identity invalidates rather than accepting a stale plan.
        coax_t_TiTarget key=info.target;key.compiler.e[1]^=1;CHECK(!pti_matches(planned,graph,key));key=info.target;key.backend.e[2]^=1;CHECK(!pti_matches(planned,graph,key));key=info.target;key.kernel.e[0]^=1;CHECK(!pti_matches(planned,graph,key));key=info.target;key.device=(device+1)%3;CHECK(!pti_matches(planned,graph,key));
        graph.nodes.e[0].shape.dims.e[0]++;CHECK(!pti_matches(planned,graph,info.target));graph=oracle_program(which);graph.nodes.e[0].strides.e[0]++;CHECK(!pti_matches(planned,graph,info.target));graph=oracle_program(which);graph.nodes.e[0].dtype=1;CHECK(!pti_matches(planned,graph,info.target));
        OK(pti_close(&reference));OK(pti_close(&planned));CHECK(!reference&&!planned);free(cpu);free(actual);
    }
    printf("%d independent graph cases, three planned replays each\n",cases);
}
static void failures(int device,const char *module,size_t bytes){
    coax_t_TiProgram graph=oracle_program(0);Pti *p=NULL;coax_t_TiTarget t=target(device);t.budget=1;CHECK(pti_open(graph,t,false,module,bytes,&p)==TI(budget)&&!p);t=target(device);
    graph.nodes.e[5].a=5;CHECK(pti_open(graph,t,false,module,bytes,&p)==TI(dependency)&&!p);graph=oracle_program(0);graph.nodes.e[0].dtype=1;CHECK(pti_open(graph,t,false,module,bytes,&p)==TI(dtype));
    graph=oracle_program(0);OK(pti_open(graph,t,false,module,bytes,&p));Pti *saved=p;CHECK(pti_open(graph,t,false,module,bytes,&p)==TI(state)&&p==saved);
    float input[32];memcpy(input,oracles[0].input,sizeof input);float out[7]={-7,-7,-7,-7,-7,-7,-7};
    CHECK(pti_run(p,input,31,out,7)==TI(capacity)&&out[0]==-7);CHECK(pti_run(p,input,32,out,3)==TI(capacity)&&out[0]==-7);
    CHECK(pti_run(p,input,32,input,32)==TI(alias));CHECK(pti_run(p,input,32,(float*)p,7)==TI(alias));CHECK(pti_run(p,input,32,(float*)p->metadata,7)==TI(alias));
    if(ti_pointer(p,p->workspace))CHECK(pti_run(p,input,32,(float*)ti_pointer(p,p->workspace),7)==TI(alias));
    uint32_t nonfinite[]={0x7f800000,0xff800000,0x7f800001,0x7fc00001};for(int i=0;i<4;i++){memcpy(input,&nonfinite[i],4);CHECK(pti_run(p,input,32,out,7)==TI(number)&&out[0]==-7);}memcpy(input,oracles[0].input,sizeof input);CHECK(pti_stats(p).runs==0);
    // Numerical failure does not publish, poison or allocate a replacement arena.
    graph=(coax_t_TiProgram){.count=3,.output=2};graph.nodes.e[0]=ti_input((coax_t_TvShape){0},0);graph.nodes.e[1]=ti_input((coax_t_TvShape){0},1);graph.nodes.e[2]=(coax_t_TiNode){.op=COAX_TiOp_multiply,.a=0,.b=1};Pti *arith=NULL;OK(pti_open(graph,t,false,module,bytes,&arith));float huge[2];uint32_t largest=0x7f7fffff;memcpy(huge,&largest,4);huge[1]=2;CHECK(pti_run(arith,huge,2,out,7)==TI(arithmetic)&&out[0]==-7);huge[0]=3;OK(pti_run(arith,huge,2,out,7));CHECK(out[0]==6&&pti_stats(arith).allocations==5);OK(pti_close(&arith));
    // Simulated effect failure tests the host transition; no real driver fault injected.
    p->life=ti_end(ti_begin(p->life).state,TI(driver));CHECK(pti_run(p,input,32,out,7)==TI(driver)&&out[0]==6);CHECK(!pti_matches(p,p->plan.program,p->plan.target));OK(pti_close(&p));
}
int main(int argc,char **argv){CHECK(argc>=2);int device=atoi(argv[1]);char *module=NULL;size_t bytes=0;if(device){CHECK(argc==3);FILE *file=fopen(argv[2],"rb");CHECK(file);fseek(file,0,SEEK_END);bytes=(size_t)ftell(file);rewind(file);module=malloc(bytes+1);CHECK(module&&fread(module,1,bytes,file)==bytes);module[bytes]=0;fclose(file);}
    execute_cases(device,module,bytes);failures(device,module,bytes);printf("tensor IR backend %d: native graphs, lifetimes, cache and refusals pass; max scaled error %.9g\n",device,maximum);free(module);return 0;}
