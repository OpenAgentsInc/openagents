// DeepSeek-V4.1 on Metal (#12420): helpers every V4.1 kernel shares.
//
// The kernels port the CUDA kernels in psionic-backend-cuda/src/kernels/
// (v41_*.cu) one launch for one launch: the same grids, the same thread
// roles, and the same order of every floating-point operation, so a Metal
// kernel equals the CUDA kernel's host model bit for bit. Three differences
// follow from the hardware and are documented in docs/psionic/v41-metal-plan.md:
//
// - Apple GPUs have no double precision. The CUDA kernels round exp and
//   log1p from double; here they are float-float (about 47 bits)
//   implementations whose rounded result was compared with the host's
//   double-rounded result for every float input (see the exhaustive tests).
//   cos and sin come from host tables (see v41_rope). The sampler's double
//   sums use a software binary64 (sd_*), which rounds exactly as CUDA's
//   double additions do.
// - Apple GPUs flush float32 subnormals to zero. A result below 2^-126 is
//   +-0 here where CUDA keeps the subnormal. Decoders use exact tables, not
//   the subnormal bit tricks of the CUDA decoders.
// - Kernel arguments are structs of GPU addresses (Metal 3), so each launch
//   keeps the CUDA launcher's pointer arguments.
//
// Compile without fast math. The pragma below stops the compiler from
// contracting a multiply and an add into a fused multiply-add; division and
// square roots use the precise (correctly rounded) forms.

#include <metal_stdlib>
using namespace metal;
#pragma clang fp contract(off)

#define V41_NAN as_type<float>(0x7fc00000u)
#define V41_INF as_type<float>(0x7f800000u)

inline float fdiv(float a, float b) { return precise::divide(a, b); }
inline float fsqrt(float a) { return precise::sqrt(a); }

// Round to the nearest BF16, ties to even, returned as float32. NaN stays a
// quiet NaN with its sign, as half::bf16::from_f32 does.
inline float bf16_round(float value) {
    const uint bits = as_type<uint>(value);
    if ((bits & 0x7fffffffu) > 0x7f800000u) {
        return as_type<float>((bits | 0x00400000u) & 0xffff0000u);
    }
    const uint rounded = bits + 0x7fffu + ((bits >> 16) & 1u);
    return as_type<float>(rounded & 0xffff0000u);
}

inline float store_value(float value, int round_bf16) {
    return round_bf16 ? bf16_round(value) : value;
}

inline ushort bf16_bits(float value) {
    return ushort(as_type<uint>(bf16_round(value)) >> 16);
}

inline float bf16_value(ushort bits) {
    return as_type<float>(uint(bits) << 16);
}

// E4M3 magnitudes for codes 0 to 127; code 127 is NaN.
constant float kE4m3[128] = {
    0.0f, 0x1p-9f, 0x1p-8f, 0x1.8p-8f,
    0x1p-7f, 0x1.4p-7f, 0x1.8p-7f, 0x1.cp-7f,
    0x1p-6f, 0x1.2p-6f, 0x1.4p-6f, 0x1.6p-6f,
    0x1.8p-6f, 0x1.ap-6f, 0x1.cp-6f, 0x1.ep-6f,
    0x1p-5f, 0x1.2p-5f, 0x1.4p-5f, 0x1.6p-5f,
    0x1.8p-5f, 0x1.ap-5f, 0x1.cp-5f, 0x1.ep-5f,
    0x1p-4f, 0x1.2p-4f, 0x1.4p-4f, 0x1.6p-4f,
    0x1.8p-4f, 0x1.ap-4f, 0x1.cp-4f, 0x1.ep-4f,
    0x1p-3f, 0x1.2p-3f, 0x1.4p-3f, 0x1.6p-3f,
    0x1.8p-3f, 0x1.ap-3f, 0x1.cp-3f, 0x1.ep-3f,
    0x1p-2f, 0x1.2p-2f, 0x1.4p-2f, 0x1.6p-2f,
    0x1.8p-2f, 0x1.ap-2f, 0x1.cp-2f, 0x1.ep-2f,
    0x1p-1f, 0x1.2p-1f, 0x1.4p-1f, 0x1.6p-1f,
    0x1.8p-1f, 0x1.ap-1f, 0x1.cp-1f, 0x1.ep-1f,
    0x1p+0f, 0x1.2p+0f, 0x1.4p+0f, 0x1.6p+0f,
    0x1.8p+0f, 0x1.ap+0f, 0x1.cp+0f, 0x1.ep+0f,
    0x1p+1f, 0x1.2p+1f, 0x1.4p+1f, 0x1.6p+1f,
    0x1.8p+1f, 0x1.ap+1f, 0x1.cp+1f, 0x1.ep+1f,
    0x1p+2f, 0x1.2p+2f, 0x1.4p+2f, 0x1.6p+2f,
    0x1.8p+2f, 0x1.ap+2f, 0x1.cp+2f, 0x1.ep+2f,
    0x1p+3f, 0x1.2p+3f, 0x1.4p+3f, 0x1.6p+3f,
    0x1.8p+3f, 0x1.ap+3f, 0x1.cp+3f, 0x1.ep+3f,
    0x1p+4f, 0x1.2p+4f, 0x1.4p+4f, 0x1.6p+4f,
    0x1.8p+4f, 0x1.ap+4f, 0x1.cp+4f, 0x1.ep+4f,
    0x1p+5f, 0x1.2p+5f, 0x1.4p+5f, 0x1.6p+5f,
    0x1.8p+5f, 0x1.ap+5f, 0x1.cp+5f, 0x1.ep+5f,
    0x1p+6f, 0x1.2p+6f, 0x1.4p+6f, 0x1.6p+6f,
    0x1.8p+6f, 0x1.ap+6f, 0x1.cp+6f, 0x1.ep+6f,
    0x1p+7f, 0x1.2p+7f, 0x1.4p+7f, 0x1.6p+7f,
    0x1.8p+7f, 0x1.ap+7f, 0x1.cp+7f, 0x1.ep+7f,
    0x1p+8f, 0x1.2p+8f, 0x1.4p+8f, 0x1.6p+8f,
    0x1.8p+8f, 0x1.ap+8f, 0x1.cp+8f, NAN,
};

