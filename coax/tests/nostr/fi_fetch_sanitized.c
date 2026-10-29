#define _POSIX_C_SOURCE 200809L
#include "fixtures/fi_transport/fi_fetch.h"
#include <assert.h>
#include <stdint.h>
#include <signal.h>
#include <string.h>
#include <time.h>
static void pause_ms(long ms){struct timespec t={ms/1000,(ms%1000)*1000000};nanosleep(&t,0);}
static int wait_result(FiNativeFetch *f,uint8_t *out,int64_t capacity,int64_t *written){
 for(int i=0;i<2000;i++){int state=fi_fetch_poll(f,out,capacity,written);if(state)return state;pause_ms(5);}return 0;
}
int main(void){
 uint8_t out[524288];int64_t written=-1;
 const char *bad="https://mixed.fixture/";FiNativeFetch *f=fi_fetch_start(bad,strlen(bad),1,9999);assert(f);
 assert(wait_result(f,out,sizeof(out),&written)==2 && written==0);fi_fetch_free(f);pause_ms(50);
 const char *stall="https://stall.fixture/";f=fi_fetch_start(stall,strlen(stall),1,50);assert(f);
 assert(wait_result(f,out,sizeof(out),&written)==2 && written==0);fi_fetch_free(f);pause_ms(100);
 /* Repeated timeout/free and immediate cancellation must not reuse live state. */
 for(int i=0;i<12;i++){
  f=NULL;for(int j=0;j<100 && !f;j++){f=fi_fetch_start(stall,strlen(stall),1,20);if(!f)pause_ms(5);}assert(f);
  assert(wait_result(f,out,sizeof(out),&written)==2 && written==0);fi_fetch_free(f);
 }
 pause_ms(100);f=fi_fetch_start(stall,strlen(stall),1,9999);assert(f);fi_fetch_free(f);pause_ms(100);
 /* Host-wide auto-reaping must fail closed without signalling a reused PID. */
 struct sigaction ignore={0},old={0};ignore.sa_handler=SIG_IGN;sigemptyset(&ignore.sa_mask);assert(sigaction(SIGCHLD,&ignore,&old)==0);
 f=fi_fetch_start(stall,strlen(stall),1,50);assert(f);assert(wait_result(f,out,sizeof(out),&written)==2);fi_fetch_free(f);pause_ms(150);
 assert(sigaction(SIGCHLD,&old,NULL)==0);pause_ms(50);
 const char *live="https://ipv4.icanhazip.com/";f=fi_fetch_start(live,strlen(live),1,9999);assert(f);
 assert(wait_result(f,out,sizeof(out),&written)==1 && written>0);fi_fetch_free(f);pause_ms(50);
 return 0;
}
