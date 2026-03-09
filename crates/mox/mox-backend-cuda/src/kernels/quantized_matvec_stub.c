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

int mox_cuda_rms_norm(
    const void *input,
    const void *weight,
    int element_count,
    float epsilon,
    void *output,
    void *stream
) {
    (void)input;
    (void)weight;
    (void)element_count;
    (void)epsilon;
    (void)output;
    (void)stream;
    return 1;
}

int mox_cuda_add_f32_offset_in_place(
    void *destination,
    int element_offset,
    const void *rhs,
    int element_count,
    void *stream
) {
    (void)destination;
    (void)element_offset;
    (void)rhs;
    (void)element_count;
    (void)stream;
    return 1;
}

int mox_cuda_rope_neox_in_place(
    void *values,
    int element_offset,
    int head_count,
    int head_dim,
    int rotary_dim,
    int position,
    float freq_scale,
    float ext_factor,
    float corr_low,
    float corr_high,
    float theta_scale,
    void *stream
) {
    (void)values;
    (void)element_offset;
    (void)head_count;
    (void)head_dim;
    (void)rotary_dim;
    (void)position;
    (void)freq_scale;
    (void)ext_factor;
    (void)corr_low;
    (void)corr_high;
    (void)theta_scale;
    (void)stream;
    return 1;
}

int mox_cuda_attention_decode(
    const void *query,
    int query_offset,
    const void *current_key,
    int key_offset,
    const void *current_value,
    int value_offset,
    const void *cache_keys,
    const void *cache_values,
    int cache_width,
    int layer_offset,
    int past_tokens,
    int sliding_window,
    int head_count,
    int kv_head_count,
    int head_dim,
    const void *attention_sinks,
    void *output,
    void *stream
) {
    (void)query;
    (void)query_offset;
    (void)current_key;
    (void)key_offset;
    (void)current_value;
    (void)value_offset;
    (void)cache_keys;
    (void)cache_values;
    (void)cache_width;
    (void)layer_offset;
    (void)past_tokens;
    (void)sliding_window;
    (void)head_count;
    (void)kv_head_count;
    (void)head_dim;
    (void)attention_sinks;
    (void)output;
    (void)stream;
    return 1;
}
