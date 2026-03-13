int psionic_cuda_quantized_kernels_compiled(void) {
    return 0;
}

int psionic_cuda_q8_0_matvec(
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

int psionic_cuda_mxfp4_matvec(
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

int psionic_cuda_quantize_q8_1(
    const void *input,
    int rows,
    int cols,
    void *output,
    void *stream
) {
    (void)input;
    (void)rows;
    (void)cols;
    (void)output;
    (void)stream;
    return 1;
}

int psionic_cuda_q8_0_matvec_q8_1(
    const void *weights,
    int rows,
    int cols,
    int row_stride,
    const void *input_q8_1,
    const void *bias,
    void *output,
    void *stream
) {
    (void)weights;
    (void)rows;
    (void)cols;
    (void)row_stride;
    (void)input_q8_1;
    (void)bias;
    (void)output;
    (void)stream;
    return 1;
}

int psionic_cuda_mxfp4_matvec_q8_1(
    const void *weights,
    int rows,
    int cols,
    int row_stride,
    const void *input_q8_1,
    const void *bias,
    void *output,
    void *stream
) {
    (void)weights;
    (void)rows;
    (void)cols;
    (void)row_stride;
    (void)input_q8_1;
    (void)bias;
    (void)output;
    (void)stream;
    return 1;
}

int psionic_cuda_q8_0_matvec_q8_1_argmax(
    const void *weights,
    int rows,
    int cols,
    int row_stride,
    const void *input_q8_1,
    const void *bias,
    void *output,
    void *stream
) {
    (void)weights;
    (void)rows;
    (void)cols;
    (void)row_stride;
    (void)input_q8_1;
    (void)bias;
    (void)output;
    (void)stream;
    return 1;
}

int psionic_cuda_mxfp4_matvec_q8_1_argmax(
    const void *weights,
    int rows,
    int cols,
    int row_stride,
    const void *input_q8_1,
    const void *bias,
    void *output,
    void *stream
) {
    (void)weights;
    (void)rows;
    (void)cols;
    (void)row_stride;
    (void)input_q8_1;
    (void)bias;
    (void)output;
    (void)stream;
    return 1;
}

int psionic_cuda_moe_down_aggregate_q8_1_f32(
    const void *weights,
    int mode,
    int row_stride,
    int rows,
    int columns,
    const void *selected_ids,
    const void *selected_weights,
    int selected_count,
    const void *activated,
    const void *bias,
    const void *residual,
    void *output,
    void *stream
) {
    (void)weights;
    (void)mode;
    (void)row_stride;
    (void)rows;
    (void)columns;
    (void)selected_ids;
    (void)selected_weights;
    (void)selected_count;
    (void)activated;
    (void)bias;
    (void)residual;
    (void)output;
    (void)stream;
    return 1;
}

int psionic_cuda_accumulate_selected4(
    const void *input,
    const void *selected_weights,
    int selected_count,
    int rows,
    const void *residual,
    void *output,
    void *stream
) {
    (void)input;
    (void)selected_weights;
    (void)selected_count;
    (void)rows;
    (void)residual;
    (void)output;
    (void)stream;
    return 1;
}

int psionic_cuda_cast_f32_to_f16(
    const void *input,
    int element_count,
    void *output,
    void *stream
) {
    (void)input;
    (void)element_count;
    (void)output;
    (void)stream;
    return 1;
}

int psionic_cuda_gather_f16_row_to_f32(
    const void *input,
    int rows,
    int cols,
    const void *decode_params,
    void *output,
    void *stream
) {
    (void)input;
    (void)rows;
    (void)cols;
    (void)decode_params;
    (void)output;
    (void)stream;
    return 1;
}

int psionic_cuda_moe_down_project_q8_1_selected4(
    const void *weights,
    int mode,
    int row_stride,
    int rows,
    int columns,
    const void *selected_ids,
    int selected_count,
    const void *activated_q8_1,
    const void *bias,
    void *output,
    void *stream
) {
    (void)weights;
    (void)mode;
    (void)row_stride;
    (void)rows;
    (void)columns;
    (void)selected_ids;
    (void)selected_count;
    (void)activated_q8_1;
    (void)bias;
    (void)output;
    (void)stream;
    return 1;
}

int psionic_cuda_moe_gate_up_swiglu_q8_1_selected4_quantized(
    const void *weights,
    int mode,
    int row_stride,
    int rows_per_expert,
    int columns,
    int gate_rows,
    int up_rows,
    const void *selected_ids,
    int selected_count,
    const void *input_q8_1,
    const void *gate_bias,
    const void *up_bias,
    void *output_q8_1,
    void *stream
) {
    (void)weights;
    (void)mode;
    (void)row_stride;
    (void)rows_per_expert;
    (void)columns;
    (void)gate_rows;
    (void)up_rows;
    (void)selected_ids;
    (void)selected_count;
    (void)input_q8_1;
    (void)gate_bias;
    (void)up_bias;
    (void)output_q8_1;
    (void)stream;
    return 1;
}

int psionic_cuda_mxfp4_dequantize_row_to_f32(
    const void *weights,
    int rows,
    int cols,
    int row_stride,
    const void *decode_params,
    void *output,
    void *stream
) {
    (void)weights;
    (void)rows;
    (void)cols;
    (void)row_stride;
    (void)decode_params;
    (void)output;
    (void)stream;
    return 1;
}

int psionic_cuda_q8_0_dequantize_row_to_f32(
    const void *weights,
    int rows,
    int cols,
    int row_stride,
    const void *decode_params,
    void *output,
    void *stream
) {
    (void)weights;
    (void)rows;
    (void)cols;
    (void)row_stride;
    (void)decode_params;
    (void)output;
    (void)stream;
    return 1;
}

int psionic_cuda_router_topk_delayed_softmax(
    const void *logits,
    int expert_count,
    int top_k,
    void *selected_ids,
    void *selected_weights,
    void *stream
) {
    (void)logits;
    (void)expert_count;
    (void)top_k;
    (void)selected_ids;
    (void)selected_weights;
    (void)stream;
    return 1;
}

int psionic_cuda_argmax_f32(
    const void *input,
    int rows,
    int cols,
    void *output,
    void *stream
) {
    (void)input;
    (void)rows;
    (void)cols;
    (void)output;
    (void)stream;
    return 1;
}

int psionic_cuda_rms_norm(
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

int psionic_cuda_rms_norm_q8_1(
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

int psionic_cuda_add_residual_rms_norm(
    const void *input,
    const void *residual,
    const void *input_bias,
    const void *weight,
    int element_count,
    float epsilon,
    void *summed_output,
    void *normalized_output,
    void *stream
) {
    (void)input;
    (void)residual;
    (void)input_bias;
    (void)weight;
    (void)element_count;
    (void)epsilon;
    (void)summed_output;
    (void)normalized_output;
    (void)stream;
    return 1;
}

int psionic_cuda_add_residual_rms_norm_q8_1(
    const void *input,
    const void *residual,
    const void *input_bias,
    const void *weight,
    int element_count,
    float epsilon,
    void *summed_output,
    void *normalized_output,
    void *quantized_output,
    void *stream
) {
    (void)input;
    (void)residual;
    (void)input_bias;
    (void)weight;
    (void)element_count;
    (void)epsilon;
    (void)summed_output;
    (void)normalized_output;
    (void)quantized_output;
    (void)stream;
    return 1;
}

int psionic_cuda_add_f32_offset_in_place(
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

int psionic_cuda_rope_neox_in_place(
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

int psionic_cuda_attention_decode_rope_cache(
    const void *qkv,
    int query_offset,
    int key_offset,
    int value_offset,
    void *cache_keys,
    void *cache_values,
    int cache_width,
    int layer_offset,
    int past_tokens,
    int sliding_window,
    int head_count,
    int kv_head_count,
    int head_dim,
    int rotary_dim,
    int position,
    float freq_scale,
    float ext_factor,
    float corr_low,
    float corr_high,
    float theta_scale,
    const void *attention_sinks,
    void *output,
    void *stream
) {
    (void)qkv;
    (void)query_offset;
    (void)key_offset;
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
    (void)rotary_dim;
    (void)position;
    (void)freq_scale;
    (void)ext_factor;
    (void)corr_low;
    (void)corr_high;
    (void)theta_scale;
    (void)attention_sinks;
    (void)output;
    (void)stream;
    return 1;
}

int psionic_cuda_attention_decode_rope_cache_f16_kv(
    const void *qkv,
    int query_offset,
    int key_offset,
    int value_offset,
    void *cache_keys,
    void *cache_values,
    int cache_width,
    int layer_offset,
    int past_tokens,
    int sliding_window,
    int head_count,
    int kv_head_count,
    int head_dim,
    int rotary_dim,
    int position,
    float freq_scale,
    float ext_factor,
    float corr_low,
    float corr_high,
    float theta_scale,
    const void *attention_sinks,
    void *output,
    void *stream
) {
    (void)qkv;
    (void)query_offset;
    (void)key_offset;
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
    (void)rotary_dim;
    (void)position;
    (void)freq_scale;
    (void)ext_factor;
    (void)corr_low;
    (void)corr_high;
    (void)theta_scale;
    (void)attention_sinks;
    (void)output;
    (void)stream;
    return 1;
}

int psionic_cuda_attention_decode_rope_cache_f16_kv_q8_1(
    const void *qkv,
    int query_offset,
    int key_offset,
    int value_offset,
    void *cache_keys,
    void *cache_values,
    int cache_width,
    int layer_offset,
    int past_tokens,
    int sliding_window,
    int head_count,
    int kv_head_count,
    int head_dim,
    int rotary_dim,
    int position,
    float freq_scale,
    float ext_factor,
    float corr_low,
    float corr_high,
    float theta_scale,
    const void *attention_sinks,
    void *output_q8_1,
    void *stream
) {
    (void)qkv;
    (void)query_offset;
    (void)key_offset;
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
    (void)rotary_dim;
    (void)position;
    (void)freq_scale;
    (void)ext_factor;
    (void)corr_low;
    (void)corr_high;
    (void)theta_scale;
    (void)attention_sinks;
    (void)output_q8_1;
    (void)stream;
    return 1;
}

int psionic_cuda_attention_decode_rope_cache_f16_kv_graph(
    const void *qkv,
    int query_offset,
    int key_offset,
    int value_offset,
    void *cache_keys,
    void *cache_values,
    int cache_width,
    int layer_offset,
    const void *decode_params,
    int sliding_window,
    int head_count,
    int kv_head_count,
    int head_dim,
    int rotary_dim,
    float freq_scale,
    float ext_factor,
    float corr_low,
    float corr_high,
    float theta_scale,
    const void *attention_sinks,
    void *output,
    void *stream
) {
    (void)qkv;
    (void)query_offset;
    (void)key_offset;
    (void)value_offset;
    (void)cache_keys;
    (void)cache_values;
    (void)cache_width;
    (void)layer_offset;
    (void)decode_params;
    (void)sliding_window;
    (void)head_count;
    (void)kv_head_count;
    (void)head_dim;
    (void)rotary_dim;
    (void)freq_scale;
    (void)ext_factor;
    (void)corr_low;
    (void)corr_high;
    (void)theta_scale;
    (void)attention_sinks;
    (void)output;
    (void)stream;
    return 1;
}

int psionic_cuda_attention_decode_rope_cache_f16_kv_graph_q8_1(
    const void *qkv,
    int query_offset,
    int key_offset,
    int value_offset,
    void *cache_keys,
    void *cache_values,
    int cache_width,
    int layer_offset,
    const void *decode_params,
    int sliding_window,
    int head_count,
    int kv_head_count,
    int head_dim,
    int rotary_dim,
    float freq_scale,
    float ext_factor,
    float corr_low,
    float corr_high,
    float theta_scale,
    const void *attention_sinks,
    void *output_q8_1,
    void *stream
) {
    (void)qkv;
    (void)query_offset;
    (void)key_offset;
    (void)value_offset;
    (void)cache_keys;
    (void)cache_values;
    (void)cache_width;
    (void)layer_offset;
    (void)decode_params;
    (void)sliding_window;
    (void)head_count;
    (void)kv_head_count;
    (void)head_dim;
    (void)rotary_dim;
    (void)freq_scale;
    (void)ext_factor;
    (void)corr_low;
    (void)corr_high;
    (void)theta_scale;
    (void)attention_sinks;
    (void)output_q8_1;
    (void)stream;
    return 1;
}

int psionic_cuda_attention_decode(
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

int psionic_cuda_router_topk_softmax(
    const void *weights,
    const void *bias,
    const void *input,
    int expert_count,
    int input_size,
    int top_k,
    void *selected_ids,
    void *selected_weights,
    void *stream
) {
    (void)weights;
    (void)bias;
    (void)input;
    (void)expert_count;
    (void)input_size;
    (void)top_k;
    (void)selected_ids;
    (void)selected_weights;
    (void)stream;
    return 1;
}

int psionic_cuda_moe_gate_up_swiglu(
    const void *weights,
    int mode,
    int row_stride,
    int rows_per_expert,
    int columns,
    int gate_rows,
    int up_rows,
    const void *selected_ids,
    int selected_count,
    const void *input,
    const void *gate_bias,
    const void *up_bias,
    void *output,
    void *stream
) {
    (void)weights;
    (void)mode;
    (void)row_stride;
    (void)rows_per_expert;
    (void)columns;
    (void)gate_rows;
    (void)up_rows;
    (void)selected_ids;
    (void)selected_count;
    (void)input;
    (void)gate_bias;
    (void)up_bias;
    (void)output;
    (void)stream;
    return 1;
}

int psionic_cuda_moe_gate_up_swiglu_q8_1(
    const void *weights,
    int mode,
    int row_stride,
    int rows_per_expert,
    int columns,
    int gate_rows,
    int up_rows,
    const void *selected_ids,
    int selected_count,
    const void *input_q8_1,
    const void *gate_bias,
    const void *up_bias,
    void *output,
    void *stream
) {
    (void)weights;
    (void)mode;
    (void)row_stride;
    (void)rows_per_expert;
    (void)columns;
    (void)gate_rows;
    (void)up_rows;
    (void)selected_ids;
    (void)selected_count;
    (void)input_q8_1;
    (void)gate_bias;
    (void)up_bias;
    (void)output;
    (void)stream;
    return 1;
}

int psionic_cuda_moe_down_aggregate(
    const void *weights,
    int mode,
    int row_stride,
    int rows,
    int columns,
    const void *selected_ids,
    const void *selected_weights,
    int selected_count,
    const void *activated,
    const void *bias,
    const void *residual,
    void *output,
    void *stream
) {
    (void)weights;
    (void)mode;
    (void)row_stride;
    (void)rows;
    (void)columns;
    (void)selected_ids;
    (void)selected_weights;
    (void)selected_count;
    (void)activated;
    (void)bias;
    (void)residual;
    (void)output;
    (void)stream;
    return 1;
}

int psionic_cuda_moe_down_aggregate_q8_1(
    const void *weights,
    int mode,
    int row_stride,
    int rows,
    int columns,
    const void *selected_ids,
    const void *selected_weights,
    int selected_count,
    const void *activated_q8_1,
    const void *bias,
    const void *residual,
    void *output,
    void *stream
) {
    (void)weights;
    (void)mode;
    (void)row_stride;
    (void)rows;
    (void)columns;
    (void)selected_ids;
    (void)selected_weights;
    (void)selected_count;
    (void)activated_q8_1;
    (void)bias;
    (void)residual;
    (void)output;
    (void)stream;
    return 1;
}
