/* Independent literal A0 oracles. This is test tooling, not a borrow resolver. */
#include "borrow-summary.h"
#undef NDEBUG
#include <assert.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>

#define ABI(T,S,A) _Static_assert(sizeof(T)==S && _Alignof(T)==A,"LP64 " #T)
#define OFFSET(T,F,N) _Static_assert(offsetof(T,F)==N,"LP64 " #T "." #F)
_Static_assert(sizeof(void*)==8 && sizeof(int64_t)==8,"named native LP64 profile");
ABI(coax_t_BS_Step,16,8); OFFSET(coax_t_BS_Step,kind,0); OFFSET(coax_t_BS_Step,index,8);
ABI(coax_t_BS_Path,392,8); OFFSET(coax_t_BS_Path,count,0); OFFSET(coax_t_BS_Path,steps,8);
_Static_assert(sizeof(((coax_t_BS_Path*)0)->steps.e)==384,"twenty-four inline steps");
ABI(coax_t_BS_Origin,424,8); OFFSET(coax_t_BS_Origin,kind,0); OFFSET(coax_t_BS_Origin,root,8);
OFFSET(coax_t_BS_Origin,role,16); OFFSET(coax_t_BS_Origin,type,24); OFFSET(coax_t_BS_Origin,path,32);
ABI(coax_t_BS_Binding,464,8); OFFSET(coax_t_BS_Binding,formal,0); OFFSET(coax_t_BS_Binding,actual_type,424);
OFFSET(coax_t_BS_Binding,complete,432); OFFSET(coax_t_BS_Binding,callee_root,440);
OFFSET(coax_t_BS_Binding,start,448); OFFSET(coax_t_BS_Binding,count,456);
ABI(coax_t_BS_Result,16,8); OFFSET(coax_t_BS_Result,status,0); OFFSET(coax_t_BS_Result,count,8);
ABI(coax_v_t_BS_Origin,16,8); OFFSET(coax_v_t_BS_Origin,data,0); OFFSET(coax_v_t_BS_Origin,count,8);
ABI(coax_v_t_BS_Binding,16,8); OFFSET(coax_v_t_BS_Binding,data,0); OFFSET(coax_v_t_BS_Binding,count,8);

