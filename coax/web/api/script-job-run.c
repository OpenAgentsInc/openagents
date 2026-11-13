// The compile job run on one source (script-job-run.h). Moved from scripts.c so the compile service runs
// the job exactly as the API does. It runs tools that are not a sandbox, under limits of their own
// (coax/web/script-job.sh); nothing it writes is believed by the API but the module's bytes, which are
// checked as any upload, and the name and state fields of its manifest.
#include "script-job-run.h"

#include <fcntl.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

bool script_job_name_safe(const char *name) {
  size_t n = strlen(name);
  if (n < 1 || n > 48) return false;
  for (size_t i = 0; i < n; i++)
    if (!((name[i] >= 'a' && name[i] <= 'z') || (name[i] >= '0' && name[i] <= '9') || name[i] == '-')) return false;
  return true;
}

void script_job_words(char *words, long n, size_t cap) {
  if (n < 0) n = 0;
  if ((size_t)n > cap - 1) n = (long)cap - 1;
  while (n > 0 && (words[n - 1] == '\n' || words[n - 1] == ' ')) n--;
  for (long i = 0; i < n; i++) if ((unsigned char)words[i] < 32 || (unsigned char)words[i] > 126) words[i] = ' ';
  words[n] = 0;
}

// A file's bytes (at most cap) into out: how many, or -1.
static long file_read(const char *path, char *out, size_t cap) {
  int fd = open(path, O_RDONLY | O_NOFOLLOW);
  if (fd < 0) return -1;
  size_t n = 0; ssize_t k;
  while (n < cap && (k = read(fd, out + n, cap - n)) > 0) n += (size_t)k;
  char one;
  bool more = n == cap && read(fd, &one, 1) > 0;
  close(fd);
  return more ? -1 : (long)n;
}

int script_job_run(const char *cmd, const char *name, const char *source, size_t source_n, long wait_ms,
                   char *wasm, long *wasm_n, char *made, long *made_n, char *words, size_t words_cap) {
  const char *tmp = getenv("TMPDIR");
  char dir[400], src[480], out[480], log[480], path[600];
  snprintf(dir, sizeof dir, "%s/oa-script-XXXXXX", tmp && *tmp ? tmp : "/tmp");
  snprintf(words, words_cap, "the compile job could not be run");
  *wasm_n = 0; *made_n = 0;
  if (!script_job_name_safe(name) || !mkdtemp(dir)) return 2;
  snprintf(src, sizeof src, "%s/%s.coax", dir, name); snprintf(out, sizeof out, "%s/out", dir); snprintf(log, sizeof log, "%s/log", dir);
  int result = 2;
  int sfd = open(src, O_WRONLY | O_CREAT | O_EXCL, 0600);
  bool wrote = sfd >= 0 && write(sfd, source, source_n) == (ssize_t)source_n;
  if (sfd >= 0) close(sfd);
  pid_t pid = wrote ? fork() : -1;
  if (pid == 0) {
    setpgid(0, 0);
    int lfd = open(log, O_WRONLY | O_CREAT | O_TRUNC, 0600), nfd = open("/dev/null", O_RDONLY);
    if (lfd < 0 || nfd < 0) _exit(126);
    dup2(nfd, 0); dup2(lfd, 1); dup2(lfd, 2);
    for (int fd = 3; fd < 256; fd++) close(fd);   // the job holds no socket and no database connection of its caller
    // the job inherits none of its caller's environment: only where its tools are
    static const char *const keep[] = {"PATH", "HOME", "TMPDIR", "COAX", "COAXIAL_CLANG", "COAXIAL_RESOURCES", "COAXIAL_TOOLCHAIN", "OA_SCRIPT_JOB_SANDBOX"};
    static char vars[8][1200]; char *envp[9]; int en = 0;
    for (size_t i = 0; i < sizeof keep / sizeof keep[0]; i++) {
      const char *v = getenv(keep[i]);
      if (v && *v && snprintf(vars[en], sizeof vars[en], "%s=%s", keep[i], v) < (int)sizeof vars[en]) { envp[en] = vars[en]; en++; }
    }
    envp[en] = NULL;
    execle(cmd, cmd, src, name, out, (char *)NULL, envp);
    _exit(127);
  }
  if (pid > 0) {
    int status = 0; long waited = 0; pid_t got = 0;
    while ((got = waitpid(pid, &status, WNOHANG)) == 0 && waited < wait_ms) { struct timespec ts = {0, 20000000}; nanosleep(&ts, NULL); waited += 20; }
    if (got == 0) { kill(-pid, SIGKILL); kill(pid, SIGKILL); waitpid(pid, &status, 0); snprintf(words, words_cap, "the compile job ran past its time limit"); }
    else if (got == pid && WIFEXITED(status) && WEXITSTATUS(status) == 0) {
      snprintf(path, sizeof path, "%s/%s.wasm", out, name); *wasm_n = file_read(path, wasm, SCRIPT_JOB_MODULE);
      snprintf(path, sizeof path, "%s/%s.json", out, name); *made_n = file_read(path, made, SCRIPT_JOB_MANIFEST);
      if (*wasm_n > 0 && *made_n > 0) result = 0; else snprintf(words, words_cap, "the compile job made no module");
    } else if (got == pid && WIFEXITED(status) && WEXITSTATUS(status) == 1) {
      long n = file_read(log, words, words_cap - 1);
      if (n < 0) n = (long)words_cap - 1;   // longer than a refusal's words: its start
      script_job_words(words, n, words_cap);
      if (!words[0]) snprintf(words, words_cap, "the compile job refused the source");
      result = 1;
    }
    kill(-pid, SIGKILL);   // nothing the job started outlives it
  }
  // the directory goes: the files its caller and the job's contract name, then the directory
  snprintf(path, sizeof path, "%s/%s.wasm", out, name); unlink(path);
  snprintf(path, sizeof path, "%s/%s.json", out, name); unlink(path);
  rmdir(out); unlink(src); unlink(log);
  if (rmdir(dir) != 0) fprintf(stderr, "scripts: the compile job left files in its directory\n");
  return result;
}
