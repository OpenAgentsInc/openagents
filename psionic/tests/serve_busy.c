#define _GNU_SOURCE
#include "serve.h"
#include "serve-busy.h"
#include <arpa/inet.h>
#include <assert.h>
#include <fcntl.h>
#include <stdio.h>
#include <string.h>
#include <sys/socket.h>
#include <unistd.h>
int main(void){int listener=socket(AF_INET,SOCK_STREAM,0);assert(listener>=0);struct sockaddr_in a={.sin_family=AF_INET,.sin_addr={htonl(INADDR_LOOPBACK)}};assert(!bind(listener,(void*)&a,sizeof a)&&!listen(listener,16));socklen_t size=sizeof a;assert(!getsockname(listener,(void*)&a,&size));fcntl(listener,F_SETFL,O_NONBLOCK);
    int client=socket(AF_INET,SOCK_STREAM,0);assert(!connect(client,(void*)&a,sizeof a));const char ask[]="POST /v1/systemone HTTP/1.1\r\nContent-Length: 2\r\n\r\n{}";assert(send(client,ask,sizeof ask-1,0)==sizeof ask-1);
    PsBusy busy;psionic_busy_init(&busy);assert(!psionic_busy_sweep(&busy,listener,true,(coax_t_SvLimits){1000,1000},NULL,NULL));char response[512]={0};size_t used=0;while(used<sizeof response-1){ssize_t n=recv(client,response+used,sizeof response-1-used,0);if(n<=0)break;used+=(size_t)n;}assert(used>0 && !strncmp(response,"HTTP/1.1 503",12) && strstr(response,"\"busy\""));assert(psionic_busy_wait(&busy)<=200);close(client);
    assert(!psionic_busy_sweep(&busy,-1,false,(coax_t_SvLimits){1000,1000},NULL,NULL));psionic_busy_close(&busy);assert(psionic_busy_wait(&busy)==1000);close(listener);puts("busy sockets: immediate refusal, bounded cleanup and no queued model work pass");}
