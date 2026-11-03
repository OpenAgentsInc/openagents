/* The system calls behind confine.coax (the file tools stay inside the
   checkout). Nothing here decides: each call reports what the kernel says
   about one path, or counts what a walk of the checkout found, and
   confine.coax judges it. Linked into oa-turn and oa-chat-turn with
   turn.coax and chat.coax (confine-host.coax declares these).

   cfn_root(path)            the checkout as an O_PATH directory fd, or -1.
   cfn_facts(root, rel, out) what rel (relative to root; "" is root) is:
                             out[0] CFN_FOUND, CFN_MISSING, CFN_LINK,
                             CFN_ESCAPE or CFN_ERROR; out[1] its kind
                             (CFK_DIR, CFK_FILE, CFK_LINK, CFK_OTHER);
                             out[2] its link count. It is opened with
                             openat2(RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS |
                             RESOLVE_NO_MAGICLINKS), O_PATH | O_NOFOLLOW, so
                             the kernel refuses a symbolic link anywhere on
                             the way and any step out of root. Where openat2
                             is missing (ENOSYS, or a filter's EPERM), each
                             component is opened in turn with openat(O_PATH |
                             O_NOFOLLOW) from the one before and checked, the
                             same rule by hand.
   cfn_sweep(root, counts)   walks the whole checkout without following
                             links (openat O_NOFOLLOW, fstatat
                             AT_SYMLINK_NOFOLLOW) and counts directories,
                             files, files with more than one link, symbolic
                             links, anything else, errors, and whether the
                             walk was cut short (CFS_* in confine.coax).
   cfn_close(fd)             closes root.
   cfn_force_walk(on)        tests only: use the by-hand walk even where
                             openat2 works. */
#define _GNU_SOURCE
#include <dirent.h>
#include <errno.h>
#include <fcntl.h>
#include <stdint.h>
#include <stdio.h>
#include <string.h>
#include <sys/stat.h>
#include <sys/syscall.h>
#include <unistd.h>
#ifdef __linux__
#include <linux/openat2.h>
#endif

typedef struct { uint8_t *data; int64_t count; } coax_v_u8;
typedef struct { int64_t *data; int64_t count; } coax_v_s64;

enum { CFN_FOUND, CFN_MISSING, CFN_LINK, CFN_ESCAPE, CFN_ERROR };
enum { CFK_DIR = 1, CFK_FILE, CFK_LINK, CFK_OTHER };
enum { CFS_DIRS, CFS_FILES, CFS_MULTI, CFS_LINKS, CFS_OTHER, CFS_ERRORS, CFS_CUT, CFS_COUNT };
#define CFN_PATH 4096
#define CFN_DEPTH 256
#define CFN_ENTRIES 2000000

static int force_walk;
int64_t cfn_force_walk(int64_t on) { force_walk = on != 0; return 0; }

static int copy_path(coax_v_u8 p, char *out) {
  if (p.count < 0 || p.count >= CFN_PATH) return -1;
  if (p.count) memcpy(out, p.data, (size_t)p.count);
  out[p.count] = 0;
  if (memchr(out, 0, (size_t)p.count)) return -1;
  if (!out[0]) strcpy(out, ".");
  return 0;
}

int64_t cfn_root(coax_v_u8 path) {
  char p[CFN_PATH];
  if (copy_path(path, p)) return -1;
  int fd = open(p, O_PATH | O_DIRECTORY | O_CLOEXEC);
  return fd < 0 ? -1 : fd;
}

int64_t cfn_close(int64_t fd) { return fd >= 0 ? close((int)fd) : 0; }

static int64_t kind_of(mode_t m) {
  return S_ISDIR(m) ? CFK_DIR : S_ISREG(m) ? CFK_FILE : S_ISLNK(m) ? CFK_LINK : CFK_OTHER;
}
static int64_t code_of(int e) {
  return e == ENOENT ? CFN_MISSING : e == ELOOP ? CFN_LINK : e == EXDEV ? CFN_ESCAPE : CFN_ERROR;
}

