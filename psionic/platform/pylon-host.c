/* The chapter-19 Pylon's C host: effects only. It runs the Coax main
   (psionic/lib/pylon/run.coax, compiled with -Dmain=coax_pylon_main) on a
   thread with a 512 MiB stack, hands the arguments over one byte at a time,
   reads the wall clock, and reports a stop request: SIGTERM, SIGINT, or a
   stop file the Coax side named. The relay connection, the WebSocket, NIP-42,
   NIP-44, admission, the queue, the engine's HTTP request and every answer
   are Coax. Sockets are coax/platform/socket-native.c, TLS for wss://
   tls-native.c (OpenSSL), entropy entropy-foreign.c; signing is Coax
   (event_sign.coax). */
#if defined(__APPLE__)
#define _DARWIN_C_SOURCE
#endif
#define _POSIX_C_SOURCE 200809L
#include <pthread.h>
#include <fcntl.h>
#include <sys/stat.h>
#include <signal.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <errno.h>
#include <sys/file.h>
#include <time.h>
#include <unistd.h>

int coax_pylon_main(void);

static int g_argc;
static char **g_argv;
static volatile sig_atomic_t g_stop;
static int g_status;
static char g_stop_path[1024];
static size_t g_stop_used;

int64_t pyh_argc(void) { return g_argc; }
int64_t pyh_arg_len(int64_t i) { return i >= 0 && i < g_argc ? (int64_t)strlen(g_argv[i]) : -1; }
int64_t pyh_arg_byte(int64_t i, int64_t k) {
  if (i < 0 || i >= g_argc || k < 0 || (size_t)k >= strlen(g_argv[i])) return -1;
  return (unsigned char)g_argv[i][k];
}
/* The exit status the Coax main reports (chapter 159: the doctor's verdict). */
int64_t pyh_status(int64_t code) { g_status = (int)(code & 255); return 0; }
int64_t pyh_now_s(void) { return (int64_t)time(NULL); }
/* Sleeps ms (at most 10 s), for the reconnect backoff. */
int64_t pyh_sleep_ms(int64_t ms) {
  if (ms <= 0) return 0;
  if (ms > 10000) ms = 10000;
  struct timespec t = { (time_t)(ms / 1000), (long)(ms % 1000) * 1000000L };
  while (nanosleep(&t, &t) != 0) {}
  return 0;
}
/* Names the stop file, one byte at a time (at most 1023). */
int64_t pyh_stop_path_push(int64_t byte) {
  if (byte <= 0 || byte > 255 || g_stop_used + 1 >= sizeof g_stop_path) return -1;
  g_stop_path[g_stop_used++] = (char)byte; g_stop_path[g_stop_used] = 0;
  return 0;
}
int64_t pyh_stop(void) {
  if (g_stop) return 1;
  return g_stop_used > 0 && access(g_stop_path, F_OK) == 0 ? 1 : 0;
}

/* Chapter 159: one named file, read whole (at most 1 MiB) or written whole
   through a temporary file and a rename. The Coax side names the path a byte
   at a time, as it does the stop file. */
static char g_file_path[1024];
static size_t g_file_used;
static unsigned char g_file[1 << 20];
static size_t g_file_len, g_write_len;
int64_t pyh_file_path_clear(void) { g_file_used = 0; g_file_path[0] = 0; return 0; }
int64_t pyh_file_path_push(int64_t byte) {
  if (byte <= 0 || byte > 255 || g_file_used + 1 >= sizeof g_file_path) return -1;
  g_file_path[g_file_used++] = (char)byte; g_file_path[g_file_used] = 0;
  return 0;
}
/* The file's length, or -1 when it can't be read; -2 when it is missing. */
int64_t pyh_file_read(void) {
  g_file_len = 0;
  FILE *f = fopen(g_file_path, "rb");
  if (!f) return errno == ENOENT ? -2 : -1;
  size_t n = fread(g_file, 1, sizeof g_file, f);
  int more = fgetc(f) != EOF, bad = ferror(f);
  fclose(f);
  if (more || bad) return -1;
  g_file_len = n; return (int64_t)n;
}
int64_t pyh_file_byte(int64_t i) { return i >= 0 && (size_t)i < g_file_len ? g_file[i] : -1; }
/* Demo-label bytes have their own bounded buffer and are never argv values.
   Metadata is observed on the opened file, never on a path checked earlier. */
