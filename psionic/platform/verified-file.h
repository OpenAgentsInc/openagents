/* Verified-file records for a session's mapped weights: the host's effects
   only. Coax (psionic/lib/model-io/access.coax) writes each record and
   decides whether one admits a file; the host stats the file and keeps the
   records, one per file identity, in PSIONIC_VERIFIED_DIR (default
   $XDG_CACHE_HOME/psionic/verified, else ~/.cache/psionic/verified). */
#ifndef PSIONIC_VERIFIED_FILE_H
#define PSIONIC_VERIFIED_FILE_H
#include <stdint.h>
/* Include the generated header that declares coax_t_PaFile and
   coax_t_PaStat before this one. */

typedef struct {
    coax_t_PaStat stat;
    uint8_t record[512];
    int64_t length;
    int found;
    int64_t cached_pages,total_pages; /* mincore observation before authentication */
    char path[4096];
} VfEntry;

/* Observe the descriptor which supplied f->data, never re-stat its pathname.
   Caller keeps it open through this observation and its mapping immutable. */
void vf_attach_fd(VfEntry *e, int fd, coax_t_PaFile *f);
/* Open, fstat, map and attach on the same descriptor; caller owns the mapping.
   Returns 0 on success, -1 on I/O failure. No failed result is published. */
int vf_map_file(VfEntry *e, const char *path, coax_t_PaFile *f);
/* After a session opened: keep the record Coax writes for shard `shard` of
   target `kind`, unless the attached one already is it. Returns 1 if a record
   was written, 0 if none was needed, -1 if it could not be written. */
int vf_keep(VfEntry *e, uint8_t kind, int64_t capacity, int64_t shard);

#endif
