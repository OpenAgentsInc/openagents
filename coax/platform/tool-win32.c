/* The host of a Coax command-line tool on Windows (GitHub #13782): tool-native.h's contract over
   Win32, as tool-native.c is on POSIX (which stays the POSIX build's). argv comes from the UTF-16
   command line as UTF-8; tool_main runs on a thread with COAX_TOOL_STACK of reserved stack
   (committed as it is touched); stdin and stdout are binary (a worker's JSON lines cross pipes
   byte for byte). cxtool_os() is 3 on Windows. Full-screen terminal tools are not served:
   cxterm_raw, cxterm_keys and cxterm_size refuse, while cxterm_byte and cxterm_write move bytes
   on stdin and stdout, which is what a worker on pipes needs.

   The generated C of some tools has fields named stdout and stderr, which MinGW's stdio.h makes
   macros. Such a tool is compiled with -include win32-stdio.h, which turns the three names into
   the variables this file defines and fills before main. */
#include "win32-native.h"
#include "win32-stdio.h"
#include "tool-native.h"
#include <fcntl.h>
#include <io.h>
#include <shellapi.h>

FILE *stdin, *stdout, *stderr;
__attribute__((constructor)) static void cxtool_stdio(void) {
  stdin = __acrt_iob_func(0); stdout = __acrt_iob_func(1); stderr = __acrt_iob_func(2);
}

static int g_argc;
static char **g_argv;
static int64_t g_status;

int64_t cxargs_count(void) { return g_argc; }
int64_t cxargs_len(int64_t i) { return i < 0 || i >= g_argc ? -1 : (int64_t)strlen(g_argv[i]); }
int64_t cxargs_byte(int64_t i, int64_t j) {
  if (i < 0 || i >= g_argc || j < 0 || (size_t)j >= strlen(g_argv[i])) return -1;
  return (unsigned char)g_argv[i][j];
}
int64_t cxtool_err(int64_t byte) { if (byte < 0 || byte > 255) return -1; return fputc((int)byte, stderr) == EOF ? -1 : 0; }
int64_t cxtool_err_flush(void) { return fflush(stderr) == 0 ? 0 : -1; }
int64_t cxtool_out_flush(void) { return fflush(stdout) == 0 ? 0 : -1; }
int64_t cxtool_os(void) { return 3; }

int64_t cxterm_raw(int64_t on) { return on ? -1 : 0; }
int64_t cxterm_keys(int64_t on) { return on ? -1 : 0; }
int64_t cxterm_size(void) { return -1; }
/* One byte of stdin within timeout_ms: the byte, -1 for none, -2 at its end. A pipe is peeked
   (it cannot be waited on with a timeout); a console or file is read as it comes. */
int64_t cxterm_byte(int64_t timeout_ms) {
  HANDLE in = GetStdHandle(STD_INPUT_HANDLE);
  if (!in || in == INVALID_HANDLE_VALUE) return -2;
  if (GetFileType(in) == FILE_TYPE_PIPE) {
    int64_t deadline = timeout_ms < 0 ? -1 : (int64_t)GetTickCount64() + timeout_ms;
    for (;;) {
      DWORD avail = 0;
      if (!PeekNamedPipe(in, NULL, 0, NULL, &avail, NULL)) return -2;
      if (avail) break;
      if (deadline >= 0 && (int64_t)GetTickCount64() >= deadline) return -1;
      Sleep(1);
    }
  } else if (GetFileType(in) == FILE_TYPE_CHAR) {
    if (WaitForSingleObject(in, timeout_ms < 0 ? INFINITE : (DWORD)timeout_ms) != WAIT_OBJECT_0) return -1;
  }
  unsigned char c; DWORD got = 0;
  if (!ReadFile(in, &c, 1, &got, NULL) || got == 0) return -2;
  return c;
}
int64_t cxterm_write(coax_v_u8 s) {
  fflush(stdout);
  HANDLE out = GetStdHandle(STD_OUTPUT_HANDLE);
  int64_t done = 0;
  while (done < s.count) {
    DWORD n = 0;
    if (!WriteFile(out, s.data + done, (DWORD)(s.count - done), &n, NULL) || !n) return -1;
    done += n;
  }
  return 0;
}
static volatile LONG g_stop;
static BOOL WINAPI on_console(DWORD kind) { g_stop = kind == CTRL_C_EVENT ? 2 : 15; return TRUE; }
int64_t cxterm_watch(void) { return SetConsoleCtrlHandler(on_console, TRUE) ? 0 : -1; }
int64_t cxterm_stopped(void) { return g_stop; }
int64_t cxterm_resized(void) { return 0; }

static DWORD WINAPI run(LPVOID unused) { (void)unused; g_status = tool_main(); return 0; }

int main(void) {
  int wargc = 0;
  wchar_t **wargv = CommandLineToArgvW(GetCommandLineW(), &wargc);
  g_argv = calloc((size_t)(wargc > 0 ? wargc : 1) + 1, sizeof *g_argv);
  for (int i = 0; wargv && g_argv && i < wargc; i++) {
    char u[32768 * 3];
    if (cxwin_utf8_into(wargv[i], u, sizeof u) < 0) u[0] = 0;
    g_argv[g_argc++] = _strdup(u);
  }
  if (wargv) LocalFree(wargv);
  _setmode(_fileno(stdin), _O_BINARY);
  _setmode(_fileno(stdout), _O_BINARY);
  HANDLE thread = CreateThread(NULL, COAX_TOOL_STACK, run, NULL, STACK_SIZE_PARAM_IS_A_RESERVATION, NULL);
  if (!thread) { fputs("coax tool: could not start the program thread\n", stderr); return 70; }
  WaitForSingleObject(thread, INFINITE);
  CloseHandle(thread);
  fflush(stdout); fflush(stderr);
  return g_status < 0 || g_status > 255 ? 1 : (int)g_status;
}
