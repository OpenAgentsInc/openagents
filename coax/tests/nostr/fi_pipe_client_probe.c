#define _POSIX_C_SOURCE 200809L
#include "fi_pipe_client_experiment.h"
#include <assert.h>
#include <pthread.h>
#include <signal.h>
#include <stdatomic.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>
static atomic_int stop,reaped;
static void pause_ms(int ms){struct timespec t={ms/1000,(ms%1000)*1000000L};nanosleep(&t,NULL);}
static void *reaper(void *x){(void)x;while(!atomic_load(&stop)){while(waitpid(-1,NULL,WNOHANG)>0)atomic_fetch_add(&reaped,1);pause_ms(1);}return NULL;}
static int result(FiPipeExperiment *f,uint8_t *out,int64_t *written){
 for(int i=0;i<4000;i++){int state=fi_pipe_experiment_poll(f,out,397,written);if(state)return state;pause_ms(1);}return -1;
}
static FiPipeExperiment *start_when_free(const char *host,int timeout){
 for(int i=0;i<4000;i++){FiPipeExperiment *f=fi_pipe_experiment_start(host,443,timeout);if(f)return f;pause_ms(1);}return NULL;
}
int main(int argc,char **argv){
 alarm(15);assert(argc==2);int auto_reap=atoi(argv[1]);assert(signal(SIGCHLD,auto_reap?SIG_IGN:SIG_DFL)!=SIG_ERR);
 pthread_t t;assert(!pthread_create(&t,NULL,reaper,NULL));uint8_t out[397];int64_t written=0;
#ifdef FI_TEST_DISABLED
 assert(fi_pipe_experiment_start("good.fixture",443,1000)==NULL);
#elif defined(FI_TEST_EXIT)
 /* The injected worker exits this test process immediately after spawn. */
 assert(fi_pipe_experiment_start("stall.fixture",443,4000));
 for(;;)pause_ms(1000);
#elif defined(FI_TEST_CRASH)
 FiPipeExperiment *f=fi_pipe_experiment_start("stall.fixture",443,4000);assert(f);
 assert(result(f,out,&written)==2 && written==0);fi_pipe_experiment_free(f);
 /* A crashed supervisor can leave a live resolver: no FDE1 means quarantine. */
 for(int i=0;i<200;i++){assert(fi_pipe_experiment_start("good.fixture",443,1000)==NULL);pause_ms(1);}
#elif defined(FI_TEST_FAKE)
 /* Negative control: unauthenticated helper fakes cleanup and discards lock. */
 FiPipeExperiment *f=fi_pipe_experiment_start("first.fixture",443,2000);assert(f);
 assert(result(f,out,&written)==2 && written==0);fi_pipe_experiment_free(f);
 f=start_when_free("second.fixture",2000);assert(f);
 assert(result(f,out,&written)==2 && written==0);fi_pipe_experiment_free(f);
 FILE *first=fopen(FI_TEST_MARKER,"r"),*second=fopen(FI_TEST_MARKER2,"r");assert(first && second);
 long p1=0,p2=0;char end1=0,end2=0;assert(fscanf(first,"%ld%c",&p1,&end1)==2 && fscanf(second,"%ld%c",&p2,&end2)==2 && end1=='\n' && end2=='\n');fclose(first);fclose(second);
 assert(p1>1 && p2>1 && kill((pid_t)p1,0)==0 && kill((pid_t)p2,0)==0);
#else
 FiPipeExperiment *f=fi_pipe_experiment_start("good.fixture",443,2000);assert(f);
 assert(result(f,out,&written)==1 && written==28 && out[0]=='F' && out[3]=='1');
 memset(out,165,sizeof(out));written=7;assert(fi_pipe_experiment_poll(f,out,1,&written)==2 && written==0 && out[0]==165);
 assert(result(f,out,&written)==1 && written==28);fi_pipe_experiment_free(f);
 f=start_when_free("good.fixture",1000);assert(f);assert(result(f,out,&written)==1);pause_ms(1050);
 memset(out,165,sizeof(out));written=7;assert(fi_pipe_experiment_poll(f,out,sizeof(out),&written)==2 && written==0 && out[0]==165);fi_pipe_experiment_free(f);
 f=start_when_free("stall.fixture",300);assert(f);
 for(int i=0;i<2000 && access(FI_TEST_MARKER,F_OK)!=0;i++)pause_ms(1);
 assert(access(FI_TEST_MARKER,F_OK)==0);assert(result(f,out,&written)==2 && written==0);fi_pipe_experiment_free(f);
 /* Test-only supervisor delays the cleanup terminal, keeping the slot busy. */
 assert(fi_pipe_experiment_start("good.fixture",443,1000)==NULL);
 f=start_when_free("good.fixture",2000);assert(f);assert(result(f,out,&written)==1 && written==28);fi_pipe_experiment_free(f);
 unlink(FI_TEST_MARKER);f=start_when_free("stall.fixture",2000);assert(f);
 for(int i=0;i<2000 && access(FI_TEST_MARKER,F_OK)!=0;i++)pause_ms(1);assert(access(FI_TEST_MARKER,F_OK)==0);
 fi_pipe_experiment_free(f);assert(fi_pipe_experiment_start("good.fixture",443,1000)==NULL);
 f=start_when_free("good.fixture",2000);assert(f);assert(result(f,out,&written)==1);fi_pipe_experiment_free(f);
#endif
 atomic_store(&stop,1);pthread_join(t,NULL);
#ifndef FI_TEST_DISABLED
 if(!auto_reap)assert(atomic_load(&reaped)>0);
#endif
 puts("test-only pipe client lifecycle passed");return 0;
}
