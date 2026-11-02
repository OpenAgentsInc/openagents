/* Offline verification of generated Coax decisions. Process effects are scripted
 * here: this is not process-native.c kill/reap or live-service qualification. */
#include "systemone-lifecycle.h"
#include <stdint.h>
#include <stdio.h>
#include <string.h>
static FILE *diagnostics(void){return stderr;}
#undef stderr

enum {SUCCESS, TIMEOUT, CHILD_ERROR, HTTP_ERROR, INVALID_READ, SPAWN_ERROR};
static int scenario, line_index, initial_idle, live, releases, effects, reads;
static const char response[]="{\"answers\":{},\"usage\":{\"input_tokens\":7,\"output_tokens\":2}}";
static const char child_error[]="scripted child stderr";
static const char *line;

#define REQUIRE(x) do {if(!(x)){fprintf(diagnostics(),"failed line %d: %s\n",__LINE__,#x);return 1;}}while(0)
typedef struct {uint8_t before[8],data[512],after[8];} Guard;
static void fill(Guard *g){memset(g,0xa5,sizeof *g);}
static int guards(const Guard*g){for(int i=0;i<8;i++)if(g->before[i]!=0xa5||g->after[i]!=0xa5)return 0;return 1;}
static coax_string text(const char*s){return (coax_string){s,(int64_t)strlen(s)};}
static coax_v_u8 bytes(Guard*g){return (coax_v_u8){g->data,512};}
static coax_v_t_SoCall owner(coax_t_SoCall*c){return (coax_v_t_SoCall){c,1};}
static int same_text(coax_string a,coax_string b){return a.data==b.data&&a.count==b.count;}
static int content(coax_string s,const char*t){return s.count==(int64_t)strlen(t)&&memcmp(s.data,t,(size_t)s.count)==0;}
static int same_call(const coax_t_SoCall*a,const coax_t_SoCall*b){
  return a->h==b->h&&a->out.data==b->out.data&&a->out.count==b->out.count&&a->err.data==b->err.data&&a->err.count==b->err.count&&
    a->at==b->at&&a->last==b->last&&a->lines==b->lines&&a->timeout_s==b->timeout_s&&a->done==b->done&&
    a->r.status==b->r.status&&same_text(a->r.body,b->r.body)&&same_text(a->r.problem,b->r.problem)&&same_text(a->r.stderr,b->r.stderr)&&
    a->r.usage.ok==b->r.usage.ok&&a->r.usage.input_tokens==b->r.usage.input_tokens&&a->r.usage.output_tokens==b->r.usage.output_tokens;
}
static void disturb(void){volatile uint8_t scratch[16384];for(size_t i=0;i<sizeof scratch;i++)scratch[i]=(uint8_t)i;}
static void reset(int which){scenario=which;line_index=0;initial_idle=1;live=0;releases=0;effects=0;reads=0;line="";}

/* Scripted foreign boundary: each test starts from a fresh single slot0. */
int64_t cxproc_new(void){effects++;if(live)return -1;live=1;return 0;}
int64_t cxproc_push(int64_t h,int64_t field,int64_t byte){(void)field;(void)byte;effects++;return h==0&&live?0:-1;}
int64_t cxproc_end(int64_t h,int64_t field){(void)field;effects++;return h==0&&live?0:-1;}
int64_t cxproc_spawn(int64_t h){effects++;return h==0&&live?(scenario==SPAWN_ERROR?2:0):-1;}
int64_t cxproc_write(int64_t h,int64_t byte){(void)byte;effects++;return h==0&&live?0:-1;}
int64_t cxproc_flush(int64_t h){effects++;return h==0&&live?0:-1;}
int64_t cxproc_close_stdin(int64_t h){effects++;return h==0&&live?0:-1;}
int64_t cxproc_read_line(int64_t h,int64_t wait){
  effects++;reads++;if(h!=0||!live)return -9;
  if(scenario==TIMEOUT)return -2;
  if(scenario==INVALID_READ)return -9;
  if(scenario==CHILD_ERROR)return -1;
  if(wait==0&&initial_idle){initial_idle=0;return -2;}
  if(line_index==0){line=response;line_index++;return (int64_t)strlen(line);}
  if(line_index==1){line=scenario==HTTP_ERROR?"503":"200";line_index++;return 3;}
  return -1;
}
int64_t cxproc_line_byte(int64_t h,int64_t i){effects++;return h==0&&live?(uint8_t)line[i]:0;}
int64_t cxproc_wait(int64_t h,int64_t timeout){(void)timeout;effects++;return h==0&&live?(scenario==CHILD_ERROR?28:0):-1;}
int64_t cxproc_stderr_count(int64_t h){effects++;return h==0&&live?(int64_t)strlen(child_error):0;}
int64_t cxproc_stderr_byte(int64_t h,int64_t i){effects++;return h==0&&live?(uint8_t)child_error[i]:0;}
int64_t cxproc_release(int64_t h){effects++;if(h!=0||!live)return -1;releases++;live=0;return 0;}
int64_t cxproc_getenv_push(int64_t byte){(void)byte;effects++;return 0;}
int64_t cxproc_getenv(void){effects++;return 0;}

