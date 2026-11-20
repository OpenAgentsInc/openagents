/* What the Windows effects in coax/platform share (GitHub #13782): UTF-8 text from Coax to the
   UTF-16 the Win32 calls take and back, and the stdio names the generated C uses as fields.
   Paths cross from Coax with forward slashes (C:/Users/me/x), which every Win32 file call accepts;
   paths given back to Coax have their backslashes turned into forward slashes, so the core's
   one path rule (coax/lib/agent/desktop/paths.coax) holds on every system. */
#ifndef COAX_WIN32_NATIVE_H
#define COAX_WIN32_NATIVE_H
#ifndef WIN32_LEAN_AND_MEAN
#define WIN32_LEAN_AND_MEAN
#endif
#ifndef UNICODE
#define UNICODE
#endif
#include <windows.h>
#include <bcrypt.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

/* UTF-8 to a new UTF-16 string (free it), or NULL. */
static inline wchar_t *cxwin_wide(const char *s) {
  if (!s) return NULL;
  int n = MultiByteToWideChar(CP_UTF8, 0, s, -1, NULL, 0);
  if (n <= 0) return NULL;
  wchar_t *w = malloc((size_t)n * sizeof *w);
  if (w && MultiByteToWideChar(CP_UTF8, 0, s, -1, w, n) <= 0) { free(w); return NULL; }
  return w;
}
/* UTF-16 to UTF-8 in out (cap bytes, NUL-ended): its length, or -1 when it does not fit. */
static inline int64_t cxwin_utf8_into(const wchar_t *w, char *out, size_t cap) {
  if (!w || !cap) return -1;
  int n = WideCharToMultiByte(CP_UTF8, 0, w, -1, NULL, 0, NULL, NULL);
  if (n <= 0 || (size_t)n > cap) return -1;
  WideCharToMultiByte(CP_UTF8, 0, w, -1, out, n, NULL, NULL);
  return n - 1;
}
/* A path for Coax: backslashes become forward slashes, and the \\?\ prefix
   GetFinalPathNameByHandle puts in front goes ("\\?\UNC\host\x" is "//host/x"). */
static inline void cxwin_forward(char *p) {
  if (!strncmp(p, "\\\\?\\UNC\\", 8)) { memmove(p + 2, p + 8, strlen(p + 8) + 1); }
  else if (!strncmp(p, "\\\\?\\", 4)) memmove(p, p + 4, strlen(p + 4) + 1);
  for (char *s = p; *s; s++) if (*s == '\\') *s = '/';
}
/* n bytes of the system's entropy (BCryptGenRandom; link bcrypt): 0, or -1. */
static inline int cxwin_random(unsigned char *out, size_t n) {
  return BCryptGenRandom(NULL, out, (ULONG)n, BCRYPT_USE_SYSTEM_PREFERRED_RNG) == 0 ? 0 : -1;
}
#endif