enum { OK=0, INVALID=1, UNKNOWN=2, CAPACITY=3, WORK=4 };
enum { STATIC=1, LOCAL=2, PARAMETER=3, INLINE=1, DESCRIPTOR=2, FIELD=1, ELEMENT=2, DEREFERENCE=3 };
static unsigned cases,calls,refusals;
static coax_t_BS_Origin origin(uint8_t kind,int64_t root,uint8_t role,int64_t type) {
  coax_t_BS_Origin r={0};r.kind=kind;r.root=root;r.role=role;r.type=type;return r;
}
static coax_t_BS_Origin projected(uint8_t kind,int64_t root,uint8_t role,int64_t type,uint8_t step,int64_t index) {
  coax_t_BS_Origin r=origin(kind,root,role,type);r.path.count=1;r.path.steps.e[0].kind=step;r.path.steps.e[0].index=index;return r;
}
static coax_t_BS_Binding binding(coax_t_BS_Origin formal,int complete,int64_t callee,int64_t start,int64_t count) {
  coax_t_BS_Binding b={0};b.formal=formal;b.actual_type=formal.type;b.complete=complete;b.callee_root=callee;b.start=start;b.count=count;return b;
}
static int same(coax_t_BS_Origin a,coax_t_BS_Origin b) {
  if(a.kind!=b.kind || a.root!=b.root || a.role!=b.role || a.type!=b.type || a.path.count!=b.path.count)return 0;
  for(int i=0;i<24;i++)if(a.path.steps.e[i].kind!=b.path.steps.e[i].kind || a.path.steps.e[i].index!=b.path.steps.e[i].index)return 0;
  return 1;
}
struct Guard { uint64_t before[2];coax_t_BS_Origin out[66];uint64_t after[2]; };
static void prepare(struct Guard *g) {
  memset(g,0xa7,sizeof(*g));g->before[0]=91;g->before[1]=92;g->after[0]=93;g->after[1]=94;
}
static void finish(struct Guard *g,const struct Guard *old,coax_t_BS_Result got,int status,const coax_t_BS_Origin *expected,int64_t count) {
  cases++;assert(got.status==status);assert(got.count==(status==OK?count:0));
  assert(!memcmp(g->before,old->before,sizeof(g->before)));assert(!memcmp(g->after,old->after,sizeof(g->after)));
  if(status!=OK){refusals++;assert(!memcmp(g->out,old->out,sizeof(g->out)));return;}
  for(int64_t i=0;i<count;i++)assert(same(g->out[i],expected[i]));
  /* Prefix padding is unspecified; the entire untouched tail stays byte-exact. */
  assert(!memcmp(g->out+count,old->out+count,(66-(size_t)count)*sizeof(g->out[0])));
}
static void expect_union(coax_t_BS_Origin *a,int64_t na,int complete_a,coax_t_BS_Origin *b,int64_t nb,int complete_b,int64_t out_count,int status,coax_t_BS_Origin *expected,int64_t n) {
  struct Guard g,old;prepare(&g);memcpy(&old,&g,sizeof(g));
  unsigned char saved_a[65*sizeof(coax_t_BS_Origin)],saved_b[65*sizeof(coax_t_BS_Origin)];
  if(na>0 && na<=65)memcpy(saved_a,a,(size_t)na*sizeof(*a));if(nb>0 && nb<=65)memcpy(saved_b,b,(size_t)nb*sizeof(*b));
  calls++;coax_t_BS_Result r=BS_union((coax_v_t_BS_Origin){a,na},complete_a,(coax_v_t_BS_Origin){b,nb},complete_b,(coax_v_t_BS_Origin){g.out,out_count});
  finish(&g,&old,r,status,expected,n);
  if(na>0 && na<=65)assert(!memcmp(saved_a,a,(size_t)na*sizeof(*a)));if(nb>0 && nb<=65)assert(!memcmp(saved_b,b,(size_t)nb*sizeof(*b)));
}
static void expect_sub(coax_t_BS_Origin *summary,int64_t ns,int complete,coax_t_BS_Binding *bindings,int64_t nb,coax_t_BS_Origin *pool,int64_t np,int64_t out_count,int status,coax_t_BS_Origin *expected,int64_t n) {
  struct Guard g,old;prepare(&g);memcpy(&old,&g,sizeof(g));
  unsigned char ss[65*sizeof(coax_t_BS_Origin)],bs[65*sizeof(coax_t_BS_Binding)],ps[257*sizeof(coax_t_BS_Origin)];
  if(ns>0 && ns<=65)memcpy(ss,summary,(size_t)ns*sizeof(*summary));if(nb>0 && nb<=65)memcpy(bs,bindings,(size_t)nb*sizeof(*bindings));if(np>0 && np<=257)memcpy(ps,pool,(size_t)np*sizeof(*pool));
  calls++;coax_t_BS_Result r=BS_substitute((coax_v_t_BS_Origin){summary,ns},complete,(coax_v_t_BS_Binding){bindings,nb},(coax_v_t_BS_Origin){pool,np},(coax_v_t_BS_Origin){g.out,out_count});
  finish(&g,&old,r,status,expected,n);
  if(ns>0 && ns<=65)assert(!memcmp(ss,summary,(size_t)ns*sizeof(*summary)));if(nb>0 && nb<=65)assert(!memcmp(bs,bindings,(size_t)nb*sizeof(*bindings)));if(np>0 && np<=257)assert(!memcmp(ps,pool,(size_t)np*sizeof(*pool)));
}
static void valid_path(coax_t_BS_Path p,int expected){calls++;cases++;assert(BS_path_valid(p)==expected);}
static void valid_origin(coax_t_BS_Origin o,int expected){calls++;cases++;assert(BS_origin_valid(o)==expected);}

