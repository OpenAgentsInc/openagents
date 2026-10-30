#include "deepseek-hot-memory.h"
#include <assert.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#define V(p,n) ((coax_v_s64){(int64_t *)(p),(n)})
#define B(p,n) ((coax_v_u8){(uint8_t *)(p),(n)})
int main(void){
    const int64_t n=43*384,k=43*116;int64_t *profile=calloc((size_t)n,8),*heat=calloc((size_t)n,8),*hw=calloc((size_t)n,8),*next_heat=calloc((size_t)n,8),*slots=calloc((size_t)k,8),*sw=calloc((size_t)k,8),*proposed=calloc((size_t)k,8);coax_t_DcMove *moves=calloc((size_t)k,sizeof(*moves)),*mw=calloc((size_t)k,sizeof(*mw));uint8_t *ready=malloc((size_t)k),*pending=malloc((size_t)k),*resident=malloc((size_t)k+1);assert(profile&&heat&&hw&&next_heat&&slots&&sw&&proposed&&moves&&mw&&ready&&pending&&resident);
    for(int64_t key=0;key<n;key++)profile[key]=key%384+1;
    // Metadata topology uses one-byte owned packets; no original coefficient or
    // 93.8 GB allocation claim follows from this separate topology check.
    coax_t_DcConfig c={.layers=43,.experts=384,.slots=116,.stride=1,.budget=k,.identity={"owned-full-topology",19}};
    coax_t_DcResult r=psionic_dc_seed(c,V(profile,n),V(heat,n),V(slots,k),V(hw,n),V(sw,k));assert(!r.status);coax_t_DcState state={.head=r.head,.heat=V(heat,n),.slots=V(slots,k)};
    coax_t_DcPlan p=psionic_dc_plan(c,state,V(proposed,k),(coax_v_t_DcMove){moves,k},V(sw,k),(coax_v_t_DcMove){mw,k});assert(!p.status&&p.ticket.count==k);
    for(int64_t slot=0;slot<k;slot++){int64_t id=383-slot%116;assert(proposed[slot]==id&&moves[slot].slot==slot&&moves[slot].key==(slot/116)*384+id);ready[slot]=1;pending[slot]=(uint8_t)(moves[slot].key%251);}resident[k]=77;
    r=psionic_dc_install(c,state,p.ticket,V(proposed,k),(coax_v_t_DcMove){moves,k},B(ready,k),B(pending,k),B(resident,k+1));assert(!r.status&&resident[k]==77&&!memcmp(resident,pending,(size_t)k));state.head=r.head;
    heat[42*384]=2000000;p=psionic_dc_plan(c,state,V(proposed,k),(coax_v_t_DcMove){moves,k},V(sw,k),(coax_v_t_DcMove){mw,k});assert(!p.status&&p.ticket.count==1&&moves[0].slot==k-1&&moves[0].old==268&&moves[0].key==42*384);
    r=dc_begin(c,state);assert(!r.status);state.head=r.head;int64_t ids[]={383,0};coax_t_DcLane lanes[2],draft[2];r=psionic_dc_dispatch(c,state,42,V(ids,2),(coax_v_t_DcLane){lanes,2},(coax_v_t_DcLane){draft,2});assert(!r.status&&lanes[0].key==n-1&&lanes[0].slot==42*116&&lanes[1].slot==-1);
    assert(psionic_dc_install(c,state,p.ticket,V(proposed,k),(coax_v_t_DcMove){moves,1},B(ready,1),B(pending,1),B(resident,k+1)).status==COAX_DsStatus_state);
    int64_t used[]={42*384};r=psionic_dc_finish(c,state,V(used,1),V(next_heat,n),V(hw,n));assert(!r.status);state.head=r.head;state.heat=V(next_heat,n);
    assert(psionic_dc_install(c,state,p.ticket,V(proposed,k),(coax_v_t_DcMove){moves,1},B(ready,1),B(pending,1),B(resident,k+1)).status==COAX_DsStatus_state);
    p=psionic_dc_plan(c,state,V(proposed,k),(coax_v_t_DcMove){moves,k},V(sw,k),(coax_v_t_DcMove){mw,k});assert(!p.status&&p.ticket.count==1);pending[0]=42;
    r=psionic_dc_install(c,state,p.ticket,V(proposed,k),(coax_v_t_DcMove){moves,1},B(ready,1),B(pending,1),B(resident,k+1));assert(!r.status&&slots[k-1]==0&&resident[k-1]==42&&resident[k]==77);
    free(profile);free(heat);free(hw);free(next_heat);free(slots);free(sw);free(proposed);free(moves);free(mw);free(ready);free(pending);free(resident);
    puts("Hot experts: 43x384/116 topology, 4988 seeded slots, final expert/layer/slot endpoints and active/stale fencing pass with owned one-byte packets");return 0;
}
