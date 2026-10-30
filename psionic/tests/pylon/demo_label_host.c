/* Test-only private demo-label file observations; no real key handling. */
#define main pylon_host_main
#include "../../platform/pylon-host.c"
#undef main
#include <assert.h>
#include <stdlib.h>
#include <sys/types.h>
int coax_pylon_main(void) { return 0; }
static void path(const char *s) {
  assert(pyh_file_path_clear()==0);
  while (*s) assert(pyh_file_path_push((unsigned char)*s++)==0);
}
int main(void) {
  char dir[]="/tmp/pylon-label-test-XXXXXX";assert(mkdtemp(dir));
  char good[1100],link[1100],fifo[1100];
  snprintf(good,sizeof good,"%s/label",dir);snprintf(link,sizeof link,"%s/link",dir);snprintf(fifo,sizeof fifo,"%s/fifo",dir);
  int fd=open(good,O_CREAT|O_EXCL|O_WRONLY,0600);assert(fd>=0);
  unsigned char bytes[257];memset(bytes,'a',sizeof bytes);assert(write(fd,bytes,64)==64);close(fd);
  path(good);assert(pyh_label_read()==64);assert(pyh_label_byte(0)=='a'&&pyh_label_byte(63)=='a'&&pyh_label_byte(64)==-1);
  assert(pyh_label_clear()==0&&pyh_label_byte(0)==-1);for(size_t i=0;i<sizeof g_demo_label;i++)assert(g_demo_label[i]==0);
  assert(chmod(good,0644)==0);path(good);assert(pyh_label_read()==-1);assert(chmod(good,0600)==0);
  assert(symlink(good,link)==0);path(link);assert(pyh_label_read()==-1);
  assert(mkfifo(fifo,0600)==0);path(fifo);assert(pyh_label_read()==-1);
  fd=open(good,O_WRONLY|O_TRUNC);assert(fd>=0);assert(write(fd,bytes,sizeof bytes)==sizeof bytes);close(fd);path(good);assert(pyh_label_read()==-1);
  unlink(good);path(good);assert(pyh_label_read()==-1);unlink(link);unlink(fifo);rmdir(dir);
  return 0;
}
