/* Effects only: Coax chooses symbols, variants, cache victims and lifetimes. */
#include "tensor-template-runtime.h"
#include "spans.h"
#include <stdlib.h>
struct Ptg {coax_t_TgTemplate model;coax_t_TgCache cache;coax_t_TiLife life;Pti *owner,*slots[4];PtgStats stats;};
#define TG(name) COAX_TiStatus_##name
coax_t_TiStatus ptg_open(coax_t_TgTemplate model,coax_t_TiTarget target,const char *source,size_t bytes,Ptg **out){
    if(!out || *out)return TG(state);
    coax_t_TiStatus z=tg_validate(model,target);if(z)return z;
    Ptg *p=calloc(1,sizeof *p);if(!p)return TG(capacity);p->model=model;p->life=ti_initial_life();
    coax_t_TiProgram keeper={.count=1};keeper.nodes.e[0]=ti_input((coax_t_TvShape){0},0);
    z=pti_open(keeper,target,false,source,bytes,&p->owner);if(z){free(p);return z;}p->cache=tg_cache(pti_info(p->owner).bytes,target.budget);p->stats.module_compiles=pti_stats(p->owner).module_compiles;*out=p;return TG(ok);
}
PtgStats ptg_stats(const Ptg *p){PtgStats s=p?p->stats:(PtgStats){0};if(p)s.cached_bytes=p->cache.bytes;return s;}
coax_t_TiTarget ptg_target(const Ptg *p){return pti_info(p?p->owner:NULL).target;}
bool ptg_matches(const Ptg *p,coax_t_TgTemplate model,coax_t_TiTarget target){return p && ti_cache_ready(p->life) && tg_template_equal(p->model,model) && pti_matches(p->owner,pti_info(p->owner).program,target);}
coax_t_TiStatus ptg_run(Ptg *p,coax_t_TgBinding binding,const float *input,int64_t count,float *out,int64_t capacity){
    if(!p)return TG(state);
    coax_t_TgBound bound=tg_bind(p->model,binding);if(bound.status)return bound.status;
    int64_t found=tg_find(p->cache,binding);coax_t_TiPlan plan=found<0?ti_compile(bound.program,ptg_target(p),true):pti_info(p->slots[found]);
    bool overlap=psionic_spans_overlap(out,capacity,sizeof(float),p,1,sizeof *p) || psionic_spans_overlap(input,count,sizeof(float),p,1,sizeof *p);
    coax_t_TiStatus z=ti_inputs(plan,(coax_v_f32){(float*)input,count},capacity,overlap);if(z)return z;
    coax_t_TiLifeResult begin=ti_begin(p->life);if(begin.status)return begin.status;p->life=begin.state;
    coax_t_TgSelect selected=tg_select(p->cache,binding,plan.bytes);if(selected.status){p->life=ti_end(p->life,selected.status);return selected.status;}
    if(selected.hit){p->stats.hits++;p->cache=selected.next;}
    else {
        for(int i=0;i<4;i++)if(selected.evict&(1u<<i)){z=pti_close(&p->slots[i]);if(z){p->life=ti_end(p->life,z);return z;}p->stats.evictions++;}
        p->cache=selected.next;z=pti_derive(p->owner,bound.program,&p->slots[selected.slot]);if(z){p->life=ti_end(p->life,z);return z;}
        p->cache=tg_install(p->cache,selected.slot,binding,plan.bytes);p->stats.specializations++;
    }
    z=pti_run(p->slots[selected.slot],input,count,out,capacity);p->life=ti_end(p->life,z);return z;
}
coax_t_TiStatus ptg_close(Ptg **out){if(!out || !*out)return TG(state);Ptg *p=*out;coax_t_TiStatus z=ti_close_admit(p->life);if(z)return z;for(int i=0;i<4;i++)if(p->slots[i]){z=pti_close(&p->slots[i]);if(z)return z;}z=pti_close(&p->owner);if(z)return z;free(p);*out=NULL;return TG(ok);}
