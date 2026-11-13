// Firm reason: 5 (a thin wrapper with no logic: C spellings over the Coax pack reader's face).
// The one pack reader for a C, Objective-C or Swift host (GitHub #13304). The reader is Coax:
// pack_table in metaverse-engine/host/pack-table.coax, whose C face the host's build compiles as
//   coax c metaverse-engine/host/pack-table-face.coax --library --header aethyr-pack-table.h > aethyr-pack-table.c
// A host hands it the pack's bytes and gets each entry's kind, place and length, every extent
// already checked against the pack's size. No host reads the header or the table itself.
#ifndef ENGINE_PACK_TABLE_H
#define ENGINE_PACK_TABLE_H
#include <stddef.h>
#include <stdint.h>
// A host whose unit already holds the face's C (metaverse/native/realm-host.c includes the committed
// pack-table-gen.h; a host on the host core includes aethyr-core-face.h) defines this first.
#ifndef ENGINE_PACK_TABLE_PROVIDED
#include "aethyr-pack-table.h"
#endif

// The kinds a host sorts entries by (metaverse-engine/host/pack-table.coax PACK_CATALOG to PACK_CELLS).
enum { ENGINE_PACK_CATALOG = 1, ENGINE_PACK_VERTICES = 2, ENGINE_PACK_INDICES = 3, ENGINE_PACK_LAYER = 4, ENGINE_PACK_SOUND = 5, ENGINE_PACK_DATA = 6, ENGINE_PACK_RIG = 7, ENGINE_PACK_CELLS = 8 };

typedef coax_t_PackTable EnginePack;

// Reads the table of the whole pack at bytes[0..<n]. 1 when it is a pack; 0 with engine_pack_why.
static inline int engine_pack_open(EnginePack *pack, const void *bytes, size_t n) {
  return engine_pack_table((coax_string){(const char *)bytes, (int64_t)n}, (int64_t)n, (coax_v_t_PackTable){pack, 1}) ? 1 : 0;
}
// Why the last read was refused, not NUL-terminated: its bytes and their count.
static inline const char *engine_pack_why(const EnginePack *pack, size_t *count) { *count = (size_t)pack->why.count; return pack->why.data; }
static inline int engine_pack_count(const EnginePack *pack) { return (int)pack->count; }
static inline int engine_pack_kind(const EnginePack *pack, int k) { return (int)pack->entries.e[k].kind; }
static inline size_t engine_pack_at(const EnginePack *pack, int k) { return (size_t)pack->entries.e[k].at; }
static inline size_t engine_pack_bytes(const EnginePack *pack, int k) { return (size_t)pack->entries.e[k].n; }
// An entry's name, not NUL-terminated: its bytes and their count (at most 32).
static inline const char *engine_pack_name(const EnginePack *pack, int k, size_t *count) { *count = (size_t)pack->entries.e[k].name.count; return pack->entries.e[k].name.data; }
// A world cell's content in the whole pack at bytes[0..<n] (metaverse-engine/host/pack-cells.coax, #13304): its
// digest and how many byte ranges of the pack it is, or 0 ranges when the catalog has no one good
// line for the cell. key is engine_pack_cell_key(cx, cz). Range k is at[k], bytes[k].
typedef coax_t_PackCell EnginePackCell;
static inline int engine_pack_cell_ranges(EnginePackCell *cell, const void *bytes, size_t n, int64_t key) {
  *cell = engine_pack_cell((coax_string){(const char *)bytes, (int64_t)n}, key);
  return cell->ok ? (int)cell->ranges : 0;
}
static inline uint32_t engine_pack_cell_digest(const EnginePackCell *cell) { return (uint32_t)cell->digest; }
static inline size_t engine_pack_cell_at(const EnginePackCell *cell, int k) { return (size_t)cell->at.e[k]; }
static inline size_t engine_pack_cell_bytes(const EnginePackCell *cell, int k) { return (size_t)cell->n.e[k]; }
#endif
