#define _POSIX_C_SOURCE 200809L
#include "fi_fetch.h"
#include "fi_transport_generated.h"
#include <arpa/inet.h>
#include <curl/curl.h>
#include <errno.h>
#include <fcntl.h>
#include <poll.h>
#include <signal.h>
#include <spawn.h>
#include <stddef.h>
#include <sys/wait.h>
#include <netdb.h>
#include <pthread.h>
#include <stdatomic.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <time.h>
#include <unistd.h>

/* Network effects only; URI and address admission are the compiled Coax policy.
   No result represents an installed snapshot or authority to admit a session. */
#ifndef FI_DNS_HELPER_PATH
#define FI_DNS_HELPER_PATH ""
#endif
#ifndef FI_FETCH_TEST_UNSAFE_REAPER
#define FI_FETCH_TEST_UNSAFE_REAPER 0
#endif
#define FI_DNS_RECORD 23
#define FI_BODY_LIMIT 524288
#define FI_HEADER_LIMIT 16384
struct FiNativeFetch {
 pthread_mutex_t mutex;
 atomic_int refs, cancelled;
 int state;
 int64_t deadline_ns;
 char uri[2049], host[254], port_text[6];
 int port;
 struct sockaddr_storage pinned;
 socklen_t pinned_length;
 int connections, peer_checked;
 curl_socket_t socket_fd;
 size_t used, headers;
 uint8_t body[FI_BODY_LIMIT];
 CURL *easy;
};
static atomic_flag fi_slot=ATOMIC_FLAG_INIT;
static pthread_once_t fi_curl_once=PTHREAD_ONCE_INIT;
static int fi_curl_init_ok;
static void fi_curl_init(void) { fi_curl_init_ok=(curl_global_init(CURL_GLOBAL_DEFAULT)==CURLE_OK); }
static int64_t fi_clock(void) {
 struct timespec t;if(clock_gettime(CLOCK_MONOTONIC,&t)!=0)return -1;
 if(t.tv_sec<0 || t.tv_sec>INT64_MAX/1000000000LL || t.tv_nsec<0 || t.tv_nsec>=1000000000L)return -1;
 int64_t seconds=(int64_t)t.tv_sec*1000000000LL;if(seconds>INT64_MAX-t.tv_nsec)return -1;
 return seconds+t.tv_nsec;
}
static int fi_expired(FiNativeFetch *f) {
 int64_t now=fi_clock();return atomic_load(&f->cancelled) || now<0 || now>=f->deadline_ns;
}
static void fi_release(FiNativeFetch *f) {
 if(atomic_fetch_sub(&f->refs,1)==1){pthread_mutex_destroy(&f->mutex);free(f);}
}
static int fi_same_socket(const struct sockaddr *a,socklen_t alen,const struct sockaddr *b,socklen_t blen) {
 size_t family_end=offsetof(struct sockaddr,sa_family)+sizeof(a->sa_family);
 if(!a || !b || alen<family_end || blen<family_end || a->sa_family!=b->sa_family)return 0;
 if(a->sa_family==AF_INET){if(alen<sizeof(struct sockaddr_in) || blen<sizeof(struct sockaddr_in))return 0;
  const struct sockaddr_in *x=(const void *)a,*y=(const void *)b;
  return x->sin_port==y->sin_port && memcmp(&x->sin_addr,&y->sin_addr,4)==0;}
 if(a->sa_family==AF_INET6){if(alen<sizeof(struct sockaddr_in6) || blen<sizeof(struct sockaddr_in6))return 0;
  const struct sockaddr_in6 *x=(const void *)a,*y=(const void *)b;
  return x->sin6_port==y->sin6_port && x->sin6_scope_id==y->sin6_scope_id && memcmp(&x->sin6_addr,&y->sin6_addr,16)==0;}
 return 0;
}
/* The resolver runs in a separately killable process. The parent validates the
   complete bounded response and classifies every address in Coax. */