inline float e4m3(uint byte) {
    const float value = kE4m3[byte & 0x7fu];
    return (byte & 0x80u) ? -value : value;
}

constant float kE2m1[8] = {0.0f, 0.5f, 1.0f, 1.5f, 2.0f, 3.0f, 4.0f, 6.0f};

// Code 8 is negative zero.
inline float e2m1(uint code) {
    const float value = kE2m1[code & 7u];
    return (code & 8u) ? -value : value;
}

// UE8M0. Byte 0 is 2^-127, a subnormal, which the GPU flushes to zero.
inline float e8m0(uint byte) {
    if (byte == 0xffu) {
        return V41_NAN;
    }
    return byte == 0 ? 0.0f : as_type<float>(byte << 23);
}

// ---------------------------------------------------------------------------
// Float-float arithmetic
// ---------------------------------------------------------------------------

struct ff {
    float hi;
    float lo;
};

inline ff two_sum(float a, float b) {
    const float s = a + b;
    const float bb = s - a;
    return {s, (a - (s - bb)) + (b - bb)};
}

// Requires |a| >= |b| or a == 0.
inline ff fast_two_sum(float a, float b) {
    const float s = a + b;
    return {s, b - (s - a)};
}

inline ff two_prod(float a, float b) {
    const float p = a * b;
    return {p, fma(a, b, -p)};
}

inline ff ff_add(ff a, ff b) {
    ff s = two_sum(a.hi, b.hi);
    const ff t = two_sum(a.lo, b.lo);
    s = fast_two_sum(s.hi, s.lo + t.hi);
    return fast_two_sum(s.hi, s.lo + t.lo);
}

inline ff ff_mul(ff a, ff b) {
    ff p = two_prod(a.hi, b.hi);
    p.lo = fma(a.hi, b.lo, p.lo);
    p.lo = fma(a.lo, b.hi, p.lo);
    return fast_two_sum(p.hi, p.lo);
}

inline ff ff_mul_f(ff a, float b) {
    ff p = two_prod(a.hi, b);
    p.lo = fma(a.lo, b, p.lo);
    return fast_two_sum(p.hi, p.lo);
}

inline ff ff_from(float a) { return {a, 0.0f}; }

