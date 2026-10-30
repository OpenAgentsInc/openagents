#define _DARWIN_C_SOURCE
#define _POSIX_C_SOURCE 200809L
#include "mesh-input.h"
#include <assert.h>
#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>
#if defined(__APPLE__)
#include <util.h>
#else
#include <pty.h>
#endif

static int saved_stdin;
static void input(int fd) { assert(dup2(fd,STDIN_FILENO)==STDIN_FILENO); }
static void restore(void) { assert(dup2(saved_stdin,STDIN_FILENO)==STDIN_FILENO); }
static int64_t now_ms(void) { struct timespec t; assert(clock_gettime(CLOCK_MONOTONIC,&t)==0); return (int64_t)t.tv_sec*1000+t.tv_nsec/1000000; }
static void send_bytes(int fd,size_t n) {
  unsigned char bytes[997];size_t at=0;
  while(at<n) {size_t chunk=n-at;if(chunk>sizeof bytes)chunk=sizeof bytes;for(size_t i=0;i<chunk;i++)bytes[i]=(unsigned char)(at+i);
    size_t done=0;while(done<chunk){ssize_t w=write(fd,bytes+done,chunk-done);if(w<0&&errno==EINTR)continue;assert(w>0);done+=(size_t)w;}at+=chunk;}
}
static void pipe_case(size_t n,int expected) {
  int p[2];assert(pipe(p)==0);pid_t child=fork();assert(child>=0);
  if(child==0){close(p[0]);send_bytes(p[1],n);close(p[1]);_exit(0);}
  close(p[1]);input(p[0]);assert(mw_stdin_kind()==1);int flags=fcntl(STDIN_FILENO,F_GETFL);assert(flags>=0);
  assert(mw_stdin_read(5000)==expected);assert(fcntl(STDIN_FILENO,F_GETFL)==flags);
  if(expected>=0){for(size_t i=0;i<n;i++)assert(mw_stdin_byte((int64_t)i)==(unsigned char)i);assert(mw_stdin_byte((int64_t)n)==-1);}else assert(mw_stdin_byte(0)==-1);
  mw_stdin_clear();assert(mw_stdin_byte(0)==-1);assert(mw_stdin_byte(-1)==-1);
  restore();close(p[0]);int status;assert(waitpid(child,&status,0)==child);assert(WIFEXITED(status)&&WEXITSTATUS(status)==0);
}
int main(void) {
  saved_stdin=dup(STDIN_FILENO);assert(saved_stdin>=0);
  pipe_case(0,0);pipe_case(4097,4097);pipe_case(MW_INPUT_MAX,MW_INPUT_MAX);pipe_case(MW_INPUT_MAX+1,-3);
  int p[2];assert(pipe(p)==0);input(p[0]);int64_t start=now_ms();assert(mw_stdin_read(40)==-2);assert(now_ms()-start<1000);assert(mw_stdin_byte(0)==-1);
  assert(write(p[1],"key",3)==3);assert(mw_stdin_read(40)==-2);assert(mw_stdin_byte(0)==-1);
  assert(mw_stdin_read(-1)==-3);assert(mw_stdin_read(60001)==-3);restore();close(p[0]);close(p[1]);
  int pair[2];assert(socketpair(AF_UNIX,SOCK_STREAM,0,pair)==0);input(pair[0]);assert(mw_stdin_kind()==2);send_bytes(pair[1],31);close(pair[1]);assert(mw_stdin_read(100)==31);for(int i=0;i<31;i++)assert(mw_stdin_byte(i)==i);restore();close(pair[0]);mw_stdin_clear();
  char path[]="/tmp/coax-mesh-input-XXXXXX";int file=mkstemp(path);assert(file>=0);assert(unlink(path)==0);assert(write(file,"private-witness",15)==15);assert(lseek(file,0,SEEK_SET)==0);input(file);assert(mw_stdin_kind()==3);assert(mw_stdin_read(0)==-3);assert(lseek(file,0,SEEK_CUR)==0);assert(mw_stdin_byte(0)==-1);restore();close(file);
  int master,slave;assert(openpty(&master,&slave,NULL,NULL,NULL)==0);input(slave);assert(mw_stdin_kind()==4);assert(mw_stdin_read(0)==-3);restore();close(master);close(slave);
  int tcp=socket(AF_INET,SOCK_STREAM,0);assert(tcp>=0);input(tcp);assert(mw_stdin_kind()==0);assert(mw_stdin_read(0)==-3);restore();close(tcp);
  int devnull=open("/dev/null",O_RDONLY);assert(devnull>=0);input(devnull);assert(mw_stdin_kind()==0);assert(mw_stdin_read(0)==-3);restore();close(devnull);
  close(STDIN_FILENO);assert(mw_stdin_kind()==0);assert(mw_stdin_read(0)==-3);restore();close(saved_stdin);
  puts("mesh-input: binary/chunks/max/overflow/timeout/unsafe-fd/clear checks pass");return 0;
}