static int open_beneath(int root, const char *rel) {
#if defined(__linux__) && defined(SYS_openat2)
  if (!force_walk) {
    struct open_how how = {0};
    how.flags = O_PATH | O_NOFOLLOW | O_CLOEXEC;
    how.resolve = RESOLVE_BENEATH | RESOLVE_NO_SYMLINKS | RESOLVE_NO_MAGICLINKS;
    long fd = syscall(SYS_openat2, root, rel, &how, sizeof how);
    if (fd >= 0 || (errno != ENOSYS && errno != EPERM)) return (int)fd;
  }
#endif
  /* by hand: one component at a time, never following a link */
  char buf[CFN_PATH];
  snprintf(buf, sizeof buf, "%s", rel);
  int at = dup(root);
  if (at < 0) return -1;
  char *save = NULL;
  for (char *c = strtok_r(buf, "/", &save); c; c = strtok_r(NULL, "/", &save)) {
    if (!strcmp(c, ".")) continue;
    if (!strcmp(c, "..")) { close(at); errno = EXDEV; return -1; }
    struct stat sb;
    if (fstat(at, &sb)) { int e = errno; close(at); errno = e; return -1; }
    if (S_ISLNK(sb.st_mode)) { close(at); errno = ELOOP; return -1; }
    if (!S_ISDIR(sb.st_mode)) { close(at); errno = ENOTDIR; return -1; }
    int next = openat(at, c, O_PATH | O_NOFOLLOW | O_CLOEXEC);
    int e = errno;
    close(at);
    if (next < 0) { errno = e; return -1; }
    at = next;
  }
  /* a link as the last component is returned as itself (its kind says so),
     as openat2 does with O_PATH | O_NOFOLLOW; a link before it is ELOOP above */
  return at;
}

int64_t cfn_facts(int64_t root, coax_v_u8 rel, coax_v_s64 out) {
  if (out.count < 3) return -1;
  out.data[0] = CFN_ERROR; out.data[1] = 0; out.data[2] = 0;
  char p[CFN_PATH];
  if (root < 0 || copy_path(rel, p)) return 0;
  int fd = open_beneath((int)root, p);
  if (fd < 0) { out.data[0] = code_of(errno); return 0; }
  struct stat sb;
  int r = fstat(fd, &sb);
  close(fd);
  if (r) return 0;
  out.data[0] = CFN_FOUND; out.data[1] = kind_of(sb.st_mode); out.data[2] = (int64_t)sb.st_nlink;
  return 0;
}

static void sweep(int dirfd, int depth, int64_t *n, int64_t *entries) {
  if (depth > CFN_DEPTH) { n[CFS_CUT]++; close(dirfd); return; }
  DIR *d = fdopendir(dirfd);
  if (!d) { n[CFS_ERRORS]++; close(dirfd); return; }
  struct dirent *e;
  while ((errno = 0, e = readdir(d))) {
    if (!strcmp(e->d_name, ".") || !strcmp(e->d_name, "..")) continue;
    if (++*entries > CFN_ENTRIES) { n[CFS_CUT]++; break; }
    struct stat sb;
    if (fstatat(dirfd, e->d_name, &sb, AT_SYMLINK_NOFOLLOW)) { n[CFS_ERRORS]++; continue; }
    if (S_ISDIR(sb.st_mode)) {
      n[CFS_DIRS]++;
      int sub = openat(dirfd, e->d_name, O_RDONLY | O_DIRECTORY | O_NOFOLLOW | O_CLOEXEC);
      if (sub < 0) { n[CFS_ERRORS]++; continue; }
      sweep(sub, depth + 1, n, entries);
    } else if (S_ISREG(sb.st_mode)) {
      n[sb.st_nlink > 1 ? CFS_MULTI : CFS_FILES]++;
    } else if (S_ISLNK(sb.st_mode)) {
      n[CFS_LINKS]++;
    } else {
      n[CFS_OTHER]++;
    }
  }
  if (errno) n[CFS_ERRORS]++;
  closedir(d);
}

int64_t cfn_sweep(int64_t root, coax_v_s64 counts) {
  if (counts.count < CFS_COUNT) return -1;
  for (int i = 0; i < CFS_COUNT; i++) counts.data[i] = 0;
  int fd = root < 0 ? -1 : openat((int)root, ".", O_RDONLY | O_DIRECTORY | O_CLOEXEC);
  if (fd < 0) { counts.data[CFS_ERRORS] = 1; return 0; }
  int64_t entries = 0;
  sweep(fd, 0, counts.data, &entries);
  return 0;
}
