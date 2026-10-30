#define _DARWIN_C_SOURCE
#define _POSIX_C_SOURCE 200809L
#include "mesh-public-output.h"
#include <assert.h>
#include <errno.h>
#include <fcntl.h>
#include <signal.h>
#include <stdio.h>
#include <string.h>
#include <unistd.h>
#if defined(__APPLE__)
#include <util.h>
#else
#include <pty.h>
#endif
int main(void) {
  signal(SIGPIPE,SIG_IGN);
  int original_out=dup(STDOUT_FILENO),original_err=dup(STDERR_FILENO);assert(original_out>=0&&original_err>=0);
  int master,slave;assert(openpty(&master,&slave,NULL,NULL,NULL)==0);assert(dup2(slave,STDOUT_FILENO)==STDOUT_FILENO);assert(mpub_stdout_tty()==1);
  int pipefd[2];assert(pipe(pipefd)==0);assert(dup2(pipefd[1],STDOUT_FILENO)==STDOUT_FILENO);assert(mpub_stdout_tty()==0);
  close(STDOUT_FILENO);assert(mpub_stdout_tty()==0);assert(dup2(original_out,STDOUT_FILENO)==STDOUT_FILENO);close(master);close(slave);
  assert(dup2(pipefd[1],STDERR_FILENO)==STDERR_FILENO);assert(mpub_stderr_byte(-1)==-3&&mpub_stderr_byte(256)==-3);
  for(int i=0;i<256;i++)assert(mpub_stderr_byte(i)==0);
  unsigned char got[256];size_t used=0;while(used<sizeof got){ssize_t n=read(pipefd[0],got+used,sizeof got-used);assert(n>0);used+=(size_t)n;}
  for(int i=0;i<256;i++)assert(got[i]==i);
  int flags=fcntl(STDERR_FILENO,F_GETFL);assert(flags>=0&&fcntl(STDERR_FILENO,F_SETFL,flags|O_NONBLOCK)==0);
  unsigned char fill[4096];memset(fill,0,sizeof fill);while(write(STDERR_FILENO,fill,sizeof fill)>0){}assert(errno==EAGAIN||errno==EWOULDBLOCK);
  /* A full nonblocking pipe may retain a short final slot; fill it bytewise. */
  while(mpub_stderr_byte(0)==0){}assert(mpub_stderr_byte(1)==-3);assert(fcntl(STDERR_FILENO,F_GETFL)==(flags|O_NONBLOCK));
  close(pipefd[0]);assert(mpub_stderr_byte(2)==-3);close(STDERR_FILENO);assert(mpub_stderr_byte(3)==-3);
  assert(dup2(original_err,STDERR_FILENO)==STDERR_FILENO);close(pipefd[1]);close(original_out);close(original_err);
  puts("mesh-public-output: tty/pipe/closed/binary/bounds/full-pipe checks pass");return 0;
}
