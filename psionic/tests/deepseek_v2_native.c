/* I/O, address observations and assertions only; model logic is Coax. */
#include "deepseek-v2-memory.h"
#include "v2-tiny-goldens.h"
#include "v2-wide-goldens.h"
#include <assert.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#define LEN(a) ((int64_t)(sizeof(a)/sizeof((a)[0])))
#define V(a) {a,LEN(a)}
static coax_string str(const char *s) {return (coax_string){s,(int64_t)strlen(s)};}
static void *allocate(int64_t n,size_t size) {void *p=calloc((size_t)n,size);assert(p || n==0);return p;}
static uint8_t *read_file(const char *path,int64_t *size) {
    FILE *f=fopen(path,"rb");assert(f);assert(fseek(f,0,SEEK_END)==0);long n=ftell(f);assert(n>0);rewind(f);
    uint8_t *data=allocate(n,1);assert(fread(data,1,(size_t)n,f)==(size_t)n);fclose(f);*size=n;return data;
}
static void model(_Bool wide,const char *path) {
    int64_t bytes=0;uint8_t *data=read_file(path,&bytes);
    coax_t_DsArtifact pins[1];coax_t_DvTensor tensors[128];int64_t payload[1];
    coax_t_DvProfile profile=dv_test_profile(wide,(coax_v_t_DsArtifact)V(pins),(coax_v_t_DvTensor)V(tensors),(coax_v_s64)V(payload));
    coax_t_DvShape s=profile.shape;int64_t count=(s.layers-s.dense_layers)*s.experts;
    coax_t_DvBlock *blocks=allocate(s.layers,sizeof(*blocks)),*draft=allocate(s.layers,sizeof(*draft));
    coax_t_DvExpert *experts=allocate(count,sizeof(*experts)),*de=allocate(count,sizeof(*de));
    coax_t_DsFile file={.name=str("model.safetensors"),.data={data,bytes}};coax_t_DvModel m={0};
    coax_t_DsResult bound=psionic_dv_bind(profile,(coax_v_t_DsFile){&file,1},(coax_v_t_DvBlock){blocks,s.layers},
        (coax_v_t_DvExpert){experts,count},(coax_v_t_DvBlock){draft,s.layers},(coax_v_t_DvExpert){de,count},(coax_v_t_DvModel){&m,1});
    assert(bound.status==COAX_DsStatus_ok && bound.written==s.layers);
    /* The returned model must borrow public metadata, never temporary drafts. */
    memset(draft,0,(size_t)s.layers*sizeof(*draft));memset(de,0,(size_t)count*sizeof(*de));
    const int64_t *tokens=wide?dvw_tokens:dvt_tokens;int64_t traces=2*s.layers*s.width;
    for (uint8_t mode=0;mode<2;mode++) {
        coax_t_DvPlan p=dv_plan(s,mode);assert(p.status==COAX_DsStatus_ok);
        float *cache=allocate(p.cache+1,sizeof(float)),*logits=allocate(s.vocab+1,sizeof(float)),*trace=allocate(traces+1,sizeof(float));
        coax_t_DrRoute *routes=allocate(s.layers+1,sizeof(*routes)),*route_work=allocate(s.layers,sizeof(*route_work));
        float *floats=allocate(p.floats,sizeof(float));double *scores=allocate(p.scores,sizeof(double));coax_t_PsionicAccumulator *acc=allocate(p.accumulators,sizeof(*acc));
        coax_t_DvOutput out={.logits={logits,s.vocab+1},.trace={trace,traces+1},.routes={routes,s.layers+1}};
        coax_t_DvWork work={.floats={floats,p.floats},.scores={scores,p.scores},.accumulators={acc,p.accumulators},.routes={route_work,s.layers}};
        cache[p.cache]=-13;logits[s.vocab]=-7;trace[traces]=-9;routes[s.layers].count=-11;
        for (int64_t t=0;t<5;t++) {
            const float *ref_logits=wide?(mode?dvw_expanded_logits[t]:dvw_compressed_logits[t]):(mode?dvt_expanded_logits[t]:dvt_compressed_logits[t]);
            const float *ref_trace=wide?(mode?dvw_expanded_trace[t]:dvw_compressed_trace[t]):(mode?dvt_expanded_trace[t]:dvt_compressed_trace[t]);
            const float *ref_cache=wide?(mode?dvw_expanded_cache[t]:dvw_compressed_cache[t]):(mode?dvt_expanded_cache[t]:dvt_compressed_cache[t]);
            for (int64_t i=0;i<s.layers;i++) {routes[i].count=19;route_work[i].count=17;}
            coax_t_DsResult r=psionic_dv_step(m,mode,tokens[t],t,(coax_v_f32){cache,p.cache+1},out,work);
            assert(r.status==COAX_DsStatus_ok && r.written==s.vocab && r.next==t+1);
            for (int64_t i=0;i<s.vocab;i++) assert(fabsf(logits[i]-ref_logits[i])<2e-6f);
            for (int64_t i=0;i<traces;i++) assert(fabsf(trace[i]-ref_trace[i])<2e-6f);
            for (int64_t i=0;i<p.cache;i++) assert(fabsf(cache[i]-ref_cache[i])<2e-6f);
            for (int64_t layer=0;layer<s.layers;layer++) {
                if (layer<s.dense_layers) assert(routes[layer].count==0);
                else {assert(routes[layer].count==s.selected);for (int64_t k=0;k<s.selected;k++) {
                    int64_t id=wide?(mode?dvw_expanded_ids[t][layer][k]:dvw_compressed_ids[t][layer][k]):(mode?dvt_expanded_ids[t][layer][k]:dvt_compressed_ids[t][layer][k]);
                    double weight=wide?(mode?dvw_expanded_weights[t][layer][k]:dvw_compressed_weights[t][layer][k]):(mode?dvt_expanded_weights[t][layer][k]:dvt_compressed_weights[t][layer][k]);
                    assert(routes[layer].ids.e[k]==id && fabs(routes[layer].weights.e[k]-weight)<1e-12);
                }}
                for (int64_t pos=t+1;pos<s.attention.capacity;pos++) for (int64_t j=0;j<p.row;j++) assert(cache[(layer*s.attention.capacity+pos)*p.row+j]==0);
            }
            assert(cache[p.cache]==-13 && logits[s.vocab]==-7 && trace[traces]==-9 && routes[s.layers].count==-11);
        }
        /* Fail after every layer and preserve all previously committed state. */
        float *saved_cache=allocate(p.cache+1,sizeof(float)),*saved_logits=allocate(s.vocab+1,sizeof(float)),*saved_trace=allocate(traces+1,sizeof(float));
        coax_t_DrRoute *saved_routes=allocate(s.layers+1,sizeof(*saved_routes));
        memcpy(saved_cache,cache,(size_t)(p.cache+1)*sizeof(float));memcpy(saved_logits,logits,(size_t)(s.vocab+1)*sizeof(float));memcpy(saved_trace,trace,(size_t)(traces+1)*sizeof(float));memcpy(saved_routes,routes,(size_t)(s.layers+1)*sizeof(*routes));
        uint8_t *last=m.p.e[2].data+m.p.e[2].count-2;uint8_t lo=last[0],hi=last[1];last[0]=0x80;last[1]=0x7f;
        coax_t_DsResult failed=psionic_dv_step(m,mode,tokens[0],0,(coax_v_f32){cache,p.cache+1},out,work);assert(failed.status==COAX_DsStatus_number && failed.written==0);last[0]=lo;last[1]=hi;
        assert(memcmp(saved_cache,cache,(size_t)(p.cache+1)*sizeof(float))==0 && memcmp(saved_logits,logits,(size_t)(s.vocab+1)*sizeof(float))==0 && memcmp(saved_trace,trace,(size_t)(traces+1)*sizeof(float))==0 && memcmp(saved_routes,routes,(size_t)(s.layers+1)*sizeof(*routes))==0);
        float *overlap=allocate(p.floats,sizeof(float));coax_t_DvOutput aliased=out;coax_t_DvWork bad_work=work;aliased.logits=(coax_v_f32){overlap,s.vocab};bad_work.floats=(coax_v_f32){overlap,p.floats};
        assert(psionic_dv_step(m,mode,1,0,(coax_v_f32){cache,p.cache+1},aliased,bad_work).status==COAX_DsStatus_alias);
        coax_t_DvModel nested=m;nested.p.e[0]=(coax_v_u8){(uint8_t *)overlap,2*s.vocab*s.width};
        assert(psionic_dv_step(nested,mode,1,0,(coax_v_f32){cache,p.cache+1},aliased,work).status==COAX_DsStatus_alias);
        free(overlap);free(saved_cache);free(saved_logits);free(saved_trace);free(saved_routes);
        free(cache);free(logits);free(trace);free(routes);free(route_work);free(floats);free(scores);free(acc);
    }
    assert(psionic_dv_bind(profile,(coax_v_t_DsFile){&file,1},(coax_v_t_DvBlock){blocks,s.layers},(coax_v_t_DvExpert){experts,count},
        (coax_v_t_DvBlock){blocks,s.layers},(coax_v_t_DvExpert){de,count},(coax_v_t_DvModel){&m,1}).status==COAX_DsStatus_alias);
    free(data);free(blocks);free(draft);free(experts);free(de);
    printf("V2 %s: both complete cache forms, five prefix references, public metadata ownership, atomic failure and aliases pass\n",wide?"wide":"tiny");
}
static void text(const char *path,const char *record) {
    int64_t size;uint8_t *table=read_file(path,&size);assert(dv_text_admit((coax_v_u8){table,size}));
    table[size-1]^=1;assert(!dv_text_admit((coax_v_u8){table,size}));table[size-1]^=1;
    int64_t ids[]={11598,100001};uint8_t out[8]={0},stage[8]={0};
    coax_t_DsResult r=psionic_dv_decode((coax_v_u8){table,size},(coax_v_s64)V(ids),(coax_v_u8)V(out),(coax_v_u8)V(stage));
    assert(r.status==COAX_DsStatus_ok && r.written==3 && memcmp(out," Hi",3)==0);
    assert(psionic_dv_decode((coax_v_u8){table,size},(coax_v_s64)V(ids),(coax_v_u8)V(stage),(coax_v_u8)V(stage)).status==COAX_DsStatus_alias);
    assert(psionic_dv_decode((coax_v_u8){table,size},(coax_v_s64)V(ids),(coax_v_u8){table,8},(coax_v_u8)V(stage)).status==COAX_DsStatus_alias);
    float logits[]={1,NAN,3};assert(dv_greedy((coax_v_f32)V(logits)).status==COAX_DsStatus_number);
    assert(dv_decide((coax_v_f32)V(logits),2,0,8,1,8,0).status==COAX_DsStatus_number);
    for (int mode=0;mode<2;mode++) {
        char name[4096];const char *form=mode?"expanded":"compressed";int64_t token_bytes,answer_bytes;
        assert(snprintf(name,sizeof(name),"%s/%s-generated.i64",record,form)>0);uint8_t *tokens=read_file(name,&token_bytes);assert(token_bytes%8==0);
        assert(snprintf(name,sizeof(name),"%s/%s.txt",record,form)>0);uint8_t *answer=read_file(name,&answer_bytes);
        uint8_t decoded[65536],work[65536];r=psionic_dv_decode((coax_v_u8){table,size},(coax_v_s64){(int64_t *)tokens,token_bytes/8},(coax_v_u8){decoded,65536},(coax_v_u8){work,65536});
        assert(r.status==COAX_DsStatus_ok && r.written==answer_bytes && memcmp(answer,decoded,(size_t)answer_bytes)==0);free(tokens);free(answer);
    }
    free(table);puts("V2 original-derived token-byte admission, typed EOS, native aliases and nonfinite greedy refusal pass");
}
int main(int argc,char **argv) {assert(argc==5);model(0,argv[1]);model(1,argv[2]);text(argv[3],argv[4]);return 0;}
