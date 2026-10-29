#define _POSIX_C_SOURCE 200809L
#define _DARWIN_C_SOURCE 1
/* Isolated lifecycle experiment only. A caller-supplied helper is not trusted
   and its frames are not provider evidence. Normal builds refuse all starts. */
#include "fi_pipe_client_experiment.h"
#include <errno.h>
#include <fcntl.h>
#include <poll.h>
#include <pthread.h>
#include <spawn.h>
#include <stdatomic.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/file.h>
#include <sys/stat.h>
#include <time.h>
#include <unistd.h>
#ifndef FI_PIPE_TEST_HELPER_PATH
#define FI_PIPE_TEST_HELPER_PATH ""
#endif
#ifndef FI_PIPE_TEST_ONLY
#define FI_PIPE_TEST_ONLY 0
#endif
#ifndef FI_PIPE_TEST_LOCK_PATH
#define FI_PIPE_TEST_LOCK_PATH ""
#endif
#define FI_FRAME_MAX (5+17*23)
struct FiPipeExperiment {
 pthread_mutex_t mutex;
 atomic_int refs,cancelled;
 int state;
 int64_t deadline_ns;
 char host[254],port[6];
 uint8_t frame[FI_FRAME_MAX];size_t used;
};
/* 0 free, 1 occupied including deferred cleanup, 2 permanently quarantined. */
static atomic_int fi_pipe_slot;
static int64_t clock_ns(void) {
 struct timespec t;if(clock_gettime(CLOCK_MONOTONIC,&t)!=0 || t.tv_sec<0 || t.tv_sec>INT64_MAX/1000000000LL || t.tv_nsec<0 || t.tv_nsec>=1000000000L)return -1;
 int64_t s=(int64_t)t.tv_sec*1000000000LL;return s>INT64_MAX-t.tv_nsec?-1:s+t.tv_nsec;
}
static int expired(FiPipeExperiment *f) {int64_t now=clock_ns();return atomic_load(&f->cancelled) || now<0 || now>=f->deadline_ns;}
static void release(FiPipeExperiment *f){if(atomic_fetch_sub(&f->refs,1)==1){pthread_mutex_destroy(&f->mutex);free(f);}}
static int frame_kind(const uint8_t *p,size_t n){
 if(n==4 && memcmp(p,"FDE1",4)==0)return 2;
 if(n>=5 && memcmp(p,"FDR1",4)==0 && p[4]>=1 && p[4]<=17 && n==5+(size_t)p[4]*23)return 1;
 return 0;
}
static void close_if(int *fd){if(*fd>=0){close(*fd);*fd=-1;}}
static void *worker(void *arg){
 FiPipeExperiment *f=arg;int data[2]={-1,-1},control[2]={-1,-1},lockfd=-1;int spawned=0,eof=0,overflow=0,kind=0;
 uint8_t bytes[FI_FRAME_MAX+1];size_t used=0;posix_spawn_file_actions_t actions;int actions_ok=0;
 /* Test-only advisory lock: a trusted, stable inode is not provided here. */
 int initial=open(FI_PIPE_TEST_LOCK_PATH,O_RDWR|O_CREAT|O_NOFOLLOW|O_CLOEXEC,0600);struct stat st;
 if(initial<0)goto done;
 if(initial<=2 || fstat(initial,&st)!=0 || !S_ISREG(st.st_mode) || flock(initial,LOCK_EX|LOCK_NB)!=0){close(initial);goto done;}
 lockfd=fcntl(initial,F_DUPFD_CLOEXEC,65);close(initial);if(lockfd<0)goto done;
 if(pipe(data)!=0 || pipe(control)!=0)goto done;
 if(data[0]<=2 || data[1]<=2 || control[0]<=2 || control[1]<=2 || data[0]==64 || data[1]==64 || control[0]==64 || control[1]==64)goto done;
 for(int i=0;i<2;i++)if(fcntl(data[i],F_SETFD,FD_CLOEXEC)<0 || fcntl(control[i],F_SETFD,FD_CLOEXEC)<0)goto done;
 if(fcntl(data[0],F_SETFL,O_NONBLOCK)<0 || posix_spawn_file_actions_init(&actions)!=0)goto done;actions_ok=1;
 if(posix_spawn_file_actions_adddup2(&actions,data[1],STDOUT_FILENO)!=0 || posix_spawn_file_actions_adddup2(&actions,control[0],STDIN_FILENO)!=0 || posix_spawn_file_actions_adddup2(&actions,lockfd,64)!=0 || posix_spawn_file_actions_addclose(&actions,lockfd)!=0)goto done;
 for(int i=0;i<2;i++)if(posix_spawn_file_actions_addclose(&actions,data[i])!=0 || posix_spawn_file_actions_addclose(&actions,control[i])!=0)goto done;
 char deadline[32];int n=snprintf(deadline,sizeof(deadline),"%lld",(long long)f->deadline_ns);if(n<1 || (size_t)n>=sizeof(deadline) || expired(f))goto done;
 char *argv[]={(char *)FI_PIPE_TEST_HELPER_PATH,f->host,f->port,deadline,NULL};char *envp[]={NULL};pid_t ignored_pid;
 if(posix_spawn(&ignored_pid,FI_PIPE_TEST_HELPER_PATH,&actions,NULL,argv,envp)!=0)goto done;
 /* PID is deliberately never waited on, signalled, or used as an identity. */
 spawned=1;close_if(&data[1]);close_if(&control[0]);
 while(!eof){
  if(expired(f))close_if(&control[1]);
  struct pollfd p={data[0],POLLIN|POLLHUP,0};int r=poll(&p,1,10);
  if(r<0 && errno!=EINTR)goto done;
  if(r>0){uint8_t discard[512];void *dest=overflow?discard:bytes+used;size_t cap=overflow?sizeof(discard):sizeof(bytes)-used;
   ssize_t got=read(data[0],dest,cap);if(got>0){if(!overflow){used+=(size_t)got;if(used==sizeof(bytes)){overflow=1;close_if(&control[1]);}}}
   else if(got==0)eof=1;else if(errno!=EAGAIN && errno!=EINTR)goto done;
  }
 }
 if(!overflow)kind=frame_kind(bytes,used);
 done:
 close_if(&data[0]);close_if(&data[1]);close_if(&control[0]);close_if(&control[1]);close_if(&lockfd);if(actions_ok)posix_spawn_file_actions_destroy(&actions);
 pthread_mutex_lock(&f->mutex);
 if(f->state==0){
  if(kind==1 && !expired(f)){memcpy(f->frame,bytes,used);f->used=used;f->state=1;}else f->state=2;
 }
 pthread_mutex_unlock(&f->mutex);
 /* No terminal is not proof of cleanup, including a supervisor crash. */
 atomic_store(&fi_pipe_slot,spawned && (!eof || !kind)?2:0);release(f);return NULL;
}
FiPipeExperiment *fi_pipe_experiment_start(const char *host,int port,int64_t timeout_ms){
 if(!FI_PIPE_TEST_ONLY || FI_PIPE_TEST_HELPER_PATH[0]!='/' || FI_PIPE_TEST_LOCK_PATH[0]!='/' || !host || port<1 || port>65535 || timeout_ms<1 || timeout_ms>9999)return NULL;
 size_t n=strnlen(host,254);if(n<1 || n>=254)return NULL;
 int64_t now=clock_ns();if(now<0 || now>INT64_MAX-timeout_ms*1000000LL)return NULL;
 int expected=0;if(!atomic_compare_exchange_strong(&fi_pipe_slot,&expected,1))return NULL;
 FiPipeExperiment *f=calloc(1,sizeof(*f));if(!f){atomic_store(&fi_pipe_slot,0);return NULL;}
 if(pthread_mutex_init(&f->mutex,NULL)!=0){free(f);atomic_store(&fi_pipe_slot,0);return NULL;}
 atomic_init(&f->refs,2);atomic_init(&f->cancelled,0);f->deadline_ns=now+timeout_ms*1000000LL;
 memcpy(f->host,host,n);snprintf(f->port,sizeof(f->port),"%d",port);
 pthread_t t;if(pthread_create(&t,NULL,worker,f)!=0){atomic_store(&fi_pipe_slot,0);atomic_store(&f->refs,1);release(f);return NULL;}
 pthread_detach(t);return f;
}
int fi_pipe_experiment_poll(FiPipeExperiment *f,uint8_t *out,int64_t capacity,int64_t *written){
 if(!f || !written)return 2;*written=0;pthread_mutex_lock(&f->mutex);
 if(expired(f)){atomic_store(&f->cancelled,1);f->state=2;}
 int state=f->state;if(state==1){if(!out || capacity<0 || (uint64_t)capacity<f->used)state=2;else{memcpy(out,f->frame,f->used);*written=(int64_t)f->used;}}
 pthread_mutex_unlock(&f->mutex);return state;
}
void fi_pipe_experiment_free(FiPipeExperiment *f){if(f){atomic_store(&f->cancelled,1);release(f);}}
