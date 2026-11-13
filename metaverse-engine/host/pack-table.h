// Firm reason: 5 (a thin wrapper with no logic: C spellings over the Coax pack reader's face).
// The one pack reader for a C, Objective-C or Swift host (GitHub #13304). The reader is Coax:
// pack_table in metaverse-engine/host/pack-table.coax, whose C face the host's build compiles as
//   coax c metaverse-engine/host/pack-table-face.coax --library --header aethyr-pack-table.h > aethyr-pack-table.c
// A host hands it the pack's bytes and gets each entry's kind, place and length, every extent
// already checked against the pack's size. No host reads the header or the table itself.
#ifndef AETHYR_PACK_TABLE_H
#define AETHYR_PACK_TABLE_H
#include <stddef.h>
#include <stdint.h>
#include "aethyr-pack-table.h"

// The kinds a host sorts entries by (metaverse-engine/host/pack-table.coax PACK_CATALOG to PACK_DATA).
enum { AETHYR_PACK_CATALOG = 1, AETHYR_PACK_VERTICES = 2, AETHYR_PACK_INDICES = 3, AETHYR_PACK_LAYER = 4, AETHYR_PACK_SOUND = 5, AETHYR_PACK_DATA = 6 };

typedef coax_t_PackTable AethyrPack;

// Reads the table of the whole pack at bytes[0..<n]. 1 when it is a pack; 0 with aethyr_pack_why.
static inline int aethyr_pack_open(AethyrPack *pack, const void *bytes, size_t n) {
  return aethyr_pack_table((coax_string){(const char *)bytes, (int64_t)n}, (int64_t)n, (coax_v_t_PackTable){pack, 1}) ? 1 : 0;
}
// Why the last read was refused, not NUL-terminated: its bytes and their count.
static inline const char *aethyr_pack_why(const AethyrPack *pack, size_t *count) { *count = (size_t)pack->why.count; return pack->why.data; }
static inline int aethyr_pack_count(const AethyrPack *pack) { return (int)pack->count; }
static inline int aethyr_pack_kind(const AethyrPack *pack, int k) { return (int)pack->entries.e[k].kind; }
static inline size_t aethyr_pack_at(const AethyrPack *pack, int k) { return (size_t)pack->entries.e[k].at; }
static inline size_t aethyr_pack_bytes(const AethyrPack *pack, int k) { return (size_t)pack->entries.e[k].n; }
// An entry's name, not NUL-terminated: its bytes and their count (at most 32).
static inline const char *aethyr_pack_name(const AethyrPack *pack, int k, size_t *count) { *count = (size_t)pack->entries.e[k].name.count; return pack->entries.e[k].name.data; }
// A world cell's content in the whole pack at bytes[0..<n] (metaverse-engine/host/pack-cells.coax, #13304): its
// digest and how many byte ranges of the pack it is, or 0 ranges when the catalog has no one good
// line for the cell. key is aethyr_pack_cell_key(cx, cz). Range k is at[k], bytes[k].
typedef coax_t_PackCell AethyrPackCell;
static inline int aethyr_pack_cell_ranges(AethyrPackCell *cell, const void *bytes, size_t n, int64_t key) {
  *cell = aethyr_pack_cell((coax_string){(const char *)bytes, (int64_t)n}, key);
  return cell->ok ? (int)cell->ranges : 0;
}
static inline uint32_t aethyr_pack_cell_digest(const AethyrPackCell *cell) { return (uint32_t)cell->digest; }
static inline size_t aethyr_pack_cell_at(const AethyrPackCell *cell, int k) { return (size_t)cell->at.e[k]; }
static inline size_t aethyr_pack_cell_bytes(const AethyrPackCell *cell, int k) { return (size_t)cell->n.e[k]; }
#endif
