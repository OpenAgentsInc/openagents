/* Actual generated Coax plus the actual native process pool. No network,
 * production key, service, callbacks or copied-owner/handle safety claim. */
#define _POSIX_C_SOURCE 200809L
#include "systemone-lifecycle.h"
#include "process-native.h"
#include <errno.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>
static FILE*diagnostics(void){return stderr;}
#undef stderr
#define REQUIRE(x) do{if(!(x)){fprintf(diagnostics(),"failed line %d: %s\n",__LINE__,#x);return 1;}}while(0)
typedef struct{uint8_t before[8],data[512],after[8];}Guard;
static void fill(Guard*g){memset(g,0xa5,sizeof *g);}
static int guards(const Guard*g){for(int i=0;i<8;i++)if(g->before[i]!=0xa5||g->after[i]!=0xa5)return 0;return 1;}
static coax_string text(const char*s){return(coax_string){s,(int64_t)strlen(s)};}
static coax_v_u8 bytes(Guard*g){return(coax_v_u8){g->data,512};}
static coax_v_t_SoCall owner(coax_t_SoCall*c){return(coax_v_t_SoCall){c,1};}
static int content(coax_string s,const char*t){return s.count==(int64_t)strlen(t)&&!memcmp(s.data,t,(size_t)s.count);}
static void disturb(void){volatile uint8_t scratch[16384];for(size_t i=0;i<sizeof scratch;i++)scratch[i]=(uint8_t)(i*3);}
/* Every slot in this standalone process belongs to this harness. Only0/1
 * are ever allocated; a failed REQUIRE must not leave the sleeping child. */
static void cleanup(void){(void)cxproc_release(1);(void)cxproc_release(0);}
static coax_t_SoClient client(const char*exe,const char*mode){coax_t_SoClient c;coax_default_t_SoClient(&c);c.curl=text(exe);c.url=text(mode);c.key_env=text("");return c;}
static int reaped(int64_t pid){
  errno=0;if(waitpid((pid_t)pid,NULL,WNOHANG)!=-1||errno!=ECHILD)return 0;
  errno=0;return kill((pid_t)pid,0)==-1&&errno==ESRCH;
}
static int preserved_reclaimed_slot(coax_t_SoCall*call){
  int64_t unrelated=cxproc_new();REQUIRE(unrelated==0);
  so_life_cancel(owner(call));so_life_cancel(owner(call));
  REQUIRE(so_life_step(owner(call),0));
  int64_t next=cxproc_new();REQUIRE(next==1);
  REQUIRE(cxproc_release(next)==0&&cxproc_release(unrelated)==0);
  return 0;
}
static int completed(const char*exe,const char*mode,int is_error){
  Guard out,err;fill(&out);fill(&err);coax_t_SoCall call;coax_default_t_SoCall(&call);
  REQUIRE(so_life_start(client(exe,mode),text("{}"),owner(&call),bytes(&out),bytes(&err)));
  REQUIRE(call.h==0&&call.out.data==out.data&&call.err.data==err.data);
  int64_t pid=cxproc_pid(call.h);REQUIRE(pid>0);
  REQUIRE(so_life_step(owner(&call),2000)&&call.done&&call.h==-1);
  REQUIRE(reaped(pid));coax_t_SoResult result=call.r;
  if(is_error){REQUIRE(result.status==0&&result.stderr.data==(const char*)err.data&&content(result.stderr,"offline child exit28")&&content(result.problem,"the request timed out"));}
  else{REQUIRE(result.status==200&&result.problem.count==0&&result.body.data==(const char*)out.data&&content(result.body,"{\"answers\":{},\"usage\":{\"input_tokens\":7,\"output_tokens\":2}}"));REQUIRE(result.usage.ok&&result.usage.input_tokens==7&&result.usage.output_tokens==2);}
  disturb();REQUIRE(guards(&out)&&guards(&err));
  if(is_error)REQUIRE(content(result.stderr,"offline child exit28"));
  else REQUIRE(content(result.body,"{\"answers\":{},\"usage\":{\"input_tokens\":7,\"output_tokens\":2}}"));
  REQUIRE(preserved_reclaimed_slot(&call)==0&&reaped(pid));
  if(is_error)REQUIRE(result.stderr.data==(const char*)err.data&&content(result.stderr,"offline child exit28"));
  else REQUIRE(result.body.data==(const char*)out.data&&content(result.body,"{\"answers\":{},\"usage\":{\"input_tokens\":7,\"output_tokens\":2}}"));
  return 0;
}
static int cancelled(const char*exe){
  Guard out,err;fill(&out);fill(&err);coax_t_SoCall call;coax_default_t_SoCall(&call);
  REQUIRE(so_life_start(client(exe,"offline-sleep"),text("{}"),owner(&call),bytes(&out),bytes(&err)));
  int64_t pid=cxproc_pid(call.h);REQUIRE(pid>0);
  int64_t deadline=cxproc_now_ms()+2000;
  while(!call.lines&&cxproc_now_ms()<deadline){REQUIRE(!so_life_step(owner(&call),0));struct timespec pause={0,1000000};nanosleep(&pause,NULL);}
  REQUIRE(call.lines==1&&call.at==8&&!memcmp(out.data,"sleeping",8)&&kill((pid_t)pid,0)==0);
  so_life_cancel(owner(&call));REQUIRE(call.done&&call.h==-1&&content(call.r.problem,"cancelled")&&reaped(pid));
  REQUIRE(preserved_reclaimed_slot(&call)==0&&guards(&out)&&guards(&err)&&reaped(pid));
  puts("real native process: sleeping child cancelled and reaped; duplicate cancel/poll preserves unrelated reused slot0");return 0;
}
int main(int argc,char**argv){
  REQUIRE(atexit(cleanup)==0);
  REQUIRE(argc==2);
  REQUIRE(completed(argv[1],"offline-success",0)==0);
  puts("real native process: success reaped, slots reclaimed, borrowed body pointer/extent/content survives stack disturbance and duplicate terminal uses");
  REQUIRE(completed(argv[1],"offline-exit28",1)==0);
  puts("real native process: exit28 stderr reaped and borrowed error backing preserved");
  REQUIRE(cancelled(argv[1])==0);
  puts("System One real process smoke passes; offline owned child only, no full desktop or general ownership qualification");return 0;
}
