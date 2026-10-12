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
