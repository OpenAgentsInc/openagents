struct Params {
    n: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
    weight: f32,
    _pad3: u32,
    _pad4: u32,
    _pad5: u32,
};

@group(0) @binding(0)
var<storage, read> acc: array<f32>;

@group(0) @binding(1)
var<storage, read> input: array<f32>;

@group(0) @binding(2)
var<storage, read_write> out: array<f32>;

@group(0) @binding(3)
var<uniform> params: Params;

@compute @workgroup_size({{WORKGROUP_SIZE}})
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let idx = gid.x;
    if (idx >= params.n) {
        return;
    }
    out[idx] = acc[idx] + input[idx] * params.weight;
}
