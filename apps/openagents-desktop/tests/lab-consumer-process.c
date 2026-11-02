/* Actual Lab request→typed owner→native process→validation→transcript, with
 * downstream engine dispatch stubbed. No aggregate-padding oracle. */
#define _POSIX_C_SOURCE 200809L
#include "lab-consumer.h"
#include "process-native.h"
#include <errno.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/wait.h>
#include <time.h>
static FILE*diagnostics(void){return stderr;}
#undef stderr
#define REQUIRE(x) do{if(!(x)){fprintf(diagnostics(),"FAIL %d: %s\n",__LINE__,#x);return 1;}}while(0)
static int dispatches;
void lab_consumer_effect(int64_t code){if(code!=1){fprintf(diagnostics(),"unexpected downstream effect %lld\n",(long long)code);exit(98);}dispatches++;}
static void cleanup(void){(void)cxproc_release(1);(void)cxproc_release(0);}
static int has(coax_string s,const char*needle){size_t n=strlen(needle);for(int64_t i=0;i+(int64_t)n<=s.count;i++)if(!memcmp(s.data+i,needle,n))return 1;return 0;}
static int reaped(int64_t pid){errno=0;if(waitpid((pid_t)pid,NULL,WNOHANG)!=-1||errno!=ECHILD)return 0;errno=0;return kill((pid_t)pid,0)==-1&&errno==ESRCH;}
typedef struct{uint8_t*allocation;coax_v_u8 view;}Guard;
static Guard guard(size_t n){uint8_t*p=malloc(n+32);if(!p)abort();memset(p,0xa5,n+32);return(Guard){p,{p+16,(int64_t)n}};}
static int guards(Guard g){for(int i=0;i<16;i++)if(g.allocation[i]!=0xa5||g.allocation[16+g.view.count+i]!=0xa5)return 0;return 1;}
static void pause_poll(void){struct timespec pause={0,1000000};nanosleep(&pause,NULL);}
static int pump_until_idle(coax_v_t_Lab l,coax_v_t_Engine e,coax_v_t_PsEngine p,coax_v_t_GpuSwitch g,coax_v_t_Session s){int64_t deadline=cxproc_now_ms()+3000;while(l.data[0].phase&&cxproc_now_ms()<deadline){lab_consumer_pump(l,e,p,g,s);pause_poll();}return l.data[0].phase==0;}
int main(int argc,char**argv){
 REQUIRE(argc==2&&atexit(cleanup)==0);
 /* Derive the child reply from the actual authored response. Only its usage
  * null is replaced with a predeclared scalar fixture to exercise every use. */
 coax_string sample=lab_consumer_fixture();const char*needle="\"usage\":null";int64_t at=-1;for(int64_t i=0;i+12<=sample.count;i++)if(!memcmp(sample.data+i,needle,12)){at=i;break;}REQUIRE(at>=0);
 FILE*f=fopen(argv[1],"wb");REQUIRE(f);REQUIRE(fwrite(sample.data,1,(size_t)at,f)==(size_t)at);fputs("\"usage\":{\"input_tokens\":7,\"output_tokens\":2}",f);REQUIRE(fwrite(sample.data+at+12,1,(size_t)(sample.count-at-12),f)==(size_t)(sample.count-at-12)&&fclose(f)==0);
 REQUIRE(setenv("COAX_LAB_TEST_RESPONSE",argv[1],1)==0);
 coax_t_Lab*l=malloc(sizeof*l);coax_t_Session*s=malloc(sizeof*s);coax_t_Engine*e=malloc(sizeof*e);coax_t_PsEngine*p=malloc(sizeof*p);coax_t_GpuSwitch*g=malloc(sizeof*g);REQUIRE(l&&s&&e&&p&&g);coax_default_t_Engine(e);coax_default_t_PsEngine(p);
 Guard body=guard(24576),out=guard(65536),err=guard(4096),pool=guard(65536);coax_v_t_Lab lv={l,1};coax_v_t_Session sv={s,1};coax_v_t_Engine ev={e,1};coax_v_t_PsEngine pv={p,1};coax_v_t_GpuSwitch gv={g,1};
 lab_consumer_init(lv,sv,ev,pv,gv,body.view,out.view,err.view,pool.view);
 REQUIRE(setenv("COAX_LAB_TEST_MODE","success",1)==0);lab_consumer_ask(lv,sv);REQUIRE(l->ticket==1&&l->owner.e[0].generation==1&&l->owner.e[0].call.e[0].h==0);int64_t pid=cxproc_pid(0);REQUIRE(pid>0&&pump_until_idle(lv,ev,pv,gv,sv)&&reaped(pid));
 REQUIRE(dispatches==1&&s->n==2&&s->parts.e[1].kind==5&&l->owner.e[0].phase==0);
 coax_string headline=lab_consumer_text(sv,1),details=s->parts.e[1].output,arg=s->parts.e[1].arg,detail=s->parts.e[1].detail;
 REQUIRE(has(headline,"Routed to Claude Code · Sonnet 5.5 · Medium")&&has(details,"claude_model  claude-sonnet-5-5")&&has(details,"7 tokens in · 2 out")&&has(details,"elephant-1@sha256:")&&has(arg,"POST http://127.0.0.1:8790/v1/systemone")&&has(detail,"elephant-1 · "));
 char*saved=malloc((size_t)details.count);char*saved_head=malloc((size_t)headline.count);char*saved_arg=malloc((size_t)arg.count);char*saved_detail=malloc((size_t)detail.count);REQUIRE(saved&&saved_head&&saved_arg&&saved_detail);memcpy(saved,details.data,(size_t)details.count);memcpy(saved_head,headline.data,(size_t)headline.count);memcpy(saved_arg,arg.data,(size_t)arg.count);memcpy(saved_detail,detail.data,(size_t)detail.count);
 memset(out.view.data,0x31,(size_t)out.view.count);memset(err.view.data,0x32,(size_t)err.view.count);
 REQUIRE(setenv("COAX_LAB_TEST_MODE","delayed",1)==0);lab_consumer_again(lv,sv);REQUIRE(l->ticket==2&&l->owner.e[0].generation==2);pid=cxproc_pid(0);REQUIRE(pid>0);int64_t deadline=cxproc_now_ms()+3000;
 while(l->owner.e[0].call.e[0].lines<2&&cxproc_now_ms()<deadline){lab_consumer_pump(lv,ev,pv,gv,sv);pause_poll();}
 REQUIRE(l->phase==1&&l->owner.e[0].phase==1&&l->owner.e[0].call.e[0].lines==2&&l->owner.e[0].call.e[0].r.body.count==0&&kill((pid_t)pid,0)==0);
 REQUIRE(!lab_consumer_cancel_ticket(lv,1)&&!lab_consumer_stale(lv,1)&&kill((pid_t)pid,0)==0&&l->ticket==2);
 REQUIRE(lab_consumer_stop(lv,sv)&&l->owner.e[0].phase==0&&l->phase==0&&reaped(pid)&&!s->live);
 int64_t unrelated=cxproc_new();REQUIRE(unrelated==0);REQUIRE(!lab_consumer_stop(lv,sv)&&!lab_consumer_cancel_ticket(lv,2)&&!lab_consumer_stale(lv,2));lab_consumer_pump(lv,ev,pv,gv,sv);int64_t next=cxproc_new();REQUIRE(next==1&&cxproc_release(next)==0&&cxproc_release(unrelated)==0);
 REQUIRE(!memcmp(saved,details.data,(size_t)details.count)&&!memcmp(saved_head,headline.data,(size_t)headline.count)&&!memcmp(saved_arg,arg.data,(size_t)arg.count)&&!memcmp(saved_detail,detail.data,(size_t)detail.count));
 const char*modes[]={"error","busy","malformed","not-connected"};const char*expected[]={"offline Lab child exit28","Elephant is busy with another decision (503 busy)","a question has no answer","Elephant isn't reachable"};
 for(int i=0;i<4;i++){REQUIRE(setenv("COAX_LAB_TEST_MODE",modes[i],1)==0);lab_consumer_again(lv,sv);REQUIRE(l->ticket==i+3&&l->owner.e[0].generation==i+3);pid=cxproc_pid(0);REQUIRE(pid>0&&pump_until_idle(lv,ev,pv,gv,sv)&&reaped(pid)&&l->owner.e[0].phase==0&&!s->live&&dispatches==1);REQUIRE(has(lab_consumer_text(sv,s->n-1),expected[i]));int64_t n=s->n;lab_consumer_pump(lv,ev,pv,gv,sv);REQUIRE(s->n==n&&l->ticket==i+3);}
 REQUIRE(!memcmp(saved,details.data,(size_t)details.count)&&guards(body)&&guards(out)&&guards(err)&&guards(pool));free(saved);free(saved_head);free(saved_arg);free(saved_detail);free(body.allocation);free(out.allocation);free(err.allocation);free(pool.allocation);free(l);free(s);free(e);free(p);free(g);
 puts("{\"actual_lab_native_process_cases\":6,\"generations\":6,\"all_transcript_borrows_retained\":true,\"cancel_reaped\":true,\"stale_tickets_inert\":true}");return 0;
}
