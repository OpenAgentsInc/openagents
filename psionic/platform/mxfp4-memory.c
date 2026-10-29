/* The host observes addresses; format, admission and decoding stay in Coax. */
#include "mxfp4-memory.h"
#include "spans.h"

coax_t_MxResult psionic_mxfp4_decode(int64_t rows, int64_t columns,
        coax_v_u8 packed, coax_v_u8 scales, coax_v_f32 out) {
    bool overlap = psionic_spans_overlap(out.data, out.count, sizeof(float),
        packed.data, packed.count, sizeof(uint8_t)) ||
        psionic_spans_overlap(out.data, out.count, sizeof(float),
        scales.data, scales.count, sizeof(uint8_t));
    return mx_decode(rows, columns, packed, scales, out, overlap);
}