static coax_t_SoClient client(void){coax_t_SoClient c;coax_default_t_SoClient(&c);c.key_env=text("");c.url=text("http://offline.invalid");c.curl=text("scripted-effect-boundary");return c;}

static int active_refusal(void){
  Guard oldout,olderr,newout,newerr;fill(&oldout);fill(&olderr);fill(&newout);fill(&newerr);
  coax_t_SoCall call;coax_default_t_SoCall(&call);
  call.h=0;call.out=bytes(&oldout);call.err=bytes(&olderr);call.at=9;call.last=4;call.lines=3;call.timeout_s=17;
  call.r.status=201;call.r.body=(coax_string){(const char*)oldout.data+2,7};call.r.stderr=(coax_string){(const char*)olderr.data+3,5};
  call.r.problem=text("existing result");call.r.usage.ok=1;call.r.usage.input_tokens=11;call.r.usage.output_tokens=12;
  coax_t_SoCall before=call;Guard oldout_before=oldout,olderr_before=olderr,newout_before=newout,newerr_before=newerr;
  reset(SUCCESS);live=1;
  REQUIRE(!so_life_start(client(),text("{}"),owner(&call),bytes(&newout),bytes(&newerr)));
  REQUIRE(same_call(&call,&before)&&effects==0&&live&&releases==0);
  REQUIRE(!memcmp(&oldout,&oldout_before,sizeof oldout)&&!memcmp(&olderr,&olderr_before,sizeof olderr));
  REQUIRE(!memcmp(&newout,&newout_before,sizeof newout)&&!memcmp(&newerr,&newerr_before,sizeof newerr));
  call.h=-2;before=call;
  REQUIRE(!so_life_start(client(),text("{}"),owner(&call),bytes(&newout),bytes(&newerr)));
  REQUIRE(same_call(&call,&before)&&effects==0);
  puts("active and invalid owner refusal preserves every named field, descriptor, result and backing byte");return 0;
}

static int success_and_restart(void){
  Guard out,err;fill(&out);fill(&err);coax_t_SoCall call;coax_default_t_SoCall(&call);reset(SUCCESS);
  REQUIRE(call.h==-1&&!call.done&&call.r.body.count==0);
  REQUIRE(so_life_start(client(),text("{}"),owner(&call),bytes(&out),bytes(&err))&&call.h==0&&!call.done);
  REQUIRE(call.out.data==out.data&&call.err.data==err.data);
  REQUIRE(!so_life_step(owner(&call),0)&&!call.done&&live);
  REQUIRE(so_life_step(owner(&call),1)&&call.done&&call.h==-1&&!live&&releases==1);
  REQUIRE(call.r.status==200&&call.r.problem.count==0&&call.r.body.data==(const char*)out.data&&content(call.r.body,response));
  REQUIRE(call.r.usage.ok&&call.r.usage.input_tokens==7&&call.r.usage.output_tokens==2);
  coax_t_SoCall done=call;coax_string copied_body=call.r.body;int completed_effects=effects;
  disturb();REQUIRE(content(copied_body,response)&&guards(&out)&&guards(&err));
  REQUIRE(so_life_step(owner(&call),0)&&same_call(&call,&done)&&effects==completed_effects);
  so_life_cancel(owner(&call));so_life_cancel(owner(&call));
  REQUIRE(same_call(&call,&done)&&effects==completed_effects&&releases==1&&content(copied_body,response));
  /* All copied results above are consumed before this owner/buffer reuse. */
  reset(SUCCESS);REQUIRE(so_life_start(client(),text("{}"),owner(&call),bytes(&out),bytes(&err)));
  REQUIRE(so_life_step(owner(&call),1)&&releases==1&&content(call.r.body,response));
  puts("fresh/default start, idle poll, success, borrowed use after stack disturbance, duplicate poll/cancel and retired restart pass");return 0;
}

