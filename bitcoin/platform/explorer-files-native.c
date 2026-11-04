/* Native memory and file effects only. Parsing, policy, replay and the
   decision to publish are all in explorer-node.coax. */
#include "tool-native.h"
#include <sys/file.h>
#include <sys/stat.h>
#include <fcntl.h>
#include <unistd.h>
#include <errno.h>
#include <stdlib.h>
#include <string.h>
#include <limits.h>
#include <stdbool.h>
#include <sys/statvfs.h>
int64_t bf_native_free_bytes(coax_v_u8 path) {
  char p[4097];
  if(path.count<1||path.count>4096||memchr(path.data,0,(size_t)path.count))return -1;
  memcpy(p,path.data,(size_t)path.count);p[path.count]=0;struct statvfs s;
  if(statvfs(p,&s)||s.f_frsize==0||s.f_bavail>(uint64_t)INT64_MAX/s.f_frsize)return -1;
  return (int64_t)(s.f_bavail*s.f_frsize);
}
static int path_copy(coax_v_u8 p, char out[4097]) {
  if(p.count<1 || p.count>4096 || memchr(p.data,0,(size_t)p.count))return 0;
  memcpy(out,p.data,(size_t)p.count);out[p.count]=0;return 1;
}
int64_t bf_native_read_at(coax_v_u8 path,int64_t offset,coax_v_u8 out) {
  char p[4097];if(!path_copy(path,p)||offset<0||out.count<0)return -1;
  int fd=open(p,O_RDONLY|O_CLOEXEC);if(fd<0)return -1;int64_t at=0;
  while(at<out.count){ssize_t n=pread(fd,out.data+at,(size_t)(out.count-at),(off_t)(offset+at));if(n<0&&errno==EINTR)continue;if(n<0){at=-1;break;}if(!n)break;at+=n;}
  close(fd);return at;
}
int64_t bf_native_lock(coax_v_u8 path) {
  char p[4097];if(!path_copy(path,p))return -1;
  int fd=open(p,O_RDWR|O_CREAT|O_CLOEXEC,0600);if(fd<0)return -1;
  if(flock(fd,LOCK_EX|LOCK_NB)){close(fd);return -1;}return fd;
}
int64_t bf_unlock(int64_t handle){return close((int)handle);}
bool bf_native_append_sync(coax_v_u8 path,coax_v_u8 data) {
  char p[4097];if(!path_copy(path,p)||data.count<0)return false;
  int fd=open(p,O_WRONLY|O_CREAT|O_APPEND|O_CLOEXEC,0600);if(fd<0)return false;int64_t at=0;bool ok=true;
  while(at<data.count){ssize_t n=write(fd,data.data+at,(size_t)(data.count-at));if(n<0&&errno==EINTR)continue;if(n<=0){ok=false;break;}at+=n;}
  if(fsync(fd))ok=false;if(close(fd))ok=false;
  char *slash=strrchr(p,'/');if(slash){if(slash==p)slash[1]=0;else *slash=0;}else strcpy(p,".");
  fd=open(p,O_RDONLY|O_DIRECTORY|O_CLOEXEC);if(fd<0)return false;if(fsync(fd))ok=false;if(close(fd))ok=false;return ok;
}
