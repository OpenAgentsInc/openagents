// Checks coax/platform/win32-args.h on any computer (GitHub #13782): the command line Windows
// children get, and the name rules process-win32.c uses. Run by windows/build.sh --check and
// apps/openagents-desktop/check.sh:
//   cc -std=c11 -Icoax/platform apps/openagents-desktop/windows/args-test.c -o /tmp/t && /tmp/t
#include "win32-args.h"
#include <stdio.h>

static int failed;
static void line_is(const char *const *argv, size_t argc, const char *want) {
  char out[512];
  long n = cxwin_command_line((char *const *)argv, argc, out, sizeof out);
  if (n < 0 || strcmp(out, want) != 0) { printf("FAIL: got [%s] want [%s]\n", n < 0 ? "(too long)" : out, want); failed = 1; }
}
#define ARGS(...) (const char *const[]){__VA_ARGS__}, sizeof((const char *const[]){__VA_ARGS__}) / sizeof(const char *)
int main(void) {
  // plain arguments go as they are; spaces, tabs, quotes and empty ones are quoted
  line_is(ARGS("git", "-C", "C:/r", "status"), "git -C C:/r status");
  line_is(ARGS("sh", "-c", "ls -la"), "sh -c \"ls -la\"");
  line_is(ARGS("a", ""), "a \"\"");
  line_is(ARGS("say", "a\tb"), "say \"a\tb\"");
  // a quote inside is escaped; backslashes are literal unless a quote follows them
  line_is(ARGS("x", "he said \"hi\""), "x \"he said \\\"hi\\\"\"");
  line_is(ARGS("C:\\Program Files\\Git\\cmd\\git.exe", "x"), "\"C:\\Program Files\\Git\\cmd\\git.exe\" x");
  line_is(ARGS("x", "C:\\dir with space\\"), "x \"C:\\dir with space\\\\\"");
  line_is(ARGS("x", "a\\\\\"b"), "x \"a\\\\\\\\\\\"b\"");
  line_is(ARGS("x", "C:\\no\\space\\"), "x C:\\no\\space\\");
  // a line too long for its buffer is refused, not cut
  { char out[8]; const char *a[] = {"abcdefgh"}; if (cxwin_command_line((char *const *)a, 1, out, sizeof out) != -1) { printf("FAIL: long line not refused\n"); failed = 1; } }
  // names
  if (strcmp(cxwin_leaf("/bin/sh"), "sh") || strcmp(cxwin_leaf("C:\\x\\git.exe"), "git.exe") || strcmp(cxwin_leaf("curl"), "curl")) { printf("FAIL: leaf\n"); failed = 1; }
  if (!cxwin_is_posix_root("/bin/sh") || cxwin_is_posix_root("//host/x") || cxwin_is_posix_root("C:/x")) { printf("FAIL: posix root\n"); failed = 1; }
  if (!cxwin_is_drive_path("C:/x") || !cxwin_is_drive_path("d:\\x") || cxwin_is_drive_path("C:x") || !cxwin_is_share_path("\\\\host\\s")) { printf("FAIL: drive\n"); failed = 1; }
  // variables are compared by name, case-blind
  if (cxwin_env_cmp("Path=a", "PATH=b") != 0 || cxwin_env_cmp("PATHX=1", "PATH=1") <= 0 || cxwin_env_cmp("A=1", "B=1") >= 0 || cxwin_env_cmp("=C:=C:\\", "A=1") >= 0) { printf("FAIL: env names\n"); failed = 1; }
  printf(failed ? "windows args: FAILED\n" : "windows args: ok\n");
  return failed;
}
