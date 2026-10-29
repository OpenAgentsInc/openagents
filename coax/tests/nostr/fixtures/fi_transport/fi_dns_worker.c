#define _POSIX_C_SOURCE 200809L
/* Dedicated killable resolver process. It reports OS resolver results only;
   address admission and all trust decisions remain in the parent Coax policy. */
#include <arpa/inet.h>
#include <errno.h>
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

/* This process has exclusive ownership of its resolver child. The caller never
   signals this supervisor by PID: closing the pipe requests cancellation. */
static int64_t now_ns(void) {
 struct timespec t;if(clock_gettime(CLOCK_MONOTONIC,&t)!=0 || t.tv_sec<0 || t.tv_sec>INT64_MAX/1000000000LL)return -1;
 int64_t n=(int64_t)t.tv_sec*1000000000LL;if(t.tv_nsec<0 || t.tv_nsec>=1000000000L || n>INT64_MAX-t.tv_nsec)return -1;
 return n+t.tv_nsec;
}
int main(int argc,char **argv) {
 if(argc!=4)return 2;char *end=NULL;errno=0;long long deadline=strtoll(argv[3],&end,10);
 if(errno || !end || *end || deadline<=0 || now_ns()<0 || now_ns()>=deadline)return 2;
 if(signal(SIGCHLD,SIG_DFL)==SIG_ERR)return 2;
 pid_t pid=fork();if(pid<0)return 2;if(pid==0)_exit(resolve(argv[1],argv[2]));
 int status=0;
 for(;;){
  pid_t result=waitpid(pid,&status,WNOHANG);
  if(result==pid)return WIFEXITED(status) && WEXITSTATUS(status)==0?0:2;
  if(result<0 && errno!=EINTR)return 2;
  int64_t now=now_ns();struct pollfd out={STDOUT_FILENO,POLLOUT,0};int p=poll(&out,1,0);
  if(now<0 || now>=deadline || (p<0 && errno!=EINTR) || (p>0 && (out.revents&(POLLERR|POLLHUP|POLLNVAL)))){
   kill(pid,SIGKILL);while(waitpid(pid,&status,0)<0 && errno==EINTR){}return 2;
  }
  struct timespec pause={0,5000000};nanosleep(&pause,NULL);
 }
}
