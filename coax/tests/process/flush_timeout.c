#define _GNU_SOURCE
#include "../../platform/process-native.c"
#include <assert.h>
#include <limits.h>
#include <sys/stat.h>
#include <sys/time.h>
static volatile sig_atomic_t alarms;
static void tick(int sig) { (void)sig; alarms++; }
static void pause_ms(long ms) { struct timespec t={ms/1000,(ms%1000)*1000000}; while(nanosleep(&t,&t)<0 && errno==EINTR){} }
static int child(const char *mode) {
  if (!strcmp(mode,"closed")) return 0;
  if (!strcmp(mode,"hold")) { pause_ms(1000); return 0; }
  if (!strcmp(mode,"slow")) pause_ms(150);
  if (!strcmp(mode,"chatty")) {
    char noise[4096]; memset(noise,'x',sizeof noise);
    for(int i=0;i<256;i++) { size_t n=0;while(n<sizeof noise){ssize_t r=write(2,noise+n,sizeof noise-n);if(r<0&&errno==EINTR)continue;if(r<=0)return 3;n+=(size_t)r;} }
  }
  unsigned char data[8192]; size_t count=0; int valid=1;
  for(;;) { ssize_t n=read(0,data,sizeof data);if(n<0&&errno==EINTR)continue;if(n<0)return 4;if(!n)break;for(ssize_t i=0;i<n;i++)if(data[i]!=(unsigned char)((count+(size_t)i)%251))valid=0;count+=(size_t)n; }
  printf("received %zu valid %d\n",count,valid); return valid?0:5;
}
static int64_t spawn(const char *self,const char *mode) {
  int64_t h=cxproc_new();assert(h>=0);
  for(const char *s=self;*s;s++)assert(cxproc_push(h,0,(unsigned char)*s)==0);
  assert(cxproc_end(h,0)==0);
  const char *prefix="--child";for(const char *s=prefix;*s;s++)assert(cxproc_push(h,0,*s)==0);assert(cxproc_end(h,0)==0);
  for(const char *s=mode;*s;s++){assert(cxproc_push(h,0,*s)==0);}
  assert(cxproc_end(h,0)==0);
  assert(cxproc_spawn(h)==0);return h;
}
/* Darwin can add OS-maintained status bits after the first pipe write.
   Preserve the public mutable flags, and separately compare a completed
   write with the legacy flush on an otherwise identical child. */
