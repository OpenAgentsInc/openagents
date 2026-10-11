// GGML Q6_K: 256 weights in 210 bytes (128 low-nibble bytes, 64 high-bit
// bytes, 16 signed sub-scales, and one f16 scale). The output uses the same
// resident f16 projection path as Q4_K and Q8_0.

kernel void clef_dequant_q6_k(
    device const uchar *source [[buffer(0)]],
    device half *destination [[buffer(1)]],
    constant ulong &count [[buffer(2)]],
    uint index [[thread_position_in_grid]]) {
    if (ulong(index) >= count) return;
    device const uchar *block = source + (ulong(index) / 256) * 210;
    const uint within = index % 256;
    const uint chunk = within / 128;
    const uint part = (within % 128) / 32;
    const uint lane = within % 32;
    const uchar low = block[chunk * 64 + (part % 2) * 32 + lane];
    const uchar high = block[128 + chunk * 32 + lane];
    const int quant = int(((part < 2 ? low : low >> 4) & 15)
                         | (((high >> (part * 2)) & 3) << 4)) - 32;
    const int subscale = int(as_type<char>(block[192 + chunk * 8 + part * 2 + lane / 16]));
    const ushort scale_bits = ushort(uint(block[208]) | (uint(block[209]) << 8));
    const float scale = float(as_type<half>(scale_bits));
    destination[index] = half((scale * float(subscale)) * float(quant));
}
