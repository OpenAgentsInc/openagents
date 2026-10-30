#define _POSIX_C_SOURCE 200809L
#include "deepseek-prefill-memory.h"
#include "prefill-goldens.h"
#include "deepseek-hot-memory.h"
#include "deepseek-expert-memory.h"
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>
#define V(p,n) ((coax_v_s64){(int64_t *)(p),(n)})
#define B(p,n) ((coax_v_u8){(uint8_t *)(p),(n)})
#define F(p,n) ((coax_v_f32){(float *)(p),(n)})
#define D(p,n) ((coax_v_f64){(double *)(p),(n)})
#define A(p,n) ((coax_v_t_PsionicAccumulator){(p),(n)})
#define TOKENS 13
#define WIDTH 64
#define STRIDE 3264
static int64_t now(void){struct timespec t;assert(!clock_gettime(CLOCK_MONOTONIC,&t));return t.tv_sec*1000000000LL+t.tv_nsec;}
static float reference[TOKENS*64];static coax_t_PsionicAccumulator reference_cache[128];
static int64_t reference_ids[2][TOKENS*2];static float reference_weights[2][TOKENS*2];
static int64_t serial_reference(coax_t_DeExpert *experts,coax_t_DeExpert shared){
    float input[64],attention[64],mlp[64],work_f[352];double work_q[64];coax_t_PsionicAccumulator sums[64];coax_t_DeWork work={F(work_f,352),D(work_q,64)};int64_t began=now();
    for(int t=0;t<TOKENS;t++){dp_test_input(t,1,F(input,64));for(int layer=0;layer<2;layer++){
        dp_test_attention(t,1,layer,F(input,64),A(reference_cache,128),F(attention,64));int64_t *ids=reference_ids[layer]+t*2;float *weights=reference_weights[layer]+t*2;dp_test_routes(t,1,layer,F(attention,64),V(ids,2),F(weights,2));
        coax_t_DeResult r=psionic_de_mixture((coax_v_t_DeExpert){experts,3},shared,F(attention,64),V(ids,2),F(weights,2),10,F(mlp,64),work,A(sums,64));assert(!r.status);dp_test_residual(1,F(attention,64),F(mlp,64),F(input,64));}
        memcpy(reference+t*64,input,sizeof input);
    }for(int l=0;l<2;l++)assert(!memcmp(reference_ids[l],dpf_ids+l*26,26*8));return now()-began;
}
static void chunked(int fd,coax_t_DeExpert shared,int max_chunk,int64_t token_ns,int64_t expert_ns,int fail){
    float public_output[TOKENS*64+1];for(int i=0;i<TOKENS*64+1;i++)public_output[i]=77;coax_t_PsionicAccumulator public_cache[128]={0},shadow[128];
    int64_t first=0,epoch=0,fetches=0,steps=0,predicted_total=0,began=now();
    while(first<TOKENS){
        coax_t_DpChunkConfig cfg={.max_chunk=max_chunk,.workspace=13*4096,.bytes_per_token=4096,.token_ns=token_ns,.expert_ns=expert_ns,.experts=3,.k=2,.target_ns=max_chunk*token_ns+3*expert_ns};
        coax_t_DpChunk choice=dp_chunk(cfg,TOKENS-first);assert(!choice.status&&choice.count<=max_chunk&&(choice.count>=2||choice.tail));predicted_total+=choice.predicted_ns;int64_t n=choice.count;
        coax_t_DpNext start=dp_start(first,n,2,3,2,epoch);assert(!start.status);coax_t_DpCursor cursor=start.value;memcpy(shadow,public_cache,sizeof shadow);coax_t_PsionicAccumulator before_cache[128];memcpy(before_cache,public_cache,sizeof before_cache);
        float hidden[13*64],attention[13*64],contribution[13*2*64],group_input[13*64],group_output[13*64],mlp[13*64],shared_out[13*64],combine_work[13*64],weights[26],group_weights[13],ones[13];for(int i=0;i<13;i++)ones[i]=1;
        float ef[13*224];double eq[13*64];uint8_t flags[13*64];coax_t_DpWork work={F(ef,13*224),D(eq,13*64),B(flags,13*64)};
        int64_t ids[26],order[26],offsets[4],keys[3],so[26],sf[4],sk[3];uint8_t receipts[3]={0};int counts[2][3]={{0}},shared_calls[2]={0};_Bool shared_ready=0;int64_t groups=0;
        dp_test_input(first,n,F(hidden,n*64));
        while(cursor.phase!=COAX_DpPhase_done){
            coax_t_DpResult r={0};int64_t layer=cursor.layer;
            switch(cursor.phase){
            case COAX_DpPhase_attention:dp_test_attention(first,n,layer,F(hidden,n*64),A(shadow,128),F(attention,n*64));break;
            case COAX_DpPhase_route:
                dp_test_routes(first,n,layer,F(attention,n*64),V(ids,n*2),F(weights,n*2));assert(!memcmp(ids,reference_ids[layer]+first*2,(size_t)n*2*8)&&!memcmp(weights,reference_weights[layer]+first*2,(size_t)n*2*4));
                r=psionic_dp_groups(3,n,2,V(ids,n*2),F(weights,n*2),V(order,n*2),V(offsets,4),V(keys,3),V(so,n*2),V(sf,4),V(sk,3));assert(!r.status);groups=r.groups;const DpfPlan *gold=NULL;for(size_t gi=0;gi<sizeof(dpf_plans)/sizeof(dpf_plans[0]);gi++)if(dpf_plans[gi].cap==max_chunk&&dpf_plans[gi].first==first&&dpf_plans[gi].layer==layer)gold=dpf_plans+gi;assert(gold&&gold->groups==groups&&!memcmp(order,gold->order,(size_t)n*2*8)&&!memcmp(offsets,gold->offsets,4*8)&&!memcmp(keys,gold->keys,(size_t)groups*8));memset(receipts,0,3);shared_ready=0;break;
            case COAX_DpPhase_expert:{
                int64_t id=cursor.keys.data[cursor.group];assert(++counts[layer][id]==1);fetches++;uint8_t packet[STRIDE],io[STRIDE];int64_t offset[1];
                coax_t_DcResult read=psionic_dc_fetch(fd,(first+layer)%2,3,STRIDE,4096,4096+3*STRIDE,V(&id,1),B(packet,STRIDE),B(io,STRIDE),V(offset,1));assert(!read.status);
                if(fail&&first==(fail==1?0:4)&&layer==1&&cursor.group==groups-1)packet[STRIDE-1]=255;
                coax_t_DcBound bound=dc_bind(64,32,B(packet,STRIDE));assert(!bound.status);
                r=psionic_dp_gather(3,n,2,64,id,V(ids,n*2),F(weights,n*2),V(order,n*2),V(offsets,4),F(attention,n*64),F(group_input,13*64),F(group_weights,13));assert(!r.status);int64_t count=r.groups;
                r=psionic_dp_expert(bound.value,count,F(group_input,count*64),F(group_weights,count),10,F(group_output,count*64),work);
                if(r.status){assert(fail&&r.status==COAX_DsStatus_number);coax_t_DpReceipt bad={cursor.epoch,cursor.layer,cursor.phase,cursor.group,0};assert(dp_advance(cursor,bad,V(keys,0)).status==COAX_DsStatus_state);assert(!memcmp(public_output,reference,(size_t)first*64*4));for(int64_t j=first*64;j<TOKENS*64+1;j++)assert(public_output[j]==77);assert(!memcmp(public_cache,before_cache,sizeof before_cache));puts("Prefill: late final-layer expert failure preserves the complete public prefix and every output word");return;}
                r=psionic_dp_scatter(3,n,2,64,id,V(ids,n*2),V(order,n*2),V(offsets,4),F(group_output,count*64),F(contribution,n*2*64));assert(!r.status);receipts[id]=1;break;}
            case COAX_DpPhase_shared:r=psionic_dp_expert(shared,n,F(attention,n*64),F(ones,n),10,F(shared_out,n*64),work);assert(!r.status);assert(++shared_calls[layer]==1);shared_ready=1;break;
            case COAX_DpPhase_combine:r=psionic_dp_combine(3,n,2,64,V(ids,n*2),F(weights,n*2),B(receipts,3),shared_ready,F(contribution,n*2*64),F(shared_out,n*64),F(mlp,n*64),F(combine_work,n*64));assert(!r.status);dp_test_residual(n,F(attention,n*64),F(mlp,n*64),F(hidden,n*64));break;
            case COAX_DpPhase_commit:break;
            default:abort();}
            coax_t_DpReceipt receipt={cursor.epoch,cursor.layer,cursor.phase,cursor.group,1};coax_t_DpNext next=dp_advance(cursor,receipt,V(keys,cursor.phase==COAX_DpPhase_route?groups:0));assert(!next.status);if(cursor.phase==COAX_DpPhase_commit){memcpy(public_cache,shadow,sizeof shadow);memcpy(public_output+first*64,hidden,(size_t)n*64*4);}cursor=next.value;
        }
        assert(cursor.epoch==epoch+1&&public_output[TOKENS*64]==77);epoch=cursor.epoch;first+=n;steps++;
    }
    int which=max_chunk==2?0:max_chunk==4?1:max_chunk==7?2:3;assert(fetches==dpf_fetches[which]);
    assert(!fail&&!memcmp(public_output,reference,sizeof reference)&&!memcmp(public_cache,reference_cache,sizeof reference_cache));
    printf("Prefill: chunk %d, %lld chunks, %lld expert fetches versus %d token-major fetches; all 832 outputs, every route/weight and complete causal cache bit-identical; elapsed %lld ns, budget estimate %lld ns\n",max_chunk,(long long)steps,(long long)fetches,TOKENS*2*2,(long long)(now()-began),(long long)predicted_total);
}
static void aliases(coax_t_DeExpert e){float x[128],out[128],f[448],weights[2]={1,1};double q[128];uint8_t flags[128];dp_test_input(0,2,F(x,128));coax_t_DpWork work={F(f,448),D(q,128),B(flags,128)};
    assert(psionic_dp_expert(e,2,F(x,128),F(weights,2),10,F(f,128),work).status==COAX_DsStatus_alias);
    assert(psionic_dp_project(e.gate,2,F(x,128),F(out,64),D(q,128),F(out,64),B(flags,64)).status==COAX_DsStatus_alias);
    int64_t ids[4]={0,1,1,2},offsets[4],keys[3],so[4],sf[4],sk[3];float ws[4]={1,1,1,1};assert(psionic_dp_groups(3,2,2,V(ids,4),F(ws,4),V(ids,4),V(offsets,4),V(keys,3),V(so,4),V(sf,4),V(sk,3)).status==COAX_DsStatus_alias);
}
int main(void){uint8_t packets[3*STRIDE],shared_bytes[8192];coax_t_DeExpert experts[3];for(int id=0;id<3;id++)experts[id]=dp_test_packet(id,B(packets+id*STRIDE,STRIDE));coax_t_DeExpert shared=dp_test_shared(B(shared_bytes,8192));FILE *file=tmpfile();assert(file);int fd=fileno(file);assert(!ftruncate(fd,4096+sizeof packets)&&pwrite(fd,packets,sizeof packets,4096)==sizeof packets);
    int64_t serial_ns=serial_reference(experts,shared),token_ns=(serial_ns+TOKENS-1)/TOKENS;uint8_t read[STRIDE],io[STRIDE];int64_t id=0,offset=0,began=now();assert(!psionic_dc_fetch(fd,0,3,STRIDE,4096,4096+sizeof packets,V(&id,1),B(read,STRIDE),B(io,STRIDE),V(&offset,1)).status);int64_t expert_ns=2*(now()-began);
    printf("Prefill calibration: serial 13-token two-layer reference %lld ns; per-token bound %lld ns, per-expert two-layer file cost %lld ns\n",(long long)serial_ns,(long long)token_ns,(long long)expert_ns);
    for(int chunk=2;chunk<=13;chunk+=chunk==2?2:chunk==4?3:6)chunked(fd,shared,chunk,token_ns,expert_ns,0);
    chunked(fd,shared,4,token_ns,expert_ns,1);chunked(fd,shared,4,token_ns,expert_ns,2);aliases(experts[0]);fclose(file);puts("Prefill: native aliases, measured-cost chunks, file reuse and layer-major transaction boundaries pass");return 0;}
