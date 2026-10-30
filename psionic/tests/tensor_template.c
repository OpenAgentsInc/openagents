#include "tensor-template-runtime.c"
#include "tensor-template-oracle.h"
#include <math.h>
#include "cuda-host.h"
#include <stdio.h>
#include <string.h>
#define CHECK(x) do{if(!(x)){fprintf(stderr,"FAIL %d: %s\n",__LINE__,#x);abort();}}while(0)
#define OK(x) do{coax_t_TiStatus z_=(x);if(z_){fprintf(stderr,"status %d at %d: %s; CUDA %s\n",z_,__LINE__,#x,pc_log());abort();}}while(0)
int main(int argc,char **argv){CHECK(argc>=2);int device=atoi(argv[1]);char *source=NULL;size_t bytes=0;if(device){FILE *f=fopen(argv[2],"rb");CHECK(f);fseek(f,0,SEEK_END);bytes=ftell(f);rewind(f);source=malloc(bytes+1);CHECK(fread(source,1,bytes,f)==bytes);fclose(f);source[bytes]=0;}
    coax_t_TiTarget target={.device=device,.budget=67108864};memcpy(target.compiler.e,qualified_compiler,sizeof qualified_compiler);Ptg *p=NULL;coax_t_TgTemplate model=tg_fixture();OK(ptg_open(model,target,source,bytes,&p));target=ptg_target(p);CHECK(ptg_matches(p,model,target));double maximum=0;
    for(size_t i=0;i<sizeof template_cases/sizeof *template_cases;i++){const TemplateOracle *o=&template_cases[i];coax_t_TgBinding b={.symbols={.e={o->batch,0,0,0}},.branch=o->branch,.repeat=o->repeat};float out[19];for(int j=0;j<19;j++)out[j]=-7;
        for(int pass=0;pass<2;pass++){OK(ptg_run(p,b,o->input,o->n,out,19));for(int64_t j=0;j<o->m;j++){double err=fabs(out[j]-o->expected[j])/fmax(1,fabs(o->expected[j]));if(err>maximum)maximum=err;CHECK(err<=0.0001);}CHECK(out[o->m]==-7);}
    }
    PtgStats stats=ptg_stats(p);CHECK(stats.hits==16&&stats.specializations==16&&stats.evictions==12&&stats.module_compiles==(device?1:0));CHECK(stats.cached_bytes<=target.budget);
    coax_t_TgBinding bad={.symbols={.e={9,0,0,0}}};float out[19];for(int i=0;i<19;i++)out[i]=-7;CHECK(ptg_run(p,bad,template_cases[0].input,template_cases[0].n,out,19)==COAX_TiStatus_shape&&out[0]==-7);bad.symbols.e[0]=1;CHECK(ptg_run(p,bad,template_cases[0].input,1,out,19)==COAX_TiStatus_capacity&&out[0]==-7);CHECK(ptg_run(p,bad,template_cases[0].input,template_cases[0].n,(float*)p,19)==COAX_TiStatus_alias);
    model.symbols.e[0].maximum=4;CHECK(!ptg_matches(p,model,target));model=tg_fixture();target.compiler.e[0]^=1;CHECK(!ptg_matches(p,model,target));CHECK(pti_close(&p->owner)==COAX_TiStatus_state);OK(ptg_close(&p));CHECK(!p);
    printf("template backend %d: 16 variants, 16 hits, 12 evictions, %d module compilations, max error %.9g\n",device,device?1:0,maximum);free(source);return 0;}
