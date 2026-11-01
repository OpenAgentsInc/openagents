/* Platform effects for the openagents CLI (apps/openagents-cli). Effects only:
   every decision (which file, which account, what a key is) is Coax.

   - cxcli_now_s: wall-clock seconds (Nostr created_at).
   - cxcli_isatty: whether fd 0, 1 or 2 is a terminal.
   - cxcli_read_line: one line from stdin into a borrowed view, echo off on a terminal.
   - cxcli_read_stdin: all of stdin to its end (a plan file piped in).
   - cxcli_write_private: a file written mode 0600 through a temporary file and rename.
   - cxcli_mkdir_private: a directory (and its parents) mode 0700.
   - cxcli_alive: whether a process id exists (kill 0).
   - cxcli_exe_path: this program's absolute path.
   - cxcli_keychain_set/get/remove: macOS Keychain generic passwords, service
     com.openagents.cli. Other systems answer -2 (no keychain).
   Views are borrowed for the call only (Coax spec 7.1). */
#ifndef _GNU_SOURCE
#define _GNU_SOURCE
#endif
#include "entropy-foreign.h" /* the coax_v_u8 view type, spelled as Coax spells it */
#include <errno.h>
#include <fcntl.h>
#include <limits.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <termios.h>
#include <time.h>
#include <unistd.h>
#if defined(__APPLE__)
#include <mach-o/dyld.h>
#include <Security/Security.h>
#endif

#define CLI_PATH_MAX 4096

static int cstr(coax_v_u8 v, char *out, size_t cap) {
  if (v.count <= 0 || (size_t)v.count >= cap) return -1;
  for (int64_t i = 0; i < v.count; i++) { if (v.data[i] == 0) return -1; out[i] = (char)v.data[i]; }
  out[v.count] = 0;
  return 0;
}

int64_t cxcli_now_s(void) { return (int64_t)time(NULL); }

/* This computer's name (gethostname), for the machine a login names: its length, or -1. */
int64_t cxcli_hostname(coax_v_u8 out) {
  char name[256];
  if (gethostname(name, sizeof name) != 0) return -1;
  name[sizeof name - 1] = 0;
  size_t n = strlen(name);
  if ((int64_t)n > out.count) n = (size_t)out.count;
  memcpy(out.data, name, n);
  return (int64_t)n;
}

int64_t cxcli_isatty(int64_t fd) { return fd >= 0 && fd <= 2 && isatty((int)fd) ? 1 : 0; }

int64_t cxcli_read_line(coax_v_u8 out, int64_t hide) {
  struct termios saved, t;
  int quiet = hide && isatty(0) && tcgetattr(0, &saved) == 0;
  if (quiet) { t = saved; t.c_lflag &= ~(tcflag_t)ECHO; tcsetattr(0, TCSANOW, &t); }
  int64_t n = 0, over = 0;
  for (;;) {
    unsigned char c;
    ssize_t r = read(0, &c, 1);
    if (r < 0 && errno == EINTR) continue;
    if (r <= 0 || c == '\n') break;
    if (c == '\r') continue;
    if (n < out.count) out.data[n++] = c; else over = 1;
  }
  if (quiet) { tcsetattr(0, TCSANOW, &saved); fputc('\n', stderr); }
  return over ? -1 : n;
}

int64_t cxcli_mkdir_private(coax_v_u8 path) {
  char d[CLI_PATH_MAX];
  if (cstr(path, d, sizeof d)) return -1;
  for (char *s = d + 1; *s; s++) {
    if (*s != '/') continue;
    *s = 0;
    if (mkdir(d, 0700) != 0 && errno != EEXIST) return -1;
    *s = '/';
  }
  if (mkdir(d, 0700) != 0 && errno != EEXIST) return -1;
  struct stat st;
  if (lstat(d, &st) != 0 || !S_ISDIR(st.st_mode)) return -1;
  return 0;
}

int64_t cxcli_write_private(coax_v_u8 path, coax_v_u8 data) {
  char p[CLI_PATH_MAX], tmp[CLI_PATH_MAX + 16];
  if (cstr(path, p, sizeof p)) return -1;
  struct stat st;
  if (lstat(p, &st) == 0 && !S_ISREG(st.st_mode)) return -1; /* never through a symlink */
  snprintf(tmp, sizeof tmp, "%s.tmp%ld", p, (long)getpid());
  int f = open(tmp, O_WRONLY | O_CREAT | O_EXCL | O_CLOEXEC | O_NOFOLLOW, 0600);
  if (f < 0) return -1;
  int64_t done = 0;
  while (done < data.count) {
    ssize_t w = write(f, data.data + done, (size_t)(data.count - done));
    if (w < 0 && errno == EINTR) continue;
    if (w <= 0) { close(f); unlink(tmp); return -1; }
    done += w;
  }
  if (fsync(f) != 0 || close(f) != 0 || rename(tmp, p) != 0) { unlink(tmp); return -1; }
  return 0;
}

