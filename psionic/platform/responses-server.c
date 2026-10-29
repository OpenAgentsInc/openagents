/* Socket, clock, threading, file and device effects only. All wire admission,
   authorization, limits, Harmony mapping and serialization are Coax procedures. */
#define _POSIX_C_SOURCE 200809L
#include "device-host.h"
#ifndef PSIONIC_RESPONSES_FIXTURE
#include "gptoss-device.h"
#endif
#include <arpa/inet.h>
#include <errno.h>
#include <fcntl.h>
#include <netinet/in.h>
#include <poll.h>
#include <pthread.h>
#include <signal.h>
#include <stdatomic.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/socket.h>
#include <sys/stat.h>
#include <time.h>
#include <unistd.h>
static coax_v_u8 bv(void *p,int64_t n){return(coax_v_u8){p,n};}
static coax_v_s64 iv(int64_t *p,int64_t n){return(coax_v_s64){p,n};}
static coax_string text(const char *s){return(coax_string){s,(int64_t)strlen(s)};}
static int64_t now_ms(void){struct timespec t;clock_gettime(CLOCK_MONOTONIC,&t);return (int64_t)t.tv_sec*1000+t.tv_nsec/1000000;}
static coax_v_u8 table;
static const char *api_key;
static pthread_mutex_t worker=PTHREAD_MUTEX_INITIALIZER, connections=PTHREAD_MUTEX_INITIALIZER;
static pthread_cond_t drained=PTHREAD_COND_INITIALIZER;
static int active=0;
static atomic_int healthy=1;
static volatile sig_atomic_t stopping=0;
static int listener=-1;
#ifndef PSIONIC_RESPONSES_FIXTURE
static PgSession *session;
#endif
static void stop(int sig){(void)sig;stopping=1;if(listener>=0)close(listener);listener=-1;}
static void fail(const char *s){fprintf(stderr,"%s\n",s);exit(1);}
static void *map_file(const char *name,size_t *n){int fd=open(name,O_RDONLY);struct stat st;if(fd<0 || fstat(fd,&st) || st.st_size<1)fail("cannot map input artifact");*n=(size_t)st.st_size;void *p=mmap(NULL,*n,PROT_READ,MAP_PRIVATE,fd,0);close(fd);if(p==MAP_FAILED)fail("artifact mmap failed");return p;}
typedef struct {
    int fd,decision;int64_t start;coax_t_RsHttp http;coax_t_RsRequest request;
    char frame[73728],arena[65536],content[65536],message_work[65536],system_work[1024],decoded[65536];
    coax_t_RsNode nodes[1024];coax_t_RsItem input[48],output[64];coax_t_RsTool tools[8];coax_t_HmMessage messages[64];
    int64_t prompt[16384],stage[16384],symbols[1024],generated[512],control[8];
    unsigned char wire[4194304];coax_t_RsStream stream;char identity[32];int64_t created,prefix,count;coax_t_RsOutput snapshot;
#ifdef PSIONIC_RESPONSES_FIXTURE
    int64_t fixture[1024],fixture_count;
#endif
} Request;
static int connected(Request *r){char b;ssize_t n=recv(r->fd,&b,1,MSG_PEEK);return !stopping && (n>0 || (n<0 && (errno==EAGAIN || errno==EWOULDBLOCK || errno==EINTR)));}
static int alive(void *context){Request *r=context;r->decision=(int)rs_liveness(r->start,now_ms(),connected(r),atomic_load(&healthy));return r->decision;}
static int send_bytes(Request *r,const void *bytes,size_t count){const char *p=bytes;while(count){if(rs_liveness(r->start,now_ms(),connected(r),1))return 0;ssize_t n=send(r->fd,p,count,0);if(n>0){p+=n;count-=(size_t)n;continue;}if(n<0 && (errno==EAGAIN || errno==EWOULDBLOCK || errno==EINTR)){struct pollfd f={r->fd,POLLOUT,0};poll(&f,1,20);continue;}r->decision=499;return 0;}return 1;}
static int send_headers(Request *r,int status,int64_t n,int stream){unsigned char h[512];int64_t k=rs_headers(status,n,stream,bv(h,sizeof h));return k>=0 && send_bytes(r,h,(size_t)k);}
static void error_reply(Request *r,int status,coax_string code,coax_string param){int64_t n=rs_error(status,code,param,bv(r->wire,sizeof r->wire));if(n>=0 && send_headers(r,status,n,0))send_bytes(r,r->wire,(size_t)n);}
static int identity(Request *r){unsigned char random[16];int fd=open("/dev/urandom",O_RDONLY);if(fd<0)return 0;size_t at=0;while(at<sizeof random){ssize_t n=read(fd,random+at,sizeof random-at);if(n<0 && errno==EINTR)continue;if(n<=0){close(fd);return 0;}at+=(size_t)n;}close(fd);return rs_random_id(bv(random,16),bv(r->identity,32))==32;}
static int emit(Request *r,int status,coax_string code){coax_t_RsUsage usage={r->prefix,r->count,r->snapshot.reasoning};coax_v_t_RsItem items={r->output,r->snapshot.count};coax_string id={r->identity,32};int64_t n;
    if(r->request.stream)n=rs_events((coax_v_t_RsStream){&r->stream,1},r->request,id,r->created,time(NULL),items,usage,status,code,r->http.profile,bv(r->wire,sizeof r->wire));
    else n=rs_response(r->request,id,r->created,time(NULL),items,usage,status,code,bv(r->wire,sizeof r->wire));
    if(n<0)return 0;if(!r->request.stream && !send_headers(r,200,n,0))return 0;return send_bytes(r,r->wire,(size_t)n);
}
static void generate(Request *r){
    int locked=0,stream_started=0;coax_string code=text("");int response_status=1;
    r->request=rs_request((coax_string){r->frame+r->http.body,r->http.length},(coax_v_t_RsNode){r->nodes,1024},bv(r->arena,sizeof r->arena),bv(r->content,sizeof r->content),(coax_v_t_RsItem){r->input,48},(coax_v_t_RsTool){r->tools,8});
    if(r->request.status!=200){error_reply(r,(int)r->request.status,r->request.code,r->request.param);return;}
    int64_t nm=rs_messages(r->request,(coax_v_t_RsItem){r->input,48},(coax_v_t_RsTool){r->tools,8},(coax_v_t_RsNode){r->nodes,r->request.nodes},(coax_v_t_HmMessage){r->messages,64},bv(r->message_work,sizeof r->message_work),bv(r->system_work,sizeof r->system_work));
    if(nm<0){error_reply(r,400,text("input_too_large"),text("input"));return;}
    coax_t_HmResult prompt=hm_render_checked(table,(coax_v_t_HmMessage){r->messages,nm},1,iv(r->prompt,16384),iv(r->stage,16384),iv(r->symbols,1024),0);
    if(prompt.status || !rs_context(prompt.written,r->request.limit)){error_reply(r,400,text("context_length_exceeded"),text("input"));return;}r->prefix=prompt.written;
    if(!atomic_load(&healthy)){error_reply(r,503,text("worker_unavailable"),text(""));return;}
    if(pthread_mutex_trylock(&worker)){error_reply(r,503,text("worker_busy"),text(""));return;}locked=1;
    if(!identity(r)){error_reply(r,500,text("random_source_failed"),text(""));goto done;}r->created=time(NULL);r->start=now_ms();
#ifdef PSIONIC_RESPONSES_FIXTURE
    r->fixture_count=rs_fixture(table,r->request,(coax_v_t_RsItem){r->input,r->request.items},(coax_v_t_RsTool){r->tools,r->request.tools},iv(r->fixture,1024));if(r->fixture_count<1){code=text("fixture_failed");goto failed;}
#else
    PcDeviceInfo info;if(pc_open(&info)!=PC_OK || pg_reset(session)!=PC_OK){atomic_store(&healthy,0);code=text("device_failed");goto failed;}
#endif
    if(r->request.stream){if(!send_headers(r,200,0,1))goto done;stream_started=1;if(!emit(r,0,text("")))goto done;}
#ifndef PSIONIC_RESPONSES_FIXTURE
    int interrupted=0;PcStatus effect=pg_prefill_poll(session,r->prompt,r->prefix,0,1,r->request.limit,r->control,alive,r,&interrupted);
    if(interrupted)goto cancelled;if(effect!=PC_OK){atomic_store(&healthy,0);code=text("device_failed");goto failed;}if(r->control[0]){code=text("model_arithmetic_refused");goto failed;}
#endif
    while(1){if(alive(r))goto cancelled;
#ifdef PSIONIC_RESPONSES_FIXTURE
        struct timespec delay={0,5000000};nanosleep(&delay,NULL);
        r->control[1]=r->fixture[r->count];r->control[4]=rs_fixture_stop(r->control[1],r->count+1,r->request.limit);
#else
        if(r->count>0){effect=pg_step(session,-1,r->control[2],1,r->request.limit,1,r->control);if(effect!=PC_OK){atomic_store(&healthy,0);code=text("device_failed");goto failed;}if(r->control[0]){code=text("model_arithmetic_refused");goto failed;}}
#endif
        r->generated[r->count++]=r->control[1];
        coax_t_RsOutput next_snapshot=rs_snapshot(table,iv(r->generated,r->count),r->control[4]!=0,(coax_v_t_RsTool){r->tools,r->request.choice?0:r->request.tools},(coax_v_t_RsNode){r->nodes,r->request.nodes},(coax_v_t_RsItem){r->output,64},bv(r->decoded,sizeof r->decoded));
        if(next_snapshot.status!=200){code=next_snapshot.code;goto failed;}r->snapshot=next_snapshot;
        if(r->control[4]){response_status=r->control[4]==3?2:1;break;}
        if(r->request.stream && !emit(r,0,text("")))goto done;
    }
    emit(r,response_status,text(""));goto done;
cancelled:
    if(r->decision==499)goto done;code=text("request_deadline_exceeded");
failed:
    if(stream_started){/* A failed stream retains already published items but grants no call. */
        for(int64_t i=0;i<r->snapshot.count;i++)if(!r->output[i].status)r->output[i].status=2;
        /* A deadline is terminal data; allow its small write a fresh transport window. */
        r->start=now_ms();emit(r,3,code);
    }else error_reply(r,502,code,text(""));
done:
    if(locked)pthread_mutex_unlock(&worker);
}
static void *connection(void *context){Request *r=context;int64_t size=0;r->start=now_ms();
    while(1){if(!rs_ingress(r->start,now_ms())){error_reply(r,408,text("request_timeout"),text(""));break;}
        r->http=rs_http((coax_string){r->frame,size},text(api_key));if(r->http.status){if(r->http.status!=200)error_reply(r,(int)r->http.status,text("http_request_refused"),text(""));
            else if(r->http.route==1)generate(r);else {int ok=atomic_load(&healthy);int64_t n=rs_static(r->http.route,ok,bv(r->wire,sizeof r->wire));if(send_headers(r,r->http.route==4 && !ok?503:200,n,0))send_bytes(r,r->wire,(size_t)n);}break;}
        struct pollfd f={r->fd,POLLIN,0};int p=poll(&f,1,20);if(p<0 && errno==EINTR)continue;if(p<0)break;if(!p)continue;
        ssize_t n=recv(r->fd,r->frame+size,sizeof r->frame-(size_t)size,0);if(n<0 && (errno==EAGAIN || errno==EWOULDBLOCK || errno==EINTR))continue;if(n<=0)break;size+=n;
    }
    /* FIN before draining prevents unread request bytes from turning an error
       reply into a TCP reset. Drain under the Coax ingress deadline. */
    shutdown(r->fd,SHUT_WR);int64_t drain=now_ms();char discard[4096];
    while(!stopping && rs_ingress(drain,now_ms())){struct pollfd f={r->fd,POLLIN,0};int p=poll(&f,1,20);if(p<=0)continue;ssize_t n=recv(r->fd,discard,sizeof discard,0);if(n==0)break;if(n<0 && errno!=EINTR && errno!=EAGAIN && errno!=EWOULDBLOCK)break;}
    close(r->fd);free(r);pthread_mutex_lock(&connections);active--;pthread_cond_signal(&drained);pthread_mutex_unlock(&connections);return NULL;
}
int main(int argc,char **argv){
#ifdef PSIONIC_RESPONSES_FIXTURE
    if(argc!=3)fail("usage: responses-fixture HARMONY_TABLE PORT");const char *table_path=argv[1],*port_arg=argv[2];
#else
    if(argc!=5)fail("usage: responses-server ORIGINAL_CACHE HARMONY_TABLE GENERATED_CUDA PORT");const char *table_path=argv[2],*port_arg=argv[4];
#endif
    api_key=getenv("PSIONIC_API_KEY");if(!api_key)fail("set PSIONIC_API_KEY");
    char *end=NULL;long port=strtol(port_arg,&end,10);if(!end || *end || !rs_settings(text(api_key),port))fail("invalid server settings: key needs 16..128 visible ASCII bytes; port needs 1024..65535");
    signal(SIGPIPE,SIG_IGN);signal(SIGTERM,stop);signal(SIGINT,stop);size_t table_bytes;void *table_map=map_file(table_path,&table_bytes);table=bv(table_map,(int64_t)table_bytes);if(!hs_admit(table))fail("Harmony table identity refused");
#ifndef PSIONIC_RESPONSES_FIXTURE
    coax_t_GArtifact pins[10];coax_t_GSpec specs[543];coax_t_GProfile profile=g_profile(0,2048,(coax_v_t_GArtifact){pins,10},(coax_v_t_GSpec){specs,543});if(profile.status)fail("model profile refused");coax_t_GFile files[3];void *maps[3];size_t sizes[3];
    for(int i=0;i<3;i++){char name[4096];int n=snprintf(name,sizeof name,"%s/%.*s",argv[1],(int)pins[i].name.count,pins[i].name.data);if(n<1 || (size_t)n>=sizeof name)fail("cache path too long");maps[i]=map_file(name,&sizes[i]);files[i]=(coax_t_GFile){pins[i].name,bv(maps[i],(int64_t)sizes[i])};}
    size_t cuda_bytes;const char *cuda=map_file(argv[3],&cuda_bytes);coax_t_GResult why;PcStatus effect=pg_open(profile,(coax_v_t_GFile){files,3},cuda,cuda_bytes,17179869184,&session,&why);if(effect || why.status || !session)fail("resident model admission failed");
#endif
    listener=socket(AF_INET,SOCK_STREAM,0);if(listener<0)fail("socket failed");int yes=1;setsockopt(listener,SOL_SOCKET,SO_REUSEADDR,&yes,sizeof yes);struct sockaddr_in address={.sin_family=AF_INET,.sin_port=htons((uint16_t)port),.sin_addr={0}};
    if(inet_pton(AF_INET,"127.0.0.1",&address.sin_addr)!=1)fail("loopback address failed");
    if(bind(listener,(struct sockaddr*)&address,sizeof address) || listen(listener,8))fail("loopback listener failed");fprintf(stderr,"Psionic Responses ready on 127.0.0.1:%ld\n",port);
    while(!stopping){int fd=accept(listener,NULL,NULL);if(fd<0){if(stopping)break;if(errno==EINTR)continue;break;}fcntl(fd,F_SETFL,fcntl(fd,F_GETFL,0)|O_NONBLOCK);
        pthread_mutex_lock(&connections);int admit=rs_connections(active);if(admit)active++;pthread_mutex_unlock(&connections);if(!admit){close(fd);continue;}
        Request *r=calloc(1,sizeof *r);if(!r)fail("request allocation failed");r->fd=fd;pthread_t t;if(pthread_create(&t,NULL,connection,r))fail("request thread failed");pthread_detach(t);
    }
    pthread_mutex_lock(&connections);while(active)pthread_cond_wait(&drained,&connections);pthread_mutex_unlock(&connections);
#ifndef PSIONIC_RESPONSES_FIXTURE
    pg_close(session);for(int i=0;i<3;i++)munmap(maps[i],sizes[i]);munmap((void*)cuda,cuda_bytes);
#endif
    munmap(table_map,table_bytes);return 0;
}
