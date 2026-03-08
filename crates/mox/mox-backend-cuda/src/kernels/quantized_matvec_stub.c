int mox_cuda_quantized_kernels_compiled(void) {
    return 0;
}

int mox_cuda_q8_0_matvec(
    const void *weights,
    int rows,
    int cols,
    int row_stride,
    const void *input,
    void *output,
    void *stream
) {
    (void)weights;
    (void)rows;
    (void)cols;
    (void)row_stride;
    (void)input;
    (void)output;
    (void)stream;
    return 1;
}

int mox_cuda_mxfp4_matvec(
    const void *weights,
    int rows,
    int cols,
    int row_stride,
    const void *input,
    void *output,
    void *stream
) {
    (void)weights;
    (void)rows;
    (void)cols;
    (void)row_stride;
    (void)input;
    (void)output;
    (void)stream;
    return 1;
}