static int terminal_cases(void){
  for(int which=TIMEOUT;which<=INVALID_READ;which++){
    Guard out,err;fill(&out);fill(&err);coax_t_SoCall call;coax_default_t_SoCall(&call);reset(which);
    REQUIRE(so_life_start(client(),text("{}"),owner(&call),bytes(&out),bytes(&err)));
    REQUIRE(so_life_step(owner(&call),1)&&call.done&&call.h==-1&&releases==1&&!live&&call.r.problem.count>0);
    if(which==CHILD_ERROR){REQUIRE(call.r.stderr.data==(const char*)err.data&&content(call.r.stderr,child_error));}
    if(which==HTTP_ERROR){REQUIRE(call.r.status==503&&call.r.body.data==(const char*)out.data&&content(call.r.body,response));}
    coax_t_SoCall done=call;int terminal_effects=effects;disturb();
    REQUIRE(so_life_step(owner(&call),100)&&same_call(&call,&done));so_life_cancel(owner(&call));
    REQUIRE(effects==terminal_effects&&releases==1&&guards(&out)&&guards(&err));
    if(which==CHILD_ERROR)REQUIRE(content(done.r.stderr,child_error));
  }
  puts("scripted timeout, child error/stderr, HTTP refusal and invalid read terminate once; late polls and release are idempotent");return 0;
}

static int cancellation_and_refusal(void){
  Guard out,err;fill(&out);fill(&err);coax_t_SoCall call;coax_default_t_SoCall(&call);reset(SUCCESS);
  REQUIRE(so_life_start(client(),text("{}"),owner(&call),bytes(&out),bytes(&err)));
  so_life_cancel(owner(&call));REQUIRE(call.done&&call.h==-1&&releases==1&&!live&&content(call.r.problem,"cancelled"));
  coax_t_SoCall cancelled=call;int cancelled_effects=effects;so_life_cancel(owner(&call));
  REQUIRE(so_life_step(owner(&call),1)&&same_call(&call,&cancelled)&&effects==cancelled_effects);
  reset(SUCCESS);REQUIRE(so_life_start(client(),text("{}"),owner(&call),bytes(&out),bytes(&err)));so_life_cancel(owner(&call));REQUIRE(releases==1);
  reset(SPAWN_ERROR);REQUIRE(!so_life_start(client(),text("{}"),owner(&call),bytes(&out),bytes(&err)));
  REQUIRE(call.done&&call.h==-1&&releases==1&&content(call.r.problem,"curl not found"));
  reset(SUCCESS);REQUIRE(!so_life_start(client(),text(""),owner(&call),bytes(&out),bytes(&err)));
  REQUIRE(call.done&&call.h==-1&&effects==0&&content(call.r.problem,"no request body"));
  coax_v_t_SoCall empty={NULL,0};int before=effects;
  REQUIRE(!so_life_start(client(),text("{}"),empty,bytes(&out),bytes(&err)));
  REQUIRE(effects==before&&guards(&out)&&guards(&err));
  puts("cancellation, cancelled restart, setup refusal and empty owner views preserve bounded behavior");return 0;
}

int main(void){if(active_refusal()||success_and_restart()||terminal_cases()||cancellation_and_refusal())return 1;puts("System One lifecycle: actual generated Coax decisions pass; scripted process effects only");return 0;}
