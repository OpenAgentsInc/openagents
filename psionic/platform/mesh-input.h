#ifndef PSIONIC_MESH_INPUT_H
#define PSIONIC_MESH_INPUT_H
#include <stdint.h>
#define MW_INPUT_MAX 262144
/* Inherited private-input observation: FIFO1, AF_UNIXsocket2, regular3,
   TTY4, other/error0. Network sockets are never classified as private. */
int64_t mw_stdin_kind(void);
/* One complete binary witness until EOF, total timeout0..60000ms. Only FIFO
   and AF_UNIX input capabilities are read. count>=0, -2 timeout, -3 error
   (including overflow/unsafeinput). Every failure clears buffered bytes.
   The caller owns the input exclusively for the call; descriptor flags are
   restored. No file output, diagnostic output or protocol decisions. */
int64_t mw_stdin_read(int64_t timeout_ms);
int64_t mw_stdin_byte(int64_t i);
int64_t mw_stdin_clear(void);
#endif
