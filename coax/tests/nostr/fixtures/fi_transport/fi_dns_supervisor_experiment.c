#define _POSIX_C_SOURCE 200809L
/* Dedicated killable resolver process. It reports OS resolver results only;
   address admission and all trust decisions remain in the parent Coax policy. */
#include <arpa/inet.h>
#include <errno.h>
#include <fcntl.h>
#include <netdb.h>
#include <stddef.h>
#include <poll.h>
#include <signal.h>
#include <stdlib.h>
#include <sys/wait.h>
#include <time.h>
#include <stdint.h>
#include <string.h>
#include <unistd.h>
#define FI_DNS_RECORD 23
#ifndef FI_DNS_EXPERIMENT_LOCK_FD
#define FI_DNS_EXPERIMENT_LOCK_FD (-1)
#endif
static int resolve(const char *host,const char *port) {
 struct addrinfo hints={0},*answers=NULL;hints.ai_family=AF_UNSPEC;hints.ai_socktype=SOCK_STREAM;hints.ai_protocol=IPPROTO_TCP;
 if(getaddrinfo(host,port,&hints,&answers)!=0 || !answers)return 2;
 uint8_t data[5+17*FI_DNS_RECORD]={ 'F','D','R','1',0 };int count=0;
 for(struct addrinfo *a=answers;a && count<17;a=a->ai_next){
  uint8_t *r=data+5+count*FI_DNS_RECORD;count++;
  if(!a->ai_addr || a->ai_addrlen<offsetof(struct sockaddr,sa_family)+sizeof(a->ai_addr->sa_family))continue;
  if(a->ai_addr->sa_family!=a->ai_family)continue;
  if(a->ai_family==AF_INET && a->ai_addrlen>=sizeof(struct sockaddr_in)){
   const struct sockaddr_in *v=(const void *)a->ai_addr;r[0]=4;memcpy(r+1,&v->sin_port,2);memcpy(r+3,&v->sin_addr,4);
  } else if(a->ai_family==AF_INET6 && a->ai_addrlen>=sizeof(struct sockaddr_in6)){
   const struct sockaddr_in6 *v=(const void *)a->ai_addr;r[0]=16;memcpy(r+1,&v->sin6_port,2);memcpy(r+3,&v->sin6_addr,16);
   uint32_t scope=htonl(v->sin6_scope_id);memcpy(r+19,&scope,4);
  }
 }
 freeaddrinfo(answers);data[4]=(uint8_t)count;size_t n=5+(size_t)count*FI_DNS_RECORD,at=0;
 while(at<n){ssize_t wrote=write(STDOUT_FILENO,data+at,n-at);if(wrote<0 && errno==EINTR)continue;if(wrote<=0)return 2;at+=(size_t)wrote;}
 return 0;
}

/* Experimental standalone supervisor, not a provider. No process outside this
   single-threaded program may reap its child. The receiving service must not
   wait on or signal this supervisor by PID; a private pipe carries completion.
   A separate control pipe signals cancellation without losing the EOF witness. */
static int64_t now_ns(void) {
 struct timespec t;if(clock_gettime(CLOCK_MONOTONIC,&t)!=0 || t.tv_sec<0 || t.tv_sec>INT64_MAX/1000000000LL)return -1;
 int64_t n=(int64_t)t.tv_sec*1000000000LL;if(t.tv_nsec<0 || t.tv_nsec>=1000000000L || n>INT64_MAX-t.tv_nsec)return -1;
 return n+t.tv_nsec;
}
static int output_gone(void) {
 struct pollfd out={STDOUT_FILENO,POLLOUT,0};int n=poll(&out,1,0);
 return (n<0 && errno!=EINTR) || (n>0 && (out.revents&(POLLERR|POLLHUP|POLLNVAL)));
}
static int cancel_requested(void) {
 struct pollfd in={STDIN_FILENO,POLLIN,0};int n=poll(&in,1,0);
 return (n<0 && errno!=EINTR) || (n>0 && (in.revents&(POLLIN|POLLERR|POLLHUP|POLLNVAL)));
}
/* A terminal frame is cleanup evidence only under a trusted, sole-owner
   supervisor and private descriptor contract. It does not authenticate a
   key source or permit a response after cancellation/deadline. */
static int publish(const uint8_t *bytes,size_t n) {
 if(output_gone() || fcntl(STDOUT_FILENO,F_SETFL,O_NONBLOCK)<0)return 2;
 return write(STDOUT_FILENO,bytes,n)==(ssize_t)n?0:2;
}
int main(int argc,char **argv) {
 if(argc!=4)return 2;char *end=NULL;errno=0;long long deadline=strtoll(argv[3],&end,10);
 if(errno || !end || *end || deadline<=0 || now_ns()<0 || now_ns()>=deadline)return 2;
 if(FI_DNS_EXPERIMENT_LOCK_FD>=0 && fcntl(FI_DNS_EXPERIMENT_LOCK_FD,F_GETFD)<0)return 2;
 if(signal(SIGCHLD,SIG_DFL)==SIG_ERR)return 2;
 int fds[2];if(pipe(fds)!=0)return 2;if(fds[0]<=2 || fds[1]<=2){close(fds[0]);close(fds[1]);return 2;}
 pid_t pid=fork();if(pid<0){close(fds[0]);close(fds[1]);return 2;}
 if(pid==0){close(fds[0]);if(dup2(fds[1],STDOUT_FILENO)<0)_exit(2);close(fds[1]);_exit(resolve(argv[1],argv[2]));}
 close(fds[1]);int status=0,aborted=0;
 for(;;){
  pid_t result=waitpid(pid,&status,WNOHANG);
  if(result==pid)break;
  if(result<0 && errno!=EINTR){close(fds[0]);return 2;}
  int64_t now=now_ns();if(now<0 || now>=deadline || output_gone() || cancel_requested()){
   aborted=1;kill(pid,SIGKILL);do{result=waitpid(pid,&status,0);}while(result<0 && errno==EINTR);
   if(result!=pid){close(fds[0]);return 2;}break;
  }
  struct timespec pause={0,5000000};nanosleep(&pause,NULL);
 }
 /* EOF on the private resolver pipe is required even for a failure terminal.
    Missing EOF or an oversized frame leaves the service slot quarantined. */
 if(fcntl(fds[0],F_SETFL,O_NONBLOCK)<0){close(fds[0]);return 2;}
 uint8_t wire[5+17*FI_DNS_RECORD+1];size_t used=0;int eof=0;
 while(used<sizeof(wire)){
  ssize_t n=read(fds[0],wire+used,sizeof(wire)-used);
  if(n>0){used+=(size_t)n;continue;}if(n==0){eof=1;break;}if(errno==EINTR)continue;break;
 }
 close(fds[0]);if(!eof)return 2;
 int valid=used>=5 && memcmp(wire,"FDR1",4)==0 && wire[4]>=1 && wire[4]<=17 && used==5+(size_t)wire[4]*FI_DNS_RECORD;
 if(aborted || !WIFEXITED(status) || WEXITSTATUS(status)!=0 || !valid || now_ns()<0 || now_ns()>=deadline || cancel_requested()){
  const uint8_t terminal[4]={'F','D','E','1'};return publish(terminal,sizeof(terminal));
 }
 return publish(wire,used);
}