static void vocabulary(void) {
  coax_t_BS_Path p={0};valid_path(p,1);p.count=-1;valid_path(p,0);p.count=25;valid_path(p,0);
  p.count=24;for(int i=0;i<24;i++){p.steps.e[i].kind=FIELD;p.steps.e[i].index=i;}valid_path(p,1);
  p.count=23;p.steps.e[23].kind=0;p.steps.e[23].index=0;valid_path(p,1);p.count=24;p.steps.e[23].kind=FIELD;
  p.steps.e[23].index=INT64_C(2147483647);valid_path(p,1);p.steps.e[23].index=INT64_C(2147483648);valid_path(p,0);
  memset(&p,0,sizeof(p));p.count=1;p.steps.e[0].kind=ELEMENT;p.steps.e[0].index=-1;valid_path(p,0);p.steps.e[0].index=0;valid_path(p,1);
  p.steps.e[0].kind=DEREFERENCE;valid_path(p,1);p.steps.e[0].index=1;valid_path(p,0);p.steps.e[0].kind=4;valid_path(p,1);p.steps.e[0].index=63;valid_path(p,1);p.steps.e[0].index=64;valid_path(p,0);p.steps.e[0].kind=5;valid_path(p,0);
  memset(&p,0,sizeof(p));p.steps.e[23].kind=FIELD;valid_path(p,0);p.steps.e[23].kind=0;p.steps.e[23].index=1;valid_path(p,0);
  coax_t_BS_Origin o=origin(STATIC,1,0,1);valid_origin(o,1);o.root=INT64_C(2147483647);valid_origin(o,1);o.root=INT64_C(2147483648);valid_origin(o,0);
  o=origin(LOCAL,1,0,1);valid_origin(o,1);o.root=0;valid_origin(o,0);o.root=-1;valid_origin(o,0);
  o=origin(PARAMETER,0,INLINE,1);valid_origin(o,1);o.root=63;valid_origin(o,1);o.root=64;valid_origin(o,0);
  o=origin(PARAMETER,0,DESCRIPTOR,1);o.path.count=1;o.path.steps.e[0].kind=DEREFERENCE;valid_origin(o,1);
  o.role=INLINE;valid_origin(o,0);o.kind=STATIC;o.root=1;o.role=0;valid_origin(o,0);
  o=origin(STATIC,1,DESCRIPTOR,1);valid_origin(o,0);o=origin(PARAMETER,0,0,1);valid_origin(o,0);
  o=origin(STATIC,1,0,0);valid_origin(o,0);o.type=INT64_C(2147483648);valid_origin(o,0);o.type=1;o.kind=0;valid_origin(o,0);
}
static void unions(void) {
  coax_t_BS_Origin a[65],b[65],e[64];
  a[0]=origin(PARAMETER,0,DESCRIPTOR,7);a[1]=origin(STATIC,2,0,7);a[2]=origin(LOCAL,5,0,7);
  b[0]=projected(STATIC,1,0,9,FIELD,3);b[1]=origin(STATIC,1,0,8);b[2]=a[1];
  e[0]=b[1];e[1]=b[0];e[2]=a[1];e[3]=a[2];e[4]=a[0];
  expect_union(a,3,1,b,3,1,6,OK,e,5);expect_union(a,3,1,b,3,1,4,CAPACITY,0,0);
  a[0]=projected(PARAMETER,0,DESCRIPTOR,7,FIELD,4);a[1]=origin(PARAMETER,0,INLINE,7);
  a[2]=projected(PARAMETER,0,DESCRIPTOR,7,ELEMENT,0);a[3]=projected(PARAMETER,0,DESCRIPTOR,7,DEREFERENCE,0);
  b[0]=projected(PARAMETER,0,DESCRIPTOR,7,FIELD,2);b[0].path.count=2;b[0].path.steps.e[1].kind=FIELD;
  b[1]=projected(PARAMETER,0,DESCRIPTOR,7,FIELD,2);b[2]=origin(PARAMETER,0,DESCRIPTOR,7);
  e[0]=a[1];e[1]=b[2];e[2]=b[1];e[3]=a[0];e[4]=a[2];e[5]=a[3];e[6]=b[0];
  expect_union(a,4,1,b,3,1,7,OK,e,7); /* role, depth, step kind, then step index */
  expect_union(0,0,1,0,0,1,0,OK,0,0);expect_union(0,0,0,0,0,1,2,UNKNOWN,0,0);
  expect_union(a,-1,1,0,0,1,2,INVALID,0,0);expect_union(a,1,1,0,0,1,-1,INVALID,0,0);
  expect_union(a,1,1,0,0,1,65,INVALID,0,0);expect_union(a,1,0,0,0,1,0,UNKNOWN,0,0);
  b[0]=origin(0,0,0,0);expect_union(a,1,0,b,1,1,3,INVALID,0,0);
  for(int i=0;i<65;i++)a[i]=origin(STATIC,i+1,0,1);
  expect_union(a,63,1,0,0,1,63,OK,a,63);expect_union(a,64,1,0,0,1,64,OK,a,64);expect_union(a,65,1,0,0,1,64,INVALID,0,0);
  for(int i=0;i<64;i++)b[i]=origin(STATIC,i+65,0,1);
  expect_union(a,64,1,b,64,1,64,CAPACITY,0,0);
  expect_union(a,64,1,a,64,1,64,OK,a,64);
  a[0]=origin(STATIC,1,0,1);a[1]=a[0];expect_union(a,2,1,0,0,1,1,OK,a,1);
  /* Equal logical records with deliberately distinct native padding bytes. */
  memset(&a[0],0x11,sizeof(a[0]));memset(&b[0],0xee,sizeof(b[0]));
  a[0].kind=b[0].kind=STATIC;a[0].root=b[0].root=1;a[0].role=b[0].role=0;a[0].type=b[0].type=1;
  memset(&a[0].path,0,sizeof(a[0].path));memset(&b[0].path,0,sizeof(b[0].path));expect_union(a,1,1,b,1,1,1,OK,a,1);
}
static void substitution(void) {
  coax_t_BS_Origin s[65],p[257],e[64];coax_t_BS_Binding b[65];
  s[0]=projected(PARAMETER,0,DESCRIPTOR,7,FIELD,2);p[0]=origin(STATIC,10,0,7);p[1]=origin(LOCAL,900,0,7);
  b[0]=binding(s[0],1,0,0,1);b[1]=binding(origin(PARAMETER,1,DESCRIPTOR,7),0,0,0,0);
  b[2]=binding(origin(PARAMETER,2,INLINE,7),1,700,0,0);
  expect_sub(s,1,1,b,3,p,2,3,OK,p,1); /* unused local/incomplete actuals do not taint */
  b[0].start=1;expect_sub(s,1,1,b,3,p,2,3,OK,p+1,1); /* preserve existing local pointee, do not rebase */
  s[0]=projected(PARAMETER,0,INLINE,7,FIELD,2);b[0]=binding(s[0],1,800,0,0);e[0]=projected(LOCAL,800,0,7,FIELD,2);
  expect_sub(s,1,1,b,1,p,2,2,OK,e,1); /* by-value inline storage is callee-local */
  b[0].complete=0;expect_sub(s,1,1,b,1,p,2,2,INVALID,0,0);b[0].complete=1;b[0].count=1;expect_sub(s,1,1,b,1,p,2,2,INVALID,0,0);
  s[0]=projected(PARAMETER,0,DESCRIPTOR,7,FIELD,2);b[0]=binding(s[0],1,0,0,0);
  expect_sub(s,1,1,b,1,p,2,0,OK,0,0);b[0].complete=0;expect_sub(s,1,1,b,1,p,2,2,UNKNOWN,0,0);
  expect_sub(s,1,1,0,0,p,2,2,UNKNOWN,0,0);expect_sub(0,0,0,0,0,p,2,2,UNKNOWN,0,0);
  b[0]=binding(s[0],1,0,0,1);b[0].formal.path.steps.e[0].index=3;expect_sub(s,1,1,b,1,p,2,2,UNKNOWN,0,0);
  b[0]=binding(s[0],1,0,0,1);b[0].formal.root=1;expect_sub(s,1,1,b,1,p,2,2,UNKNOWN,0,0);
  b[0]=binding(s[0],1,0,0,1);b[0].formal.role=INLINE;b[0].callee_root=800;b[0].count=0;expect_sub(s,1,1,b,1,p,2,2,UNKNOWN,0,0);
  b[0]=binding(s[0],1,0,0,1);b[0].actual_type=8;expect_sub(s,1,1,b,1,p,2,2,INVALID,0,0);
  b[0]=binding(s[0],1,0,0,1);b[0].formal.type=b[0].actual_type=8;p[0].type=8;expect_sub(s,1,1,b,1,p,1,2,INVALID,0,0);p[0].type=7;
  b[0]=binding(s[0],1,0,0,1);b[1]=b[0];expect_sub(s,1,1,b,2,p,2,2,INVALID,0,0);
  b[1].formal.type=b[1].actual_type=8;b[1].start=1;p[1].type=8;expect_sub(s,1,1,b,2,p,2,2,INVALID,0,0);p[1].type=7;
  b[0]=binding(s[0],1,0,-1,1);expect_sub(s,1,1,b,1,p,2,2,INVALID,0,0);
  b[0]=binding(s[0],1,0,2,1);expect_sub(s,1,1,b,1,p,2,2,INVALID,0,0);
  b[0]=binding(s[0],1,0,2,0);expect_sub(s,1,1,b,1,p,2,0,OK,0,0);
  b[0]=binding(s[0],1,0,INT64_MAX,1);expect_sub(s,1,1,b,1,p,2,2,INVALID,0,0);
  b[0]=binding(s[0],1,0,0,-1);expect_sub(s,1,1,b,1,p,2,2,INVALID,0,0);
  b[0]=binding(s[0],0,0,0,1);expect_sub(s,1,1,b,1,p,2,2,INVALID,0,0);
  b[0]=binding(s[0],1,1,0,1);expect_sub(s,1,1,b,1,p,2,2,INVALID,0,0);
  b[0]=binding(s[0],1,0,0,1);p[0].type=8;expect_sub(0,0,1,b,1,p,1,2,INVALID,0,0);p[0].type=7;
  p[2]=origin(0,0,0,0);expect_sub(0,0,0,0,0,p,3,2,INVALID,0,0);
  /* A late absent formal refuses atomically after an otherwise valid literal. */
  s[1]=s[0];s[0]=origin(STATIC,10,0,7);expect_sub(s,2,1,0,0,p,2,3,UNKNOWN,0,0);
  /* One-level caller-relative terms never chase the callee table again. */
  s[0]=origin(PARAMETER,0,DESCRIPTOR,7);p[0]=origin(PARAMETER,1,DESCRIPTOR,7);p[1]=origin(LOCAL,900,0,7);
  b[0]=binding(s[0],1,0,0,1);b[1]=binding(p[0],1,0,1,1);expect_sub(s,1,1,b,2,p,2,2,OK,p,1);
  p[0]=s[0];expect_sub(s,1,1,b,2,p,2,2,OK,p,1);
  for(int i=0;i<65;i++){s[i]=origin(STATIC,1,0,7);b[i]=binding(projected(PARAMETER,0,DESCRIPTOR,7,FIELD,i),0,0,0,0);}
  for(int i=0;i<257;i++)p[i]=origin(STATIC,i+1,0,7);
  b[0]=binding(origin(PARAMETER,0,DESCRIPTOR,7),1,0,0,65);expect_sub(0,0,1,b,1,p,65,0,INVALID,0,0);
  for(int i=0;i<65;i++)b[i]=binding(projected(PARAMETER,0,DESCRIPTOR,7,FIELD,i),0,0,0,0);
  expect_sub(s,63,1,0,0,p,255,2,OK,s,1);expect_sub(s,64,1,0,0,p,256,2,OK,s,1);expect_sub(s,65,1,0,0,p,256,2,INVALID,0,0);
  expect_sub(0,0,1,b,63,p,256,0,OK,0,0);expect_sub(0,0,1,b,64,p,256,0,OK,0,0);expect_sub(0,0,1,b,65,p,256,0,INVALID,0,0);
  expect_sub(0,0,1,0,0,p,257,0,INVALID,0,0);expect_sub(s,-1,1,0,0,p,1,2,INVALID,0,0);
  expect_sub(0,0,1,b,-1,p,1,2,INVALID,0,0);expect_sub(0,0,1,0,0,p,-1,2,INVALID,0,0);expect_sub(0,0,1,0,0,p,0,65,INVALID,0,0);
  /* Exactly 127/128 duplicate candidates fit; 129 refuses before publication. */
  for(int i=0;i<3;i++){s[i]=origin(PARAMETER,i,DESCRIPTOR,7);b[i]=binding(s[i],1,0,0,64);}
  for(int i=0;i<64;i++)e[i]=p[i];b[1].count=63;
  expect_sub(s,2,1,b,2,p,64,64,OK,e,64);b[1].count=64;expect_sub(s,2,1,b,2,p,64,64,OK,e,64);
  b[2].count=1;expect_sub(s,3,1,b,3,p,64,64,WORK,0,0);
  expect_sub(s,3,0,b,3,p,64,0,UNKNOWN,0,0);
  expect_sub(s,3,1,b,2,p,64,0,UNKNOWN,0,0); /* missing input precedes work/capacity */
  b[1].count=2;expect_sub(s,2,1,b,2,p,64,1,CAPACITY,0,0);
}
static void high_paths(void) {
  for(int position=8;position<24;position++)for(int field=0;field<2;field++){
    coax_t_BS_Origin a=origin(PARAMETER,0,DESCRIPTOR,7);a.path.count=24;
    for(int k=0;k<24;k++){a.path.steps.e[k].kind=FIELD;a.path.steps.e[k].index=k;}
    coax_t_BS_Origin b=a;if(field)b.path.steps.e[position].kind=ELEMENT;else b.path.steps.e[position].index++;
    coax_t_BS_Origin values[3]={b,a,a},expected[2]={a,b};
    expect_union(values,3,1,0,0,1,3,OK,expected,2);
    coax_t_BS_Origin refs[2]={a,b},pool[2]={origin(STATIC,101,0,7),origin(LOCAL,202,0,7)};
    coax_t_BS_Binding bindings[2]={binding(a,1,0,0,1),binding(b,1,0,1,1)};
    expect_sub(refs,2,1,bindings,2,pool,2,3,OK,pool,2);
    a.kind=STATIC;a.root=1;a.role=0;a.path.count=8;
    for(int k=8;k<24;k++){a.path.steps.e[k].kind=0;a.path.steps.e[k].index=0;}
    if(field)a.path.steps.e[position].kind=FIELD;else a.path.steps.e[position].index=1;
    valid_path(a.path,0);expect_union(&a,1,0,0,0,0,2,INVALID,0,0);
  }
  coax_t_BS_Origin a=origin(PARAMETER,0,INLINE,7);a.path.count=24;
  for(int k=0;k<24;k++){a.path.steps.e[k].kind=FIELD;a.path.steps.e[k].index=k;}
  coax_t_BS_Binding b=binding(a,1,501,0,0);coax_t_BS_Origin expected=a;expected.kind=LOCAL;expected.root=501;expected.role=0;
  expect_sub(&a,1,1,&b,1,0,0,1,OK,&expected,1);
  a.role=DESCRIPTOR;b=binding(a,1,0,0,0);b.formal.path.count=23;b.formal.path.steps.e[23].kind=0;b.formal.path.steps.e[23].index=0;
  expect_sub(&a,1,1,&b,1,0,0,1,UNKNOWN,0,0);
  a.path.steps.e[23].kind=DEREFERENCE;a.path.steps.e[23].index=0;valid_origin(a,1);a.role=INLINE;valid_origin(a,0);a.kind=LOCAL;a.role=0;a.root=1;valid_origin(a,0);
}
int main(void) {
  vocabulary();unions();substitution();high_paths();
  printf("{\"scope\":\"standalone set operations, not lifetime safety\",\"cases\":%u,\"calls\":%u,\"refusals\":%u,\"origin_size\":%zu,\"binding_size\":%zu}\n",cases,calls,refusals,sizeof(coax_t_BS_Origin),sizeof(coax_t_BS_Binding));return 0;
}
