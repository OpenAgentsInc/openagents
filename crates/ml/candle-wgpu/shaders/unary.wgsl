struct Params {
    len: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
};

@group(0) @binding(0) var<storage, read> input: array<f32>;
@group(0) @binding(1) var<storage, read_write> output: array<f32>;
@group(0) @binding(2) var<uniform> params: Params;

fn relu_fn(x: f32) -> f32 {
    return max(x, 0.0);
}

fn silu_fn(x: f32) -> f32 {
    return x / (1.0 + exp(-x));
}

fn gelu_fn(x: f32) -> f32 {
    let k = 0.7978845608;
    let inner = x + 0.044715 * x * x * x;
    return 0.5 * x * (1.0 + tanh(k * inner));
}

@compute @workgroup_size(256)
fn exp_kernel(@builtin(global_invocation_id) gid: vec3<u32>) {
    let idx = gid.x;
    if (idx >= params.len) { return; }
    output[idx] = exp(input[idx]);
}

@compute @workgroup_size(256)
fn log_kernel(@builtin(global_invocation_id) gid: vec3<u32>) {
    let idx = gid.x;
    if (idx >= params.len) { return; }
    output[idx] = log(input[idx]);
}

@compute @workgroup_size(256)
fn relu_kernel(@builtin(global_invocation_id) gid: vec3<u32>) {
    let idx = gid.x;
    if (idx >= params.len) { return; }
    output[idx] = relu_fn(input[idx]);
}

@compute @workgroup_size(256)
fn silu_kernel(@builtin(global_invocation_id) gid: vec3<u32>) {
    let idx = gid.x;
    if (idx >= params.len) { return; }
    output[idx] = silu_fn(input[idx]);
}

@compute @workgroup_size(256)
fn gelu_kernel(@builtin(global_invocation_id) gid: vec3<u32>) {
    let idx = gid.x;
    if (idx >= params.len) { return; }
    output[idx] = gelu_fn(input[idx]);
}

@compute @workgroup_size(256)
fn tanh_kernel(@builtin(global_invocation_id) gid: vec3<u32>) {
    let idx = gid.x;
    if (idx >= params.len) { return; }
    output[idx] = tanh(input[idx]);
}
