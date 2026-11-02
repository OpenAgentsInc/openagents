/* B2 independent source observer. Lexer/parser/CK_run transport is reused
 * mechanically from retained-borrows-native.c. No native borrow resolver.
 * Query rows are test-only internal observations after a successful CK_run,
 * not a public diagnostics API or host-lifetime proof. */
#include "lexer.h"
#include "parser.h"
#include "checker.h"
#include <assert.h>
#include <inttypes.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
static void *allocate(int64_t count,size_t width){assert(count>0&&(uint64_t)count<=SIZE_MAX/width);void*r=calloc((size_t)count,width);assert(r);return r;}
static coax_t_CK_Loc location(coax_t_PS_Loc x){return(coax_t_CK_Loc){x.line,x.column,x.offset};}
static void json_text(coax_string x){putchar('"');for(int64_t i=0;i<x.count;i++){unsigned char c=x.data[i];if(c=='"'||c=='\\'){putchar('\\');putchar(c);}else if(c<32||c>=127)printf("\\u%04x",c);else putchar(c);}putchar('"');}
static void path(coax_t_BS_Path p){assert(p.count>=0&&p.count<=8);putchar('[');for(int64_t i=0;i<p.count;i++){if(i)putchar(',');printf("[%u,%"PRId64"]",p.steps.e[i].kind,p.steps.e[i].index);}putchar(']');}
static void observe(const char *source){int mode=1;
 size_t source_size=strlen(source);assert(source_size<INT64_MAX-3);
 int64_t nsource=(int64_t)source_size,ntoken=0;
 coax_string text={source,nsource};
 coax_t_LX_Caps lex_caps=LX_capacities(nsource);
 uint8_t *decoded=allocate(lex_caps.decoded,1),*lex_message=allocate(lex_caps.diagnostic,1);
 coax_t_PS_Token *tokens=allocate(nsource+3,sizeof(*tokens));
 char **owned=allocate(nsource+3,sizeof(*owned));
 coax_t_LX_State lex=LX_begin(nsource);
 int ended=0;
 for(int64_t i=0;i<nsource+3;i++) {
  coax_t_LX_Item item=LX_next(text,(coax_v_t_LX_State){&lex,1},
      (coax_v_u8){decoded,lex_caps.decoded},(coax_v_u8){lex_message,lex_caps.diagnostic});
  assert(item.diagnostic.code.count==0);
  if(!item.has_token)continue;
  coax_t_LX_Token v=item.token;assert(v.text_begin>=0 && v.text_end>=v.text_begin && v.text_end<=nsource);
  coax_string spelling={text.data+v.text_begin,v.text_end-v.text_begin};
  if(v.decoded) {
   assert(v.decoded_count>=0 && v.decoded_count<=lex_caps.decoded);
   owned[ntoken]=allocate(v.decoded_count+1,1);
   memcpy(owned[ntoken],decoded,(size_t)v.decoded_count);
   spelling=(coax_string){owned[ntoken],v.decoded_count};
  }
  coax_t_PS_Token *p=&tokens[ntoken++];p->kind=v.kind;p->text=spelling;
  p->loc=(coax_t_PS_Loc){v.loc.line,v.loc.column,v.loc.offset};
  p->value=v.int_value;memcpy(&p->floating,&v.float_value,sizeof(p->floating));
  p->width=v.float_bits;p->raw_count=v.text_end-v.text_begin;
  if(v.kind==COAX_LX_Kind_k_eof){ended=1;break;}
 }
 assert(ended);
 coax_t_PS_Caps parse_caps=PS_caps(ntoken,nsource);
 coax_t_PS_Node *parsed=allocate(parse_caps.nodes,sizeof(*parsed));
 uint8_t *parse_message=allocate(parse_caps.message,1);coax_t_PS_State ps={0};
 coax_t_PS_Result parsed_result=PS_parse(text,(coax_v_t_PS_Token){tokens,ntoken},
     (coax_v_t_PS_Node){parsed,parse_caps.nodes},(coax_v_t_PS_State){&ps,1},
     (coax_v_u8){parse_message,parse_caps.message});
 assert(parsed_result.ok && ps.used>=1 && ps.used<parse_caps.nodes);
 coax_t_CK_Caps caps=CK_caps(ps.used+1,nsource);
 coax_t_CK_Node *nodes=allocate(caps.nodes,sizeof(*nodes));assert(caps.nodes>ps.used);
 for(int64_t i=0;i<=ps.used;i++) {
  coax_t_PS_Node v=parsed[i];coax_t_CK_Node *n=&nodes[i];
  n->kind=v.kind;n->loc=location(v.loc);n->loc2=location(v.loc2);
  n->name=v.name;n->name2=v.name2;n->a=v.a;n->b=v.b;n->c=v.c;n->d=v.d;
  n->head=v.head;n->tail=v.tail;n->next=v.next;n->op=v.op;n->flag=v.flag;n->flag2=v.flag2;
  n->value=v.value;memcpy(&n->floating,&v.floating,sizeof(n->floating));n->field_index=-1;
  n->quantity=v.quantity;n->unit=(coax_t_Q_Unit){v.unit.e[0],v.unit.e[1],v.unit.e[2],v.unit.e[3],v.unit.e[4],v.unit.e[5]};
  n->frame_decl=v.frame_decl;n->frame=(coax_t_SP_Frame){v.frame.e[0],(uint8_t)v.frame.e[1],(uint8_t)v.frame.e[2],v.frame.e[3]};
  if(v.kind==COAX_PS_Tag_expr_float_lit && v.width)n->type=v.width==32?6:7;
  if(v.kind==COAX_PS_Tag_decl_field)n->field_index=0;
 }
 int64_t extra=CK_meta_capacity((coax_v_t_CK_Node){nodes,caps.nodes},ps.used);
 assert(extra>=0 && extra<=INT64_MAX-caps.nodes);
 if(extra) {
  assert((uint64_t)(caps.nodes+extra)<=SIZE_MAX/sizeof(*nodes));
  void *expanded=realloc(nodes,(size_t)(caps.nodes+extra)*sizeof(*nodes));assert(expanded);nodes=expanded;
  memset(nodes+caps.nodes,0,(size_t)extra*sizeof(*nodes));caps.nodes+=extra;
 }
 coax_t_CK_Type *types=allocate(caps.types,sizeof(*types));
 coax_t_CK_Local *locals=allocate(caps.locals,sizeof(*locals));
 coax_t_CK_Error *errors=allocate(caps.errors,sizeof(*errors));
 uint8_t *bytes=allocate(caps.bytes,1);coax_t_CK_State state={0};
 /* This is an old committed count, not a valid output prefix for this run. */
 state.retain_report_count=777;
 coax_t_CK_BorrowCaps borrow_caps=CK_borrow_caps(ps.used+1);
 coax_t_CK_BorrowQuery *queries=allocate(borrow_caps.queries,sizeof(*queries));
 coax_t_BS_Origin *origins=allocate(borrow_caps.origins,sizeof(*origins));
 coax_t_CK_RetainCaps retain_caps=CK_retain_caps(ps.used+1);
 coax_t_CK_RetainEffect *effects=allocate(retain_caps.effects,sizeof(*effects));
 assert(retain_caps.reports>1);
 int64_t report_capacity=mode==2?1:retain_caps.reports;
 assert((uint64_t)report_capacity<=(SIZE_MAX-32)/sizeof(coax_t_CK_RetainObservation));
 uint8_t *guarded=allocate(32+report_capacity*(int64_t)sizeof(coax_t_CK_RetainObservation),1);
 uint64_t *before=(uint64_t*)guarded;
 coax_t_CK_RetainObservation *reports=(void*)(guarded+16);
 uint64_t *after=(void*)(guarded+16+(size_t)report_capacity*sizeof(*reports));
 before[0]=91;before[1]=92;after[0]=93;after[1]=94;
 coax_t_CK_Context context={0};
 context.nodes=(coax_v_t_CK_Node){nodes,caps.nodes};context.types=(coax_v_t_CK_Type){types,caps.types};
 context.locals=(coax_v_t_CK_Local){locals,caps.locals};context.errors=(coax_v_t_CK_Error){errors,caps.errors};
 context.bytes=(coax_v_u8){bytes,caps.bytes};context.state=(coax_v_t_CK_State){&state,1};
 context.borrow_queries=(coax_v_t_CK_BorrowQuery){queries,borrow_caps.queries};
 context.borrow_origins=(coax_v_t_BS_Origin){origins,borrow_caps.origins};
 context.retain_effects=(coax_v_t_CK_RetainEffect){effects,retain_caps.effects};
 if(mode!=1)context.retain_reports=(coax_v_t_CK_RetainObservation){reports,report_capacity};
 int ok=CK_run(context,ps.used,(coax_t_CK_Options){.library=1,.authority=parsed[1].flag});

 printf("{\"ok\":%s,\"fatal\":%s,\"errors\":[",ok?"true":"false",state.fatal?"true":"false");
 assert(state.error_count>=0&&state.error_count<=caps.errors);
 for(int64_t i=0;i<state.error_count;i++){if(i)putchar(',');printf("{\"code\":");json_text(errors[i].code);printf(",\"line\":%"PRId64",\"column\":%"PRId64"}",errors[i].loc.line,errors[i].loc.column);}
 fputs("],\"procedures\":[",stdout);int comma=0;
 if(ok&&!state.fatal&&!state.error_count)for(int64_t id=2;id<=state.used;id++)if(nodes[id].kind==COAX_CK_Tag_proc&&!nodes[id].generic&&!nodes[id].flag2){
  if(comma++)putchar(',');fputs("{\"name\":",stdout);json_text(nodes[id].name);printf(",\"line\":%"PRId64",\"borrow_status\":%u,\"retain_incomplete\":%s,\"retain_relation\":%s,\"retain_pending\":%"PRId64",\"queries\":[",nodes[id].loc.line,nodes[id].borrow_status,nodes[id].retain_incomplete?"true":"false",nodes[id].retain_relation?"true":"false",nodes[id].retain_count);int qc=0;int64_t link=nodes[id].borrow_query;
  while(link){assert(link>0&&link<=borrow_caps.queries);coax_t_CK_BorrowQuery q=queries[link-1];assert(q.decl==id&&q.next>=0&&q.next<link);if(q.kind==1){assert(q.status==2||q.status==3);assert(q.count>=0&&q.count<=64&&q.start>=0&&q.start+q.count<=borrow_caps.origins);if(qc++)putchar(',');printf("{\"status\":%u,\"path\":",q.status);path(q.path);printf(",\"origins\":[");for(int64_t i=0;i<q.count;i++){coax_t_BS_Origin o=origins[q.start+i];if(i)putchar(',');printf("{\"kind\":%u,\"root\":%"PRId64",\"role\":%u,\"type\":%"PRId64",\"path\":",o.kind,o.root,o.role,o.type);path(o.path);putchar('}');}fputs("]}",stdout);}link=q.next;
  }fputs("]}",stdout);
 }
 fputs("]}\n",stdout);
 assert(before[0]==91&&before[1]==92&&after[0]==93&&after[1]==94);
 free(guarded);free(effects);free(origins);free(queries);free(bytes);free(errors);free(locals);free(types);free(nodes);free(parse_message);free(parsed);for(int64_t i=0;i<ntoken;i++)free(owned[i]);free(owned);free(tokens);free(lex_message);free(decoded);
}
int main(int argc,char**argv){assert(argc==2);FILE*f=fopen(argv[1],"rb");assert(f);assert(!fseek(f,0,SEEK_END));long n=ftell(f);assert(n>=0);rewind(f);char*source=allocate(n+1,1);assert(fread(source,1,(size_t)n,f)==(size_t)n);assert(!fclose(f));observe(source);free(source);return 0;}
