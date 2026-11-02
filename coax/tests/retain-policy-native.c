/* Independent supplied-fact lifetime tables. Test tooling, not a resolver. */
#include "retain-policy.h"
#undef NDEBUG
#include <assert.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>

#define ABI(T,S) _Static_assert(sizeof(T)==S && _Alignof(T)==8,"LP64 " #T)
#define OFF(T,F,N) _Static_assert(offsetof(T,F)==N,"LP64 " #T "." #F)
typedef coax_t_RP_Term Term;
typedef coax_t_RP_Address Address;
typedef coax_t_RP_Obligation Obligation;
typedef coax_t_RP_Result Result;
ABI(Term,432);OFF(Term,frame,0);OFF(Term,origin,8);
ABI(Address,440);OFF(Address,storage,0);OFF(Address,value_type,432);
ABI(Obligation,872);OFF(Obligation,source,0);OFF(Obligation,destination,432);
ABI(Result,32);OFF(Result,status,0);OFF(Result,facts_complete,1);
OFF(Result,relation_complete,2);OFF(Result,forbidden,3);
OFF(Result,obligation_count,8);OFF(Result,bad_source,16);OFF(Result,bad_destination,24);
ABI(coax_v_t_RP_Term,16);ABI(coax_v_t_RP_Address,16);ABI(coax_v_t_RP_Obligation,16);
OFF(coax_v_t_RP_Term,data,0);OFF(coax_v_t_RP_Term,count,8);
enum {OK,INVALID,CAPACITY,WORK};
enum {STATIC,LOCAL,INLINE,DESC};
static unsigned cases,refusals,calls,independent_check_cases,independent_bounds_cases;
static Term term(int kind,int root,int type) {
 Term t={0};t.frame=kind==STATIC?0:9;t.origin.kind=kind==STATIC?1:kind==LOCAL?2:3;
 t.origin.root=root;t.origin.role=kind==INLINE?1:kind==DESC?2:0;t.origin.type=type;return t;
}
static Address address(int kind,int root) {
 Address a={0};a.storage=term(kind,root,99);a.value_type=7;
 if(kind==DESC){a.storage.origin.path.count=1;a.storage.origin.path.steps.e[0].kind=3;}
 return a;
}
static int same_term(Term a,Term b) {
 if(a.frame!=b.frame||a.origin.kind!=b.origin.kind||a.origin.root!=b.origin.root||a.origin.role!=b.origin.role||a.origin.type!=b.origin.type||a.origin.path.count!=b.origin.path.count)return 0;
 for(int i=0;i<24;i++)if(a.origin.path.steps.e[i].kind!=b.origin.path.steps.e[i].kind||a.origin.path.steps.e[i].index!=b.origin.path.steps.e[i].index)return 0;
 return 1;
}
static int same_address(Address a,Address b){return a.value_type==b.value_type&&same_term(a.storage,b.storage);}
static int same(Obligation a,Obligation b){return same_term(a.source,b.source)&&same_address(a.destination,b.destination);}
struct Guard {uint64_t before[2];Obligation out[66];uint64_t after[2];};
static void poison(struct Guard *g){
 g->before[0]=91;g->before[1]=92;g->after[0]=93;g->after[1]=94;
 for(int i=0;i<66;i++){g->out[i].source=term(LOCAL,700+i,7);g->out[i].destination=address(LOCAL,900+i);}
}
/* Exact literal expected fields; deliberately no aggregate-padding comparisons. */
static void check(int64_t frame,int64_t type,Term *s,int64_t ns,int sc,Address *d,int64_t nd,int dc,int64_t capacity,int status,int facts,int relation,int forbidden,int64_t bs,int64_t bd,Obligation *want,int64_t nw){
 struct Guard g,old;poison(&g);old=g;Term saved_s[65];Address saved_d[65];
 if(ns>=0&&ns<=65)for(int64_t i=0;i<ns;i++)saved_s[i]=s[i];
 if(nd>=0&&nd<=65)for(int64_t i=0;i<nd;i++)saved_d[i]=d[i];
 Result r=RP_check(frame,type,(coax_v_t_RP_Term){s,ns},sc,(coax_v_t_RP_Address){d,nd},dc,(coax_v_t_RP_Obligation){g.out,capacity});
 cases++;calls++;assert(r.status==status);assert(r.facts_complete==facts);assert(r.relation_complete==relation);assert(r.forbidden==forbidden);assert(r.obligation_count==nw);assert(r.bad_source==bs&&r.bad_destination==bd);
 for(int i=0;i<2;i++){assert(g.before[i]==old.before[i]);assert(g.after[i]==old.after[i]);}
 for(int64_t i=0;i<nw;i++)assert(same(g.out[i],want[i]));
 for(int64_t i=nw;i<66;i++)assert(same(g.out[i],old.out[i]));
 if(ns>=0&&ns<=65)for(int64_t i=0;i<ns;i++)assert(same_term(s[i],saved_s[i]));
 if(nd>=0&&nd<=65)for(int64_t i=0;i<nd;i++)assert(same_address(d[i],saved_d[i]));
 if(status!=OK)refusals++;
}
static void bad(int64_t frame,int64_t type,Term*s,int64_t ns,Address*d,int64_t nd,int64_t cap,int status){check(frame,type,s,ns,0,d,nd,0,cap,status,0,0,0,-1,-1,NULL,0);}
static void matrix(void){
 /* Rows STATIC,LOCAL,INLINE,DESC; columns STATIC,LOCAL,INLINE,DESC. */
 const int forbidden[4][4]={{0,0,0,0},{1,0,0,1},{1,0,0,1},{0,0,0,0}};
 const int pending[4][4]={{0,0,0,0},{0,0,0,0},{0,0,0,0},{1,0,0,1}};
 const int decided[4][4]={{1,1,1,1},{1,0,0,1},{1,0,0,1},{0,1,1,0}};
 for(int a=0;a<4;a++)for(int b=0;b<4;b++)for(int sc=0;sc<2;sc++)for(int dc=0;dc<2;dc++){
  Term s=term(a,3,7);Address d=address(b,3);Obligation want={s,d};
  check(9,7,&s,1,sc,&d,1,dc,64,OK,sc&&dc,decided[a][b],forbidden[a][b],forbidden[a][b]?0:-1,forbidden[a][b]?0:-1,&want,pending[a][b]);
 }
}
static void limits(void){
 const int64_t n[]={-1,0,1,2,8,16,43,64,65,INT64_MAX};
 for(unsigned a=0;a<10;a++)for(unsigned b=0;b<10;b++)for(unsigned c=0;c<10;c++){
  int expected=n[a]<0||n[a]>64||n[b]<0||n[b]>64||n[c]<0||n[c]>64?INVALID:n[a]*n[b]>128?WORK:OK;
  assert(RP_bounds(n[a],n[b],n[c])==expected);cases++;calls++;
 }
 Term s[65];Address d[65];for(int i=0;i<65;i++){s[i]=term(DESC,i%64,7);d[i]=address(DESC,i%64);}
 bad(9,7,s,-1,d,1,64,INVALID);bad(9,7,s,65,d,1,64,INVALID);bad(9,7,s,1,d,-1,64,INVALID);bad(9,7,s,1,d,65,64,INVALID);bad(9,7,s,1,d,1,-1,INVALID);bad(9,7,s,1,d,1,65,INVALID);
 bad(9,7,s,3,d,43,64,WORK);d[42].storage.frame=10;bad(9,7,s,3,d,43,64,INVALID);d[42]=address(DESC,42);
 Obligation want[64];for(int j=0;j<8;j++)for(int i=0;i<8;i++)want[j*8+i]=(Obligation){s[i],d[j]};
 check(9,7,s,8,1,d,8,1,64,OK,1,0,0,-1,-1,want,64);
 bad(9,7,s,8,d,8,63,CAPACITY);bad(9,7,s,5,d,13,64,CAPACITY);
 bad(9,7,s,1,d,1,0,CAPACITY);
}
static void empty_and_witness(void){
 Term s[3]={term(STATIC,3,7),term(LOCAL,4,7),term(DESC,5,7)};Address d[3]={address(LOCAL,3),address(DESC,4),address(STATIC,5)};
 for(int sc=0;sc<2;sc++)for(int dc=0;dc<2;dc++){
  check(9,7,s,0,sc,d,1,dc,0,OK,sc&&dc,1,0,-1,-1,NULL,0);
  check(9,7,s,1,sc,d,0,0,0,OK,0,1,0,-1,-1,NULL,0);
 }
 check(9,7,s,0,1,d,0,1,0,INVALID,0,0,0,-1,-1,NULL,0);
 Obligation want[2]={{s[2],d[2]},{s[2],d[1]}}; /* STATIC destination sorts first. */
 check(9,7,s,3,0,d,3,0,64,OK,0,0,1,1,1,want,2);
 Term t[2]={s[1],s[0]};check(9,7,t,2,0,d+1,1,0,0,OK,0,1,1,0,0,NULL,0);
 Term repeated[2]={s[2],s[2]};Address repeated_d[2]={d[1],d[1]};Obligation one={s[2],d[1]};
 check(9,7,repeated,2,1,repeated_d,2,1,1,OK,1,0,0,-1,-1,&one,1);
}
static void metadata(void){
 /* Scalar admission witnesses: no live descriptor supplies a second refusal. */
 Term empty_source={0};Address scalar=address(STATIC,1);scalar.value_type=1;
 check(1,1,&empty_source,0,1,&scalar,1,1,0,OK,1,1,0,-1,-1,NULL,0);
 scalar.value_type=INT64_C(2147483647);check(INT64_C(2147483647),INT64_C(2147483647),&empty_source,0,1,&scalar,1,1,0,OK,1,1,0,-1,-1,NULL,0);
 scalar.value_type=INT64_C(2147483648);check(1,INT64_C(2147483648),&empty_source,0,1,&scalar,1,1,0,INVALID,0,0,0,-1,-1,NULL,0);
 scalar.value_type=1;check(INT64_C(2147483648),1,&empty_source,0,1,&scalar,1,1,0,INVALID,0,0,0,-1,-1,NULL,0);
 Term good=term(DESC,3,7),s;Address target=address(DESC,4),d;
 int64_t invalid[]={-1,0,INT64_C(2147483648),INT64_MAX};
 for(unsigned i=0;i<4;i++){bad(invalid[i],7,&good,1,&target,1,64,INVALID);bad(9,invalid[i],&good,1,&target,1,64,INVALID);}
 s=good;s.frame=10;bad(9,7,&s,1,&target,1,64,INVALID);s=term(STATIC,3,7);s.frame=9;bad(9,7,&s,1,&target,1,64,INVALID);
 s=good;s.origin.type=8;bad(9,7,&s,1,&target,1,64,INVALID);d=target;d.value_type=8;bad(9,7,&good,1,&d,1,64,INVALID);
 d=target;d.storage.origin.path.count=0;d.storage.origin.path.steps.e[0].kind=0;bad(9,7,&good,1,&d,1,64,INVALID);
 for(int i=0;i<24;i++){
  s=good;s.origin.path.steps.e[i].kind=1;bad(9,7,&s,1,&target,1,64,INVALID);
  d=target;d.storage.origin.path.steps.e[i].index=1;bad(9,7,&good,1,&d,1,64,INVALID);
 }
 /* Exact FIELD before wildcard DEREFERENCE, with full depth and preserved rows. */
 s=good;s.origin.path.count=24;d=target;d.storage.origin.path.count=24;
 for(int i=0;i<24;i++){s.origin.path.steps.e[i].kind=i%2?2:1;s.origin.path.steps.e[i].index=i;d.storage.origin.path.steps.e[i].kind=i==4?3:1;d.storage.origin.path.steps.e[i].index=i==4?0:i;}
 Obligation one={s,d};check(9,7,&s,1,1,&d,1,1,1,OK,1,0,0,-1,-1,&one,1);
 d.storage.origin.path.steps.e[4].index=1;bad(9,7,&s,1,&d,1,64,INVALID);
}
#include "retain-policy-vectors.inc"
int main(void){matrix();limits();empty_and_witness();metadata();independent_vectors();printf("{\"cases\":%u,\"calls\":%u,\"refusals\":%u,\"independent_check_cases\":%u,\"independent_bounds_cases\":%u,\"term_size\":%zu,\"address_size\":%zu,\"obligation_size\":%zu,\"result_size\":%zu}\n",cases,calls,refusals,independent_check_cases,independent_bounds_cases,sizeof(Term),sizeof(Address),sizeof(Obligation),sizeof(Result));}
