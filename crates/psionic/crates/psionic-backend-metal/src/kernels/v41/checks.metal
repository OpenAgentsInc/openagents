// Check kernels for the V4.1 Metal math (#12420): each evaluates a helper
// from common.metal over inputs, for comparison with the host.

struct CheckArgs {
    device const uint* input;
    device uint* out;
    uint first;
    uint count;
};

// out[i] = exp_once of the float whose bits are first + i.
kernel void v41_check_exp(constant CheckArgs& a [[buffer(0)]], uint i [[thread_position_in_grid]]) {
    if (i < a.count) {
        a.out[i] = as_type<uint>(exp_once(as_type<float>(a.first + i)));
    }
}

kernel void v41_check_log1p(constant CheckArgs& a [[buffer(0)]], uint i [[thread_position_in_grid]]) {
    if (i < a.count) {
        a.out[i] = as_type<uint>(log1p_once(as_type<float>(a.first + i)));
    }
}

// input holds pairs of doubles (four words each); out holds a + b, a - b,
// a * b, and a >= b for each pair.
kernel void v41_check_double(constant CheckArgs& a [[buffer(0)]], uint i [[thread_position_in_grid]]) {
    if (i >= a.count) {
        return;
    }
    device const ulong* in = (device const ulong*)a.input;
    device ulong* out = (device ulong*)a.out;
    const ulong x = in[2 * i];
    const ulong y = in[2 * i + 1];
    out[4 * i] = sd_add(x, y);
    out[4 * i + 1] = sd_sub(x, y);
    out[4 * i + 2] = sd_mul(x, y);
    out[4 * i + 3] = sd_ge(x, y) ? 1 : 0;
}