static int fi_resolve(FiNativeFetch *f) {
 int pipes[2]={-1,-1},status=0,reaped=0,eof=0,ok=0;pid_t pid=-1;size_t used=0;
 uint8_t wire[5+17*FI_DNS_RECORD+1];posix_spawn_file_actions_t actions;int actions_ok=0;
 if(!FI_DNS_HELPER_PATH[0] || fi_expired(f) || pipe(pipes)!=0)goto done;
 if(pipes[0]<=STDERR_FILENO || pipes[1]<=STDERR_FILENO)goto done;
 if(fcntl(pipes[0],F_SETFD,FD_CLOEXEC)<0 || fcntl(pipes[1],F_SETFD,FD_CLOEXEC)<0 ||
    fcntl(pipes[0],F_SETFL,O_NONBLOCK)<0)goto done;
 if(posix_spawn_file_actions_init(&actions)!=0)goto done;actions_ok=1;
 if(posix_spawn_file_actions_adddup2(&actions,pipes[1],STDOUT_FILENO)!=0 ||
    posix_spawn_file_actions_addclose(&actions,pipes[0])!=0)goto done;
 if(pipes[1]!=STDOUT_FILENO && posix_spawn_file_actions_addclose(&actions,pipes[1])!=0)goto done;
 char deadline[32];int dl=snprintf(deadline,sizeof(deadline),"%lld",(long long)f->deadline_ns);if(dl<1 || (size_t)dl>=sizeof(deadline))goto done;
 char *argv[]={(char *)FI_DNS_HELPER_PATH,f->host,f->port_text,deadline,NULL};char *envp[]={NULL};
 if(posix_spawn(&pid,FI_DNS_HELPER_PATH,&actions,NULL,argv,envp)!=0){pid=-1;goto done;}
 close(pipes[1]);pipes[1]=-1;
 while(!eof || !reaped){
  if(fi_expired(f))goto done;
  if(!reaped){pid_t result=waitpid(pid,&status,WNOHANG);if(result==pid)reaped=1;else if(result<0 && errno!=EINTR){if(errno==ECHILD)pid=-1;goto done;}}
  if(!eof){
   struct pollfd pfd={pipes[0],POLLIN|POLLHUP,0};int n=poll(&pfd,1,10);
   if(n<0 && errno!=EINTR)goto done;
   if(n>0){ssize_t nread=read(pipes[0],wire+used,sizeof(wire)-used);
    if(nread>0){used+=(size_t)nread;if(used==sizeof(wire))goto done;}
    else if(nread==0)eof=1;else if(errno!=EAGAIN && errno!=EINTR)goto done;
   }
  } else if(!reaped){struct timespec delay={0,1000000};nanosleep(&delay,NULL);}
 }
 if(!WIFEXITED(status) || WEXITSTATUS(status)!=0 || used<5 || memcmp(wire,"FDR1",4)!=0)goto done;
 int count=wire[4];if(count<1 || count>16 || used!=5+(size_t)count*FI_DNS_RECORD)goto done;
 for(int i=0;i<count;i++){
  uint8_t *r=wire+5+(size_t)i*FI_DNS_RECORD;int bytes=r[0];uint16_t port;memcpy(&port,r+1,2);
  if((bytes!=4 && bytes!=16) || port!=htons((uint16_t)f->port) || !fi_address_allowed((coax_v_u8){r+3,bytes},1))goto done;
  if(bytes==16 && (r[19] || r[20] || r[21] || r[22]))goto done;
  if(i==0){
   memset(&f->pinned,0,sizeof(f->pinned));
   if(bytes==4){struct sockaddr_in *v=(void *)&f->pinned;v->sin_family=AF_INET;v->sin_port=port;memcpy(&v->sin_addr,r+3,4);f->pinned_length=sizeof(*v);}
   else {struct sockaddr_in6 *v=(void *)&f->pinned;v->sin6_family=AF_INET6;v->sin6_port=port;memcpy(&v->sin6_addr,r+3,16);f->pinned_length=sizeof(*v);}
  }
 }
 ok=!fi_expired(f);
 done:
 /* Closing the read end asks the supervisor to stop its exclusively-owned
    resolver child. Never signal a parent-visible PID: another host reaper may
    already have reaped and recycled it. */
 if(pipes[0]>=0){close(pipes[0]);pipes[0]=-1;}if(pipes[1]>=0){close(pipes[1]);pipes[1]=-1;}
 if(pid>0 && !reaped){while(waitpid(pid,&status,0)<0 && errno==EINTR){}}
 if(actions_ok)posix_spawn_file_actions_destroy(&actions);return ok;
}
static curl_socket_t fi_socket(void *ctx,curlsocktype purpose,struct curl_sockaddr *address) {
 FiNativeFetch *f=ctx;
 if(fi_expired(f) || purpose!=CURLSOCKTYPE_IPCXN || ++f->connections!=1 ||
    !fi_same_socket(&address->addr,address->addrlen,(const struct sockaddr *)&f->pinned,f->pinned_length))return CURL_SOCKET_BAD;
 f->socket_fd=socket(address->family,address->socktype,address->protocol);return f->socket_fd;
}
static int fi_prereq(void *ctx,char *primary,char *local,int primary_port,int local_port) {
 (void)primary;(void)local;(void)primary_port;(void)local_port;
 FiNativeFetch *f=ctx;struct sockaddr_storage peer;socklen_t n=sizeof(peer);
 if(fi_expired(f) || f->socket_fd==CURL_SOCKET_BAD ||
    getpeername(f->socket_fd,(struct sockaddr *)&peer,&n)!=0 || !fi_same_socket((struct sockaddr *)&peer,n,(struct sockaddr *)&f->pinned,f->pinned_length))return CURL_PREREQFUNC_ABORT;
 f->peer_checked=1;return CURL_PREREQFUNC_OK;
}
static size_t fi_body(char *data,size_t size,size_t nmemb,void *ctx) {
 FiNativeFetch *f=ctx;if(size && nmemb>SIZE_MAX/size)return 0;size_t n=size*nmemb;
 if(fi_expired(f) || n>FI_BODY_LIMIT-f->used)return 0;
 memcpy(f->body+f->used,data,n);f->used+=n;return n;
}
static size_t fi_headers(char *data,size_t size,size_t nmemb,void *ctx) {
 (void)data;FiNativeFetch *f=ctx;if(size && nmemb>SIZE_MAX/size)return 0;size_t n=size*nmemb;
 if(fi_expired(f) || n>FI_HEADER_LIMIT-f->headers)return 0;f->headers+=n;return n;
}
static int fi_progress(void *ctx,curl_off_t a,curl_off_t b,curl_off_t c,curl_off_t d) {
 (void)a;(void)b;(void)c;(void)d;return fi_expired(ctx);
}
static void *fi_worker(void *context) {
 FiNativeFetch *f=context;struct curl_slist *resolve=NULL;int ok=0;
 if(!fi_resolve(f))goto done;
 char addr[INET6_ADDRSTRLEN],entry[512];const void *ip;
 int family=((struct sockaddr *)&f->pinned)->sa_family;
 if(family==AF_INET)ip=&((struct sockaddr_in *)&f->pinned)->sin_addr;
 else if(family==AF_INET6)ip=&((struct sockaddr_in6 *)&f->pinned)->sin6_addr;
 else goto done;
 if(!inet_ntop(family,ip,addr,sizeof(addr)))goto done;
 int length=snprintf(entry,sizeof(entry),family==AF_INET6?"%s:%d:[%s]":"%s:%d:%s",f->host,f->port,addr);
 if(length<0 || (size_t)length>=sizeof(entry) || !(resolve=curl_slist_append(NULL,entry)))goto done;
 pthread_once(&fi_curl_once,fi_curl_init);if(!fi_curl_init_ok || !(f->easy=curl_easy_init()))goto done;
 int64_t now=fi_clock();if(now<0 || now>=f->deadline_ns)goto done;
 long remaining=(long)((f->deadline_ns-now)/1000000LL);if(remaining<1)goto done;
#define FI_OPT(option,value) do { if(curl_easy_setopt(f->easy,(option),(value))!=CURLE_OK)goto done; } while(0)
 FI_OPT(CURLOPT_URL,f->uri);FI_OPT(CURLOPT_RESOLVE,resolve);FI_OPT(CURLOPT_PROXY,"");FI_OPT(CURLOPT_NOPROXY,"*");
 FI_OPT(CURLOPT_FOLLOWLOCATION,0L);FI_OPT(CURLOPT_MAXREDIRS,0L);FI_OPT(CURLOPT_FRESH_CONNECT,1L);FI_OPT(CURLOPT_FORBID_REUSE,1L);
 FI_OPT(CURLOPT_PROTOCOLS_STR,"https");FI_OPT(CURLOPT_SSL_VERIFYPEER,1L);FI_OPT(CURLOPT_SSL_VERIFYHOST,2L);
 FI_OPT(CURLOPT_HTTP_VERSION,(long)CURL_HTTP_VERSION_1_1);FI_OPT(CURLOPT_PATH_AS_IS,1L);FI_OPT(CURLOPT_NOSIGNAL,1L);
 FI_OPT(CURLOPT_TIMEOUT_MS,remaining);FI_OPT(CURLOPT_CONNECTTIMEOUT_MS,remaining);
 FI_OPT(CURLOPT_OPENSOCKETFUNCTION,fi_socket);FI_OPT(CURLOPT_OPENSOCKETDATA,f);
 FI_OPT(CURLOPT_PREREQFUNCTION,fi_prereq);FI_OPT(CURLOPT_PREREQDATA,f);
 FI_OPT(CURLOPT_WRITEFUNCTION,fi_body);FI_OPT(CURLOPT_WRITEDATA,f);FI_OPT(CURLOPT_HEADERFUNCTION,fi_headers);FI_OPT(CURLOPT_HEADERDATA,f);
 FI_OPT(CURLOPT_NOPROGRESS,0L);FI_OPT(CURLOPT_XFERINFOFUNCTION,fi_progress);FI_OPT(CURLOPT_XFERINFODATA,f);
 CURLcode result=curl_easy_perform(f->easy);long status=0;
 if(result==CURLE_OK && curl_easy_getinfo(f->easy,CURLINFO_RESPONSE_CODE,&status)==CURLE_OK &&
    status>=200 && status<300 && f->peer_checked && f->connections==1 && f->used>0 && !fi_expired(f))ok=1;
#undef FI_OPT
 done:
 if(f->easy)curl_easy_cleanup(f->easy);if(resolve)curl_slist_free_all(resolve);
 pthread_mutex_lock(&f->mutex);if(f->state==0 && ok && !fi_expired(f))f->state=1;else if(f->state==0)f->state=2;pthread_mutex_unlock(&f->mutex);
 atomic_flag_clear(&fi_slot);fi_release(f);return NULL;
}
FiNativeFetch *fi_fetch_start(const char *uri,int64_t length,int64_t transport,int64_t deadline_ms) {
 if(!FI_FETCH_TEST_UNSAFE_REAPER || FI_DNS_HELPER_PATH[0]!='/' || !uri || length<1 || length>2048 || transport!=1 || deadline_ms<1 || deadline_ms>9999)return NULL;
 int64_t now=fi_clock();if(now<0 || now>INT64_MAX-deadline_ms*1000000LL)return NULL;
 coax_string text={uri,length};coax_t_UriParts p=uri_parse(text);
 if(!p.ok || !fi_source_uri(text,transport) || p.host_to-p.host_from<1 || p.host_to-p.host_from>253)return NULL;
 if(atomic_flag_test_and_set(&fi_slot))return NULL;
 FiNativeFetch *f=calloc(1,sizeof(*f));if(!f){atomic_flag_clear(&fi_slot);return NULL;}
 if(pthread_mutex_init(&f->mutex,NULL)!=0){free(f);atomic_flag_clear(&fi_slot);return NULL;}
 atomic_init(&f->refs,2);atomic_init(&f->cancelled,0);f->socket_fd=CURL_SOCKET_BAD;f->deadline_ns=now+deadline_ms*1000000LL;
 memcpy(f->uri,uri,(size_t)length);memcpy(f->host,uri+p.host_from,(size_t)(p.host_to-p.host_from));f->port=(int)p.port;
 snprintf(f->port_text,sizeof(f->port_text),"%d",f->port);
 pthread_t t;if(pthread_create(&t,NULL,fi_worker,f)!=0){atomic_flag_clear(&fi_slot);atomic_store(&f->refs,1);fi_release(f);return NULL;}
 pthread_detach(t);return f;
}
int fi_fetch_poll(FiNativeFetch *f,uint8_t *out,int64_t capacity,int64_t *written) {
 if(!f || !written)return 2;*written=0;pthread_mutex_lock(&f->mutex);
 if(f->state==0 && fi_expired(f)){atomic_store(&f->cancelled,1);f->state=2;}
 int state=f->state;if(state==1){if(!out || capacity<0 || (uint64_t)capacity<f->used)state=2;else{memcpy(out,f->body,f->used);*written=(int64_t)f->used;}}
 pthread_mutex_unlock(&f->mutex);return state;
}
void fi_fetch_free(FiNativeFetch *f) {if(f){atomic_store(&f->cancelled,1);fi_release(f);}}
