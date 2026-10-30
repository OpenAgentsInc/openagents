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
    char path[4096];
} VfEntry;

/* Stat path (the file already mapped into f->data) and attach the record kept
   for that identity, if any, to f. Without a stat or a record, f keeps an
   empty record and the session hashes the file in full. */
void vf_attach(VfEntry *e, const char *path, coax_t_PaFile *f);
/* After a session opened: keep the record Coax writes for shard `shard` of
   target `kind`, unless the attached one already is it. Returns 1 if a record
   was written, 0 if none was needed, -1 if it could not be written. */
int vf_keep(VfEntry *e, uint8_t kind, int64_t capacity, int64_t shard);

#endif
