#define _POSIX_C_SOURCE 200809L
#include "deepseek-hot-memory.h"
#include "hot-goldens.h"
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>
#define V(p,n) ((coax_v_s64){(int64_t *)(p),(n)})
#define B(p,n) ((coax_v_u8){(uint8_t *)(p),(n)})
#define F(p,n) ((coax_v_f32){(float *)(p),(n)})
#define D(p,n) ((coax_v_f64){(double *)(p),(n)})
#define S(n) ((coax_string){n,sizeof(n)-1})
#define STRIDE 3264
static coax_t_DcConfig config(void){return (coax_t_DcConfig){.layers=2,.experts=12,.slots=3,.stride=STRIDE,.budget=6*STRIDE,.identity=S("owned-exact-model-v1")};}
static void compare_map(coax_t_DcState state,int step){assert(!memcmp(state.slots.data,dch_slots+6*step,6*sizeof(int64_t)));assert(!memcmp(state.heat.data,dch_heat+24*step,24*sizeof(int64_t)));}
static void apply(int fd,coax_t_DcConfig c,coax_t_DcState *state,uint8_t *resident,const uint8_t *host,int step){
    int64_t proposed[6],sw[6];coax_t_DcMove moves[6],mw[6];
    coax_t_DcPlan p=psionic_dc_plan(c,*state,V(proposed,6),(coax_v_t_DcMove){moves,6},V(sw,6),(coax_v_t_DcMove){mw,6});assert(!p.status&&p.ticket.count==dch_counts[step]);
    for(int64_t i=0;i<p.ticket.count;i++){const int64_t *want=dch_moves+step*18+i*3;assert(moves[i].slot==want[0]&&moves[i].old==want[1]&&moves[i].key==want[2]);}
    uint8_t pending[6*STRIDE],work[6*STRIDE],ready[6]={1,1,1,1,1,1};int64_t keys[6],offsets[6];for(int64_t i=0;i<p.ticket.count;i++)keys[i]=moves[i].key;
    coax_t_DcResult r=psionic_dc_fetch(fd,step%2,24,STRIDE,4096,4096+24*STRIDE,V(keys,p.ticket.count),B(pending,sizeof pending),B(work,sizeof work),V(offsets,6));assert(!r.status&&r.written==p.ticket.count*STRIDE);
    r=psionic_dc_install(c,*state,p.ticket,V(proposed,6),(coax_v_t_DcMove){moves,p.ticket.count},B(ready,p.ticket.count),B(pending,p.ticket.count*STRIDE),B(resident,6*STRIDE+1));assert(!r.status);state->head=r.head;
    assert(state->head.revision==step+1&&resident[6*STRIDE]==77);
    for(int64_t slot=0;slot<6;slot++){int64_t key=(slot/3)*12+state->slots.data[slot];assert(!memcmp(resident+slot*STRIDE,host+key*STRIDE,STRIDE));}
}
static void trace(void){
    coax_t_DcConfig c=config();uint8_t host[24*STRIDE],resident[6*STRIDE+1];resident[6*STRIDE]=77;
    for(int key=0;key<24;key++){coax_t_DeExpert e=dc_test_packet(key%3,B(host+key*STRIDE,STRIDE));coax_t_DcBound b=dc_bind(64,32,B(host+key*STRIDE,STRIDE));assert(!b.status&&e.gate.data.data==b.value.gate.data.data&&e.down.scales.data==b.value.down.scales.data);}
    FILE *file=tmpfile();assert(file);int fd=fileno(file);assert(!ftruncate(fd,4096+sizeof host));assert(pwrite(fd,host,sizeof host,4096)==sizeof host);
    int64_t heats[2][24],slots[6],hw[24],sw[6];coax_t_DcResult init=psionic_dc_seed(c,V(dch_profile,24),V(heats[0],24),V(slots,6),V(hw,24),V(sw,6));assert(!init.status);
    coax_t_DcState state={.head=init.head,.heat=V(heats[0],24),.slots=V(slots,6)};apply(fd,c,&state,resident,host,0);compare_map(state,0);
    float input[64],expected[24][64],scratch[512],observed[65];double wide[64];dc_test_input(F(input,64));coax_t_DeWork work={.floats=F(scratch,512),.wide=D(wide,64)};
    for(int key=0;key<24;key++){coax_t_DcBound b=dc_bind(64,32,B(host+key*STRIDE,STRIDE));assert(!b.status&&!psionic_dc_expert(64,32,B(host+key*STRIDE,STRIDE),F(input,64),.137f,10,F(expected[key],64),work).status);}
    int64_t hit_count=0,miss_count=0;
    for(int step=0;step<300;step++){
        coax_t_DcResult r=dc_begin(c,state);assert(!r.status);state.head=r.head;
        for(int layer=0;layer<2;layer++){
            int64_t ids[3];for(int j=0;j<3;j++)ids[j]=dch_routes[step*6+layer*3+j]%12;
            // The gate's next-layer IDs stage only misses, with no slot-map update.
            int64_t prefetch[3],pw[3];r=psionic_dc_prefetch(c,state,layer,V(ids,3),V(prefetch,3),V(pw,3));assert(!r.status);int64_t misses=r.written;
            uint8_t packets[3*STRIDE],packet_work[3*STRIDE];int64_t offsets[3];
            r=psionic_dc_fetch(fd,step%2,24,STRIDE,4096,4096+sizeof host,V(prefetch,misses),B(packets,sizeof packets),B(packet_work,sizeof packet_work),V(offsets,3));assert(!r.status);
            coax_t_DcLane lanes[3],draft[3];r=psionic_dc_dispatch(c,state,layer,V(ids,3),(coax_v_t_DcLane){lanes,3},(coax_v_t_DcLane){draft,3});assert(!r.status);
            for(int j=0;j<3;j++){
                assert(lanes[j].key==dch_routes[step*6+layer*3+j]);assert((lanes[j].slot>=0)==dch_hits[step*6+layer*3+j]);
                uint8_t *bytes;if(lanes[j].slot>=0){bytes=resident+lanes[j].slot*STRIDE;hit_count++;}else{int64_t at=0;while(at<misses&&prefetch[at]!=lanes[j].key)at++;assert(at<misses);bytes=packets+at*STRIDE;miss_count++;}
                coax_t_DcBound b=dc_bind(64,32,B(bytes,STRIDE));observed[64]=77;r=(coax_t_DcResult){0};
                assert(!b.status&&!psionic_dc_expert(64,32,B(bytes,STRIDE),F(input,64),.137f,10,F(observed,65),work).status&&!memcmp(observed,expected[lanes[j].key],64*sizeof(float))&&observed[64]==77);
            }
        }
        int index=(step+1)%2;r=psionic_dc_finish(c,state,V(dch_routes+step*6,6),V(heats[index],24),V(hw,24));assert(!r.status&&r.head.step==step+1&&!r.head.busy);state.head=r.head;state.heat=V(heats[index],24);
        apply(fd,c,&state,resident,host,step+1);compare_map(state,step+1);
    }
    fclose(file);printf("Hot experts: 300 independent trace steps, %lld hits and %lld misses, 1800 bit-identical resident/mapped/pread FFNs; all heat, slots and swap plans agree\n",(long long)hit_count,(long long)miss_count);
}
static void next_gate(void){
    coax_t_DcConfig c=config();int64_t heat[24]={0},slots[6]={0,1,2,3,4,5};coax_t_DcState state={.head={.busy=1,.identity=c.identity},.heat=V(heat,24),.slots=V(slots,6)};
    float logits[12],bias[12]={0};double raw[12],ranked[12];for(int i=0;i<12;i++)logits[i]=(float)i;
    coax_t_DrConfig gate={.profile=COAX_DrProfile_v41,.k=3,.groups=1,.selected_groups=1,.scale=1.5};
    coax_t_DrRoute route=dr_route(gate,F(logits,12),F(bias,12),D(raw,12),D(ranked,12),0);assert(!route.status&&route.count==3);
    int64_t keys[3],work[3];coax_t_DcResult r=psionic_dc_prefetch(c,state,1,V(route.ids.e,3),V(keys,3),V(work,3));assert(!r.status&&r.written==3);
    for(int i=0;i<3;i++)assert(keys[i]==12+route.ids.e[i]);for(int i=0;i<6;i++)assert(slots[i]==i);
    puts("Hot experts: next-layer V4.1 gate output drives private miss prefetch while current residency stays leased");
}
static void extremes(void){
    int64_t profile[6]={0},heat[6],slots[6],hw[6],sw[6],proposed[6];coax_t_DcMove moves[6],mw[6];
    for(int count=0;count<=3;count+=3){
        coax_t_DcConfig c={.layers=2,.experts=3,.slots=count,.stride=1,.budget=2*count,.identity=S("all-or-none")};
        coax_t_DcResult r=psionic_dc_seed(c,V(profile,6),V(heat,6),V(slots,2*count),V(hw,6),V(sw,2*count));assert(!r.status);coax_t_DcState state={.head=r.head,.heat=V(heat,6),.slots=V(slots,2*count)};
        coax_t_DcPlan p=psionic_dc_plan(c,state,V(proposed,2*count),(coax_v_t_DcMove){moves,2*count},V(sw,2*count),(coax_v_t_DcMove){mw,2*count});assert(!p.status&&p.ticket.count==2*count);
        uint8_t data[6]={0,1,2,3,4,5},resident[6],ready[6]={1,1,1,1,1,1};r=psionic_dc_install(c,state,p.ticket,V(proposed,2*count),(coax_v_t_DcMove){moves,2*count},B(ready,2*count),B(data,2*count),B(resident,2*count));assert(!r.status);state.head=r.head;
        p=psionic_dc_plan(c,state,V(proposed,2*count),(coax_v_t_DcMove){moves,2*count},V(sw,2*count),(coax_v_t_DcMove){mw,2*count});assert(!p.status&&p.ticket.count==0);
        r=dc_begin(c,state);assert(!r.status);state.head=r.head;int64_t ids[3]={0,1,2};coax_t_DcLane lanes[3],draft[3];r=psionic_dc_dispatch(c,state,1,V(ids,3),(coax_v_t_DcLane){lanes,3},(coax_v_t_DcLane){draft,3});assert(!r.status);
        for(int j=0;j<3;j++)assert(lanes[j].slot==(count?3+j:-1));
    }
    puts("Hot experts: zero-slot all-miss and fully resident all-hit caches retain exact dispatch and terminate empty rebalance plans");
}
static void failures(void){
    coax_t_DcConfig c=config();int64_t heat[24],slots[6],hw[24],sw[6];coax_t_DcResult r=psionic_dc_seed(c,V(dch_profile,24),V(heat,24),V(slots,6),V(hw,24),V(sw,6));assert(!r.status);
    coax_t_DcState state={.head=r.head,.heat=V(heat,24),.slots=V(slots,6)};
    int64_t proposed[6],work_slots[6];coax_t_DcMove moves[6],work_moves[6];coax_t_DcPlan p=psionic_dc_plan(c,state,V(proposed,6),(coax_v_t_DcMove){moves,6},V(work_slots,6),(coax_v_t_DcMove){work_moves,6});assert(!p.status);
    uint8_t resident[6*STRIDE+1],pending[6*STRIDE],ready[6]={1,1,1,1,1,0};memset(resident,77,sizeof resident);memset(pending,88,sizeof pending);
    r=psionic_dc_install(c,state,p.ticket,V(proposed,6),(coax_v_t_DcMove){moves,6},B(ready,6),B(pending,sizeof pending),B(resident,sizeof resident));assert(r.status==COAX_DsStatus_state);
    for(size_t i=0;i<sizeof resident;i++)assert(resident[i]==77);for(int i=0;i<6;i++)assert(slots[i]==-1);
    ready[5]=1;assert(psionic_dc_install(c,state,p.ticket,V(proposed,6),(coax_v_t_DcMove){moves,6},B(ready,6),B(resident,6*STRIDE),B(resident,sizeof resident)).status==COAX_DsStatus_alias);
    assert(psionic_dc_plan(c,state,V(slots,6),(coax_v_t_DcMove){moves,6},V(work_slots,6),(coax_v_t_DcMove){work_moves,6}).status==COAX_DsStatus_alias);
    int64_t ids[]={0,1},offsets[2];uint8_t output[2*STRIDE],work[2*STRIDE];memset(output,77,sizeof output);
    assert(psionic_dc_fetch(-1,0,24,STRIDE,0,24*STRIDE,V(ids,2),B(output,sizeof output),B(work,sizeof work),V(offsets,2)).status==COAX_DsStatus_weights);
    assert(psionic_dc_fetch(-1,1,24,STRIDE,0,24*STRIDE,V(ids,0),B(output,0),B(work,0),V(offsets,0)).status==COAX_DsStatus_ok);
    assert(psionic_dc_fetch(-1,0,24,STRIDE,0,24*STRIDE,V(ids,2),B(output,sizeof output),B(output,sizeof output),V(offsets,2)).status==COAX_DsStatus_alias);
    FILE *file=tmpfile();assert(file);assert(!ftruncate(fileno(file),24*STRIDE-1));
    assert(psionic_dc_fetch(fileno(file),1,24,STRIDE,0,24*STRIDE,V(ids,2),B(output,sizeof output),B(work,sizeof work),V(offsets,2)).status==COAX_DsStatus_weights);fclose(file);
    for(size_t i=0;i<sizeof output;i++)assert(output[i]==77);
    _Alignas(double) uint8_t packet[STRIDE];dc_test_packet(0,B(packet,STRIDE));float input[64],out[65],scratch[512];double wide[64];dc_test_input(F(input,64));coax_t_DeWork ew={.floats=F(scratch,512),.wide=D(wide,64)};
    assert(psionic_dc_expert(64,32,B(packet,STRIDE),F(input,64),1,10,F((float *)packet,64),ew).status==COAX_DsStatus_alias);
    assert(psionic_dc_expert(64,32,B(packet,STRIDE),F(input,64),1,10,F(scratch,64),ew).status==COAX_DsStatus_alias);
    for(int j=0;j<65;j++)out[j]=77;packet[STRIDE-1]=255;
    assert(psionic_dc_expert(64,32,B(packet,STRIDE),F(input,64),1,10,F(out,65),ew).status==COAX_DsStatus_number);for(int j=0;j<65;j++)assert(out[j]==77);
    puts("Hot experts: incomplete/stale copies, real aliases, failed files and all-hit no-I/O paths preserve public bytes and slot metadata");
}
int main(void){trace();next_gate();extremes();failures();return 0;}