// ln2/64 = C1 + C2 + C3; C1 has 10 significant bits
constant float kExpC1 = 0x1.63p-7f;
constant float kExpC2 = -0x1.bd0106p-19f;
constant float kExpC3 = 0x1.cf79acp-46f;
constant float kExpInvC = 0x1.715476p+6f;
constant float2 kExpTable[64] = {
    {0x1p+0f, 0.0f},
    {0x1.02c9a4p+0f, -0x1.887fap-28f},
    {0x1.059b0ep+0f, -0x1.9d4f52p-25f},
    {0x1.087452p+0f, -0x1.e2990ep-26f},
    {0x1.0b5586p+0f, 0x1.9f3122p-25f},
    {0x1.0e3ec4p+0f, -0x1.a585ccp-25f},
    {0x1.11301ep+0f, -0x1.fdb496p-25f},
    {0x1.1429aap+0f, 0x1.d525bcp-25f},
    {0x1.172b84p+0f, -0x1.c15742p-27f},
    {0x1.1a35bep+0f, 0x1.6df96ep-25f},
    {0x1.1d4874p+0f, -0x1.d2e8cap-25f},
    {0x1.2063b8p+0f, 0x1.0c519ap-25f},
    {0x1.2387a6p+0f, 0x1.ceac48p-25f},
    {0x1.26b456p+0f, 0x1.789f38p-26f},
    {0x1.29e9ep+0f, -0x1.5c0424p-25f},
    {0x1.2d285ap+0f, 0x1.b900c2p-26f},
    {0x1.306fep+0f, 0x1.4636e2p-25f},
    {0x1.33c08cp+0f, -0x1.b37d2p-25f},
    {0x1.371a74p+0f, -0x1.18aac6p-25f},
    {0x1.3a7db4p+0f, -0x1.634c02p-25f},
    {0x1.3dea64p+0f, 0x1.824684p-25f},
    {0x1.4160a2p+0f, 0x1.f72e2ap-28f},
    {0x1.44e086p+0f, 0x1.8624b4p-30f},
    {0x1.486a2cp+0f, -0x1.47d866p-25f},
    {0x1.4bfdaep+0f, -0x1.593abcp-25f},
    {0x1.4f9b28p+0f, -0x1.2c5a6cp-25f},
    {0x1.5342b6p+0f, -0x1.2c561p-25f},
    {0x1.56f474p+0f, -0x1.295b04p-25f},
    {0x1.5ab07ep+0f, -0x1.5bd5ecp-27f},
    {0x1.5e76f2p+0f, -0x1.4a5bd6p-25f},
    {0x1.6247ecp+0f, -0x1.f8b55p-25f},
    {0x1.662388p+0f, 0x1.2a9112p-27f},
    {0x1.6a09e6p+0f, 0x1.9fcef4p-26f},
    {0x1.6dfb24p+0f, -0x1.cd72e8p-27f},
    {0x1.71f75ep+0f, 0x1.1d8beep-25f},
    {0x1.75feb6p+0f, -0x1.37b306p-25f},
    {0x1.7a1148p+0f, -0x1.829fdp-25f},
    {0x1.7e2f34p+0f, -0x1.261634p-25f},
    {0x1.82589ap+0f, -0x1.accc7cp-26f},
    {0x1.868d9ap+0f, -0x1.2edb44p-26f},
    {0x1.8ace54p+0f, 0x1.15506ep-27f},
    {0x1.8f1aeap+0f, -0x1.baa232p-26f},
    {0x1.93737cp+0f, -0x1.e64744p-25f},
    {0x1.97d82ap+0f, -0x1.0d8d84p-31f},
    {0x1.9c4918p+0f, 0x1.51f848p-27f},
    {0x1.a0c668p+0f, -0x1.2886a6p-26f},
    {0x1.a5503cp+0f, -0x1.b83b54p-25f},
    {0x1.a9e6b6p+0f, -0x1.50c048p-25f},
    {0x1.ae89fap+0f, -0x1.a94b14p-26f},
    {0x1.b33a2cp+0f, -0x1.ec3a82p-26f},
    {0x1.b7f77p+0f, -0x1.a09438p-25f},
    {0x1.bcc1eap+0f, -0x1.f687c6p-25f},
    {0x1.c199bep+0f, -0x1.3d56b2p-27f},
    {0x1.c67f12p+0f, 0x1.cafa2ap-25f},
    {0x1.cb720ep+0f, -0x1.8837ccp-27f},
    {0x1.d072d4p+0f, 0x1.40f13p-25f},
    {0x1.d5818ep+0f, -0x1.822dbcp-27f},
    {0x1.da9e6p+0f, 0x1.ed9942p-27f},
    {0x1.dfc974p+0f, -0x1.908c94p-25f},
    {0x1.e502eep+0f, 0x1.e2cffep-26f},
    {0x1.ea4afap+0f, 0x1.52486cp-27f},
    {0x1.efa1bep+0f, 0x1.cc2b44p-25f},
    {0x1.f50766p+0f, -0x1.246ebp-26f},
    {0x1.fa7c18p+0f, 0x1.9e90d8p-28f}};
