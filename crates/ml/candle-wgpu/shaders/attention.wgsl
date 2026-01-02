struct AttentionUniform {
    batch_size: u32,
    num_heads: u32,
    seq_len: u32,
    head_dim: u32,
    scale: f32,
    causal: u32,
    _pad0: u32,
    _pad1: u32,
};

@group(0) @binding(0) var<storage, read> Q: array<f32>;
@group(0) @binding(1) var<storage, read> K: array<f32>;
@group(0) @binding(2) var<storage, read> V: array<f32>;
@group(0) @binding(3) var<storage, read_write> output: array<f32>;
@group(0) @binding(4) var<uniform> params: AttentionUniform;

var<workgroup> scores: array<f32, 256>;
var<workgroup> shared_max: f32;
var<workgroup> shared_sum: f32;
var<workgroup> partial_max: array<f32, 256>;
var<workgroup> partial_sum: array<f32, 256>;

@compute @workgroup_size(256)
fn main(
    @builtin(global_invocation_id) global_id: vec3<u32>,
    @builtin(local_invocation_id) local_id: vec3<u32>,
    @builtin(workgroup_id) group_id: vec3<u32>,
) {
    let tid = local_id.x;
    let batch = group_id.z;
    let head = group_id.y;
    let query_pos = group_id.x;

    let H = params.num_heads;
    let S = params.seq_len;
    let D = params.head_dim;
    let scale = params.scale;
    let causal = params.causal;

    let q_base = batch * H * S * D + head * S * D + query_pos * D;

    var local_max: f32 = -3.402823e+38;
    for (var j = tid; j < S; j = j + 256u) {
        if (causal == 1u && j > query_pos) {
            scores[j % 256u] = -3.402823e+38;
            continue;
        }

        let k_base = batch * H * S * D + head * S * D + j * D;
        var score: f32 = 0.0;
        for (var d: u32 = 0u; d < D; d = d + 1u) {
            score = score + Q[q_base + d] * K[k_base + d];
        }
        score = score * scale;
        scores[j % 256u] = score;
        local_max = max(local_max, score);
    }

    workgroupBarrier();

    partial_max[tid] = local_max;
    workgroupBarrier();

    for (var s: u32 = 128u; s > 0u; s = s >> 1u) {
        if (tid < s) {
            partial_max[tid] = max(partial_max[tid], partial_max[tid + s]);
        }
        workgroupBarrier();
    }
    if (tid == 0u) {
        shared_max = partial_max[0];
    }
    workgroupBarrier();

    var local_sum: f32 = 0.0;
    for (var j = tid; j < S; j = j + 256u) {
        let exp_score = exp(scores[j % 256u] - shared_max);
        scores[j % 256u] = exp_score;
        local_sum = local_sum + exp_score;
    }

    partial_sum[tid] = local_sum;
    workgroupBarrier();

    for (var s: u32 = 128u; s > 0u; s = s >> 1u) {
        if (tid < s) {
            partial_sum[tid] = partial_sum[tid] + partial_sum[tid + s];
        }
        workgroupBarrier();
    }
    if (tid == 0u) {
        shared_sum = partial_sum[0];
    }
    workgroupBarrier();

    let inv_sum = 1.0 / shared_sum;
    for (var j = tid; j < S; j = j + 256u) {
        scores[j % 256u] = scores[j % 256u] * inv_sum;
    }
    workgroupBarrier();

    let out_base = batch * H * S * D + head * S * D + query_pos * D;
    for (var d = tid; d < D; d = d + 256u) {
        var acc: f32 = 0.0;
        for (var j: u32 = 0u; j < S; j = j + 1u) {
            let v_idx = batch * H * S * D + head * S * D + j * D + d;
            acc = acc + scores[j % 256u] * V[v_idx];
        }
        output[out_base + d] = acc;
    }
}
