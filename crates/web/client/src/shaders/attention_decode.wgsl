struct Params {
    heads: u32,
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
var<storage, read> v_cache: array<f32>;

@group(0) @binding(3)
var<storage, read> sinks: array<f32>;

@group(0) @binding(4)
var<storage, read_write> out: array<f32>;

@group(0) @binding(5)
var<uniform> params: Params;

@compute @workgroup_size({{WORKGROUP_SIZE}})
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let head = gid.x;
    if (head >= params.heads) {
        return;
    }
    if (params.head_dim == 0u || params.kv_heads == 0u || params.seq_len == 0u || params.capacity == 0u) {
        return;
    }
    let q_base = head * params.head_dim;
    let kv = head % params.kv_heads;
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

    var i = 0u;
    loop {
        if (i >= params.head_dim) {
            break;
        }
        out[q_base + i] = 0.0;
        i = i + 1u;
    }

    t = 0u;
    loop {
        if (t >= params.seq_len) {
            break;
        }
        let token = (params.window_start + t) % params.capacity;
        let k_base = (token * params.kv_heads + kv) * params.head_dim;
        var dot = 0.0;
        var j = 0u;
        loop {
            if (j >= params.head_dim) {
                break;
            }
            dot = dot + q[q_base + j] * k_cache[k_base + j];
            j = j + 1u;
        }
        let score = dot * sm_scale;
        let weight = exp(score - max_score) / denom;
        let v_base = k_base;
        j = 0u;
        loop {
            if (j >= params.head_dim) {
                break;
            }
            out[q_base + j] = out[q_base + j] + v_cache[v_base + j] * weight;
            j = j + 1u;
        }
        t = t + 1u;
    }
}
