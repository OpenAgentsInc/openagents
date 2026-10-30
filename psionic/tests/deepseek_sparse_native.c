/* Test orchestration and I/O only. Production math/policies are exported Coax. */
#include "deepseek-sparse-memory.h"
#include "sparse-goldens.h"
#include <assert.h>
#include <math.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#define V8(p,n) ((coax_v_u8){(uint8_t *)(p),(n)})
#define V16(p,n) ((coax_v_u16){(uint16_t *)(p),(n)})
#define VF(p,n) ((coax_v_f32){(float *)(p),(n)})
#define VD(p,n) ((coax_v_f64){(double *)(p),(n)})
#define VI(p,n) ((coax_v_s64){(int64_t *)(p),(n)})
#define COUNT(a) (sizeof(a)/sizeof((a)[0]))
static uint32_t fbits(float x){uint32_t b;memcpy(&b,&x,4);return b;}
static void policy(void) {
    for(size_t i=0;i<COUNT(das_policy);i++) {
        DasPolicy f=das_policy[i];uint8_t out[66],stage[65];float blocks[65];int64_t heap[5],ids[6];
        memset(stage,99,sizeof stage);out[f.n]=88;ids[5]=99;
        coax_t_DaSelection r=psionic_da_candidates(VF(f.scores,f.n),f.visible,f.block,f.k,V8(out,f.n),V8(stage,f.n),VF(blocks,65),VI(heap,f.k));assert(!r.status);
        assert(!memcmp(out,f.mask,(size_t)f.n)&&out[f.n]==88);
        r=psionic_da_select(VF(f.scores,f.n),f.visible,V8(out,f.n),5,VI(ids,5),VI(heap,5));assert(!r.status&&r.count==f.count);
        assert(!memcmp(ids,f.ids,(size_t)f.count*8)&&ids[5]==99);
    }
    puts("Sparse: all independent candidate masks and position selections pass natively");
}
static void pools(void) {
    for(size_t f=0;f<COUNT(das_pools);f++) {
        DasPool p=das_pools[f];int64_t n=p.width;float old[512]={0},old_gate[512]={0},next[513],ng[513],floats[1024];uint16_t out[513],bits[1536];
        for(int i=0;i<513;i++)out[i]=77;next[n]=99;ng[n]=101;
        for(int64_t t=0;t<p.steps;t++) {
            memset(floats,0x5a,sizeof floats);memset(bits,0xa5,sizeof bits);
            coax_t_DaResult r=psionic_da_pool(p.ratio,t,VF(p.kv+t*n,n),VF(p.gate+t*n,n),VF(old,n),VF(old_gate,n),V16(p.norm,n),V16(out,n+1),VF(next,n+1),VF(ng,n+1),(coax_t_DaPoolWork){VF(floats,2*n),V16(bits,3*n)});
            assert(!r.status&&r.next==t+1&&r.ready==((t+1)%p.ratio==0));
            if(r.ready)assert(!memcmp(out,p.output+t*n,(size_t)n*2));
            for(int64_t j=0;j<n;j++){assert(next[j]==(r.ready?0:p.kv[t*n+j]));assert(ng[j]==(r.ready?0:p.gate[t*n+j]));}
            assert(out[n]==77&&next[n]==99&&ng[n]==101);memcpy(old,next,(size_t)n*4);memcpy(old_gate,ng,(size_t)n*4);
        }
    }
    for(size_t i=0;i<COUNT(das_rope);i++) {
        DasRopeFixture f=das_rope[i];float out[65],stage[64];out[64]=77;
        assert(!da_coeff(f.compressed,f.rope,f.position,VF(out,65),VF(stage,64),0).status);
        for(int64_t j=0;j<f.rope;j++)assert(fbits(out[j])==f.coeff[j]);assert(out[64]==77);
    }
    for(size_t i=0;i<COUNT(das_rotate);i++) {
        DasRotateFixture f=das_rotate[i];int64_t n=f.heads*f.width;uint16_t out[32769],stage[32768];out[n]=77;
        assert(!psionic_da_rotate(f.compressed,f.position,f.heads,f.width,f.rope,f.inverse,V16(f.input,n),V16(out,n+1),V16(stage,n)).status);
        assert(!memcmp(out,f.output,(size_t)n*2)&&out[n]==77);
    }
    puts("Sparse: both full-prefix compressors, canonical tails, original width 512 and both rotary profiles pass");
}
static void math_cases(void) {
    for(size_t f=0;f<COUNT(das_indices);f++) {
        DasIndex x=das_indices[f];coax_t_DkPlan qp=da_rows(2,x.heads,x.width),kp=da_rows(2,x.rows,x.width);float out[10],stage[9];out[x.rows]=77;
        assert(!psionic_da_index(x.heads,x.width,x.rows,x.rows,(coax_t_DkPacked){V8(x.q,qp.data),V8(x.qs,qp.scales)},(coax_t_DkPacked){V8(x.k,kp.data),V8(x.ks,kp.scales)},V16(x.weights,x.heads),VF(out,x.rows+1),VF(stage,x.rows)).status);
        assert(!memcmp(out,x.output,(size_t)x.rows*4)&&out[x.rows]==77);
    }
    for(size_t f=0;f<COUNT(das_attention);f++) {
        DasAttentionFixture x=das_attention[f];coax_t_DkPlan wp=da_rows(0,x.window,x.width),cp=da_rows(1,x.rows,x.width);int64_t n=x.heads*x.width;
        uint16_t out[32769],bits[32768];double scores[640];out[n]=77;
        coax_t_DaAttention cfg={.heads=x.heads,.width=x.width,.window=x.window,.capacity=x.position+1,.ratio=x.ratio};
        coax_t_DkResult r=psionic_da_attend(cfg,x.position,V16(x.q,n),(coax_t_DkPacked){V8(x.w,wp.data),V8(x.ws,wp.scales)},x.rows,(coax_t_DkPacked){V8(x.c,cp.data),V8(x.cs,cp.scales)},VI(x.ids,3),VF(x.sinks,x.heads),V16(out,n+1),(coax_t_DaAttentionWork){VD(scores,x.window+3),V16(bits,n)});
        assert(!r.status&&r.written==n);for(int64_t j=0;j<n;j++)if(out[j]!=x.output[j]){fprintf(stderr,"attention fixture %zu lane %lld got %04x expected %04x\n",f,(long long)j,out[j],x.output[j]);abort();}assert(out[n]==77);
    }
    puts("Sparse: independent index scores at 32x128 and attention at 64x512, sinks and a wrapped 128-row window agree");
}
static void pool_chunks(void) {
    for(size_t f=0;f<COUNT(das_pools);f++)for(int mode=0;mode<3;mode++) {
        DasPool p=das_pools[f];int64_t n=p.width,maximum=p.steps*n,produced=0;uint16_t *output=malloc((size_t)(maximum+1)*2),*bits=malloc((size_t)(p.steps+4)*(size_t)n*2);
        float old[512]={0},og[512]={0},next[512],ng[512],floats[3072];assert(output&&bits);for(int64_t j=0;j<maximum+1;j++)output[j]=77;
        for(int64_t at=0;at<p.steps;) {
            int64_t count=mode==0?p.steps:mode==1?1:3-at%3;if(count>p.steps-at)count=p.steps-at;coax_t_DaChunkPlan plan=da_chunk_plan(p.ratio,at,count,n);assert(!plan.status);
            memset(bits,0xa5,(size_t)plan.bits*2);memset(floats,0x5a,sizeof floats);
            coax_t_DaResult r=psionic_da_pool_chunk(p.ratio,at,count,n,VF(p.kv+at*n,count*n),VF(p.gate+at*n,count*n),VF(old,n),VF(og,n),V16(p.norm,n),V16(output+produced,maximum-produced+1),VF(next,n),VF(ng,n),(coax_t_DaPoolWork){VF(floats,plan.floats),V16(bits,plan.bits)});
            assert(!r.status&&r.next==at+count&&r.written==plan.values);produced+=r.written;memcpy(old,next,(size_t)n*4);memcpy(og,ng,(size_t)n*4);at+=count;
        }
        int64_t position=0;for(int64_t t=0;t<p.steps;t++)if((t+1)%p.ratio==0){assert(!memcmp(output+position,p.output+t*n,(size_t)n*2));position+=n;}assert(position==produced&&output[produced]==77);
        for(int64_t j=0;j<n;j++){assert(old[j]==(p.steps%p.ratio?p.kv[(p.steps-1)*n+j]:0));assert(og[j]==(p.steps%p.ratio?p.gate[(p.steps-1)*n+j]:0));}
        free(output);free(bits);
    }
    puts("Sparse: actual whole, one-token and irregular chunk calls preserve full-prefix latents and canonical tail state");
}
typedef struct {
    uint8_t window[43][96],ws[43][3],comp[43][304],cs[43][38],keys[43][304],ks[43][19],candidates[19];
    float tail[43][32],gate[43][32];int64_t ids[43][3],counts[43];coax_t_DaStamp kv_stamp[43],index_stamp[43],candidate_stamp;
} DasState;
static void chain_token(DasState *state,int64_t t) {
    uint16_t unit[32];for(int i=0;i<32;i++)unit[i]=0x3f80;
    for(int64_t layer=0;layer<43;layer++) {
        coax_t_DaLinks link=da_flash_links(layer);assert(!link.status);int64_t at=(t*43+layer)*32,qa=at*2;
        uint16_t row[32],temp[64],rotated[64],query[64],latent[32],next_bits[96];float next[32],ng[32],pool_work[64];uint8_t pack_work[66];
        coax_t_DkPacked window={V8(state->window[layer],96),V8(state->ws[layer],3)};
        assert(!psionic_da_norm(V16(das_chain_window_input+at,32),V16(unit,32),V16(row,32),V16(temp,32)).status);
        assert(!psionic_da_rotate(link.ratio!=0,t,1,32,8,0,V16(row,32),V16(rotated,32),V16(temp,32)).status);
        coax_t_DaWindow ring=da_window(t,3,19);assert(!psionic_da_store(0,3,32,ring.slot,V16(rotated,32),window,V8(pack_work,66)).status);
        int64_t visible=link.ratio?(t+1)/link.ratio:0;
        if(link.produce_kv) {
            coax_t_DaResult p=psionic_da_pool(link.ratio,t,VF(das_chain_kv+at,32),VF(das_chain_gate+at,32),VF(state->tail[layer],32),VF(state->gate[layer],32),V16(das_chain_norm,32),V16(latent,32),VF(next,32),VF(ng,32),(coax_t_DaPoolWork){VF(pool_work,64),V16(next_bits,96)});assert(!p.status);
            memcpy(state->tail[layer],next,128);memcpy(state->gate[layer],ng,128);
            if(p.ready) {
                // Index key is projected/normed from the pre-RoPE latent first.
                assert(!psionic_da_norm(V16(latent,32),V16(unit,32),V16(row,32),V16(temp,32)).status);
                assert(!psionic_da_rotate(1,p.first,1,32,8,0,V16(row,32),V16(rotated,32),V16(temp,32)).status);
                assert(!psionic_da_store(2,19,32,visible-1,V16(rotated,32),(coax_t_DkPacked){V8(state->keys[layer],304),V8(state->ks[layer],19)},V8(pack_work,66)).status);
                assert(!psionic_da_rotate(1,p.first,1,32,8,0,V16(latent,32),V16(rotated,32),V16(temp,32)).status);
                assert(!psionic_da_store(1,19,32,visible-1,V16(rotated,32),(coax_t_DkPacked){V8(state->comp[layer],304),V8(state->cs[layer],38)},V8(pack_work,66)).status);
            }state->kv_stamp[layer]=(coax_t_DaStamp){layer,t+1};
        }
        if(link.produce_index) {
            state->counts[layer]=0;
            if(visible) {
                uint8_t qbytes[32],qscale[2],mask[19],mask_stage[19];float scores[19],score_stage[19],blocks[7];int64_t heap[3];
                assert(!psionic_da_rotate(1,t,2,32,8,0,V16(das_chain_index_query+qa,64),V16(rotated,64),V16(temp,64)).status);
                coax_t_DkPacked packed={V8(qbytes,32),V8(qscale,2)};
                assert(!dk_pack(2,2,32,V16(rotated,64),packed,V8(pack_work,66),0).status);
                assert(!psionic_da_index(2,32,19,visible,packed,(coax_t_DkPacked){V8(state->keys[link.kv],304),V8(state->ks[link.kv],19)},V16(das_chain_weights+(t*43+layer)*2,2),VF(scores,visible),VF(score_stage,visible)).status);
                if(link.produce_candidates) {
                    assert(!psionic_da_candidates(VF(scores,visible),visible,3,2,V8(state->candidates,visible),V8(mask_stage,visible),VF(blocks,7),VI(heap,3)).status);
                    state->candidate_stamp=(coax_t_DaStamp){layer,t+1};
                }
                for(int64_t j=0;j<visible;j++)mask[j]=link.candidate>=0?state->candidates[j]:1;
                coax_t_DaSelection selected=psionic_da_select(VF(scores,visible),visible,V8(mask,visible),3,VI(state->ids[layer],3),VI(heap,3));assert(!selected.status);state->counts[layer]=selected.count;
            }state->index_stamp[layer]=(coax_t_DaStamp){layer,t+1};
        }
        int64_t count=link.ratio?state->counts[link.index]:0;int64_t empty_ids[3]={0};int64_t *ids=link.ratio?state->ids[link.index]:empty_ids;
        for(int64_t j=0;j<3;j++)assert((j<count?ids[j]:-1)==das_chain_selected[(t*43+layer)*3+j]);
        coax_t_DaStamp missing={-1,0};assert(!da_reuse(layer,t,link.ratio?state->kv_stamp[link.kv]:missing,link.ratio?state->index_stamp[link.index]:missing,state->candidate_stamp));
        assert(!psionic_da_rotate(link.ratio!=0,t,2,32,8,0,V16(das_chain_query+qa,64),V16(query,64),V16(temp,64)).status);
        float sink[2]={(float)(layer%5-2)/4,(float)(layer%5-1)/4};double scores[6];uint16_t answer[64],work[64];uint8_t none[1]={0};
        coax_t_DkPacked compressed=link.ratio?(coax_t_DkPacked){V8(state->comp[link.kv],304),V8(state->cs[link.kv],38)}:(coax_t_DkPacked){V8(none,0),V8(none,0)};
        assert(!psionic_da_attend((coax_t_DaAttention){.heads=2,.width=32,.window=3,.capacity=19,.ratio=link.ratio},t,V16(query,64),window,link.ratio?19:0,compressed,VI(ids,count),VF(sink,2),V16(answer,64),(coax_t_DaAttentionWork){VD(scores,3+count),V16(work,64)}).status);
        assert(!psionic_da_rotate(link.ratio!=0,t,2,32,8,1,V16(answer,64),V16(rotated,64),V16(temp,64)).status);
        uint8_t output_bytes[64],output_scales[2];coax_t_DkPacked packed={V8(output_bytes,64),V8(output_scales,2)};
        assert(!dk_pack(0,2,32,V16(rotated,64),packed,V8(pack_work,66),0).status);
        assert(!dk_unpack(0,2,32,packed,V16(answer,64),V16(temp,64),0).status);
        for(int64_t j=0;j<64;j++)if(answer[j]!=das_chain_output[(t*43+layer)*64+j]){fprintf(stderr,"chain t=%lld layer=%lld lane=%lld got %04x expected %04x\n",(long long)t,(long long)layer,(long long)j,answer[j],das_chain_output[(t*43+layer)*64+j]);abort();}
    }
    assert(!memcmp(state->candidates,das_chain_candidates+t*19,19));
}
static void chain(void) {
    DasState *whole=calloc(1,sizeof *whole),*single=calloc(1,sizeof *single),*chunks=calloc(1,sizeof *chunks);assert(whole&&single&&chunks);
    const int schedule[]={3,2,5,1,8};int64_t next=0;
    for(int64_t t=0;t<19;t++)chain_token(whole,t);
    for(int64_t t=0;t<19;t++)chain_token(single,t);
    for(size_t i=0;i<COUNT(schedule);i++)for(int t=0;t<schedule[i];t++)chain_token(chunks,next++);assert(next==19);
    assert(!memcmp(whole,single,sizeof *whole)&&!memcmp(whole,chunks,sizeof *whole));
    assert(!memcmp(whole->window,das_chain_final_window_data,sizeof whole->window)&&!memcmp(whole->ws,das_chain_final_window_scales,sizeof whole->ws));
    assert(!memcmp(whole->comp,das_chain_final_comp_data,sizeof whole->comp)&&!memcmp(whole->cs,das_chain_final_comp_scales,sizeof whole->cs));
    assert(!memcmp(whole->keys,das_chain_final_keys_data,sizeof whole->keys)&&!memcmp(whole->ks,das_chain_final_keys_scales,sizeof whole->ks));
    assert(!memcmp(whole->tail,das_chain_final_tail,sizeof whole->tail)&&!memcmp(whole->gate,das_chain_final_gate,sizeof whole->gate));
    free(whole);free(single);free(chunks);
    puts("Sparse: all 43 pinned layers over 19 projected tokens match independent prefix outputs, routes, candidates and full cache/tail state under three chunk schedules");
}
static void refusals(void) {
    float scores[9]={1,2,3,4,5,6,7,8,NAN},blocks[3];uint8_t mask[9],out[9],stage[9];int64_t ids[3],heap[3];memset(mask,1,9);
    assert(!psionic_da_select(VF(scores,9),8,V8(mask,9),3,VI(ids,3),VI(heap,3)).status);
    assert(!psionic_da_candidates(VF(scores,9),8,3,2,V8(out,9),V8(stage,9),VF(blocks,3),VI(heap,3)).status&&out[8]==1);
    memset(out,77,9);assert(psionic_da_candidates(VF(scores,9),9,3,2,V8(out,9),V8(stage,9),VF(blocks,3),VI(heap,3)).status==COAX_DkStatus_number);for(int i=0;i<9;i++)assert(out[i]==77);
    mask[8]=0;assert(!psionic_da_select(VF(scores,9),9,V8(mask,9),3,VI(ids,3),VI(heap,3)).status);
    assert(psionic_da_select(VF(scores,9),8,V8(mask,9),3,VI(ids,3),VI(ids,3)).status==COAX_DkStatus_alias);
    const DasIndex f=das_indices[0];uint8_t keys[112],ks[7],qs[2];float iy[8],iw[7];uint16_t weights[2];memcpy(keys,f.k,112);memcpy(ks,f.ks,7);memcpy(qs,f.qs,2);memcpy(weights,f.weights,4);
    coax_t_DkPacked qp={V8(f.q,32),V8(qs,2)},kp={V8(keys,112),V8(ks,7)};ks[6]=255;
    assert(!psionic_da_index(2,32,7,6,qp,kp,V16(weights,2),VF(iy,8),VF(iw,7)).status);
    for(int i=0;i<8;i++)iy[i]=77;
    assert(psionic_da_index(2,32,7,7,qp,kp,V16(weights,2),VF(iy,8),VF(iw,7)).status==COAX_DkStatus_number);for(int i=0;i<8;i++)assert(iy[i]==77);
    qs[0]=255;assert(!psionic_da_index(2,32,7,0,qp,kp,V16(weights,2),VF(iy,8),VF(iw,7)).status);qs[0]=127;weights[1]=0x7f80;
    assert(psionic_da_index(2,32,7,6,qp,kp,V16(weights,2),VF(iy,8),VF(iw,7)).status==COAX_DkStatus_number);
    assert(psionic_da_index(2,32,7,0,qp,kp,V16(weights,2),VF(iy,8),VF(iy,7)).status==COAX_DkStatus_alias);
    uint16_t x[64]={0},norm[32],y[65],bits[192],tmp[64];float old[32],gate[32],next[32],ng[32],pf[192],kv[64];
    for(int i=0;i<32;i++){norm[i]=0x3f80;old[i]=NAN;gate[i]=NAN;kv[i]=1;}for(int i=32;i<64;i++)kv[i]=2;
    assert(!psionic_da_pool(1,0,VF(kv,32),VF(gate,32),VF(old,32),VF(gate,32),V16(norm,32),V16(y,32),VF(next,32),VF(ng,32),(coax_t_DaPoolWork){VF(pf,64),V16(bits,96)}).status);
    assert(psionic_da_pool(2,1,VF(kv,32),VF(gate,32),VF(old,32),VF(gate,32),V16(norm,32),V16(y,32),VF(next,32),VF(ng,32),(coax_t_DaPoolWork){VF(pf,64),V16(bits,96)}).status==COAX_DkStatus_number);
    assert(psionic_da_pool(1,0,VF(kv,32),VF(gate,32),VF(old,32),VF(gate,32),V16(norm,32),V16(y,32),VF(old,32),VF(ng,32),(coax_t_DaPoolWork){VF(pf,64),V16(bits,96)}).status==COAX_DkStatus_alias);
    assert(psionic_da_norm(V16(x,32),V16(norm,32),V16(x,32),V16(tmp,32)).status==COAX_DkStatus_alias);
    assert(psionic_da_rotate(0,0,1,32,8,0,V16(x,32),V16(y,32),V16(y,32)).status==COAX_DkStatus_alias);
    for(int i=0;i<65;i++)y[i]=77;for(int i=0;i<32;i++){next[i]=99;ng[i]=101;old[i]=0;gate[i]=0;}kv[63]=NAN;
    assert(psionic_da_pool_chunk(1,0,2,32,VF(kv,64),VF(kv,64),VF(old,32),VF(gate,32),V16(norm,32),V16(y,65),VF(next,32),VF(ng,32),(coax_t_DaPoolWork){VF(pf,192),V16(bits,192)}).status==COAX_DkStatus_number);
    for(int i=0;i<65;i++)assert(y[i]==77);for(int i=0;i<32;i++)assert(next[i]==99&&ng[i]==101);
    const DasAttentionFixture a=das_attention[0];uint8_t ws[3],cs[16];memcpy(ws,a.ws,3);memcpy(cs,a.cs,16);ws[1]=ws[2]=255;float sink[2]={0,0};double aw[6];int64_t selected[1]={0};
    coax_t_DkPacked window={V8(a.w,96),V8(ws,3)},comp={V8(a.c,128),V8(cs,16)},empty={V8(cs,0),V8(cs,0)};
    assert(!psionic_da_attend((coax_t_DaAttention){.heads=1,.width=32,.window=3,.capacity=8,.ratio=0},0,V16(a.q,32),window,0,empty,VI(selected,0),VF(sink,1),V16(y,65),(coax_t_DaAttentionWork){VD(aw,3),V16(tmp,32)}).status);
    memcpy(ws,a.ws,3);cs[2]=255;cs[3]=255;
    assert(!psionic_da_attend((coax_t_DaAttention){.heads=2,.width=32,.window=3,.capacity=8,.ratio=2},1,V16(a.q,64),window,8,comp,VI(selected,1),VF(sink,2),V16(y,65),(coax_t_DaAttentionWork){VD(aw,4),V16(tmp,64)}).status);
    selected[0]=1;for(int i=0;i<65;i++)y[i]=77;
    assert(psionic_da_attend((coax_t_DaAttention){.heads=2,.width=32,.window=3,.capacity=8,.ratio=2},3,V16(a.q,64),window,8,comp,VI(selected,1),VF(sink,2),V16(y,65),(coax_t_DaAttentionWork){VD(aw,4),V16(tmp,64)}).status==COAX_DkStatus_number);for(int i=0;i<65;i++)assert(y[i]==77);
    assert(psionic_da_attend((coax_t_DaAttention){.heads=2,.width=32,.window=3,.capacity=8,.ratio=2},3,V16(a.q,64),window,8,comp,VI(selected,1),VF(sink,2),V16(y,65),(coax_t_DaAttentionWork){VD(aw,4),V16(y,64)}).status==COAX_DkStatus_alias);
    puts("Sparse: real aliases, hidden future values, NaN and late failures preserve all public outputs; ratio one ignores gates");
}
int main(void){policy();pools();pool_chunks();math_cases();chain();refusals();return 0;}
