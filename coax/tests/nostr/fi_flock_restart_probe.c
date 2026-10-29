#define _DARWIN_C_SOURCE
/* Test-only macOS lock inheritance experiment. Not a deployed service. */
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/file.h>
#include <sys/stat.h>
#include <time.h>
#include <unistd.h>
static void pause_ms(long ms){struct timespec t={ms/1000,(ms%1000)*1000000};nanosleep(&t,NULL);}
int main(int argc,char **argv){
 if(argc!=4)return 2;const char *mode=argv[1],*path=argv[2],*marker=argv[3];
 if(!strncmp(mode,"supervisor-",11)){
  int fd=atoi(path);if(fd<3 || fcntl(fd,F_GETFD)<0)return 11;
  int early=!strcmp(mode,"supervisor-early"),unlock=!strcmp(mode,"supervisor-unlock");
  if(early)pause_ms(250);
  pid_t resolver=fork();if(resolver<0)return 7;
  if(resolver==0){
   if(unlock && flock(fd,LOCK_UN))_exit(8);
   int m=open(marker,O_WRONLY|O_CREAT|O_EXCL,0600);if(m<0)_exit(9);
   char text[32];int n=snprintf(text,sizeof(text),"%ld\n",(long)getpid());if(write(m,text,(size_t)n)!=n)_exit(10);close(m);
   pause_ms(2000);_exit(0);
  }
  while(access(marker,F_OK))pause_ms(1);_exit(0);
 }
 int fd=open(path,O_RDWR|O_CREAT|O_NOFOLLOW,0600);struct stat st;if(fd<0 || fstat(fd,&st) || !S_ISREG(st.st_mode))return 3;
 if(flock(fd,LOCK_EX|LOCK_NB)){return errno==EWOULDBLOCK?23:4;}
 if(!strcmp(mode,"probe") || !strcmp(mode,"before"))_exit(0);
 int early=!strcmp(mode,"early");if(!early && strcmp(mode,"unlock") && strcmp(mode,"after"))return 5;
 char fdtext[32],submode[32];snprintf(fdtext,sizeof(fdtext),"%d",fd);snprintf(submode,sizeof(submode),"supervisor-%s",mode);
 pid_t supervisor=fork();if(supervisor<0)return 6;
 if(supervisor==0){execl(argv[0],argv[0],submode,fdtext,marker,(char *)NULL);_exit(12);}
 if(!early)while(access(marker,F_OK))pause_ms(1);
 _exit(0);
}