int64_t cxcli_alive(int64_t pid) {
  if (pid <= 1 || pid > INT_MAX) return 0;
  return kill((pid_t)pid, 0) == 0 || errno == EPERM ? 1 : 0;
}

int64_t cxcli_exe_path(coax_v_u8 out) {
  char raw[CLI_PATH_MAX], real[CLI_PATH_MAX];
#if defined(__APPLE__)
  uint32_t size = sizeof raw;
  if (_NSGetExecutablePath(raw, &size) != 0) return -1;
#else
  ssize_t n = readlink("/proc/self/exe", raw, sizeof raw - 1);
  if (n <= 0) return -1;
  raw[n] = 0;
#endif
  if (!realpath(raw, real)) return -1;
  size_t k = strlen(real);
  if ((int64_t)k > out.count) return -1;
  memcpy(out.data, real, k);
  return (int64_t)k;
}

#if defined(__APPLE__)
static const char *SERVICE = "com.openagents.cli";
static CFMutableDictionaryRef query(const char *account) {
  CFMutableDictionaryRef q = CFDictionaryCreateMutable(NULL, 0, &kCFTypeDictionaryKeyCallBacks, &kCFTypeDictionaryValueCallBacks);
  CFStringRef s = CFStringCreateWithCString(NULL, SERVICE, kCFStringEncodingUTF8);
  CFStringRef a = CFStringCreateWithCString(NULL, account, kCFStringEncodingUTF8);
  CFDictionarySetValue(q, kSecClass, kSecClassGenericPassword);
  CFDictionarySetValue(q, kSecAttrService, s);
  CFDictionarySetValue(q, kSecAttrAccount, a);
  CFRelease(s); CFRelease(a);
  return q;
}
#endif

int64_t cxcli_keychain_set(coax_v_u8 account, coax_v_u8 secret) {
#if defined(__APPLE__)
  char a[256];
  if (cstr(account, a, sizeof a) || secret.count <= 0) return -1;
  CFMutableDictionaryRef q = query(a);
  SecItemDelete(q);
  CFDataRef d = CFDataCreate(NULL, secret.data, (CFIndex)secret.count);
  CFDictionarySetValue(q, kSecValueData, d);
  CFDictionarySetValue(q, kSecAttrAccessible, kSecAttrAccessibleWhenUnlockedThisDeviceOnly);
  OSStatus st = SecItemAdd(q, NULL);
  CFRelease(d); CFRelease(q);
  return st == errSecSuccess ? 0 : -1;
#else
  (void)account; (void)secret;
  return -2;
#endif
}

int64_t cxcli_keychain_get(coax_v_u8 account, coax_v_u8 out) {
#if defined(__APPLE__)
  char a[256];
  if (cstr(account, a, sizeof a)) return -1;
  CFMutableDictionaryRef q = query(a);
  CFDictionarySetValue(q, kSecReturnData, kCFBooleanTrue);
  CFDictionarySetValue(q, kSecMatchLimit, kSecMatchLimitOne);
  CFTypeRef got = NULL;
  OSStatus st = SecItemCopyMatching(q, &got);
  CFRelease(q);
  if (st == errSecItemNotFound) return 0;
  if (st != errSecSuccess || !got) return -1;
  CFIndex n = CFDataGetLength((CFDataRef)got);
  int64_t r = -1;
  if (n > 0 && n <= out.count) { memcpy(out.data, CFDataGetBytePtr((CFDataRef)got), (size_t)n); r = n; }
  CFRelease(got);
  return r;
#else
  (void)account; (void)out;
  return -2;
#endif
}

int64_t cxcli_keychain_remove(coax_v_u8 account) {
#if defined(__APPLE__)
  char a[256];
  if (cstr(account, a, sizeof a)) return -1;
  CFMutableDictionaryRef q = query(a);
  OSStatus st = SecItemDelete(q);
  CFRelease(q);
  return st == errSecSuccess ? 1 : st == errSecItemNotFound ? 0 : -1;
#else
  (void)account;
  return -2;
#endif
}

/* All of stdin to its end into a borrowed view: the bytes read, or -1 when
   it is more than the view holds (or a read fails). */
int64_t cxcli_read_stdin(coax_v_u8 out) {
  int64_t n = 0;
  for (;;) {
    unsigned char c;
    ssize_t r = read(0, &c, 1);
    if (r < 0 && errno == EINTR) continue;
    if (r < 0) return -1;
    if (r == 0) return n;
    if (n >= out.count) return -1;
    out.data[n++] = c;
  }
}
