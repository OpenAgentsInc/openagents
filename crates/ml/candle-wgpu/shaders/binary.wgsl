struct Params {
    len: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
};

@group(0) @binding(0) var<storage, read> a: array<f32>;
@group(0) @binding(1) var<storage, read> b: array<f32>;
@group(0) @binding(2) var<storage, read_write> out: array<f32>;
@group(0) @binding(3) var<uniform> params: Params;

@compute @workgroup_size(256)
fn add(@builtin(global_invocation_id) gid: vec3<u32>) {
    let idx = gid.x;
    if (idx >= params.len) { return; }
    out[idx] = a[idx] + b[idx];
}

@compute @workgroup_size(256)
fn sub(@builtin(global_invocation_id) gid: vec3<u32>) {
    let idx = gid.x;
    if (idx >= params.len) { return; }
    out[idx] = a[idx] - b[idx];
}

@compute @workgroup_size(256)
fn mul(@builtin(global_invocation_id) gid: vec3<u32>) {
    let idx = gid.x;
    if (idx >= params.len) { return; }
    out[idx] = a[idx] * b[idx];
}

@compute @workgroup_size(256)
fn div(@builtin(global_invocation_id) gid: vec3<u32>) {
    let idx = gid.x;
    if (idx >= params.len) { return; }
    out[idx] = a[idx] / b[idx];
}

@compute @workgroup_size(256)
fn minimum(@builtin(global_invocation_id) gid: vec3<u32>) {
    let idx = gid.x;
    if (idx >= params.len) { return; }
    out[idx] = min(a[idx], b[idx]);
}

@compute @workgroup_size(256)
fn maximum(@builtin(global_invocation_id) gid: vec3<u32>) {
    let idx = gid.x;
    if (idx >= params.len) { return; }
    out[idx] = max(a[idx], b[idx]);
}