constant float kLn2Hi = 0x1.62e4p-1f;
constant float kLn2Mid = 0x1.7f7d1cp-20f;
constant float kLn2Lo = 0x1.ef357ap-45f;
constant float2 kLogTable[128] = {
    {0.0f, 0.0f},
    {0x1.fe02a6p-8f, 0x1.620cf2p-33f},
    {0x1.fc0a8cp-7f, -0x1.e07f84p-32f},
    {0x1.7b91bp-6f, 0x1.f56c46p-32f},
    {0x1.f829bp-6f, 0x1.cf066p-31f},
    {0x1.39e87cp-5f, -0x1.8050a8p-31f},
    {0x1.77459p-5f, -0x1.39a46p-30f},
    {0x1.b42dd8p-5f, -0x1.dcd1c8p-30f},
    {0x1.f0a30cp-5f, 0x1.162a66p-37f},
    {0x1.16536ep-4f, 0x1.d46f5cp-29f},
    {0x1.341d7ap-4f, -0x1.3c85c6p-29f},
    {0x1.51b074p-4f, -0x1.f3cf82p-33f},
    {0x1.6f0d28p-4f, 0x1.5cad6ap-29f},
    {0x1.8c345ep-4f, -0x1.39cc9cp-29f},
    {0x1.a926d4p-4f, -0x1.6d4aa8p-30f},
    {0x1.c5e548p-4f, 0x1.eb78e8p-29f},
    {0x1.e27076p-4f, 0x1.c55e5cp-29f},
    {0x1.fec914p-4f, -0x1.c482a8p-29f},
    {0x1.0d77e8p-3f, -0x1.97b8d4p-30f},
    {0x1.1b72aep-3f, -0x1.5a130cp-28f},
    {0x1.29553p-3f, -0x1.f802b8p-29f},
    {0x1.371fc2p-3f, 0x1.e8f744p-35f},
    {0x1.44d2b6p-3f, 0x1.996fa4p-28f},
    {0x1.526e5ep-3f, 0x1.d0da1cp-30f},
    {0x1.5ff308p-3f, -0x1.eb0d86p-28f},
    {0x1.6d60fep-3f, 0x1.c67488p-29f},
    {0x1.7ab89p-3f, 0x1.086c84p-30f},
    {0x1.87fa06p-3f, 0x1.483244p-29f},
    {0x1.9525aap-3f, -0x1.85d4a6p-30f},
    {0x1.a23bc2p-3f, -0x1.d4a9cep-35f},
    {0x1.af3c94p-3f, 0x1.d017fep-28f},
    {0x1.bc2868p-3f, -0x1.7a4e66p-28f},
    {0x1.c8ff7cp-3f, 0x1.e6a688p-29f},
    {0x1.d5c216p-3f, 0x1.69f772p-28f},
    {0x1.e27076p-3f, 0x1.c55e5cp-28f},
    {0x1.ef0adcp-3f, 0x1.7b8b26p-28f},
    {0x1.fb9186p-3f, 0x1.abc7c6p-28f},
    {0x1.04025ap-2f, -0x1.6965f8p-27f},
    {0x1.0a324ep-2f, 0x1.39c872p-29f},
    {0x1.1058cp-2f, -0x1.946d4ap-28f},
    {0x1.1675cap-2f, 0x1.7574c2p-27f},
    {0x1.1c898cp-2f, 0x1.6999fap-30f},
    {0x1.22942p-2f, -0x1.0c21a6p-28f},
    {0x1.2895a2p-2f, -0x1.842f2cp-27f},
    {0x1.2e8e2cp-2f, -0x1.47b8b4p-28f},
    {0x1.347ddap-2f, -0x1.59e0aap-28f},
    {0x1.3a64c6p-2f, -0x1.52d742p-27f},
    {0x1.404308p-2f, 0x1.a1a9f8p-28f},
    {0x1.4618bcp-2f, 0x1.0e2f62p-29f},
    {0x1.4be5fap-2f, -0x1.5110ecp-27f},
    {0x1.51aad8p-2f, 0x1.cb7e0cp-28f},
    {0x1.576772p-2f, -0x1.1754b2p-27f},
    {0x1.5d1bdcp-2f, -0x1.4fec6cp-31f},
    {0x1.62c83p-2f, -0x1.a8c70ep-27f},
    {0x1.686c82p-2f, -0x1.64eb52p-30f},
    {0x1.6e08eap-2f, 0x1.45743cp-27f},
    {0x1.739d8p-2f, -0x1.2886p-27f},
    {0x1.792a56p-2f, -0x1.15c2ecp-33f},
    {0x1.7eaf84p-2f, -0x1.1f541p-28f},
    {0x1.842d1ep-2f, -0x1.785d3ap-28f},
    {0x1.89a338p-2f, 0x1.b05096p-28f},
    {0x1.8f11e8p-2f, 0x1.cd98b2p-28f},
    {0x1.947942p-2f, -0x1.ef7482p-29f},
    {0x1.99d958p-2f, 0x1.17e08ap-30f},
    {0x1.9f323ep-2f, 0x1.97f30ap-27f},
    {0x1.a4840ap-2f, -0x1.e3489ep-27f},
    {0x1.a9cecap-2f, -0x1.597deep-28f},
    {0x1.af1294p-2f, -0x1.b710f2p-27f},
    {0x1.b44f78p-2f, -0x1.0cdc28p-28f},
    {0x1.b9858ap-2f, -0x1.2d9dep-27f},
    {0x1.beb4dap-2f, -0x1.2c7242p-29f},
    {0x1.c3dd7ap-2f, 0x1.f36b54p-28f},
    {0x1.c8ff7cp-2f, 0x1.e6a688p-28f},
    {0x1.ce1afp-2f, 0x1.70be7ep-27f},
    {0x1.d32fe8p-2f, -0x1.ff142ap-30f},
    {0x1.d83e72p-2f, 0x1.628bdp-28f},
    {0x1.dd46ap-2f, 0x1.307128p-28f},
    {0x1.e24882p-2f, -0x1.60e4f6p-28f},
    {0x1.e74426p-2f, 0x1.d68788p-30f},
    {0x1.ec399ep-2f, -0x1.b72e68p-27f},
    {0x1.f128f6p-2f, -0x1.43e44ep-32f},
    {0x1.f6124p-2f, -0x1.63f5d4p-28f},
    {0x1.faf588p-2f, 0x1.ef1e64p-27f},
    {0x1.ffd2ep-2f, 0x1.0afe94p-27f},
    {0x1.02552ap-1f, 0x1.69744p-27f},
    {0x1.04bdfap-1f, -0x1.2b6c96p-28f},
    {0x1.0723e6p-1f, -0x1.f1906p-28f},
    {0x1.0986f4p-1f, 0x1.eae6a4p-26f},
    {0x1.0be72ep-1f, 0x1.094aap-27f},
    {0x1.0e4498p-1f, 0x1.747322p-27f},
    {0x1.109f3ap-1f, -0x1.d2b36ap-29f},
    {0x1.12f71ap-1f, -0x1.4d8208p-26f},
    {0x1.154c3ep-1f, -0x1.a16542p-26f},
    {0x1.179eacp-1f, -0x1.09d998p-27f},
    {0x1.19ee6cp-1f, -0x1.7306d2p-26f},
    {0x1.1c3b82p-1f, -0x1.1d87b6p-30f},
    {0x1.1e85f6p-1f, -0x1.8fbf3p-29f},
    {0x1.20cdcep-1f, -0x1.cdaa92p-26f},
    {0x1.23130ep-1f, -0x1.082818p-26f},
    {0x1.2555bcp-1f, 0x1.d31efap-26f},
    {0x1.2795e2p-1f, -0x1.aec9dcp-26f},
    {0x1.29d38p-1f, -0x1.3d4f76p-29f},
    {0x1.2c0e9ep-1f, 0x1.a891d2p-26f},
    {0x1.2e4744p-1f, -0x1.237fb2p-26f},
    {0x1.307d74p-1f, -0x1.961de8p-26f},
    {0x1.32b134p-1f, -0x1.bb78a4p-27f},
    {0x1.34e28ap-1f, -0x1.318f16p-28f},
    {0x1.37117cp-1f, -0x1.57170ap-26f},
    {0x1.393e0ep-1f, -0x1.953abcp-26f},
    {0x1.3b6844p-1f, 0x1.3fff84p-26f},
    {0x1.3d9026p-1f, 0x1.4e2aep-26f},
    {0x1.3fb5b8p-1f, 0x1.345bdp-27f},
    {0x1.41d8fep-1f, 0x1.08ce56p-26f},
    {0x1.43f9fep-1f, 0x1.7ce734p-28f},
    {0x1.4618bcp-1f, 0x1.0e2f62p-28f},
    {0x1.48353ep-1f, -0x1.c2aee4p-26f},
    {0x1.4a4f86p-1f, -0x1.27e0a2p-28f},
    {0x1.4c679ap-1f, 0x1.f99dc8p-26f},
    {0x1.4e7d82p-1f, -0x1.c9148ap-26f},
    {0x1.50913cp-1f, 0x1.802d0ep-26f},
    {0x1.52a2d2p-1f, 0x1.96f16ap-27f},
    {0x1.54b246p-1f, 0x1.e66526p-27f},
    {0x1.56bf9ep-1f, -0x1.49818cp-26f},
    {0x1.58cadcp-1f, -0x1.4650cep-26f},
    {0x1.5ad404p-1f, 0x1.86b3e6p-26f},
    {0x1.5cdb1ep-1f, -0x1.c9f44ep-28f},
    {0x1.5ee02ap-1f, 0x1.2482cep-26f},
    {0x1.60e33p-1f, -0x1.770ee4p-26f}};


