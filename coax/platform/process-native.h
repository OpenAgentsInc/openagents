#ifndef COAX_PROCESS_NATIVE_H
#define COAX_PROCESS_NATIVE_H
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif
/* Child processes for Coax, through #foreign scalars only. A handle is a
   small index; strings cross one byte at a time. Policy (what to run, which
   flags, which environment) is Coax; this file only performs the effects.

   Build: cxproc_new, then cxproc_push bytes into a field and cxproc_end to
   close each entry. Field 0 is an argument (the first is the program),
   1 an environment entry KEY=VALUE set on top of the inherited one, 2 the
   name of an inherited variable to remove, 3 the working directory.

   Run: cxproc_spawn starts the child in its own process group with stdin,
   stdout and stderr on pipes. cxproc_read_line returns the next stdout
   line's length (its bytes through cxproc_line_byte, valid until the next
   read), CXPROC_EOF, CXPROC_TIMEOUT, CXPROC_ERROR or CXPROC_TOO_LONG (the
   line was skipped); a timeout of 0 takes only what the pipe already holds,
   without waiting, and a negative one waits for ever. stderr is drained while reading and its last
   CXPROC_STDERR_KEEP bytes are kept. cxproc_write buffers a byte and
   cxproc_flush sends them. cxproc_kill signals the whole group;
   cxproc_wait reaps the child; cxproc_release kills, reaps and frees. */
#define CXPROC_EOF (-1)
#define CXPROC_TIMEOUT (-2)
#define CXPROC_ERROR (-3)
#define CXPROC_TOO_LONG (-4)
#define CXPROC_RUNNING (-2)
#define CXPROC_MAX (16)
#define CXPROC_MAX_LINE (64 * 1024 * 1024)
#define CXPROC_STDERR_KEEP (64 * 1024)
int64_t cxproc_new(void);
int64_t cxproc_push(int64_t h, int64_t field, int64_t byte);
int64_t cxproc_end(int64_t h, int64_t field);
int64_t cxproc_spawn(int64_t h);
int64_t cxproc_read_line(int64_t h, int64_t timeout_ms);
int64_t cxproc_line_byte(int64_t h, int64_t i);
int64_t cxproc_write(int64_t h, int64_t byte);
int64_t cxproc_flush(int64_t h);
/* Owned child-stdin pipes only. timeout_ms is 0..INT_MAX; zero makes one
   nonblocking attempt. One monotonic deadline covers all writes and waits.
   Returns 0 complete, CXPROC_TIMEOUT, or CXPROC_ERROR. Only unsent bytes stay
   buffered on timeout/error, so retry never repeats a delivered prefix.
   The original fd flags are restored; stderr stays in the existing ring. */
int64_t cxproc_flush_timeout(int64_t h, int64_t timeout_ms);
int64_t cxproc_close_stdin(int64_t h);
int64_t cxproc_kill(int64_t h, int64_t sig);
int64_t cxproc_wait(int64_t h, int64_t timeout_ms);
int64_t cxproc_pid(int64_t h);
int64_t cxproc_group_alive(int64_t pgid);
int64_t cxproc_stderr_count(int64_t h);
int64_t cxproc_stderr_byte(int64_t h, int64_t i);
int64_t cxproc_release(int64_t h);
int64_t cxproc_now_ms(void);
/* The host's own environment: cxproc_getenv_push the name's bytes, then
   cxproc_getenv returns the value's length (or -1 when unset) and
   cxproc_getenv_byte its bytes, until the next lookup. */
int64_t cxproc_getenv_push(int64_t byte);
int64_t cxproc_getenv(void);
int64_t cxproc_getenv_byte(int64_t i);
#ifdef __cplusplus
}
#endif
#endif