static unsigned char g_demo_label[256];
static size_t g_demo_label_len;
int64_t pyh_label_clear(void) {
  volatile unsigned char *p = g_demo_label;
  for (size_t i = 0; i < sizeof g_demo_label; i++) p[i] = 0;
  g_demo_label_len = 0; return 0;
}
int64_t pyh_label_read(void) {
  pyh_label_clear();
  int fd = open(g_file_path, O_RDONLY | O_NOFOLLOW | O_NONBLOCK | O_CLOEXEC);
  if (fd < 0) return -1;
  struct stat st;
  if (fstat(fd, &st) != 0 || !S_ISREG(st.st_mode) || st.st_uid != geteuid() ||
      (st.st_mode & 077) != 0 || st.st_size < 0 || st.st_size > (off_t)sizeof g_demo_label) {
    close(fd); return -1;
  }
  ssize_t n = read(fd, g_demo_label, sizeof g_demo_label);
  unsigned char extra; ssize_t more = read(fd, &extra, 1); close(fd);
  if (n < 0 || more != 0) { pyh_label_clear(); return -1; }
  g_demo_label_len = (size_t)n; return n;
}
int64_t pyh_label_byte(int64_t i) {
  return i >= 0 && (size_t)i < g_demo_label_len ? g_demo_label[i] : -1;
}

int64_t pyh_file_write_push(int64_t byte) {
  if (byte < 0 || byte > 255 || g_write_len >= sizeof g_file) return -1;
  g_file[g_write_len++] = (unsigned char)byte; return 0;
}
int64_t pyh_file_write_commit(void) {
  char tmp[1100],parent[1024];
  int length=snprintf(tmp,sizeof tmp,"%s.tmp.XXXXXX",g_file_path);
  size_t n = g_write_len; g_write_len = 0; g_file_len = 0;
  if(length<0||(size_t)length>=sizeof tmp)return -1;
  const char *slash=strrchr(g_file_path,'/');
  if(!slash)strcpy(parent,".");
  else if(slash==g_file_path)strcpy(parent,"/");
  else{size_t len=(size_t)(slash-g_file_path);memcpy(parent,g_file_path,len);parent[len]=0;}
  int directory=open(parent,O_RDONLY|O_DIRECTORY|O_CLOEXEC);
  if(directory<0)return -1;
  int fd=mkstemp(tmp);
  if(fd<0){close(directory);return -1;}
  FILE *f = fdopen(fd,"wb");
  if (!f) {close(fd);close(directory);unlink(tmp);return -1;}
  int ok = fwrite(g_file, 1, n, f) == n && fflush(f) == 0 && fsync(fileno(f)) == 0;
  ok = fclose(f) == 0 && ok;
  if (!ok || rename(tmp, g_file_path) != 0) { unlink(tmp);close(directory);return -1; }
  // Only report durable success after the directory entry is persisted too.
  ok=fsync(directory)==0;close(directory);return ok?0:-1;
}
/* Lifetime lease for public generation state, never a key file. Keeping a
   separate inode locked survives atomic replacement of the counter file. */
static int g_state_lock_fd=-1;
int64_t pyh_file_lock(void) {
  if(g_state_lock_fd>=0)return -1;
  int fd=open(g_file_path,O_RDWR|O_CREAT|O_NOFOLLOW|O_NONBLOCK|O_CLOEXEC,0600);
  if(fd<0)return -1;
  struct stat st;
  if(fstat(fd,&st)||!S_ISREG(st.st_mode)||st.st_uid!=geteuid()||(st.st_mode&077)||flock(fd,LOCK_EX|LOCK_NB)){
    close(fd);return -1;
  }
  g_state_lock_fd=fd;return 0;
}

static void on_signal(int sig) { (void)sig; g_stop = 1; }
/* The worker blocks the stop signals, so the main thread (parked in
   pthread_join) takes them and no socket wait in the worker sees EINTR. */
static void *run(void *arg) {
  (void)arg; sigset_t s; sigemptyset(&s); sigaddset(&s, SIGTERM); sigaddset(&s, SIGINT);
  pthread_sigmask(SIG_BLOCK, &s, NULL);
  coax_pylon_main(); return NULL;
}

int main(int argc, char **argv) {
  g_argc = argc; g_argv = argv;
  setvbuf(stdout, NULL, _IOLBF, 0); /* the log is read while the Pylon runs */
  struct sigaction sa; memset(&sa, 0, sizeof sa);
  sa.sa_handler = on_signal; sa.sa_flags = SA_RESTART; sigemptyset(&sa.sa_mask);
  sigaction(SIGTERM, &sa, NULL); sigaction(SIGINT, &sa, NULL);
  pthread_attr_t attr; pthread_attr_init(&attr);
  pthread_attr_setstacksize(&attr, (size_t)512 << 20);
  pthread_t t;
  if (pthread_create(&t, &attr, run, NULL) != 0) { fprintf(stderr, "pylon: no thread\n"); return 1; }
  pthread_join(t, NULL);
  return g_status;
}
