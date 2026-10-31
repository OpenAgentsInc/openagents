/* Firm reason2: independent variable-length graph/ABI oracle; no learned CPU
   arithmetic or shader execution. Old140 metadata is an immutable reference. */
#include "action-dynamic-plan.h"
#include <assert.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
static size_t cases;
static int bytes(int kernel,int arg,int argc){if(kernel&&arg==argc-1)return 1;if(kernel==4&&(arg==9||arg==12))return 1;if(kernel==4&&arg==10)return 8;if(kernel==5&&arg==2)return 1;return 4;}
static void same(coax_t_GvpNode a,coax_t_GvpNode b){assert(a.status==b.status&&a.kernel==b.kernel&&a.items==b.items&&a.lanes==b.lanes&&a.block==b.block&&a.argc==b.argc&&a.flag_count==b.flag_count&&a.flag_offset==b.flag_offset);for(int i=0;i<a.argc;i++){coax_t_GvpArg x=a.args.e[i],y=b.args.e[i];assert(x.kind==y.kind&&x.integer==y.integer&&x.buffer==y.buffer&&x.count==y.count&&x.offset==y.offset);assert(!memcmp(&x.real,&y.real,4));}cases++;}
static void tensor_contract(int tokens,coax_t_GacStage s,coax_t_GvpNode n,coax_t_GvpNode old){int vl=s.kind==0||s.kind==1,cross=s.kind==4&&s.layer%2==0;int64_t q=vl?tokens:41,k=vl||cross?tokens:41,width=vl?2048:1536;assert(n.kernel==old.kernel&&n.argc==old.argc&&n.lanes==old.lanes&&n.block==old.block);
 if(n.kernel==0){int64_t t=vl||(cross&&old.args.e[4].buffer==542)?tokens:old.args.e[2].integer;int64_t rows=n.args.e[0].integer,cols=n.args.e[1].integer;assert(n.args.e[2].integer==t&&n.args.e[4].count==t*cols&&n.args.e[5].count==t*rows&&n.items==((rows+63)/64)*((t+15)/16)&&!n.flag_count);}
 if(n.kernel==1){int64_t rows=vl?tokens:old.items,w=n.args.e[0].integer;assert(n.items==rows&&n.flag_count==rows&&n.args.e[3].count==rows*w&&n.args.e[6].count==rows*w);}
 if(n.kernel==7){int64_t t=vl||(cross&&(old.args.e[5].buffer==548||old.args.e[5].buffer==549))?tokens:old.items/old.args.e[0].integer,count=t*n.args.e[0].integer;assert(n.items==count&&n.flag_count==count&&n.args.e[2].count==count&&n.args.e[5].count==count&&n.args.e[4].count==(n.args.e[1].integer==1?count:0));}
 if(n.kernel==3){int64_t count=vl?tokens*8192:old.items;assert(n.items==count&&n.flag_count==count&&n.args.e[1].count==count&&n.args.e[2].count==count);}
 if(n.kernel==4){assert(n.args.e[0].integer==q&&n.args.e[1].integer==k&&n.args.e[7].count==q*width&&n.args.e[8].count==k*width&&n.args.e[11].count==q*32*k&&n.args.e[12].count==q*32*k&&n.flag_count==q*32*k&&n.items==q*32*k);}
 if(n.kernel==5){assert(n.args.e[0].integer==k&&n.args.e[1].count==q*32*k&&n.args.e[2].count==q*32*k&&n.args.e[3].count==q*32*k&&n.items==q*32&&n.flag_count==q*32);}
 if(n.kernel==6){assert(n.args.e[0].integer==q&&n.args.e[1].integer==k&&n.args.e[5].count==q*32*k&&n.args.e[6].count==k*width&&n.args.e[7].count==q*width&&n.items==q*width&&n.flag_count==q*width);}
 cases++;
}
static void graph(int tokens){assert(gad_tokens(tokens));for(int b=0;b<568;b++){int64_t capacity=gad_buffer_bytes_for(tokens,b);assert(capacity>0);if(tokens==140)assert(capacity==gac_buffer_bytes(b));cases++;}
 for(int seq=0;seq<142;seq++){coax_t_GacStage s=gad_stage_for(tokens,seq);assert(!s.status&&s.nodes==gac_stage(seq).nodes&&s.flags<=gad_buffer_bytes_for(tokens,567));int64_t offset=0;
  for(int at=0;at<s.nodes;at++){coax_t_GvpNode n=gad_node_for(tokens,seq,at),old=gac_node(seq,at);assert(!n.status&&n.flag_offset==offset);tensor_contract(tokens,s,n,old);for(int a=0;a<n.argc;a++){coax_t_GvpArg v=n.args.e[a];if(v.kind==4)assert(v.buffer>=0&&v.buffer<568&&v.count>=0&&v.offset>=0&&v.offset+v.count*bytes((int)n.kernel,a,(int)n.argc)<=gad_buffer_bytes_for(tokens,v.buffer));}if(n.flag_count){coax_t_GvpArg v=n.args.e[n.argc-1];assert(v.kind==4&&v.buffer==567&&v.count==n.flag_count&&v.offset==offset);}offset+=n.flag_count;if(tokens==140)same(n,old);}
  assert(offset==s.flags);assert(gad_node_for(tokens,seq,-1).status&&gad_node_for(tokens,seq,s.nodes).status);
  for(int slot=0;slot<3;slot++){coax_t_GacCapture c=gad_capture_for(tokens,seq,slot),old=gac_capture(seq,slot);assert(c.ok==old.ok);if(c.ok){assert(c.buffer==old.buffer&&c.offset==old.offset&&c.name.count==old.name.count&&!memcmp(c.name.data,old.name.data,(size_t)c.name.count));assert(c.count==(c.buffer==542?tokens*2048:old.count));}cases++;}
 }
}
static void abi(int tokens){int64_t ids[512]={0};uint8_t text[513],image[513],flags[512]={0};ids[4]=151655;memset(text,9,sizeof text);memset(image,9,sizeof image);assert(!gad_masks_for(tokens,(coax_v_s64){ids,tokens},(coax_v_u8){text,tokens},(coax_v_u8){image,tokens},0));for(int i=0;i<tokens;i++){assert(image[i]==(i==4)&&text[i]==(i!=4));cases++;}assert(text[tokens]==9&&image[tokens]==9);
 for(int field=0;field<6;field++){int64_t ni=tokens,nt=tokens,nm=tokens;int overlap=0;ids[tokens-1]=0;memset(text,9,sizeof text);memset(image,9,sizeof image);if(field==0)ni--;if(field==1)nt--;if(field==2)nm--;if(field==3)overlap=1;if(field==4)ids[tokens-1]=-1;if(field==5)ids[tokens-1]=151936;assert(gad_masks_for(tokens,(coax_v_s64){ids,ni},(coax_v_u8){text,nt},(coax_v_u8){image,nm},overlap));for(int i=0;i<513;i++)assert(text[i]==9&&image[i]==9);cases++;}
 coax_t_GadCursor cursor=gad_begin_for(tokens);coax_v_t_GadCursor state={&cursor,1};assert(!gad_sent_for(state,tokens,0,0,0));assert(!gad_done_for(state,tokens,0,0,(coax_v_u8){flags,tokens})&&cursor.stage==1&&!cursor.node);cases++;
 cursor=gad_begin_for(tokens);assert(gad_sent_for(state,tokens==140?141:140,0,0,0)==COAX_GvpStatus_order);cursor=gad_begin_for(tokens);assert(gad_done_for(state,tokens,0,0,(coax_v_u8){flags,tokens})==COAX_GvpStatus_order);cases+=2;
 uint32_t raw[5280]={0};float out[5281];for(int i=0;i<5281;i++)out[i]=7;raw[0]=0x40000000;raw[5279]=0xc0400000;cursor=(coax_t_GadCursor){.tokens=tokens,.stage=142};coax_t_GvpRead r=gad_publish_for(state,tokens,(coax_v_u32){raw,5280},(coax_v_f32){out,5281},0);assert(!r.status&&r.written==5280&&cursor.stage==143&&out[0]==2&&out[5279]==-3&&out[5280]==7);cases++;
}
int main(void){int lengths[]={137,140,152,160,232,233,512};for(int i=0;i<7;i++){graph(lengths[i]);abi(lengths[i]);}assert(!gad_tokens(136)&&!gad_tokens(513));assert(gad_stage_for(136,0).status&&gad_node_for(152,142,0).status);assert(gad_buffer_bytes_for(512,567)==23086080);printf("GR00T dynamic action: %zu independent graph/ABI cases passed; seven token lengths, exact old140 program\n",cases);return 0;}
