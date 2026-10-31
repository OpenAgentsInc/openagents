/* Firm reason2: independent native resident routing/receipt oracle. Numerical
   original-weight qualification is separate on actual Metal, never CPU. */
#include "panda-policy-generated.h"
#include <assert.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
static unsigned long cases;
#define CHECK(x) do{assert(x);cases++;}while(0)
int main(void){
 const int64_t lengths[]={137,140,152,512};
 for(size_t q=0;q<sizeof lengths/sizeof*lengths;q++){int64_t tokens=lengths[q];coax_t_PvnCursor state=pvn_begin(tokens);coax_v_t_PvnCursor s={&state,1};for(int prep=0;prep<6;prep++){coax_t_PvnPrep d=pvn_prep(prep);CHECK(!pvn_prep_ack(s,prep,0,d.output_count));}
  while(state.phase!=5){coax_t_PvnStep p=pvn_step(state);CHECK(!p.status);
   if(p.kind==2){CHECK(p.source>=0&&p.source<1106&&p.destination>=0&&p.destination<1106);CHECK(p.bytes==pvn_buffer_bytes(tokens,p.source));CHECK(p.bytes==pvn_buffer_bytes(tokens,p.destination));CHECK(!pvn_copied(s,0));continue;}
   CHECK(p.kind==1);coax_t_PvnStep cached=pvn_cache_stage(tokens,p.index);CHECK(cached.graph==p.graph&&cached.sequence==p.sequence&&cached.nodes==p.nodes&&cached.flags==p.flags&&cached.flag_buffer==p.flag_buffer);CHECK(p.flags<=pvn_buffer_bytes(tokens,p.flag_buffer));
   for(int64_t at=0;at<p.nodes;at++){coax_t_GvpNode n=pvn_node(tokens,p.graph,p.sequence,at),old;int offset,ko;if(p.graph==0){old=gvp_node(p.sequence,at);offset=0;ko=0;}else if(p.graph==1){old=glp_node(p.sequence,at,tokens);offset=336;ko=9;}else{old=gad_node_for(tokens,p.sequence,at);offset=538;ko=20;}
    CHECK(!n.status&&n.kernel==old.kernel+ko&&n.items==old.items&&n.lanes==old.lanes&&n.block==old.block&&n.argc==old.argc&&n.flag_count==old.flag_count&&n.flag_offset==old.flag_offset);
    for(int64_t a=0;a<n.argc;a++){coax_t_GvpArg x=n.args.e[a],y=old.args.e[a];CHECK(x.kind==y.kind&&x.integer==y.integer&&x.count==y.count&&x.offset==y.offset&&x.real==y.real&&x.buffer==y.buffer+(x.kind==4?offset:0));if(x.kind==4)CHECK(x.buffer>=0&&x.buffer<1106);}
    CHECK(!pvn_sent(s,at,0));
   }
   uint8_t*flags=calloc((size_t)p.flags+1,1);CHECK(flags!=NULL);coax_t_PvnCursor bad=state;coax_v_t_PvnCursor b={&bad,1};CHECK(pvn_waited(b,0,(coax_v_u8){flags,p.flags+1})==COAX_GvpStatus_capacity);bad=state;if(p.flags){flags[p.flags-1]=1;CHECK(pvn_waited(b,0,(coax_v_u8){flags,p.flags})==COAX_GvpStatus_number);flags[p.flags-1]=0;}CHECK(!pvn_waited(s,0,(coax_v_u8){flags,p.flags}));free(flags);
  }
  uint32_t bits[5280]={0};float normal[5280],decoded[112],out[113];for(int i=0;i<113;i++)out[i]=9999;coax_t_GvpRead r=pvn_publish(s,(coax_v_u32){bits,5280},(coax_v_f32){normal,5280},(coax_v_f32){decoded,112},(coax_v_f32){out,113},0);CHECK(!r.status&&r.written==112&&state.phase==6&&out[112]==9999);CHECK(pvn_step(state).kind==4);CHECK(pvn_publish(s,(coax_v_u32){bits,5280},(coax_v_f32){normal,5280},(coax_v_f32){decoded,112},(coax_v_f32){out,113},0).status==COAX_GvpStatus_order);
 }
 printf("panda resident native: %lu cases, four token profiles137/140/152/512, full189-stage routing and fail-closed receipts\n",cases);return 0;
}
