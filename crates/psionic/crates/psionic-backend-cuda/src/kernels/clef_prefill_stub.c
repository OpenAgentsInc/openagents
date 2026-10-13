/* Stub for builds without nvcc: every Clef prefill kernel reports failure. */

int psionic_clef_dequant_q8_0_f16(const void *src, void *dst, long long blocks, void *stream) {
    (void)src;
    (void)dst;
    (void)blocks;
    (void)stream;
    return 1;
}

int psionic_clef_dequant_q4_k_f16(const void *src, void *dst, long long super_blocks, void *stream) {
    (void)src;
    (void)dst;
    (void)super_blocks;
    (void)stream;
    return 1;
}

int psionic_clef_f32_to_f16(const void *src, void *dst, long long count, void *stream) {
    (void)src;
    (void)dst;
    (void)count;
    (void)stream;
    return 1;
}

int psionic_clef_rms_norm_to_f16(const void *x, const void *w, void *out, int rows, int d, float eps, void *stream) {
    (void)x;
    (void)w;
    (void)out;
    (void)rows;
    (void)d;
    (void)eps;
    (void)stream;
    return 1;
}

int psionic_clef_rms_norm_f32(const void *x, const void *w, void *out, int rows, int d, float eps, void *stream) {
    (void)x;
    (void)w;
    (void)out;
    (void)rows;
    (void)d;
    (void)eps;
    (void)stream;
    return 1;
}

int psionic_clef_layer_norm_f32(const void *x, const void *w, const void *b, void *out, int rows, int d, float eps, void *stream) {
    (void)x;
    (void)w;
    (void)b;
    (void)out;
    (void)rows;
    (void)d;
    (void)eps;
    (void)stream;
    return 1;
}

int psionic_clef_conv1d_seq_silu(const void *in, void *state, const void *w, void *out, int n, int channels, int k, void *stream) {
    (void)in;
    (void)state;
    (void)w;
    (void)out;
    (void)n;
    (void)channels;
    (void)k;
    (void)stream;
    return 1;
}

int psionic_clef_conv1d_state_into(const void *in, const void *state, void *out, int n, int channels, int k,
                                   void *stream) {
    (void)in;
    (void)state;
    (void)out;
    (void)n;
    (void)channels;
    (void)k;
    (void)stream;
    return 1;
}

int psionic_clef_delta_prep(const void *conv, const void *alpha, const void *beta_in, const void *ssm_a, const void *ssm_dt, void *qn, void *kn, void *decay, void *beta, void *kq, int n, int key_heads, int value_heads, int dim, int conv_width, void *stream) {
    (void)conv;
    (void)alpha;
    (void)beta_in;
    (void)ssm_a;
    (void)ssm_dt;
    (void)qn;
    (void)kn;
    (void)decay;
    (void)beta;
    (void)kq;
    (void)n;
    (void)key_heads;
    (void)value_heads;
    (void)dim;
    (void)conv_width;
    (void)stream;
    return 1;
}

int psionic_clef_delta_seq(const void *qn, const void *kn, const void *conv, const void *decay, const void *beta, const void *kq, void *state, void *out, int n, int key_heads, int value_heads, int dim, int v_head_reordered, int conv_width, int value_offset, int staged, void *stream) {
    (void)qn;
    (void)kn;
    (void)conv;
    (void)decay;
    (void)beta;
    (void)kq;
    (void)state;
    (void)out;
    (void)n;
    (void)key_heads;
    (void)value_heads;
    (void)dim;
    (void)v_head_reordered;
    (void)conv_width;
    (void)value_offset;
    (void)staged;
    (void)stream;
    return 1;
}

int psionic_clef_gated_norm_to_f16(const void *o, const void *z, const void *w, void *out, int n, int heads, int dim, float eps, void *stream) {
    (void)o;
    (void)z;
    (void)w;
    (void)out;
    (void)n;
    (void)heads;
    (void)dim;
    (void)eps;
    (void)stream;
    return 1;
}

int psionic_clef_attention_prep(const void *qg, const void *k, const void *v, const void *qw, const void *kw, const void *cos_sin, void *q16, void *gate, void *kcache, void *vcache, int n, int heads, int kv_heads, int dim, int rot, int pos0, float scale, float eps, void *stream) {
    (void)qg;
    (void)k;
    (void)v;
    (void)qw;
    (void)kw;
    (void)cos_sin;
    (void)q16;
    (void)gate;
    (void)kcache;
    (void)vcache;
    (void)n;
    (void)heads;
    (void)kv_heads;
    (void)dim;
    (void)rot;
    (void)pos0;
    (void)scale;
    (void)eps;
    (void)stream;
    return 1;
}

