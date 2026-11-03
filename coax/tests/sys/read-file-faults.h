#ifndef COAX_READ_FILE_FAULTS_H
#define COAX_READ_FILE_FAULTS_H
/* Test-only prototypes, force-included before compiling the raw adapter TU.
   Renaming the call expressions avoids Darwin's system-header asm aliases. */
/* It comes before the adapter's own feature macros, so it names the same
   ones: the first system header fixes them (glibc hides O_CLOEXEC and
   SSIZE_MAX under plain -std=c11). */
#ifndef _DARWIN_C_SOURCE
#define _DARWIN_C_SOURCE
#endif
#ifndef _DEFAULT_SOURCE
#define _DEFAULT_SOURCE
#endif
#include <stddef.h>
#include <sys/types.h>
#include <sys/stat.h>
int fr_test_open(const char *,int,...);
int fr_test_fstat(int,struct stat *);
ssize_t fr_test_read(int,void *,size_t);
int fr_test_close(int);
#endif
