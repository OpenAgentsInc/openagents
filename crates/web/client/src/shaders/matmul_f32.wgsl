struct Params {
    k: u32,
    n: u32,
    _pad0: u32,
    _pad1: u32,
};

@group(0) @binding(0)
var<storage, read> weights: array<f32>;

@group(0) @binding(1)
var<storage, read> x: array<f32>;

@group(0) @binding(2)
var<storage, read_write> y: array<f32>;

@group(0) @binding(3)
var<uniform> params: Params;

@compute @workgroup_size({{WORKGROUP_SIZE}})
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let col = gid.x;
    if (col >= params.n) {
        return;
    }
    var acc: f32 = 0.0;
    var row: u32 = 0u;
    loop {
        if (row >= params.k) {
            break;
        }
        let idx = row * params.n + col;
        acc = acc + x[row] * weights[idx];
        row = row + 1u;
    }
    y[col] = acc;
}
