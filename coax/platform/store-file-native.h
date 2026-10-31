#ifndef COAX_STORE_FILE_NATIVE_H
#define COAX_STORE_FILE_NATIVE_H
#include <stdint.h>
#ifdef __cplusplus
extern "C" {
#endif
/* The verified Nostr store's append-only file (coax/lib/nostr/store_saved.coax).
   File effects only: framing, replay and every admission decision are Coax.
   The host calls cxstore_set_path before the Coax program starts. */
int cxstore_set_path(const char *path);
/* Opens (creating, mode 0600, and syncing its directory) and exclusively
   locks the file, and reads it whole: its size, or -1. */
int64_t cxstore_load(void);
/* Byte i of what cxstore_load read, or -1. */
int64_t cxstore_byte(int64_t i);
/* Buffers one byte (0..255) for the next commit: 0, or -1. */
int64_t cxstore_put(int64_t b);
/* Appends the buffered bytes at the end of the file and fsyncs: 0, or -1
   after cutting the file back to its size before the append. If that cut
   fails, every later commit fails too. The buffer is empty afterwards
   either way. */
int64_t cxstore_commit(void);
/* Cuts the file to n bytes and fsyncs: 0, or -1. */
int64_t cxstore_truncate(int64_t n);
/* General file effects for Coax tools (coax/lib/sys/fs.coax). Paths and
   every decision are Coax; these only touch the file system. Build a path
   with cxfs_path_push (bytes, at most CXFS_PATH_MAX); each call below takes
   the pushed path and clears it. Results are 0 or the size/count asked
   for, and -1 on failure.
     cxfs_read: reads the file whole; its size. cxfs_byte(i): byte i of it.
     cxfs_put(b): buffers a byte; cxfs_write(append): writes the buffered
       bytes to the path (created mode 0644, replaced unless append) and
       empties the buffer either way.
     cxfs_kind: 0 nothing there, 1 a file, 2 a directory, 3 something else.
     cxfs_size: a file's size.
     cxfs_mkdir: the directory and its parents (mode 0755).
     cxfs_temp: a new directory $TMPDIR/<path>XXXXXX (the path pushed is the
       name's prefix); the new path's length, its bytes by cxfs_name_byte.
     cxfs_remove: the file, or the directory and everything in it.
     cxfs_list: the names in a directory, sorted bytewise, without . and
       ..; their count. cxfs_entry_len(i), cxfs_entry_byte(i, j).
     cxfs_cwd: the working directory's length; bytes by cxfs_name_byte. */
#define CXFS_PATH_MAX (4096)
int64_t cxfs_path_push(int64_t byte);
int64_t cxfs_read(void);
int64_t cxfs_byte(int64_t i);
int64_t cxfs_put(int64_t byte);
int64_t cxfs_write(int64_t append);
int64_t cxfs_kind(void);
int64_t cxfs_size(void);
int64_t cxfs_mkdir(void);
int64_t cxfs_temp(void);
int64_t cxfs_name_byte(int64_t i);
int64_t cxfs_remove(void);
int64_t cxfs_list(void);
int64_t cxfs_entry_len(int64_t i);
int64_t cxfs_entry_byte(int64_t i, int64_t j);
int64_t cxfs_cwd(void);
#ifdef __cplusplus
}
#endif
#endif
