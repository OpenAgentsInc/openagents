/* The Windows command line for an argument list (GitHub #13782). Windows hands a child one
   string, and its C runtime (and CommandLineToArgvW) splits it back into arguments by fixed
   rules: an argument is quoted when it is empty or holds a space, a tab, a newline or a quote;
   inside quotes, backslashes are literal unless they come before a quote, where each pair is one
   backslash and an odd one escapes the quote. This header only builds that string. It has no
   Windows dependency, so the rules are checked on any computer
   (apps/openagents-desktop/windows/args-test.c, run by apps/openagents-desktop/check.sh).
   Programs that parse their own command line differently (cmd.exe and .bat scripts) are not
   handled: the desktop core starts .exe programs only. */
#ifndef COAX_WIN32_ARGS_H
#define COAX_WIN32_ARGS_H
#include <stddef.h>
#include <string.h>

/* Appends arg to out at *at, quoted when it needs to be. 0, or -1 when out is too small
   (out holds at most cap bytes, and is NUL-ended whenever there is room). */
static inline int cxwin_quote_arg(const char *arg, char *out, size_t cap, size_t *at) {
  size_t n = *at;
#define CXWIN_PUT(c) do { if (n + 1 >= cap) return -1; out[n++] = (char)(c); } while (0)
  int plain = arg[0] != 0;
  for (const char *s = arg; *s; s++) if (*s == ' ' || *s == '\t' || *s == '\n' || *s == '\v' || *s == '"') { plain = 0; break; }
  if (plain) {
    for (const char *s = arg; *s; s++) CXWIN_PUT(*s);
  } else {
    CXWIN_PUT('"');
    for (const char *s = arg;; s++) {
      size_t slashes = 0;
      while (*s == '\\') { slashes++; s++; }
      if (!*s) { for (size_t i = 0; i < slashes * 2; i++) CXWIN_PUT('\\'); break; }
      if (*s == '"') { for (size_t i = 0; i < slashes * 2 + 1; i++) CXWIN_PUT('\\'); CXWIN_PUT('"'); }
      else { for (size_t i = 0; i < slashes; i++) CXWIN_PUT('\\'); CXWIN_PUT(*s); }
    }
    CXWIN_PUT('"');
  }
#undef CXWIN_PUT
  out[n] = 0;
  *at = n;
  return 0;
}

/* The whole command line, arguments separated by one space: its length, or -1 when it does
   not fit (Windows takes at most 32767 characters). */
static inline long cxwin_command_line(char *const *argv, size_t argc, char *out, size_t cap) {
  size_t at = 0;
  if (cap) out[0] = 0;
  for (size_t i = 0; i < argc; i++) {
    if (i) { if (at + 1 >= cap) return -1; out[at++] = ' '; out[at] = 0; }
    if (cxwin_quote_arg(argv[i], out, cap, &at) != 0) return -1;
  }
  return (long)at;
}

/* The last part of a POSIX path ("/bin/sh" is "sh"): a program the core names by its POSIX
   place is looked for by name on Windows. */
static inline const char *cxwin_leaf(const char *path) {
  const char *leaf = path;
  for (const char *s = path; *s; s++) if (*s == '/' || *s == '\\') leaf = s + 1;
  return leaf;
}

/* Whether path names a place on its own: a drive ("C:/x", "C:\x"), a share ("\\host\x",
   "//host/x") or the POSIX root ("/x", which is looked up by name, cxwin_leaf). */
static inline int cxwin_is_drive_path(const char *p) {
  return ((p[0] >= 'A' && p[0] <= 'Z') || (p[0] >= 'a' && p[0] <= 'z')) && p[1] == ':' && (p[2] == '/' || p[2] == '\\');
}
static inline int cxwin_is_share_path(const char *p) { return (p[0] == '/' || p[0] == '\\') && (p[1] == '/' || p[1] == '\\'); }
static inline int cxwin_is_posix_root(const char *p) { return p[0] == '/' && p[1] != '/'; }

/* Case-blind ASCII comparison of the names of two "NAME=VALUE" entries (Windows' variables
   are case-blind): <0, 0 or >0. */
static inline int cxwin_env_cmp(const char *a, const char *b) {
  for (;; a++, b++) {
    int x = *a == '=' ? 0 : (unsigned char)*a, y = *b == '=' ? 0 : (unsigned char)*b;
    if (x >= 'a' && x <= 'z') x -= 32;
    if (y >= 'a' && y <= 'z') y -= 32;
    if (x != y || !x) return x - y;
  }
}
#endif
