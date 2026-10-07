int psionic_cuda_quantized_kernels_compiled(void) {
    return 0;
}

#define PSIONIC_DEFINE_SUPER_BLOCK_STUBS(NAME)                                                   \
int psionic_cuda_##NAME##_matvec(                                                                \
    const void *weights, int rows, int cols, int row_stride,                                    \
    const void *input, void *output, void *stream                                                \
) {                                                                                              \
    (void)weights; (void)rows; (void)cols; (void)row_stride;                                    \
    (void)input; (void)output; (void)stream; return 1;                                           \
}                                                                                                \
int psionic_cuda_##NAME##_dequantize_row_to_f32(                                                 \
    const void *weights, int rows, int cols, int row_stride,                                    \
    const void *decode_params, void *output, void *stream                                        \
) {                                                                                              \
    (void)weights; (void)rows; (void)cols; (void)row_stride;                                    \
    (void)decode_params; (void)output; (void)stream; return 1;                                   \
}                                                                                                \
int psionic_cuda_##NAME##_matvec_q8_1(                                                           \
    const void *weights, int rows, int cols, int row_stride,                                    \
    const void *input_q8_1, const void *bias, void *output, void *stream                         \
) {                                                                                              \
    (void)weights; (void)rows; (void)cols; (void)row_stride;                                    \
    (void)input_q8_1; (void)bias; (void)output; (void)stream; return 1;                          \
}                                                                                                \
int psionic_cuda_##NAME##_matvec_q8_1_argmax(                                                    \
    const void *weights, int rows, int cols, int row_stride,                                    \
    const void *input_q8_1, const void *bias, void *output, void *stream                         \
) {                                                                                              \
    (void)weights; (void)rows; (void)cols; (void)row_stride;                                    \
    (void)input_q8_1; (void)bias; (void)output; (void)stream; return 1;                          \
}

PSIONIC_DEFINE_SUPER_BLOCK_STUBS(q3_k)
PSIONIC_DEFINE_SUPER_BLOCK_STUBS(q4_k)
PSIONIC_DEFINE_SUPER_BLOCK_STUBS(q5_k)
PSIONIC_DEFINE_SUPER_BLOCK_STUBS(q6_k)
PSIONIC_DEFINE_SUPER_BLOCK_STUBS(iq3_s)
PSIONIC_DEFINE_SUPER_BLOCK_STUBS(iq4_xs)

#undef PSIONIC_DEFINE_SUPER_BLOCK_STUBS

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

int psionic_cuda_split_interleaved_query_gate_f32(
    const void *input,
    int head_count,
    int head_dim,
    void *query_output,
    void *gate_output,
    void *stream
) {
    (void)input;
    (void)head_count;
    (void)head_dim;
    (void)query_output;
    (void)gate_output;
    (void)stream;
    return 1;
}

int psionic_cuda_split_interleaved_query_gate_rms_norm_f32(
    const void *input,
    int head_count,
    int head_dim,
    const void *weight,
    float epsilon,
    void *query_output,
    void *gate_output,
    void *stream
) {
    (void)input;
    (void)head_count;
    (void)head_dim;
    (void)weight;
    (void)epsilon;
    (void)query_output;
    (void)gate_output;
    (void)stream;
    return 1;
}

int psionic_cuda_pack_qwen35_key_value_rms_norm_f32(
    const void *input,
    int key_offset,
    int value_offset,
    int kv_head_count,
    int head_dim,
    const void *weight,
    float epsilon,
    void *output,
    int output_key_offset,
    int output_value_offset,
    void *stream
) {
    (void)input;
    (void)key_offset;
    (void)value_offset;
    (void)kv_head_count;
    (void)head_dim;
    (void)weight;
    (void)epsilon;
    (void)output;
    (void)output_key_offset;
    (void)output_value_offset;
    (void)stream;
    return 1;
}

