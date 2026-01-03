struct Params {
    n: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
};

@group(0) @binding(0)
var<storage, read> gate: array<f32>;

@group(0) @binding(1)
var<storage, read> up: array<f32>;

@group(0) @binding(2)
var<storage, read_write> out: array<f32>;

@group(0) @binding(3)
var<uniform> params: Params;

const SWIGLU_ALPHA: f32 = 1.702;
const SWIGLU_LIMIT: f32 = 7.0;

@compute @workgroup_size({{WORKGROUP_SIZE}})
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let idx = gid.x;
    if (idx >= params.n) {
        return;
    }
    let g = gate[idx];
    let u = up[idx];
    let g_clamped = min(g, SWIGLU_LIMIT);
    let u_clamped = clamp(u, -SWIGLU_LIMIT, SWIGLU_LIMIT);
    let sigmoid = 1.0 / (1.0 + exp(-SWIGLU_ALPHA * g_clamped));
    let glu = g_clamped * sigmoid;
    out[idx] = glu * (u_clamped + 1.0);
}
