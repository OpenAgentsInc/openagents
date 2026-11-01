/* B1 maintenance observer: real lexer/parser/checker, no borrow resolver. */
#include "lexer.h"
#include "parser.h"
#include "checker.h"
#undef NDEBUG
#include <assert.h>
#include <inttypes.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

/* Expectations derived from the approved declarations, not observed layouts.
 * The freshly generated headers carry the complete model's layout assertions.
 * No Coax entry point is called before this translation unit compiles. */
#define ABI(T,S) _Static_assert(sizeof(T)==S && _Alignof(T)==8,"LP64 " #T)
#define OFF(T,F,N) _Static_assert(offsetof(T,F)==N,"LP64 " #T "." #F)
_Static_assert(sizeof(void*)==8 && sizeof(int64_t)==8,"LP64 observer");
ABI(coax_t_BS_Origin,168);
ABI(coax_t_CK_RetainEffect,368);
OFF(coax_t_CK_RetainEffect,source,0);
OFF(coax_t_CK_RetainEffect,destination,168);
OFF(coax_t_CK_RetainEffect,value_type,336);
OFF(coax_t_CK_RetainEffect,witness,344);
OFF(coax_t_CK_RetainEffect,next,352);
OFF(coax_t_CK_RetainEffect,source_storage,360);
ABI(coax_t_CK_RetainObservation,40);
OFF(coax_t_CK_RetainObservation,kind,0);
OFF(coax_t_CK_RetainObservation,owner,8);
OFF(coax_t_CK_RetainObservation,site,16);
OFF(coax_t_CK_RetainObservation,facts_complete,24);
OFF(coax_t_CK_RetainObservation,relation_complete,25);
OFF(coax_t_CK_RetainObservation,forbidden,26);
OFF(coax_t_CK_RetainObservation,pending,32);
ABI(coax_t_CK_RetainCaps,24);
OFF(coax_t_CK_RetainCaps,effects,0);
OFF(coax_t_CK_RetainCaps,reports,8);
OFF(coax_t_CK_RetainCaps,work,16);

typedef struct {
 int kind; const char *owner; int64_t line,column;
 int facts,relation,forbidden; int64_t pending;
} ExpectedReport;
typedef struct {
 const char *id,*file,*source; int success; const char *error;
 int64_t error_line,error_column;
 const ExpectedReport *reports; size_t report_count;
} Fixture;
#include "retained-borrow-native-vectors.inc"

static void *allocate(int64_t count,size_t width) {
 assert(count>0 && (uint64_t)count<=SIZE_MAX/width);
 void *result=calloc((size_t)count,width);assert(result);return result;
}
static int text_equal(coax_string text,const char *expected) {
 size_t n=strlen(expected);return text.count==(int64_t)n && (n==0 || !memcmp(text.data,expected,n));
}
static coax_t_CK_Loc location(coax_t_PS_Loc loc) {
 return (coax_t_CK_Loc){loc.line,loc.column,loc.offset};
}
static unsigned cases,source_cases,negative_cases,matched_reports,disabled_controls,short_controls,observations;
static void json_bytes(const char *bytes,int64_t count) {
 assert(count>=0);fputc('"',stdout);
 for(int64_t i=0;i<count;i++) {
  unsigned char c=(unsigned char)bytes[i];
  if(c=='"' || c=='\\'){fputc('\\',stdout);fputc(c,stdout);}
  else if(c<32 || c>=127)printf("\\u%04x",c);
  else fputc(c,stdout);
 }
 fputc('"',stdout);
}
static void json_text(coax_string text){json_bytes(text.data,text.count);}
static void json_cstring(const char *text){json_bytes(text,(int64_t)strlen(text));}
static void dump_observation(const Fixture *fixture,int mode,int ok,coax_t_CK_Context c,int64_t capacity) {
 coax_t_CK_State s=c.state.data[0];
 if(observations++)fputc(',',stdout);
 fputs("{\"fixture_id\":",stdout);json_cstring(fixture->id);
 fputs(",\"input_source\":",stdout);json_cstring(fixture->file);
 printf(",\"mode\":%d,\"ck_ok\":%s,\"fatal\":%s,\"error_count\":%"PRId64",\"committed_report_count\":%"PRId64",\"errors\":[",
        mode,ok?"true":"false",s.fatal?"true":"false",s.error_count,s.retain_report_count);
 if(s.error_count>=0 && s.error_count<=c.errors.count)for(int64_t i=0;i<s.error_count;i++) {
  if(i)fputc(',',stdout);coax_t_CK_Error e=c.errors.data[i];
  fputs("{\"code\":",stdout);json_text(e.code);
  printf(",\"line\":%"PRId64",\"column\":%"PRId64",\"offset\":%"PRId64"}",e.loc.line,e.loc.column,e.loc.offset);
 }
 int admitted=ok && !s.fatal && s.error_count==0 && mode!=1 && s.retain_report_count>=0 && s.retain_report_count<=capacity;
 printf("],\"report_prefix_admitted\":%s,\"reports\":[",admitted?"true":"false");
 /* Rejection never makes a prefix readable, even if a buggy count is nonzero. */
 if(admitted)for(int64_t i=0;i<s.retain_report_count;i++) {
  if(i)fputc(',',stdout);coax_t_CK_RetainObservation r=c.retain_reports.data[i];
  int owner_valid=r.owner>=2 && r.owner<=s.used && r.owner<c.nodes.count;
  int site_valid=r.site>=2 && r.site<=s.used && r.site<c.nodes.count;
  printf("{\"kind\":%u,\"owner\":%"PRId64",\"site\":%"PRId64",\"owner_tag\":",(unsigned)r.kind,r.owner,r.site);
  if(owner_valid)printf("%u",(unsigned)c.nodes.data[r.owner].kind);else fputs("null",stdout);
  fputs(",\"owner_name\":",stdout);if(owner_valid)json_text(c.nodes.data[r.owner].name);else fputs("null",stdout);
  fputs(",\"site_tag\":",stdout);if(site_valid)printf("%u",(unsigned)c.nodes.data[r.site].kind);else fputs("null",stdout);
  fputs(",\"site_loc\":",stdout);
  if(site_valid){coax_t_CK_Loc l=c.nodes.data[r.site].loc;printf("{\"line\":%"PRId64",\"column\":%"PRId64",\"offset\":%"PRId64"}",l.line,l.column,l.offset);}
  else fputs("null",stdout);
  printf(",\"facts_complete\":%s,\"relation_complete\":%s,\"forbidden\":%s,\"pending\":%"PRId64"}",
         r.facts_complete?"true":"false",r.relation_complete?"true":"false",r.forbidden?"true":"false",r.pending);
 }
 fputs("]}",stdout);fflush(stdout);
}

