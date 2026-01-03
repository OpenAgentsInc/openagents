struct Params {
    head_index: u32,
    kv_heads: u32,
    head_dim: u32,
    seq_len: u32,
    window_start: u32,
    capacity: u32,
};

@group(0) @binding(0)
var<storage, read> q: array<f32>;

@group(0) @binding(1)
var<storage, read> k_cache: array<f32>;

@group(0) @binding(2)
var<storage, read> sinks: array<f32>;

@group(0) @binding(3)
var<storage, read_write> out: array<f32>;

@group(0) @binding(4)
var<uniform> params: Params;

@compute @workgroup_size(1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    if (gid.x != 0u) {
        return;
    }
    if (params.head_dim == 0u || params.kv_heads == 0u || params.seq_len == 0u || params.capacity == 0u) {
        return;
    }
    let head = params.head_index;
    let kv = head % params.kv_heads;
    let q_base = head * params.head_dim;
    let sm_scale = 1.0 / sqrt(f32(params.head_dim));
    let sink = sinks[head];

    var max_score = sink;
    var t = 0u;
    loop {
        if (t >= params.seq_len) {
            break;
        }
        let token = (params.window_start + t) % params.capacity;
        let k_base = (token * params.kv_heads + kv) * params.head_dim;
        var dot = 0.0;
        var i = 0u;
        loop {
            if (i >= params.head_dim) {
                break;
            }
            dot = dot + q[q_base + i] * k_cache[k_base + i];
            i = i + 1u;
        }
        let score = dot * sm_scale;
        if (score > max_score) {
            max_score = score;
        }
        t = t + 1u;
    }

    var denom = exp(sink - max_score);
    t = 0u;
    loop {
        if (t >= params.seq_len) {
            break;
        }
        let token = (params.window_start + t) % params.capacity;
        let k_base = (token * params.kv_heads + kv) * params.head_dim;
        var dot = 0.0;
        var i = 0u;
        loop {
            if (i >= params.head_dim) {
                break;
            }
            dot = dot + q[q_base + i] * k_cache[k_base + i];
            i = i + 1u;
        }
        let score = dot * sm_scale;
        denom = denom + exp(score - max_score);
        t = t + 1u;
    }
    if (denom <= 0.0) {
        return;
    }

    t = 0u;
    loop {
        if (t >= params.seq_len) {
            break;
        }
        let token = (params.window_start + t) % params.capacity;
        let k_base = (token * params.kv_heads + kv) * params.head_dim;
        var dot = 0.0;
        var i = 0u;
        loop {
            if (i >= params.head_dim) {
                break;
            }
            dot = dot + q[q_base + i] * k_cache[k_base + i];
            i = i + 1u;
        }
        let score = dot * sm_scale;
        out[t] = exp(score - max_score) / denom;
        t = t + 1u;
    }
}
