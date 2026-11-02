/* Exact projected production lab_ask + complete authored Lab/Session types.
 * Every unreachable request-building dependency exits97; no process is started.
 * Compare named fields/descriptors and separately guarded buffers, never padding. */
#include "lab-owner.h"
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
_Static_assert(sizeof(((coax_t_SoBody*)0)->names.e)/sizeof(coax_string)==64,"question extent changed");
_Static_assert(sizeof(((coax_t_SoBody*)0)->kinds.e)/sizeof(((coax_t_SoBody*)0)->kinds.e[0])==64,"question kinds extent changed");
_Static_assert(sizeof(((coax_t_SoBody*)0)->options.e)/sizeof(int64_t)==64,"question options extent changed");
_Static_assert(sizeof(((coax_t_Lab*)0)->call.e)/sizeof(coax_t_SoCall)==1,"call extent changed");
_Static_assert(sizeof(((coax_t_Lab*)0)->q.e)/sizeof(coax_t_SoBody)==1,"request extent changed");
_Static_assert(sizeof(((coax_t_Session*)0)->parts.e)/sizeof(coax_t_SPart)==768,"part extent changed");

#define REQUIRE(x) do{if(!(x)){fprintf(diagnostics(),"failed %d: %s\n",__LINE__,#x);return 1;}}while(0)
static FILE *diagnostics(void){return stderr;}
#undef stderr
void lab_owner_unexpected(void){fputs("unexpected nonactive Lab path\n",diagnostics());exit(97);}
static int text_equal(coax_string a,coax_string b){return a.data==b.data&&a.count==b.count;}
#define SCALAR(x) if(a->x!=b->x)return 0
#define TEXT(x) if(!text_equal(a->x,b->x))return 0
#define VIEW(x) if(a->x.data!=b->x.data||a->x.count!=b->x.count)return 0
static int same_result(const coax_t_SoResult*a,const coax_t_SoResult*b){
 SCALAR(status);TEXT(body);TEXT(problem);TEXT(stderr);SCALAR(usage.ok);SCALAR(usage.input_tokens);SCALAR(usage.output_tokens);return 1;
}
static int same_call(const coax_t_SoCall*a,const coax_t_SoCall*b){
 SCALAR(h);VIEW(out);VIEW(err);SCALAR(at);SCALAR(last);SCALAR(lines);SCALAR(timeout_s);SCALAR(done);return same_result(&a->r,&b->r);
}
static int same_body(const coax_t_SoBody*a,const coax_t_SoBody*b){
 VIEW(buf);SCALAR(at);SCALAR(stage);TEXT(problem);SCALAR(questions);
 for(int i=0;i<64;i++){if(!text_equal(a->names.e[i],b->names.e[i])||a->kinds.e[i]!=b->kinds.e[i]||a->options.e[i]!=b->options.e[i])return 0;}return 1;
}
static int same_lab(const coax_t_Lab*a,const coax_t_Lab*b){
 SCALAR(phase);SCALAR(part);SCALAR(route);SCALAR(via);VIEW(mc);SCALAR(codex_due);SCALAR(codex_effort);SCALAR(fallback);SCALAR(secs);SCALAR(codex);SCALAR(url_n);SCALAR(local);SCALAR(remote);VIEW(body);VIEW(out);VIEW(err);
 return same_call(&a->call.e[0],&b->call.e[0])&&same_body(&a->q.e[0],&b->q.e[0])&&!memcmp(a->url.e,b->url.e,sizeof a->url.e);
}
static int same_part(const coax_t_SPart*a,const coax_t_SPart*b){
 SCALAR(kind);SCALAR(at);SCALAR(n);SCALAR(notice);TEXT(id);SCALAR(tk);TEXT(arg);TEXT(detail);TEXT(before);TEXT(after);TEXT(input);TEXT(output);SCALAR(failed);SCALAR(resolved);return 1;
}
static int same_session(const coax_t_Session*a,const coax_t_Session*b){
 VIEW(pool);SCALAR(np);SCALAR(n);SCALAR(live);SCALAR(stopping);SCALAR(noticed);SCALAR(secs);SCALAR(turns);SCALAR(changed);SCALAR(asking);SCALAR(cost);SCALAR(turn_ms);SCALAR(hint);
 for(int i=0;i<768;i++)if(!same_part(&a->parts.e[i],&b->parts.e[i]))return 0;return 1;
}
typedef struct{uint8_t before[16],data[64],after[16];} Guard;
static coax_string text(const char*s){return(coax_string){s,(int64_t)strlen(s)};}
static coax_v_u8 bytes(Guard*g){return(coax_v_u8){g->data+3,41};}
static int check(int64_t handle){
 Guard body,out,err,pool;memset(&body,71,sizeof body);memset(&out,72,sizeof out);memset(&err,73,sizeof err);memset(&pool,74,sizeof pool);
 Guard oldbody=body,oldout=out,olderr=err,oldpool=pool;
 coax_t_Lab *l=malloc(sizeof*l),*before=malloc(sizeof*before);coax_t_Session *s=malloc(sizeof*s),*olds=malloc(sizeof*olds);REQUIRE(l&&before&&s&&olds);
 lab_owner_init((coax_v_t_Lab){l,1},(coax_v_t_Session){s,1});
 l->phase=1;l->part=7;l->route=3;l->via=2;l->secs=7;l->codex_due=1;l->codex_effort=4;l->fallback=1;l->codex=1;l->local=1;l->remote=1;l->url_n=5;memset(l->url.e,75,sizeof l->url.e);
 l->body=bytes(&body);l->out=bytes(&out);l->err=bytes(&err);l->call.e[0].h=handle;l->call.e[0].out=bytes(&out);l->call.e[0].err=bytes(&err);l->call.e[0].at=4;l->call.e[0].last=2;l->call.e[0].lines=2;l->call.e[0].timeout_s=17;
 l->call.e[0].r.status=202;l->call.e[0].r.body=(coax_string){(char*)out.data+4,3};l->call.e[0].r.stderr=(coax_string){(char*)err.data+5,2};l->call.e[0].r.problem=text("existing");l->call.e[0].r.usage.ok=1;l->call.e[0].r.usage.input_tokens=11;l->call.e[0].r.usage.output_tokens=12;
 l->q.e[0].buf=bytes(&body);l->q.e[0].at=5;l->q.e[0].stage=3;l->q.e[0].questions=1;l->q.e[0].problem=text("original question");for(int i=0;i<64;i++){l->q.e[0].names.e[i]=text("q");l->q.e[0].options.e[i]=i;}
 s->pool=bytes(&pool);s->np=5;s->n=1;s->live=1;s->secs=9;s->asking=-1;s->parts.e[0].at=0;s->parts.e[0].n=5;s->parts.e[0].id=text("original session");
 *before=*l;*olds=*s;
 lab_owner_ask((coax_v_t_Lab){l,1},(coax_v_t_Session){s,1});
 REQUIRE(same_lab(l,before)&&same_session(s,olds));
 REQUIRE(!memcmp(&body,&oldbody,sizeof body)&&!memcmp(&out,&oldout,sizeof out)&&!memcmp(&err,&olderr,sizeof err)&&!memcmp(&pool,&oldpool,sizeof pool));
 free(l);free(before);free(s);free(olds);return 0;
}
int main(void){int64_t cases[]={0,1,-2,INT64_MIN,INT64_MAX};for(unsigned i=0;i<sizeof cases/sizeof cases[0];i++)if(check(cases[i]))return 1;puts("Lab active/invalid admission: exact function and complete types; named state/descriptors/bytes unchanged");return 0;}
