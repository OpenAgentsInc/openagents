#ifndef PSIONIC_DEEPSEEK_ENGRAM_MEMORY_H
#define PSIONIC_DEEPSEEK_ENGRAM_MEMORY_H
#include "deepseek-engram.h"
/* Caller owns an authenticated immutable file, matching admitted table metadata,
 * valid immutable input allocations, exclusive outputs, and the FD lifetime.
 * mapped=1 maps read-only lazily; mapped=0 uses pread. Only planned bytes are read.
 * Packet and decode work are private; failed reads never publish output words. */
coax_t_EgHash psionic_eg_hash(coax_t_EgMap,coax_t_EgHistory,coax_v_s64,coax_v_u8,coax_v_s64,coax_v_s64);
coax_t_EgResult psionic_eg_lookup(int fd,_Bool mapped,coax_t_EgTable,coax_v_s64 ids,coax_v_u16 output,coax_v_u8 packet,coax_v_u16 stage);
#endif
