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
     cxfs_put(b): buffers a byte; cxfs_write(mode): writes the buffered
       bytes to the path (created mode 0644; 0 and 2 replace, 1 and 3
       append) and empties the buffer either way. Modes 2 and 3 return only
       once the bytes, and a new file's folder entry, are on storage.
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
int64_t cxfs_read_at(int64_t offset, int64_t count);
int64_t cxfs_byte(int64_t i);
int64_t cxfs_put(int64_t byte);
int64_t cxfs_write(int64_t mode);   /* 0 replace, 1 append; 2, 3 the same, synced to storage */
int64_t cxfs_kind(void);
int64_t cxfs_size(void);
int64_t cxfs_mkdir(void);
int64_t cxfs_temp(void);
int64_t cxfs_name_byte(int64_t i);
int64_t cxfs_remove(void);
/* cxfs_hold: keep the pushed path as a rename's source; cxfs_rename: rename it to the pushed path,
   replacing what is there (atomic within one file system). 0 or -1. */
int64_t cxfs_hold(void);
int64_t cxfs_rename(void);
/* cxfs_mtime_ns: the pushed path's modification time in nanoseconds, or -1. */
int64_t cxfs_mtime_ns(void);
int64_t cxfs_list(void);
int64_t cxfs_entry_len(int64_t i);
int64_t cxfs_entry_byte(int64_t i, int64_t j);
int64_t cxfs_cwd(void);
int64_t cxfs_realpath(void);
#ifdef __cplusplus
}
#endif
#endif