int psionic_clef_causal_softmax_to_f16(const void *scores, void *probs, int rows, int n, int keys, int pos0, void *stream) {
    (void)scores;
    (void)probs;
    (void)rows;
    (void)n;
    (void)keys;
    (void)pos0;
    (void)stream;
    return 1;
}

int psionic_clef_softmax_rows_f32(void *scores, int rows, int keys, float scale, void *stream) {
    (void)scores;
    (void)rows;
    (void)keys;
    (void)scale;
    (void)stream;
    return 1;
}

int psionic_clef_sigmoid_gate_to_f16(const void *o, const void *gate, void *out, long long count, void *stream) {
    (void)o;
    (void)gate;
    (void)out;
    (void)count;
    (void)stream;
    return 1;
}

int psionic_clef_silu_mul_to_f16(const void *gate, const void *up, void *out, long long count, void *stream) {
    (void)gate;
    (void)up;
    (void)out;
    (void)count;
    (void)stream;
    return 1;
}

int psionic_clef_span_sums(const void *rows, const void *spans, void *sums, int span_count, int d, int first, int n, void *stream) {
    (void)rows;
    (void)spans;
    (void)sums;
    (void)span_count;
    (void)d;
    (void)first;
    (void)n;
    (void)stream;
    return 1;
}

int psionic_clef_f16_to_f32(const void *src, void *dst, long long count, int accumulate, void *stream) {
    (void)src;
    (void)dst;
    (void)count;
    (void)accumulate;
    (void)stream;
    return 1;
}

int psionic_clef_stream_create(void **stream) { *stream = 0; return 1; }
int psionic_clef_stream_destroy(void *stream) { (void)stream; return 1; }
int psionic_clef_event_create(void **event) { *event = 0; return 1; }
int psionic_clef_event_destroy(void *event) { (void)event; return 1; }
int psionic_clef_event_record(void *event, void *stream) { (void)event; (void)stream; return 1; }
int psionic_clef_stream_wait_event(void *stream, void *event) { (void)stream; (void)event; return 1; }

int psionic_clef_fused_linear(const void *x, const void *w, void *out, int n, int rows, int k, int format, int segment,
                              int accumulate, void *stream) {
    (void)x;
    (void)w;
    (void)out;
    (void)n;
    (void)rows;
    (void)k;
    (void)format;
    (void)segment;
    (void)accumulate;
    (void)stream;
    return 1;
}

int psionic_clef_flash_attention(const void *q16, const void *kcache, const void *vcache, void *out, int n, int heads,
                                 int kv_heads, int dim, int first, void *stream) {
    (void)q16;
    (void)kcache;
    (void)vcache;
    (void)out;
    (void)n;
    (void)heads;
    (void)kv_heads;
    (void)dim;
    (void)first;
    (void)stream;
    return 1;
}

int psionic_clef_linear_f32_ordered(const void *x, const void *w, void *out, int n, int m, int k, void *stream) {
    (void)x;
    (void)w;
    (void)out;
    (void)n;
    (void)m;
    (void)k;
    (void)stream;
    return 1;
}

int psionic_clef_head_side(const void *q, const void *wk, void *side, int n, int heads, int width, void *stream) {
    (void)q;
    (void)wk;
    (void)side;
    (void)n;
    (void)heads;
    (void)width;
    (void)stream;
    return 1;
}

int psionic_clef_head_context(const void *mixed, const void *wv, const void *bv, void *out, int n, int heads, int width,
                              void *stream) {
    (void)mixed;
    (void)wv;
    (void)bv;
    (void)out;
    (void)n;
    (void)heads;
    (void)width;
    (void)stream;
    return 1;
}

int psionic_clef_bias_act(void *x, const void *b, long long rows, int cols, int gelu, void *stream) {
    (void)x;
    (void)b;
    (void)rows;
    (void)cols;
    (void)gelu;
    (void)stream;
    return 1;
}

int psionic_clef_timing_event_create(void **event) {
    (void)event;
    return 1;
}

