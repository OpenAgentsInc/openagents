struct Params {
    heads: u32,
    head_dim: u32,
    rope_dim: u32,
    position: u32,
    theta: f32,
    scaling_factor: f32,
    low: f32,
    high: f32,
    concentration: f32,
    use_yarn: u32,
    _pad0: u32,
    _pad1: u32,
};

@group(0) @binding(0)
var<storage, read_write> values: array<f32>;

@group(0) @binding(1)
var<uniform> params: Params;

@compute @workgroup_size({{WORKGROUP_SIZE}})
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let rope_dim = params.rope_dim;
    if (rope_dim == 0u) {
        return;
    }
    let pairs_per_head = rope_dim / 2u;
    let total_pairs = params.heads * pairs_per_head;
    let idx = gid.x;
    if (idx >= total_pairs) {
        return;
    }
    let head = idx / pairs_per_head;
    let pair = idx - head * pairs_per_head;
    let base = head * params.head_dim + pair * 2u;
    let i = pair * 2u;

    let freq = pow(params.theta, f32(i) / f32(rope_dim));
    var inv_freq = 1.0 / freq;
    if (params.use_yarn != 0u && params.high > params.low) {
        let t = f32(pair);
        let ramp = (t - params.low) / (params.high - params.low);
        let mask = 1.0 - clamp(ramp, 0.0, 1.0);
        let interp = 1.0 / (params.scaling_factor * freq);
        let extrap = 1.0 / freq;
        inv_freq = interp * (1.0 - mask) + extrap * mask;
    }

    let angle = f32(params.position) * inv_freq;
    let sin_val = sin(angle) * params.concentration;
    let cos_val = cos(angle) * params.concentration;
    let a = values[base];
    let b = values[base + 1u];
    values[base] = a * cos_val - b * sin_val;
    values[base + 1u] = a * sin_val + b * cos_val;
}
