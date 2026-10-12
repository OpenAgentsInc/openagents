/* Stub for builds without the sm_90a Hopper projection kernels
 * (clef_hopper.cu): no device is supported and every launch fails. */

int psionic_clef_hopper_supported(void) { return 0; }

int psionic_clef_hopper_linear(const void *x, const void *w, void *out, int n, int rows, int k, int format,
                               int accumulate, int token_tile, void *stream) {
    (void)x;
    (void)w;
    (void)out;
    (void)n;
    (void)rows;
    (void)k;
    (void)format;
    (void)accumulate;
    (void)token_tile;
    (void)stream;
    return 1;
}

int psionic_clef_hopper_linear_silu(const void *x, const void *w, const void *gate, void *out16, int n, int rows,
                                    int k, int format, int token_tile, void *stream) {
    (void)x;
    (void)w;
    (void)gate;
    (void)out16;
    (void)n;
    (void)rows;
    (void)k;
    (void)format;
    (void)token_tile;
    (void)stream;
    return 1;
}

int psionic_clef_hopper_attention(const void *q16, const void *kcache, const void *vcache, void *out, const void *gate,
                                  void *out16, int n, int heads, int kv_heads, int dim, int first, void *stream) {
    (void)q16;
    (void)kcache;
    (void)vcache;
    (void)out;
    (void)gate;
    (void)out16;
    (void)n;
    (void)heads;
    (void)kv_heads;
    (void)dim;
    (void)first;
    (void)stream;
    return 1;
}
