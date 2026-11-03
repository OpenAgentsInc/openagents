/* The fixtures for confine-check.coax: a temporary directory and the files,
   directories, links and FIFOs it puts in it. Test only; never linked into
   the engines. */
#define _GNU_SOURCE
#include <ftw.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <unistd.h>

typedef struct { uint8_t *data; int64_t count; } coax_v_u8;

static int cstr(coax_v_u8 v, char *out, size_t cap) {
  if (v.count < 0 || (size_t)v.count >= cap) return -1;
  if (v.count) memcpy(out, v.data, (size_t)v.count);
  out[v.count] = 0;
  return 0;
}

int64_t cft_tmpdir(coax_v_u8 out) {
  char t[512];
  snprintf(t, sizeof t, "%s/confine-check.XXXXXX", getenv("TMPDIR") && *getenv("TMPDIR") ? getenv("TMPDIR") : "/tmp");
  if (!mkdtemp(t)) return -1;
  size_t n = strlen(t);
  if ((int64_t)n > out.count) return -1;
  memcpy(out.data, t, n);
  return (int64_t)n;
}
int64_t cft_mkdir(coax_v_u8 p) { char a[1024]; return cstr(p, a, sizeof a) ? -1 : mkdir(a, 0700); }
int64_t cft_write(coax_v_u8 p, coax_v_u8 text) {
  char a[1024]; if (cstr(p, a, sizeof a)) return -1;
  FILE *f = fopen(a, "w"); if (!f) return -1;
  fwrite(text.data, 1, (size_t)text.count, f);
  return fclose(f);
}
int64_t cft_symlink(coax_v_u8 target, coax_v_u8 p) {
  char t[1024], a[1024]; if (cstr(target, t, sizeof t) || cstr(p, a, sizeof a)) return -1;
  return symlink(t, a);
}
int64_t cft_link(coax_v_u8 old, coax_v_u8 p) {
  char o[1024], a[1024]; if (cstr(old, o, sizeof o) || cstr(p, a, sizeof a)) return -1;
  return link(o, a);
}
int64_t cft_mkfifo(coax_v_u8 p) { char a[1024]; return cstr(p, a, sizeof a) ? -1 : mkfifo(a, 0600); }
int64_t cft_rename(coax_v_u8 from, coax_v_u8 to) {
  char f[1024], t[1024]; if (cstr(from, f, sizeof f) || cstr(to, t, sizeof t)) return -1;
  return rename(f, t);
}
static int rm_one(const char *p, const struct stat *sb, int flag, struct FTW *ftw) {
  (void)sb; (void)flag; (void)ftw;
  return remove(p);
}
int64_t cft_rmtree(coax_v_u8 p) {
  char a[1024]; if (cstr(p, a, sizeof a)) return -1;
  return nftw(a, rm_one, 16, FTW_DEPTH | FTW_PHYS);
}
void cft_exit(int64_t code) { exit((int)code); }
