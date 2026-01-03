struct Params {
    n: u32,
    _pad0: u32,
    eps: f32,
    _pad1: u32,
};

@group(0) @binding(0)
var<storage, read> input: array<f32>;

@group(0) @binding(1)
var<storage, read> weight: array<f32>;

@group(0) @binding(2)
var<storage, read_write> output: array<f32>;

@group(0) @binding(3)
var<uniform> params: Params;

var<workgroup> partial: array<f32, {{WORKGROUP_SIZE}}>;

@compute @workgroup_size({{WORKGROUP_SIZE}})
fn main(@builtin(local_invocation_id) lid: vec3<u32>) {
    if (params.n == 0u) {
        return;
    }
    let idx = lid.x;
    let stride = {{WORKGROUP_SIZE}}u;
    var sum: f32 = 0.0;
    var i = idx;
    loop {
        if (i >= params.n) {
            break;
        }
        let v = input[i];
        sum = sum + v * v;
        i = i + stride;
    }
    partial[idx] = sum;
    workgroupBarrier();

    var offset = stride / 2u;
    loop {
        if (offset == 0u) {
            break;
        }
        if (idx < offset) {
            partial[idx] = partial[idx] + partial[idx + offset];
        }
        workgroupBarrier();
        offset = offset / 2u;
    }

    let rms = sqrt(partial[0] / f32(params.n) + params.eps);
    let inv = 1.0 / rms;
    i = idx;
    loop {
        if (i >= params.n) {
            break;
        }
        output[i] = input[i] * inv * weight[i];
        i = i + stride;
    }
}