int psionic_clef_event_elapsed(void *start, void *end, float *ms) {
    (void)start;
    (void)end;
    (void)ms;
    return 1;
}

int psionic_clef_delta_block(const void *qn, const void *kn, const void *conv, const void *decay, const void *beta,
                             void *coef, void *state, void *out, int n, int key_heads, int value_heads, int dim,
                             int v_head_reordered, int conv_width, int value_offset, int variant, int snap_blocks,
                             void *snap, void *stream) {
    (void)snap_blocks;
    (void)snap;
    (void)qn;
    (void)kn;
    (void)conv;
    (void)decay;
    (void)beta;
    (void)coef;
    (void)state;
    (void)out;
    (void)n;
    (void)key_heads;
    (void)value_heads;
    (void)dim;
    (void)v_head_reordered;
    (void)conv_width;
    (void)value_offset;
    (void)variant;
    (void)stream;
    return 1;
}

int psionic_clef_embed_q4_k(const void *table, const void *ids, void *out, int n, int hidden, void *stream) {
    (void)table;
    (void)ids;
    (void)out;
    (void)n;
    (void)hidden;
    (void)stream;
    return 1;
}

int psionic_clef_fake_e4m3_rows(void *x, int rows, int k, void *stream) {
    (void)x;
    (void)rows;
    (void)k;
    (void)stream;
    return 1;
}

/* The joint head on the device (#11446). */

int psionic_clef_head_layer_norm(const void *x, const void *w, const void *b, void *out, int rows, int d, float eps,
                                 void *stream) {
    (void)x;
    (void)w;
    (void)b;
    (void)out;
    (void)rows;
    (void)d;
    (void)eps;
    (void)stream;
    return 1;
}

int psionic_clef_head_add(void *x, const void *y, long long count, void *stream) {
    (void)x;
    (void)y;
    (void)count;
    (void)stream;
    return 1;
}

int psionic_clef_head_route(const void *context, const void *lexical, const void *question, const void *owner,
                            void *routed, int options, int w, void *stream) {
    (void)context;
    (void)lexical;
    (void)question;
    (void)owner;
    (void)routed;
    (void)options;
    (void)w;
    (void)stream;
    return 1;
}

int psionic_clef_head_route_scores(const void *routed, const void *base, const void *owner, void *scores, int options,
                                   int w, void *stream) {
    (void)routed;
    (void)base;
    (void)owner;
    (void)scores;
    (void)options;
    (void)w;
    (void)stream;
    return 1;
}

int psionic_clef_head_fields(const void *routed, void *scores, const void *base, const void *global,
                             const void *types, const void *meta, const void *norm_w, const void *norm_b,
                             void *fields, int questions, int w, float eps, void *stream) {
    (void)routed;
    (void)scores;
    (void)base;
    (void)global;
    (void)types;
    (void)meta;
    (void)norm_w;
    (void)norm_b;
    (void)fields;
    (void)questions;
    (void)w;
    (void)eps;
    (void)stream;
    return 1;
}

int psionic_clef_head_self_attention(const void *q, const void *k, const void *v, void *scores, void *context,
                                     int rows, int heads, int w, float scale, void *stream) {
    (void)q;
    (void)k;
    (void)v;
    (void)scores;
    (void)context;
    (void)rows;
    (void)heads;
    (void)w;
    (void)scale;
    (void)stream;
    return 1;
}

int psionic_clef_head_features(const void *fields, const void *options, const void *owner, void *features,
                               int count, int w, void *stream) {
    (void)fields;
    (void)options;
    (void)owner;
    (void)features;
    (void)count;
    (void)w;
    (void)stream;
    return 1;
}

int psionic_clef_head_logits(const void *lexical, const void *questions, const void *global, const void *owner,
                             const void *fields, const void *options, const void *hidden, const void *scorer_out,
                             void *logits, int count, int d, int w, float scorer_out_bias, float prior_scale,
                             float joint_scale, float gate, void *stream) {
    (void)lexical;
    (void)questions;
    (void)global;
    (void)owner;
    (void)fields;
    (void)options;
    (void)hidden;
    (void)scorer_out;
    (void)logits;
    (void)count;
    (void)d;
    (void)w;
    (void)scorer_out_bias;
    (void)prior_scale;
    (void)joint_scale;
    (void)gate;
    (void)stream;
    return 1;
}
