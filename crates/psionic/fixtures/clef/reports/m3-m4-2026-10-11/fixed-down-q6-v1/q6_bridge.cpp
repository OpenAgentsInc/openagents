#include "ggml.h"
#include <cmath>
#include <cstddef>
#include <cstdint>

// The quantization algorithm is upstream ggml, not a local approximation.
extern "C" size_t clef_q6_quantize(const float *input, void *output,
                                  int64_t rows, int64_t columns, size_t capacity) {
    if (!input || !output || rows < 1 || rows > 64 || columns != 17408) return 0;
    const size_t expected = size_t(rows) * ggml_row_size(GGML_TYPE_Q6_K, columns);
    if (expected != size_t(rows) * 68 * 210 || capacity != expected) return 0;
    for (int64_t i = 0; i < rows * columns; ++i) {
        if (!std::isfinite(input[i])) return 0;
    }
    const size_t written = ggml_quantize_chunk(GGML_TYPE_Q6_K, input, output,
                                              0, rows, columns, nullptr);
    if (written != expected || !ggml_validate_row_data(GGML_TYPE_Q6_K, output, written)) return 0;
    return written;
}