int psionic_cuda_pack_qwen35_hybrid_qkv_rms_norm_f32(
    const void *input,
    int q_offset,
    int k_offset,
    int v_offset,
    int group_count,
    int state_size,
    int v_size,
    const void *q_weight,
    const void *k_weight,
    float epsilon,
    void *output,
    int output_q_offset,
    int output_k_offset,
    int output_v_offset,
    void *stream
) {
    (void)input;
    (void)q_offset;
    (void)k_offset;
    (void)v_offset;
    (void)group_count;
    (void)state_size;
    (void)v_size;
    (void)q_weight;
    (void)k_weight;
    (void)epsilon;
    (void)output;
    (void)output_q_offset;
    (void)output_k_offset;
    (void)output_v_offset;
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

int psionic_cuda_top_k_f32(
    const void *input,
    int rows,
    int cols,
    int top_k,
    void *selected_indices,
    void *selected_values,
    void *stream
) {
    (void)input;
    (void)rows;
    (void)cols;
    (void)top_k;
    (void)selected_indices;
    (void)selected_values;
    (void)stream;
    return 1;
}

int psionic_cuda_top_k_f32_one_row_radix_sort_temp_storage_bytes(
    int cols,
    unsigned long *temp_storage_bytes
) {
    (void)cols;
    (void)temp_storage_bytes;
    return 1;
}

int psionic_cuda_top_k_f32_one_row_radix_sort(
    const void *input,
    int cols,
    int top_k,
    const void *input_indices,
    void *sorted_values,
    void *sorted_indices,
    void *temp_storage,
    unsigned long temp_storage_bytes,
    void *selected_indices,
    void *selected_values,
    void *stream
) {
    (void)input;
    (void)cols;
    (void)top_k;
    (void)input_indices;
    (void)sorted_values;
    (void)sorted_indices;
    (void)temp_storage;
    (void)temp_storage_bytes;
    (void)selected_indices;
    (void)selected_values;
    (void)stream;
    return 1;
}

int psionic_cuda_rms_norm(
    const void *input,
    const void *weight,
    int element_count,
    int feature_count,
    float epsilon,
    void *output,
    void *stream
) {
    (void)input;
    (void)weight;
    (void)element_count;
    (void)feature_count;
    (void)epsilon;
    (void)output;
    (void)stream;
    return 1;
}

int psionic_cuda_rms_norm_region(
    const void *input,
    int input_offset,
    const void *weight,
    int element_count,
    int feature_count,
    float epsilon,
    void *output,
    int output_offset,
    void *stream
) {
    (void)input;
    (void)input_offset;
    (void)weight;
    (void)element_count;
    (void)feature_count;
    (void)epsilon;
    (void)output;
    (void)output_offset;
    (void)stream;
    return 1;
}

int psionic_cuda_parameter_golf_projection_loss(
    const void *logits,
    const void *target_ids,
    int row_count,
    int vocab_size,
    float logit_softcap,
    void *output,
    void *stream
) {
    (void)logits;
    (void)target_ids;
    (void)row_count;
    (void)vocab_size;
    (void)logit_softcap;
    (void)output;
    (void)stream;
    return 1;
}

int psionic_cuda_parameter_golf_token_embedding_lookup(
    const void *token_ids,
    const void *token_embedding,
    int row_count,
    int vocab_size,
    int width,
    void *output,
    void *stream
) {
    (void)token_ids;
    (void)token_embedding;
    (void)row_count;
    (void)vocab_size;
    (void)width;
    (void)output;
    (void)stream;
    return 1;
}

int psionic_cuda_parameter_golf_token_embedding_lookup_backward(
    const void *token_ids,
    const void *grad_output,
    int row_count,
    int vocab_size,
    int width,
    void *output,
    void *stream
) {
    (void)token_ids;
    (void)grad_output;
    (void)row_count;
    (void)vocab_size;
    (void)width;
    (void)output;
    (void)stream;
    return 1;
}

int psionic_cuda_parameter_golf_projection_loss_backward(
    const void *logits,
    const void *target_ids,
    const void *grad_output,
    int row_count,
    int vocab_size,
    float logit_softcap,
    void *output,
    void *stream
) {
    (void)logits;
    (void)target_ids;
    (void)grad_output;
    (void)row_count;
    (void)vocab_size;
    (void)logit_softcap;
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

int psionic_cuda_rms_norm_input_backward(
    const void *input,
    const void *weight,
    const void *grad_output,
    int element_count,
    int feature_count,
    float epsilon,
    void *output,
    void *stream
) {
    (void)input;
    (void)weight;
    (void)grad_output;
    (void)element_count;
    (void)feature_count;
    (void)epsilon;
    (void)output;
    (void)stream;
    return 1;
}

int psionic_cuda_rms_norm_weight_backward(
    const void *input,
    const void *grad_output,
    int element_count,
    int feature_count,
    float epsilon,
    void *output,
    void *stream
) {
    (void)input;
    (void)grad_output;
    (void)element_count;
    (void)feature_count;
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

int psionic_cuda_mul_f32(
    const void *left,
    const void *right,
    int element_count,
    void *output,
    void *stream
) {
    (void)left;
    (void)right;
    (void)element_count;
    (void)output;
    (void)stream;
    return 1;
}

int psionic_cuda_silu_mul_f32(
    const void *activation_input,
    int activation_offset,
    const void *rhs,
    int rhs_offset,
    int element_count,
    void *output,
    void *stream
) {
    (void)activation_input;
    (void)activation_offset;
    (void)rhs;
    (void)rhs_offset;
    (void)element_count;
    (void)output;
    (void)stream;
    return 1;
}

int psionic_cuda_silu_mul_q8_1(
    const void *activation_input,
    int activation_offset,
    const void *rhs,
    int rhs_offset,
    int element_count,
    void *output_q8_1,
    void *stream
) {
    (void)activation_input;
    (void)activation_offset;
    (void)rhs;
    (void)rhs_offset;
    (void)element_count;
    (void)output_q8_1;
    (void)stream;
    return 1;
}

int psionic_cuda_sigmoid_mul_f32(
    const void *values,
    int values_offset,
    const void *gate,
    int gate_offset,
    int element_count,
    void *output,
    void *stream
) {
    (void)values;
    (void)values_offset;
    (void)gate;
    (void)gate_offset;
    (void)element_count;
    (void)output;
    (void)stream;
    return 1;
}

int psionic_cuda_sigmoid_mul_q8_1(
    const void *values,
    int values_offset,
    const void *gate,
    int gate_offset,
    int element_count,
    void *output_q8_1,
    void *stream
) {
    (void)values;
    (void)values_offset;
    (void)gate;
    (void)gate_offset;
    (void)element_count;
    (void)output_q8_1;
    (void)stream;
    return 1;
}

int psionic_cuda_depthwise_causal_conv1d_step_f32(
    const void *input,
    void *state,
    const void *weights,
    int channels,
    int kernel_size,
    void *output,
    void *stream
) {
    (void)input;
    (void)state;
    (void)weights;
    (void)channels;
    (void)kernel_size;
    (void)output;
    (void)stream;
    return 1;
}

int psionic_cuda_depthwise_causal_conv1d_step_silu_f32(
    const void *input,
    void *state,
    const void *weights,
    int channels,
    int kernel_size,
    void *output,
    void *stream
) {
    (void)input;
    (void)state;
    (void)weights;
    (void)channels;
    (void)kernel_size;
    (void)output;
    (void)stream;
    return 1;
}

int psionic_cuda_qwen35_ssm_decay_beta_f32(
    const void *input,
    int alpha_offset,
    int beta_offset,
    const void *ssm_a,
    const void *ssm_dt,
    int element_count,
    void *decay_output,
    void *beta_output,
    void *stream
) {
    (void)input;
    (void)alpha_offset;
    (void)beta_offset;
    (void)ssm_a;
    (void)ssm_dt;
    (void)element_count;
    (void)decay_output;
    (void)beta_output;
    (void)stream;
    return 1;
}

int psionic_cuda_gated_delta_step_f32(
    const void *qkv,
    int query_offset,
    int key_offset,
    int value_offset,
    const void *decay,
    const void *beta,
    void *state,
    int key_head_count,
    int value_head_count,
    int key_dim,
    int value_dim,
    void *output,
    void *stream
) {
    (void)qkv;
    (void)query_offset;
    (void)key_offset;
    (void)value_offset;
    (void)decay;
    (void)beta;
    (void)state;
    (void)key_head_count;
    (void)value_head_count;
    (void)key_dim;
    (void)value_dim;
    (void)output;
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

int psionic_cuda_rotary_embedding_backward(
    const void *grad_output,
    const void *cos,
    const void *sin,
    int batch_size,
    int head_count,
    int sequence_length,
    int head_dim,
    int batched_tables,
    void *grad_input,
    void *stream
) {
    (void)grad_output;
    (void)cos;
    (void)sin;
    (void)batch_size;
    (void)head_count;
    (void)sequence_length;
    (void)head_dim;
    (void)batched_tables;
    (void)grad_input;
    (void)stream;
    return 1;
}

int psionic_cuda_permute_rank2_transpose_f32(
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

int psionic_cuda_permute_rank4_swap_middle_axes_f32(
    const void *input,
    int dim0,
    int dim1,
    int dim2,
    int dim3,
    void *output,
    void *stream
) {
    (void)input;
    (void)dim0;
    (void)dim1;
    (void)dim2;
    (void)dim3;
    (void)output;
    (void)stream;
    return 1;
}

int psionic_cuda_reduce_sum_rows_f32(
    const void *input,
    int row_count,
    int column_count,
    void *output,
    void *stream
) {
    (void)input;
    (void)row_count;
    (void)column_count;
    (void)output;
    (void)stream;
    return 1;
}

int psionic_cuda_reduce_sum_axis0_f32(
    const void *input,
    int axis0_extent,
    int row_width,
    void *output,
    void *stream
) {
    (void)input;
    (void)axis0_extent;
    (void)row_width;
    (void)output;
    (void)stream;
    return 1;
}

int psionic_cuda_expand_rank3_f32(
    const void *input,
    int input_dim0,
    int input_dim1,
    int input_dim2,
    int output_dim0,
    int output_dim1,
    int output_dim2,
    void *output,
    void *stream
) {
    (void)input;
    (void)input_dim0;
    (void)input_dim1;
    (void)input_dim2;
    (void)output_dim0;
    (void)output_dim1;
    (void)output_dim2;
    (void)output;
    (void)stream;
    return 1;
}

int psionic_cuda_expand_rank4_f32(
    const void *input,
    int input_dim0,
    int input_dim1,
    int input_dim2,
    int input_dim3,
    int output_dim0,
    int output_dim1,
    int output_dim2,
    int output_dim3,
    void *output,
    void *stream
) {
    (void)input;
    (void)input_dim0;
    (void)input_dim1;
    (void)input_dim2;
    (void)input_dim3;
    (void)output_dim0;
    (void)output_dim1;
    (void)output_dim2;
    (void)output_dim3;
    (void)output;
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

int psionic_cuda_attention_decode_rope_cache_turboquant_kv(
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

int psionic_cuda_attention_decode_rope_cache_f16_kv_graph_fa3(
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
    void *partial_output,
    void *partial_max,
    void *partial_sum,
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
    (void)partial_output;
    (void)partial_max;
    (void)partial_sum;
    (void)output;
    (void)stream;
    return 1;
}

int psionic_cuda_attention_decode_rope_cache_turboquant_kv_graph(
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

int psionic_cuda_cast_f32_to_bf16(
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

int psionic_cuda_cast_bf16_to_f32(
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

int psionic_cuda_parameter_golf_projection_token_losses(
    const void *logits,
    const void *target_ids,
    int row_count,
    int vocab_size,
    float logit_softcap,
    void *output,
    void *stream
) {
    (void)logits;
    (void)target_ids;
    (void)row_count;
    (void)vocab_size;
    (void)logit_softcap;
    (void)output;
    (void)stream;
    return 1;
}

int psionic_cuda_parameter_golf_token_embedding_lookup_bf16_to_f32(
    const void *token_ids,
    const void *token_embedding,
    int row_count,
    int vocab_size,
    int width,
    void *output,
    void *stream
) {
    (void)token_ids;
    (void)token_embedding;
    (void)row_count;
    (void)vocab_size;
    (void)width;
    (void)output;
    (void)stream;
    return 1;
}

int psionic_cuda_reduce_sum_axis1_rank3_f32(
    const void *input,
    int dim0,
    int dim1,
    int dim2,
    void *output,
    void *stream
) {
    (void)input;
    (void)dim0;
    (void)dim1;
    (void)dim2;
    (void)output;
    (void)stream;
    return 1;
}

int psionic_cuda_attention_causal_sequence_f32(
    const void *query,
    const void *key,
    const void *value,
    int batch_size,
    int head_count,
    int kv_head_count,
    int sequence_length,
    int head_dim,
    void *output,
    void *stream
) {
    (void)query;
    (void)key;
    (void)value;
    (void)batch_size;
    (void)head_count;
    (void)kv_head_count;
    (void)sequence_length;
    (void)head_dim;
    (void)output;
    (void)stream;
    return 1;
}

int psionic_cuda_attention_causal_row_softmax_in_place_f32(
    void *logits,
    int row_count,
    int sequence_length,
    float scale,
    void *stream
) {
    (void)logits;
    (void)row_count;
    (void)sequence_length;
    (void)scale;
    (void)stream;
    return 1;
}

int psionic_cuda_attention_causal_row_softmax_backward_in_place_f32(
    const void *probabilities,
    void *grad_probabilities,
    int row_count,
    int sequence_length,
    float post_scale,
    void *stream
) {
    (void)probabilities;
    (void)grad_probabilities;
    (void)row_count;
    (void)sequence_length;
    (void)post_scale;
    (void)stream;
    return 1;
}

int psionic_cuda_attention_causal_sequence_backward_f32(
    const void *query,
    const void *key,
    const void *value,
    const void *grad_output,
    int batch_size,
    int head_count,
    int kv_head_count,
    int sequence_length,
    int head_dim,
    void *query_gradient,
    void *key_gradient,
    void *value_gradient,
    void *stream
) {
    (void)query;
    (void)key;
    (void)value;
    (void)grad_output;
    (void)batch_size;
    (void)head_count;
    (void)kv_head_count;
    (void)sequence_length;
    (void)head_dim;
    (void)query_gradient;
    (void)key_gradient;
    (void)value_gradient;
    (void)stream;
    return 1;
}