// Inputs whose float-float result rounds to the other neighbor of the
// host's double-rounded result, found by the exhaustive checks in
// src/v41/checks.rs (float bits, then the host's result bits).
constant uint kExpHard[12][2] = {
    {0x33800000u, 0x3f800001u},
    {0x383a3ef1u, 0x3f800175u},
    {0x39c6be5bu, 0x3f800c6du},
    {0x39e5bb1du, 0x3f800e5du},
    {0x40315b33u, 0x417fa47du},
    {0x4288942bu, 0x70b7a4c5u},
    {0xbae0e25cu, 0x3f7f8fa7u},
    {0xbbf0edf1u, 0x3f7e1fe9u},
    {0xbce017bbu, 0x3f79178fu},
    {0xbf81eadfu, 0x3eb98dc1u},
    {0xc13d6631u, 0x36f28e33u},
    {0xc16912cdu, 0x34fd331bu}};
constant uint kLog1pHard[34][2] = {
    {0x36aad563u, 0x36aad547u},
    {0x36dedaceu, 0x36deda9du},
    {0x3770004bu, 0x376fffdbu},
    {0x38280093u, 0x3827ffb7u},
    {0x3ddbfec3u, 0x3dd0f671u},
    {0x3f26c9aeu, 0x3f006ef3u},
    {0x4016ee45u, 0x3f9b103bu},
    {0x41dbb2afu, 0x40564f1du},
    {0x434e23a8u, 0x40aaab25u},
    {0x46067143u, 0x4110f651u},
    {0x464d572bu, 0x4117bcf1u},
    {0x4665a5a6u, 0x41198725u},
    {0x46ca6a75u, 0x41229929u},
    {0x4e599333u, 0x41a50de3u},
    {0x50da4bf6u, 0x41c0ce7fu},
    {0x50e0323fu, 0x41c1051du},
    {0x521e4defu, 0x41cedf19u},
    {0x521fe119u, 0x41cef35fu},
    {0x52fa32c0u, 0x41d8141bu},
    {0x5b777193u, 0x421b2105u},
    {0x5b98e163u, 0x421bf9b9u},
    {0x5caffb5eu, 0x42221563u},
    {0x5d8b2d5bu, 0x4226b0b1u},
    {0x5ee8984eu, 0x422e4a21u},
    {0x64e27fa3u, 0x424f7453u},
    {0x6d1f23ebu, 0x427d2d35u},
    {0x6e7054f2u, 0x42822f73u},
    {0x7405dee8u, 0x4291a68du},
    {0x74661451u, 0x4292bbd3u},
    {0x74896a73u, 0x429316d3u},
    {0x789e5998u, 0x429e768fu},
    {0x79e7ec37u, 0x42a1ffb7u},
    {0x7a708b30u, 0x42a3754bu},
    {0x7dbdfe04u, 0x42acb0bfu}};

