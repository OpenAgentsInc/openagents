/* Process effects only. Restart admission/backoff is ls_restart in Coax. */
#define _POSIX_C_SOURCE 200809L
#include "serve.h"
#include <errno.h>
#include <stdbool.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>
static volatile sig_atomic_t stopping;
static void stop(int sig){(void)sig;stopping=1;}
int main(int argc,char **argv){
    int at=1;long limit=5,base=250;
    while(at<argc && strcmp(argv[at],"--")){
        if(at+1>=argc)return 2;char *end=NULL;long n=strtol(argv[at+1],&end,10);if(!*argv[at+1] || *end)return 2;
        if(!strcmp(argv[at],"--restart-limit"))limit=n;else if(!strcmp(argv[at],"--backoff-ms"))base=n;else return 2;at+=2;
    }
    if(at>=argc-1 || !ls_restart(0,limit,base,1,false).ok){fprintf(stderr,"usage: psionic-supervise [--restart-limit N] [--backoff-ms N] -- /absolute/program args...\n");return 2;}
    struct sigaction action={0};action.sa_handler=stop;sigemptyset(&action.sa_mask);sigaction(SIGTERM,&action,NULL);sigaction(SIGINT,&action,NULL);int64_t failures=0;
    while(!stopping){
        pid_t child=fork();if(child<0)return 2;
        if(!child){signal(SIGTERM,SIG_DFL);signal(SIGINT,SIG_DFL);execv(argv[at+1],&argv[at+1]);_exit(127);}
        fprintf(stderr,"psionic-supervise: child=%ld attempt=%lld\n",(long)child,(long long)failures);fflush(stderr);
        int status=0;if(stopping)kill(child,SIGTERM);for(;;){pid_t r=waitpid(child,&status,0);if(r==child)break;if(r<0 && errno==EINTR){if(stopping)kill(child,SIGTERM);continue;}return 2;}
        int code=WIFEXITED(status)?WEXITSTATUS(status):128+WTERMSIG(status);
        coax_t_LsRestart r=ls_restart(failures,limit,base,code,stopping!=0);
        if(!r.ok || !r.retry)return stopping || code==0?0:1;
        failures=r.failures;fprintf(stderr,"psionic-supervise: restart=%lld backoff_ms=%lld exit=%d\n",(long long)failures,(long long)r.delay_ms,code);
        struct timespec delay={r.delay_ms/1000,(r.delay_ms%1000)*1000000};while(!stopping && nanosleep(&delay,&delay)<0 && errno==EINTR){}
    }
    return 0;
}
