/* Bounded nonblocking socket effects between device layers. */
#define _GNU_SOURCE
#include "serve.h"
#include "serve-busy.h"
#include <errno.h>
#include <fcntl.h>
#include <poll.h>
#include <string.h>
#include <sys/socket.h>
#include <time.h>
#include <unistd.h>
static int64_t now(void){struct timespec t;clock_gettime(CLOCK_MONOTONIC,&t);return (int64_t)t.tv_sec*1000+t.tv_nsec/1000000;}
void psionic_busy_init(PsBusy *b){for(int i=0;i<16;i++){b->fd[i]=-1;b->since[i]=0;}}
void psionic_busy_close(PsBusy *b){for(int i=0;i<16;i++){if(b->fd[i]>=0)close(b->fd[i]);b->fd[i]=-1;}}
int psionic_busy_wait(const PsBusy *b){int wait=1000;for(int i=0;i<16;i++)if(b->fd[i]>=0){int64_t left=ls_busy_left(b->since[i],now());if(left<wait)wait=(int)left;}return wait;}
static void send_busy(int fd,const uint8_t *data,int64_t n,int64_t until){
    while(n>0){ssize_t sent=send(fd,data,(size_t)n,MSG_DONTWAIT|MSG_NOSIGNAL);if(sent>0){data+=sent;n-=sent;continue;}
        if(sent<0 && errno==EINTR)continue;
        if(sent<0 && (errno==EAGAIN || errno==EWOULDBLOCK)){int64_t left=until-now();if(left<=0)return;struct pollfd p={fd,POLLOUT,0};poll(&p,1,(int)left);continue;}return;}
}
int psionic_busy_sweep(PsBusy *b,int listener,bool active,coax_t_SvLimits limits,void (*observed)(void *),void *ctx){
    coax_t_LsBusyPolicy p=ls_busy_policy();if(p.capacity!=16)return -1;uint8_t sink[65536];
    for(int i=0;i<16;i++)if(b->fd[i]>=0){bool closed=false;int64_t drained=0;
        while(drained<p.drain_bytes){ssize_t n=recv(b->fd[i],sink,sizeof sink,MSG_DONTWAIT);if(n>0){drained+=n;continue;}if(n==0 || (errno!=EAGAIN && errno!=EWOULDBLOCK && errno!=EINTR))closed=true;break;}
        if(closed || !ls_busy_left(b->since[i],now())){close(b->fd[i]);b->fd[i]=-1;}}
    if(!active)return 0;
    for(int64_t i=0;i<p.accepts;i++){int fd=accept(listener,NULL,NULL);if(fd<0){if(errno==EINTR)continue;break;}fcntl(fd,F_SETFL,O_NONBLOCK);
        coax_t_SvStep admission=sv_accept((coax_t_SvSlot){.phase=COAX_SvPhase_deciding},limits,now());uint8_t bytes[512];int64_t n=sv_refusal((coax_v_u8){bytes,sizeof bytes},admission.code);
        send_busy(fd,bytes,n,now()+p.write_ms);shutdown(fd,SHUT_WR);int slot=-1;for(int j=0;j<16;j++)if(b->fd[j]<0){slot=j;break;}
        if(slot<0)close(fd);else{b->fd[slot]=fd;b->since[slot]=now();}if(observed)observed(ctx);
    }
    return 0;
}