inline float hard_case(constant uint (*table)[2], int n, float x, float y) {
    const uint bits = as_type<uint>(x);
    int lo = 0;
    int hi = n;
    while (lo < hi) {
        const int mid = (lo + hi) / 2;
        if (table[mid][0] < bits) {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    return lo < n && table[lo][0] == bits ? as_type<float>(table[lo][1]) : y;
}

// exp(x) rounded once to float, as the CUDA kernels' (float)exp((double)x):
// x = (64 m + j) ln2 / 64 + r, exp(x) = 2^m 2^(j/64) exp(r), with r and the
// table in float-float. Results below 2^-126 flush to zero.
inline float exp_once(float x) {
    if (isnan(x)) {
        return x;
    }
    if (x > 89.0f) {
        return V41_INF;
    }
    if (x < -104.0f) {
        return 0.0f;
    }
    if (fabs(x) < 0x1p-26f) {
        return 1.0f;
    }
    const float kf = rint(x * kExpInvC);
    const int k = int(kf);
    // r = x - k C1 - k C2 - k C3; k C1 is exact (k < 2^14, C1 has 10 bits).
    ff r = two_sum(x, -(kf * kExpC1));
    const ff p2 = two_prod(kf, kExpC2);
    r = ff_add(r, ff{-p2.hi, -p2.lo});
    r = ff_add(r, ff_from(-(kf * kExpC3)));
    // exp(r) - 1 = r + r^2/2 + r^3/6 + (r^4/24 + r^5/120 + r^6/720).
    const ff r2 = ff_mul(r, r);
    const ff r3 = ff_mul(r2, r);
    const float h = r.hi;
    const float tail = h * h * h * h * (fdiv(1.0f, 24.0f) + h * (fdiv(1.0f, 120.0f) + h * fdiv(1.0f, 720.0f)));
    ff e = ff_add(ff_mul(r3, ff{0x1.555556p-3f, -0x1.555556p-28f}), ff_from(tail));
    e = ff_add(ff{r2.hi * 0.5f, r2.lo * 0.5f}, e);
    e = ff_add(r, e);
    const float2 t = kExpTable[k & 63];
    const ff table = {t.x, t.y};
    const ff value = ff_add(table, ff_mul(table, e));
    const int m = k >> 6;
    // value.hi is the float nearest value, in [2^-0.5, 2^1.5).
    if (m > -126) {
        return hard_case(kExpHard, 12, x, ldexp(value.hi, m));
    }
    // Scaling by 2^(m + 64) keeps it normal and exact. Below 2^-126 only a
    // value that rounds up to 2^-126 at subnormal precision survives; the
    // rest flush to zero.
    const float z = ldexp(value.hi, m + 64);
    if (z >= 0x1p-62f) {
        return ldexp(z, -64);
    }
    return z >= 0x1.fffffep-63f ? 0x1p-126f : 0.0f;
}

// log1p(t) - t for |t| < 2^-4 in float-float: Horner on
// -t^2/2 + t^3/3 - ... - t^n/n.
inline ff log1p_series(ff t, int terms) {
    ff acc = ff_from(0.0f);
    for (int n = terms; n >= 2; --n) {
        const float sign = (n & 1) ? 1.0f : -1.0f;
        // 1/n in float-float.
        const float q = fdiv(1.0f, float(n));
        const float q_lo = fdiv(fma(-q, float(n), 1.0f), float(n));
        acc = ff_add(acc, ff{sign * q, sign * q_lo});
        acc = ff_mul(acc, t);
    }
    return ff_mul(acc, t);
}

// log1p(y) rounded once to float, as the CUDA kernels' (float)log1p((double)y).
// Checked for every float y >= 0; the router only passes exp results.
inline float log1p_once(float y) {
    if (isnan(y) || y == V41_INF) {
        return y;
    }
    if (y == -1.0f) {
        return -V41_INF;
    }
    if (y < -1.0f) {
        return V41_NAN;
    }
    if (fabs(y) < 0x1p-24f) {
        return y;
    }
    if (fabs(y) < 0x1p-4f) {
        const ff t = ff_from(y);
        return hard_case(kLog1pHard, 34, y, ff_add(t, log1p_series(t, 13)).hi);
    }
    // u = 1 + y exactly; u = 2^e m with m in [1, 2); c = 1 + j/128 <= m.
    const ff u = two_sum(1.0f, y);
    int e;
    const float m2 = frexp(u.hi, e);  // m2 in [0.5, 1)
    const float m = m2 * 2.0f;
    e -= 1;
    const float ul = ldexp(u.lo, -e);
    const int j = int((m - 1.0f) * 128.0f);
    const float c = 1.0f + float(j) * 0x1p-7f;
    // t = (m - c + ul) / c, |t| < 2^-7.
    const ff n = two_sum(m - c, ul);
    const float q1 = fdiv(n.hi, c);
    const float rem = fma(-q1, c, n.hi) + n.lo;
    const ff t = fast_two_sum(q1, fdiv(rem, c));
    ff value = ff_add(t, log1p_series(t, 9));
    const float2 lc = kLogTable[j];
    value = ff_add(ff{lc.x, lc.y}, value);
    const float ef = float(e);
    const ff ln2e = ff_add(ff_from(ef * kLn2Hi), two_prod(ef, kLn2Mid));
    value = ff_add(ln2e, ff_add(ff_from(ef * kLn2Lo), value));
    return hard_case(kLog1pHard, 34, y, value.hi);
}

// The CPU path's sigmoid: 1 / (1 + e^-x) for x >= 0, e^x / (1 + e^x) below.
inline float sigmoid_once(float x) {
    if (x >= 0.0f) {
        return fdiv(1.0f, 1.0f + exp_once(-x));
    }
    const float e = exp_once(x);
    return fdiv(e, 1.0f + e);
}

// ---------------------------------------------------------------------------
// Software binary64
// ---------------------------------------------------------------------------
//
// The CUDA sampler adds its weights in double precision. These functions
// round exactly as IEEE binary64 does (to nearest, ties to even) for the
// finite normal values and zeros the sampler produces; subnormal, infinite,
// and NaN doubles never occur there and aren't handled.

constant constexpr ulong kSign = 0x8000000000000000ul;
constant constexpr ulong kFrac = 0x000ffffffffffffful;
constant constexpr ulong kHidden = 0x0010000000000000ul;

inline bool sd_is_zero(ulong a) { return (a << 1) == 0; }

inline ulong sd_from_float(float x) {
    const uint b = as_type<uint>(x);
    const ulong sign = ulong(b & 0x80000000u) << 32;
    const uint e = (b >> 23) & 0xffu;
    if (e == 0) {
        return sign;  // zero (subnormal floats are flushed on the GPU)
    }
    return sign | (ulong(e + 896u) << 52) | (ulong(b & 0x7fffffu) << 29);
}

// An integer below 2^53, exactly.
inline ulong sd_from_u53(ulong n) {
    if (n == 0) {
        return 0;
    }
    const int top = 63 - int(clz(n));
    const ulong f = n << (52 - top);
    return (ulong(1023 + top) << 52) | (f & kFrac);
}

// x * 2^k for a normal x whose result stays normal.
inline ulong sd_scale(ulong a, int k) {
    if (sd_is_zero(a)) {
        return a;
    }
    return a + (ulong(long(k)) << 52);
}

inline ulong sd_add(ulong a, ulong b) {
    if (sd_is_zero(a)) {
        return sd_is_zero(b) ? (a & b) : b;
    }
    if (sd_is_zero(b)) {
        return a;
    }
    if ((b & ~kSign) > (a & ~kSign)) {
        const ulong t = a;
        a = b;
        b = t;
    }
    int e = int((a >> 52) & 0x7ff);
    const int eb = int((b >> 52) & 0x7ff);
    // 53-bit significands moved up by 10 bits for rounding.
    ulong fa = ((a & kFrac) | kHidden) << 10;
    ulong fb = ((b & kFrac) | kHidden) << 10;
    const int d = e - eb;
    if (d > 0) {
        if (d >= 64) {
            fb = fb != 0 ? 1 : 0;
        } else {
            const ulong sticky = (fb << (64 - d)) != 0 ? 1 : 0;
            fb = (fb >> d) | sticky;
        }
    }
    ulong f;
    if (((a ^ b) & kSign) == 0) {
        f = fa + fb;
        if ((f >> 63) != 0) {
            f = (f >> 1) | (f & 1);
            e += 1;
        }
    } else {
        f = fa - fb;
        if (f == 0) {
            return 0;
        }
        const int shift = int(clz(f)) - 1;
        f <<= shift;
        e -= shift;
    }
    const ulong rest = f & 0x3ff;
    f >>= 10;
    if (rest > 0x200 || (rest == 0x200 && (f & 1) != 0)) {
        f += 1;
        if ((f >> 53) != 0) {
            f >>= 1;
            e += 1;
        }
    }
    return (a & kSign) | (ulong(e) << 52) | (f & kFrac);
}

inline ulong sd_sub(ulong a, ulong b) { return sd_add(a, b ^ kSign); }

inline ulong sd_mul(ulong a, ulong b) {
    const ulong sign = (a ^ b) & kSign;
    if (sd_is_zero(a) || sd_is_zero(b)) {
        return sign;
    }
    int e = int((a >> 52) & 0x7ff) + int((b >> 52) & 0x7ff) - 1023;
    const ulong fa = (a & kFrac) | kHidden;
    const ulong fb = (b & kFrac) | kHidden;
    // The product has 105 or 106 bits.
    const ulong lo = fa * fb;
    const ulong hi = mulhi(fa, fb);
    int shift = 52;
    if (((hi >> 41) & 1) != 0) {
        shift = 53;
        e += 1;
    }
    ulong f = (hi << (64 - shift)) | (lo >> shift);
    const ulong rest = lo & ((1ul << shift) - 1);
    const ulong midpoint = 1ul << (shift - 1);
    if (rest > midpoint || (rest == midpoint && (f & 1) != 0)) {
        f += 1;
        if ((f >> 53) != 0) {
            f >>= 1;
            e += 1;
        }
    }
    return sign | (ulong(e) << 52) | (f & kFrac);
}

// A key that orders doubles as their values (both zeros equal).
inline long sd_key(ulong a) {
    return (a & kSign) != 0 ? -long(a & ~kSign) : long(a);
}

inline bool sd_ge(ulong a, ulong b) { return sd_key(a) >= sd_key(b); }
inline bool sd_lt(ulong a, ulong b) { return sd_key(a) < sd_key(b); }
