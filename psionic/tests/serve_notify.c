#define _POSIX_C_SOURCE 200809L
#include "serve-notify.h"
#include <assert.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/un.h>
#include <unistd.h>
int main(void){
    unsetenv("NOTIFY_SOCKET");assert(!psionic_notify_ready());
    char path[100];snprintf(path,sizeof path,"/tmp/psionic-notify-%ld",(long)getpid());int fd=socket(AF_UNIX,SOCK_DGRAM,0);assert(fd>=0);
    struct sockaddr_un a={0};a.sun_family=AF_UNIX;memcpy(a.sun_path,path,strlen(path)+1);assert(!bind(fd,(void*)&a,sizeof a));assert(!setenv("NOTIFY_SOCKET",path,1));assert(!psionic_notify_ready());char bytes[128]={0};assert(recv(fd,bytes,sizeof bytes,0)>0 && !strncmp(bytes,"READY=1\n",8));close(fd);unlink(path);unsetenv("NOTIFY_SOCKET");puts("notify: optional Unix datagram readiness effect passes");
}