static void assert_flags(int fd,int expected) {
  const int mutable_flags=O_NONBLOCK|O_APPEND|O_ASYNC;
  assert((fcntl(fd,F_GETFL)&mutable_flags)==(expected&mutable_flags));
  assert(fcntl(fd,F_GETFD)&FD_CLOEXEC);
}
static void fill(int64_t h,size_t start,size_t n) {for(size_t i=start;i<start+n;i++)assert(cxproc_write(h,(int64_t)(i%251))==0);}
static void result(int64_t h,size_t n) {
  assert(cxproc_close_stdin(h)==0);assert(cxproc_flush_timeout(h,1)==CXPROC_ERROR);
  char expected[80],actual[80];snprintf(expected,sizeof expected,"received %zu valid 1",n);
  int64_t got=cxproc_read_line(h,3000);assert(got>=0 && got<(int64_t)sizeof actual);
  for(int64_t i=0;i<got;i++){actual[i]=(char)cxproc_line_byte(h,i);}
  actual[got]=0;
  assert(!strcmp(expected,actual));assert(cxproc_wait(h,2000)==0);
}
static void release_checked(int64_t h) {
  Proc *p=get(h);assert(p);int fds[3]={p->in,p->out,p->err};pid_t pid=p->pid;
  assert(cxproc_release(h)==0);
  for(int i=0;i<3;i++)if(fds[i]>=0){errno=0;assert(fcntl(fds[i],F_GETFD)==-1 && errno==EBADF);}
  errno=0;assert(waitpid(pid,NULL,WNOHANG)==-1 && errno==ECHILD);
}
int main(int argc,char **argv) {
  if(argc==3 && !strcmp(argv[1],"--child"))return child(argv[2]);
  assert(argc==1);alarm(15);
  assert(cxproc_flush_timeout(-1,10)==CXPROC_ERROR);
  int64_t unused=cxproc_new();assert(unused>=0);assert(cxproc_flush_timeout(unused,10)==CXPROC_ERROR);assert(cxproc_release(unused)==0);
  int64_t legacy=spawn(argv[0],"fast");fill(legacy,0,1024);assert(cxproc_flush(legacy)==0);int legacy_flags=fcntl(get(legacy)->in,F_GETFL);result(legacy,1024);release_checked(legacy);
  int64_t h=spawn(argv[0],"fast");Proc *p=get(h);int flags=fcntl(p->in,F_GETFL);
  assert(cxproc_flush_timeout(h,-1)==CXPROC_ERROR);assert(cxproc_flush_timeout(h,INT64_MAX)==CXPROC_ERROR);
  assert(cxproc_flush_timeout(h,0)==0);assert_flags(p->in,flags);
  fill(h,0,1024);assert(cxproc_flush_timeout(h,0)==0);assert(fcntl(p->in,F_GETFL)==legacy_flags);assert_flags(p->in,flags);
  result(h,1024);release_checked(h);
  h=spawn(argv[0],"slow");p=get(h);flags=fcntl(p->in,F_GETFL);
  const size_t size=1024*1024;fill(h,0,size);int64_t start=cxproc_now_ms();
  assert(cxproc_flush_timeout(h,25)==CXPROC_TIMEOUT);
  int64_t elapsed=cxproc_now_ms()-start;assert(elapsed>=15 && elapsed<500);
  assert(p->wbuf.count>0 && p->wbuf.count<size);assert_flags(p->in,flags);
  fill(h,size,7);assert(cxproc_flush_timeout(h,3000)==0);assert(p->wbuf.count==0);assert_flags(p->in,flags);
  result(h,size+7);release_checked(h);
  h=spawn(argv[0],"hold");p=get(h);flags=fcntl(p->in,F_GETFL);assert(fcntl(p->in,F_SETFL,flags|O_NONBLOCK)==0);
  fill(h,0,size);
  struct sigaction action,old;memset(&action,0,sizeof action);action.sa_handler=tick;sigemptyset(&action.sa_mask);assert(sigaction(SIGALRM,&action,&old)==0);
  struct itimerval timer={{0,2000},{0,2000}},off={{0,0},{0,0}};assert(setitimer(ITIMER_REAL,&timer,NULL)==0);
  start=cxproc_now_ms();assert(cxproc_flush_timeout(h,40)==CXPROC_TIMEOUT);elapsed=cxproc_now_ms()-start;
  assert(setitimer(ITIMER_REAL,&off,NULL)==0);assert(sigaction(SIGALRM,&old,NULL)==0);alarm(15);
  assert(alarms>0 && elapsed>=25 && elapsed<500);assert_flags(p->in,flags|O_NONBLOCK);release_checked(h);
  h=spawn(argv[0],"closed");p=get(h);flags=fcntl(p->in,F_GETFL);assert(cxproc_wait(h,2000)==0);fill(h,0,8);
  assert(cxproc_flush_timeout(h,100)==CXPROC_ERROR);assert_flags(p->in,flags);release_checked(h);
  h=spawn(argv[0],"fast");p=get(h);int owned=p->in;
  p->in=STDOUT_FILENO;assert(cxproc_flush_timeout(h,1)==CXPROC_ERROR);p->in=owned;
  int read_flags=fcntl(p->out,F_GETFL);p->in=p->out;assert(cxproc_flush_timeout(h,1)==CXPROC_ERROR);assert(fcntl(p->out,F_GETFL)==read_flags);p->in=owned;
  char name[]="/tmp/coax-flush-file.XXXXXX";int regular=mkstemp(name);assert(regular>=3);assert(unlink(name)==0);
  fill(h,0,8);p->in=regular;assert(cxproc_flush_timeout(h,1)==CXPROC_ERROR);
  struct stat info;assert(fstat(regular,&info)==0 && info.st_size==0);p->in=owned;close(regular);
  assert(cxproc_flush_timeout(h,1000)==0);result(h,8);release_checked(h);
  h=spawn(argv[0],"chatty");p=get(h);fill(h,0,262144);assert(cxproc_flush_timeout(h,2000)==0);
  assert(cxproc_stderr_count(h)==CXPROC_STDERR_KEEP);result(h,262144);release_checked(h);
  alarm(0);
  puts("process flush timeout: real pipe, partial retry, total deadline, flags, closed pipe, owned-only, opaque stderr, cleanup passed");
  return 0;
}

