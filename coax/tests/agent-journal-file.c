/* Independent POSIX observations of the private, owned journal file effect. */
#define _POSIX_C_SOURCE 200809L
#include "store-file-native.h"
#include <assert.h>
#include <errno.h>
#include <fcntl.h>
#include <signal.h>
#include <stdio.h>
#include <string.h>
#include <sys/resource.h>
#include <sys/stat.h>
#include <unistd.h>

static void path(const char *s) {
  for (; *s; ++s) assert(cxjournal_path_push((unsigned char)*s)==0);
}
static void put(const char *s) {
  for (; *s; ++s) assert(cxjournal_put((unsigned char)*s)==0);
}
int main(int argc,char **argv) {
  assert(argc==2);
  char file[4096],link[4096];
  assert(snprintf(file,sizeof file,"%s/owned.ajr",argv[1])<(int)sizeof file);
  assert(snprintf(link,sizeof link,"%s/link.ajr",argv[1])<(int)sizeof link);
  assert(cxjournal_put('x')==-1 && cxjournal_commit()==-1);
  assert(cxjournal_path_push(0)==-1 && cxjournal_path_push(256)==-1);
  assert(cxjournal_open()==-1);
  path(file);assert(cxjournal_open()==0);
  struct stat st;assert(stat(file,&st)==0 && (st.st_mode&0777)==0600 && st.st_size==0);
  put("head");assert(cxjournal_commit()==0);
  assert(stat(file,&st)==0 && st.st_size==4);
  assert(cxjournal_path_push('x')==-1 && cxjournal_open()==-1);
  /* A physical partial write must roll back and poison the owner. */
  struct rlimit prior,small;assert(getrlimit(RLIMIT_FSIZE,&prior)==0);
  small=prior;small.rlim_cur=6;
  void (*old)(int)=signal(SIGXFSZ,SIG_IGN);assert(old!=SIG_ERR);
  assert(setrlimit(RLIMIT_FSIZE,&small)==0);
  put("a partial frame");assert(cxjournal_commit()==-1);
  assert(setrlimit(RLIMIT_FSIZE,&prior)==0);assert(signal(SIGXFSZ,old)!=SIG_ERR);
  assert(stat(file,&st)==0 && st.st_size==4);
  assert(cxjournal_put('x')==-1 && cxjournal_commit()==-1);
  assert(cxjournal_close()==0);
  int fd=open(file,O_RDONLY);assert(fd>=0);char bytes[4];
  assert(read(fd,bytes,4)==4 && memcmp(bytes,"head",4)==0);assert(close(fd)==0);
  path(file);assert(cxjournal_open()==-1); /* Never truncates an existing log. */
  assert(symlink(file,link)==0);path(link);assert(cxjournal_open()==-1);
  assert(unlink(link)==0 && unlink(file)==0);
  path(file);assert(cxjournal_open()==0);
  for(int i=0;i<32857;i++)assert(cxjournal_put(i&255)==0);
  assert(cxjournal_put(0)==-1 && cxjournal_commit()==-1);
  assert(cxjournal_close()==0);assert(stat(file,&st)==0 && st.st_size==0);
  assert(unlink(file)==0);
  for(int i=0;i<4096;i++)assert(cxjournal_path_push('a')==0);
  assert(cxjournal_path_push('a')==-1 && cxjournal_open()==-1);
  path(file);assert(cxjournal_open()==0);put("fresh");assert(cxjournal_commit()==0);
  assert(cxjournal_close()==0 && stat(file,&st)==0 && st.st_size==5);
  assert(unlink(file)==0);
  path(file);assert(cxjournal_open()==0);
  for(int i=0;i<32857;i++)assert(cxjournal_put(i&255)==0);
  assert(cxjournal_commit()==0 && cxjournal_close()==0);
  assert(stat(file,&st)==0 && st.st_size==32857 && unlink(file)==0);
  puts("private journal ownership, durability, rollback and refusal passed");
  return 0;
}
