struct Params {
    n: u32,
    _pad0: u32,
    _pad1: u32,
    _pad2: u32,
};

@group(0) @binding(0)
var<storage, read> quant: array<u32>;

@group(0) @binding(1)
var<storage, read_write> out: array<f32>;

@group(0) @binding(2)
var<uniform> params: Params;

fn unpack_f16(bits: u32) -> f32 {
    let sign = (bits >> 15u) & 1u;
    let exp = (bits >> 10u) & 0x1fu;
    let frac = bits & 0x03ffu;
    var val: f32;
    if (exp == 0u) {
        if (frac == 0u) {
            val = 0.0;
        } else {
            val = f32(frac) * exp2(-24.0);
        }
    } else if (exp == 31u) {
        val = 1.0 / 0.0;
    } else {
        val = (1.0 + f32(frac) / 1024.0) * exp2(f32(exp) - 15.0);
    }
    if (sign == 1u) {
        val = -val;
    }
    return val;
}

fn q8_0_unpack(block: u32, idx: u32) -> f32 {
    let base = block * 34u;
    let scale_bits = quant[(base + 0u) / 4u] & 0xffffu;
    let scale = unpack_f16(scale_bits);
    let byte_index = base + 2u + idx;
    let word = quant[byte_index / 4u];
    let shift = (byte_index & 3u) * 8u;
    let byte = u32((word >> shift) & 0xffu);
    let signed = i32(byte << 24u) >> 24;
    return scale * f32(signed);
}

@compute @workgroup_size({{WORKGROUP_SIZE}})
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let col = gid.x;
    if (col >= params.n) {
        return;
    }
    let block = col / 32u;
    let offset = col % 32u;
    out[col] = q8_0_unpack(block, offset);
}
