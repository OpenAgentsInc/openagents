struct Params {
    k: u32,
    n: u32,
    row_offset: u32,
    accumulate: u32,
};

@group(0) @binding(0)
var<storage, read> quant: array<u32>;

@group(0) @binding(1)
var<storage, read> x: array<f32>;

@group(0) @binding(2)
var<storage, read_write> y: array<f32>;

@group(0) @binding(3)
var<uniform> params: Params;

const FP4_TABLE: array<f32, 16> = array<f32, 16>(
    0.0, 1.0, 2.0, 3.0,
    4.0, 6.0, 8.0, 12.0,
    -0.0, -1.0, -2.0, -3.0,
    -4.0, -6.0, -8.0, -12.0
);

fn load_byte(offset: u32) -> u32 {
    let word = quant[offset / 4u];
    let shift = (offset & 3u) * 8u;
    return (word >> shift) & 0xffu;
}

fn mxfp4_unpack(block: u32, idx: u32) -> f32 {
    let base = block * 17u;
    let scale_byte = load_byte(base);
    let exp = f32(i32(scale_byte)) - 128.0;
    let scale = exp2(exp);
    let half = 16u;
    let byte_index = base + 1u + (idx % half);
    let packed = load_byte(byte_index);
    let nibble = select(packed & 0x0fu, packed >> 4u, idx >= half);
    return FP4_TABLE[nibble] * scale;
}

@compute @workgroup_size({{WORKGROUP_SIZE}})
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let col = gid.x;
    if (col >= params.n) {
        return;
    }
    var acc: f32 = select(0.0, y[col], params.accumulate != 0u);
    for (var row = 0u; row < params.k; row = row + 1u) {
        let idx = row * params.n + col;
        let block = idx / 32u;
        let offset = idx % 32u;
        let w = mxfp4_unpack(block, offset);
        acc = acc + x[params.row_offset + row] * w;
    }
    y[col] = acc;
}
