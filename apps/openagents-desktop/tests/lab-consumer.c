/* Actual projected Lab consumer, canonical owner and transport. Only native
 * process/environment and downstream engine dispatch effects are scripted. */
#include "lab-consumer.h"
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
static FILE *diagnostics(void){return stderr;}
#undef stderr
#define REQUIRE(x) do{if(!(x)){fprintf(diagnostics(),"FAIL %d: %s\n",__LINE__,#x);return 1;}}while(0)
static int scenario, live, effects, releases, line_index, idle, dispatches;
static coax_string line;
static coax_string text(const char*s){return(coax_string){s,(int64_t)strlen(s)};}
static int equal(coax_string a,const char*b){return a.count==(int64_t)strlen(b)&&!memcmp(a.data,b,(size_t)a.count);}
static int has(coax_string a,const char*b){size_t n=strlen(b);for(int64_t i=0;i+(int64_t)n<=a.count;i++)if(!memcmp(a.data+i,b,n))return 1;return 0;}
void lab_consumer_effect(int64_t code){if(code!=1){fprintf(diagnostics(),"unexpected engine effect %lld\n",(long long)code);exit(98);}dispatches++;}
int64_t cxproc_new(void){effects++;if(live)return -1;live=1;return 0;}
int64_t cxproc_push(int64_t h,int64_t field,int64_t byte){(void)field;(void)byte;effects++;return h==0&&live?0:-1;}
int64_t cxproc_end(int64_t h,int64_t field){(void)field;effects++;return h==0&&live?0:-1;}
int64_t cxproc_spawn(int64_t h){effects++;return h==0&&live?(scenario==4?2:0):-1;}
int64_t cxproc_write(int64_t h,int64_t byte){(void)byte;effects++;return h==0&&live?0:-1;}
int64_t cxproc_flush(int64_t h){effects++;return h==0&&live?0:-1;}
int64_t cxproc_close_stdin(int64_t h){effects++;return h==0&&live?0:-1;}
int64_t cxproc_read_line(int64_t h,int64_t wait){(void)wait;effects++;if(h!=0||!live)return -9;if(idle){idle=0;return -2;}if(scenario==2)return -1;if(line_index++==0){line=scenario==1?text("{\"error\":{\"code\":\"busy\",\"message\":\"another decision is running\"}}"):(scenario==5?text("{\"answers\":{}}") : lab_consumer_fixture());return line.count;}if(line_index==2){line=text(scenario==1?"503":"200");return 3;}return -1;}
int64_t cxproc_line_byte(int64_t h,int64_t i){effects++;return h==0&&live?(uint8_t)line.data[i]:0;}
int64_t cxproc_wait(int64_t h,int64_t timeout){(void)timeout;effects++;return h==0&&live?(scenario==2?28:0):-1;}
int64_t cxproc_stderr_count(int64_t h){effects++;return h==0&&live?20:0;}
int64_t cxproc_stderr_byte(int64_t h,int64_t i){effects++;return h==0&&live?(uint8_t)"scripted child error"[i]:0;}
int64_t cxproc_release(int64_t h){effects++;if(h!=0||!live)return -1;live=0;releases++;return 0;}
int64_t cxproc_getenv_push(int64_t b){(void)b;effects++;return 0;}
int64_t cxproc_getenv(void){effects++;return 0;}
// The transport's other process effects (pid, clock, kill, group, environment
// bytes): scripted children have no pid or process group, and time stands still.
int64_t cxproc_pid(int64_t h){effects++;return h==0&&live?0:-1;}
int64_t cxproc_now_ms(void){effects++;return 0;}
int64_t cxproc_kill(int64_t h,int64_t sig){(void)sig;effects++;return h==0&&live?0:-1;}
int64_t cxproc_group_alive(int64_t pgid){(void)pgid;effects++;return 0;}
int64_t cxproc_getenv_byte(int64_t i){(void)i;effects++;return 0;}
typedef struct{uint8_t *allocation;coax_v_u8 view;}Guard;
static Guard guard(size_t n){uint8_t*p=malloc(n+32);if(!p)abort();memset(p,0xa5,n+32);return(Guard){p,{p+16,(int64_t)n}};}
static int guards(Guard g){for(int i=0;i<16;i++)if(g.allocation[i]!=0xa5||g.allocation[16+g.view.count+i]!=0xa5)return 0;return 1;}
int main(void){
 unsigned cases=0;
 for(scenario=0;scenario<=5;scenario++){
  live=effects=releases=line_index=dispatches=0;idle=1;
  coax_t_Lab*l=malloc(sizeof*l);coax_t_Session*s=malloc(sizeof*s);coax_t_Engine*e=malloc(sizeof*e);coax_t_PsEngine*p=malloc(sizeof*p);coax_t_GpuSwitch*g=malloc(sizeof*g);REQUIRE(l&&s&&e&&p&&g);
  coax_default_t_Engine(e);coax_default_t_PsEngine(p);
  Guard body=guard(24576),out=guard(65536),err=guard(4096),pool=guard(65536);
  coax_v_t_Lab lv={l,1};coax_v_t_Session sv={s,1};coax_v_t_Engine ev={e,1};coax_v_t_PsEngine pv={p,1};coax_v_t_GpuSwitch gv={g,1};
  lab_consumer_init(lv,sv,ev,pv,gv,body.view,out.view,err.view,pool.view);
  REQUIRE(l->owner.e[0].generation==0&&l->owner.e[0].phase==0&&l->owner.e[0].call.e[0].h==-1&&l->ticket==0);
  lab_consumer_ask(lv,sv);REQUIRE(l->phase==1&&l->ticket==1&&s->n==1&&s->live);
  int before=effects;lab_consumer_ask(lv,sv);REQUIRE(effects==before&&l->ticket==1&&s->n==1);
  if(scenario==3){
   REQUIRE(lab_consumer_stop(lv,sv)&&!s->live&&l->phase==0&&l->owner.e[0].phase==0&&!live&&releases==1);
   before=effects;REQUIRE(!lab_consumer_stop(lv,sv));lab_consumer_pump(lv,ev,pv,gv,sv);REQUIRE(effects==before&&dispatches==0);cases++;
  }else{
   if(scenario!=4){lab_consumer_pump(lv,ev,pv,gv,sv);REQUIRE(l->phase==1&&s->n==1&&dispatches==0);}
   lab_consumer_pump(lv,ev,pv,gv,sv);
   REQUIRE(l->phase==0&&l->owner.e[0].phase==0&&l->owner.e[0].call.e[0].h==-1&&!live&&releases==1&&s->n==2);
   if(scenario==0){REQUIRE(dispatches==1&&s->parts.e[1].kind==5&&equal(lab_consumer_text(sv,1),"Routed to Claude Code · Sonnet 5.5 · Medium · 0.0 s"));REQUIRE(has(s->parts.e[1].output,"claude_model  claude-sonnet-5-5"));}
   else{REQUIRE(!s->live&&dispatches==0);coax_string shown=lab_consumer_text(sv,1);if(scenario==1)REQUIRE(equal(shown,"Elephant is busy with another decision (503 busy). Send again in a moment."));if(scenario==2)REQUIRE(has(shown,"the request timed out")&&has(shown,"scripted child error"));if(scenario==4)REQUIRE(has(shown,"curl not found"));if(scenario==5)REQUIRE(has(shown,"a question has no answer"));}
   /* Every displayed descriptor is a session-pool copy. Owner/wire reuse can
    * destroy neither it nor the detached stack snapshot from which it came. */
   coax_string shown=lab_consumer_text(sv,1);size_t n=(size_t)shown.count;char*saved=malloc(n);memcpy(saved,shown.data,n);coax_string detail=s->parts.e[1].output;char*saved_detail=malloc((size_t)detail.count);memcpy(saved_detail,detail.data,(size_t)detail.count);
   memset(out.view.data,0x3c,(size_t)out.view.count);memset(err.view.data,0x4d,(size_t)err.view.count);REQUIRE(!memcmp(saved,shown.data,n)&&!memcmp(saved_detail,detail.data,(size_t)detail.count));
   before=effects;lab_consumer_pump(lv,ev,pv,gv,sv);REQUIRE(s->n==2&&effects==before&&dispatches==(scenario==0?1:0));free(saved);free(saved_detail);cases++;
  }
  /* A copied old ticket cannot release an unrelated native slot after owner
   * retirement, cancellation or duplicate completion. */
  live=1;before=effects;REQUIRE(!lab_consumer_stale(lv,1)&&effects==before&&live&&releases==1);live=0;
  REQUIRE(guards(body)&&guards(out)&&guards(err)&&guards(pool));free(body.allocation);free(out.allocation);free(err.allocation);free(pool.allocation);free(l);free(s);free(e);free(p);free(g);
 }
 printf("{\"consumer_cases\":%u,\"actual_lab_path\":true}\n",cases);return 0;
}
