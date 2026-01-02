struct Params {
    len: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
};

@group(0) @binding(0) var<storage, read> input: array<f32>;
@group(0) @binding(1) var<storage, read_write> output: array<f32>;
@group(0) @binding(2) var<uniform> params: Params;

var<workgroup> scratch: array<f32, 256>;

@compute @workgroup_size(256)
fn sum(@builtin(local_invocation_id) lid: vec3<u32>) {
    let idx = lid.x;
    var value: f32 = 0.0;
    if (idx < params.len) {
        value = input[idx];
    }
    scratch[idx] = value;
    workgroupBarrier();

    for (var s: u32 = 128u; s > 0u; s = s >> 1u) {
        if (idx < s) {
            scratch[idx] = scratch[idx] + scratch[idx + s];
        }
        workgroupBarrier();
    }

    if (idx == 0u) {
        output[0] = scratch[0];
    }
}

@compute @workgroup_size(256)
fn max(@builtin(local_invocation_id) lid: vec3<u32>) {
    let idx = lid.x;
    var value: f32 = -3.402823e+38;
    if (idx < params.len) {
        value = input[idx];
    }
    scratch[idx] = value;
    workgroupBarrier();

    for (var s: u32 = 128u; s > 0u; s = s >> 1u) {
        if (idx < s) {
            scratch[idx] = max(scratch[idx], scratch[idx + s]);
        }
        workgroupBarrier();
    }

    if (idx == 0u) {
        output[0] = scratch[0];
    }
}

@compute @workgroup_size(256)
fn min(@builtin(local_invocation_id) lid: vec3<u32>) {
    let idx = lid.x;
    var value: f32 = 3.402823e+38;
    if (idx < params.len) {
        value = input[idx];
    }
    scratch[idx] = value;
    workgroupBarrier();

    for (var s: u32 = 128u; s > 0u; s = s >> 1u) {
        if (idx < s) {
            scratch[idx] = min(scratch[idx], scratch[idx + s]);
        }
        workgroupBarrier();
    }

    if (idx == 0u) {
        output[0] = scratch[0];
    }
}
