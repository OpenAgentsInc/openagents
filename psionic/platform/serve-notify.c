#define _POSIX_C_SOURCE 200809L
#include "serve-notify.h"
#include <stddef.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/un.h>
#include <unistd.h>
int psionic_notify_ready(void){
    const char *path=getenv("NOTIFY_SOCKET");if(!path || !*path)return 0;
    struct sockaddr_un a={0};size_t n=strlen(path);if(n>=sizeof a.sun_path || (path[0]!='/' && path[0]!='@'))return -1;
    a.sun_family=AF_UNIX;memcpy(a.sun_path,path,n+1);if(path[0]=='@')a.sun_path[0]=0;
    int fd=socket(AF_UNIX,SOCK_DGRAM,0);if(fd<0)return -1;
    static const char message[]="READY=1\nSTATUS=Authenticated; three retained golden decisions passed\n";
    ssize_t wrote=sendto(fd,message,sizeof message-1,0,(void*)&a,(socklen_t)(offsetof(struct sockaddr_un,sun_path)+n+(path[0]=='@'?0:1)));close(fd);return wrote==(ssize_t)sizeof message-1?0:-1;
}