/* mode 0 requests reports; 1 disables them; 2 requests an insufficient prefix.
 * Token bytes and indexed fields are copied mechanically. All semantic facts
 * and workspace capacities come from the exported Coax procedures. */
static void observe(const Fixture *fixture,int mode) {
 size_t source_size=strlen(fixture->source);assert(source_size<INT64_MAX-3);
 int64_t nsource=(int64_t)source_size,ntoken=0;
 coax_string text={fixture->source,nsource};
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
 dump_observation(fixture,mode,ok,context,report_capacity);
 assert(before[0]==91 && before[1]==92 && after[0]==93 && after[1]==94);
 if(mode==2) {
  assert(!ok && state.error_count==1 && text_equal(errors[0].code,"E0381"));
  assert(state.retain_report_count==0);short_controls++;
 }else if(!fixture->success) {
  assert(!ok && state.error_count==1 && text_equal(errors[0].code,fixture->error));
  assert(errors[0].loc.line==fixture->error_line && errors[0].loc.column==fixture->error_column);
  assert(state.retain_report_count==0);negative_cases++;
 }else {
  assert(ok && !state.fatal && state.error_count==0);
  if(mode==1){assert(state.retain_report_count==0);disabled_controls++;}
  else {
   assert(state.retain_report_count>0 && state.retain_report_count<=report_capacity);
   for(int64_t i=0;i<state.retain_report_count;i++) {
    coax_t_CK_RetainObservation row=reports[i];
    assert(row.kind==1 || row.kind==2);assert(row.owner>=2 && row.owner<=state.used);
    assert(row.site>=2 && row.site<=state.used);assert(nodes[row.owner].kind==COAX_CK_Tag_proc);
    assert(row.pending>=0 && row.pending<=64);
    if(row.kind==1)assert(row.site==row.owner);
    else assert(nodes[row.site].kind==COAX_CK_Tag_expr_call);
   }
   for(size_t e=0;e<fixture->report_count;e++) {
    const ExpectedReport *want=&fixture->reports[e];unsigned matches=0;
    for(int64_t i=0;i<state.retain_report_count;i++) {
     coax_t_CK_RetainObservation row=reports[i];coax_t_CK_Node owner=nodes[row.owner],site=nodes[row.site];
     if(row.kind!=want->kind || !text_equal(owner.name,want->owner) || site.loc.line!=want->line || site.loc.column!=want->column)continue;
     assert(row.facts_complete==want->facts && row.relation_complete==want->relation);
     assert(row.forbidden==want->forbidden && row.pending==want->pending);matches++;
    }
    assert(matches==1);matched_reports++;
   }
  }
 }
 cases++;if(mode==0)source_cases++;
 free(guarded);free(effects);free(origins);free(queries);free(bytes);free(errors);free(locals);free(types);free(nodes);
 free(parse_message);free(parsed);for(int64_t i=0;i<ntoken;i++)free(owned[i]);
 free(owned);free(tokens);free(lex_message);free(decoded);
}

int main(void) {
 fputs("{\"observations\":[",stdout);
 for(size_t i=0;i<sizeof(fixtures)/sizeof(fixtures[0]);i++)observe(&fixtures[i],0);
 observe(&fixtures[1],1);observe(&fixtures[1],2);
 printf("],\"cases\":%u,\"source_cases\":%u,\"negative_cases\":%u,\"matched_reports\":%u,\"disabled_controls\":%u,\"short_controls\":%u,\"node_size\":%zu,\"state_size\":%zu,\"context_size\":%zu,\"effect_size\":%zu,\"effect_source_storage_offset\":%zu,\"observation_size\":%zu,\"caps_size\":%zu}\n",
   cases,source_cases,negative_cases,matched_reports,disabled_controls,short_controls,
   sizeof(coax_t_CK_Node),sizeof(coax_t_CK_State),sizeof(coax_t_CK_Context),
   sizeof(coax_t_CK_RetainEffect),offsetof(coax_t_CK_RetainEffect,source_storage),sizeof(coax_t_CK_RetainObservation),sizeof(coax_t_CK_RetainCaps));
}
