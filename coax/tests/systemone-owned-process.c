/* New canonical owner integration with the unchanged real process host. */
#define _POSIX_C_SOURCE 200809L
#include "systemone-owner.h"
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
static coax_string text(const char*s){return(coax_string){s,(int64_t)strlen(s)};}
static int content(coax_string s,const char*t){return s.count==(int64_t)strlen(t)&&!memcmp(s.data,t,(size_t)s.count);}
static void cleanup(void){(void)cxproc_release(1);(void)cxproc_release(0);}
static int reaped(int64_t pid){errno=0;if(waitpid((pid_t)pid,NULL,WNOHANG)!=-1||errno!=ECHILD)return 0;errno=0;return kill((pid_t)pid,0)==-1&&errno==ESRCH;}
static coax_t_SoClient client(const char*exe,const char*mode){coax_t_SoClient c;coax_default_t_SoClient(&c);c.curl=text(exe);c.url=text(mode);c.key_env=text("");return c;}
int main(int argc,char**argv){
 REQUIRE(argc==2&&atexit(cleanup)==0);
 uint8_t out[512],err[512],body[512],error[512],problem[256];coax_v_u8 ov={out,512},ev={err,512},bv={body,512},sv={error,512},pv={problem,256};
 coax_t_SoOwnedCall owner;coax_v_t_SoOwnedCall own={&owner,1};so_owner_init(own);coax_t_SoResult result;coax_v_t_SoResult rv={&result,1};
 int64_t first=so_owner_start(own,client(argv[1],"offline-success"),text("{}"),ov,ev);REQUIRE(first==1&&owner.call.e[0].h==0);
 int64_t pid=cxproc_pid(owner.call.e[0].h);REQUIRE(pid>0&&so_owner_poll(own,first,2000)&&reaped(pid));
 REQUIRE(so_owner_take(own,first,rv,bv,sv,pv)&&owner.phase==0&&owner.call.e[0].h==-1);
 REQUIRE(content(result.body,"{\"answers\":{},\"usage\":{\"input_tokens\":7,\"output_tokens\":2}}")&&result.usage.ok&&result.usage.input_tokens==7&&result.usage.output_tokens==2);
 memset(out,0x55,sizeof out);memset(err,0x66,sizeof err);REQUIRE(content(result.body,"{\"answers\":{},\"usage\":{\"input_tokens\":7,\"output_tokens\":2}}"));
 int64_t second=so_owner_start(own,client(argv[1],"offline-eof-cancel"),text("{}"),ov,ev);REQUIRE(second==2);pid=cxproc_pid(owner.call.e[0].h);REQUIRE(pid>0);
 int64_t deadline=cxproc_now_ms()+2000;
 while(owner.call.e[0].lines<2&&cxproc_now_ms()<deadline){REQUIRE(!so_owner_poll(own,second,0));struct timespec pause={0,1000000};nanosleep(&pause,NULL);}
 REQUIRE(owner.call.e[0].lines==2&&owner.phase==1&&owner.call.e[0].r.body.count==0&&kill((pid_t)pid,0)==0);
 REQUIRE(!so_owner_cancel(own,first)&&!so_owner_discard(own,first)&&!so_owner_poll(own,first,0)&&kill((pid_t)pid,0)==0);
 REQUIRE(so_owner_discard(own,second)&&owner.phase==0&&owner.call.e[0].h==-1&&reaped(pid));
 int64_t unrelated=cxproc_new();REQUIRE(unrelated==0);
 REQUIRE(!so_owner_discard(own,second)&&!so_owner_cancel(own,first)&&!so_owner_poll(own,second,0));
 int64_t next=cxproc_new();REQUIRE(next==1&&cxproc_release(next)==0&&cxproc_release(unrelated)==0&&reaped(pid));
 REQUIRE(content(result.body,"{\"answers\":{},\"usage\":{\"input_tokens\":7,\"output_tokens\":2}}"));
 puts("{\"real_owner_process_cases\":2,\"reaped\":true,\"stale_tickets_inert\":true,\"snapshot_survives_reuse\":true}");return 0;
}
