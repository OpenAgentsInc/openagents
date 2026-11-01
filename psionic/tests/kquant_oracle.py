#!/usr/bin/env python3
# Firm reason: 2 (independent oracle: recomputes our answer outside Coax, so it must not share our implementation)
"""Independent K-quant and IQ4 decode vectors for chapter 51.

Each block is decoded from ggml's published block structs (ggml-common.h at
llama.cpp 0893ec98f5eafad36a43a72cd1c3723d72b0d03a, MIT, "Copyright (c)
2023-2026 The ggml authors"), walking the bytes in the order the structs and
ggml's reference loops lay them out: super-blocks of 256, sub-block scales
and mins packed in 4, 6 or 8 bits. Every value is computed as an exact
integer multiple of 2^-24 and rounded to binary32 with integer arithmetic,
nearest with ties to even, so no float type takes part. The bytes come from
a fixed counter stream; half-precision scales cover subnormals, both signs
and the largest finite half. This owned tool imports and runs no upstream
code, and no Coax.
"""
from pathlib import Path
import struct
import sys

ROOT = Path(__file__).resolve().parents[2]
TARGET = ROOT / "psionic/fixtures/kquant-vectors.coax"

# The IQ4 non-uniform values, kvalues_iq4nl in ggml-common.h (MIT, see above).
IQ4 = (-127, -104, -83, -65, -49, -35, -22, -10, 1, 13, 25, 38, 53, 69, 89, 113)

# name: (GGUF type, values per block, bytes per block, offsets of half scales)
FORMATS = {
    "Q2_K": (10, 256, 16 + 64 + 2 + 2, (80, 82)),
    "Q3_K": (11, 256, 32 + 64 + 12 + 2, (108,)),
    "Q4_K": (12, 256, 2 + 2 + 12 + 128, (0, 2)),
    "Q5_K": (13, 256, 2 + 2 + 12 + 32 + 128, (0, 2)),
    "Q6_K": (14, 256, 128 + 64 + 16 + 2, (208,)),
    "IQ4_NL": (20, 32, 2 + 16, (0,)),
    "IQ4_XS": (23, 256, 2 + 2 + 4 + 128, (0,)),
}
BLOCKS = {"IQ4_NL": 8}


def half_units(bits):
    """The half as an integer number of 2^-24 units."""
    sign = -1 if bits & 0x8000 else 1
    exponent, fraction = (bits >> 10) & 31, bits & 1023
    assert exponent != 31, "no infinities or NaNs in vectors"
    if exponent == 0:
        return sign * fraction
    return sign * (1024 + fraction) * 2 ** (exponent - 1)


def f32_bits(units):
    """units * 2^-24 rounded to binary32 (nearest, ties to even), as bits."""
    if units == 0:
        return 0
    sign = 0x80000000 if units < 0 else 0
    a = abs(units)
    shift = max(a.bit_length() - 24, 0)
    kept, rest = a >> shift, a & ((1 << shift) - 1)
    if shift and (rest > 1 << (shift - 1) or (rest == 1 << (shift - 1) and kept & 1)):
        kept += 1
    if kept == 1 << 24:
        kept, shift = kept >> 1, shift + 1
    exponent = kept.bit_length() - 1 + shift - 24  # value = kept * 2^(shift-24)
    assert -126 <= exponent < 128
    mantissa = (kept << (23 - (kept.bit_length() - 1))) & 0x7fffff
    return sign | (exponent + 127) << 23 | mantissa


def half_at(b, at):
    return half_units(b[at] | b[at + 1] << 8)


def q2_k(b):
    scales, qs, d, dmin = b[0:16], b[16:80], half_at(b, 80), half_at(b, 82)
    out, s = [], 0
    for n in range(2):
        q = qs[32 * n:32 * n + 32]
        for shift in (0, 2, 4, 6):
            for half in (0, 16):
                sc = scales[s]
                s += 1
                for l in range(16):
                    out.append(d * (sc & 15) * ((q[half + l] >> shift) & 3) - dmin * (sc >> 4))
    return out


def q3_k(b):
    hmask, qs, packed, d = b[0:32], b[32:96], b[96:108], half_at(b, 108)
    # Sixteen 6-bit scales: low nibbles of bytes 0-7 then their high
    # nibbles; the top two bits of each from bytes 8-11, two bits at a time.
    scales = []
    for j in range(16):
        low = packed[j] & 15 if j < 8 else packed[j - 8] >> 4
        scales.append((low | ((packed[8 + j % 4] >> (2 * (j // 4))) & 3) << 4) - 32)
    out, s, m = [], 0, 1
    for n in range(2):
        q = qs[32 * n:32 * n + 32]
        for shift in (0, 2, 4, 6):
            for half in (0, 16):
                for l in range(16):
                    low = (q[half + l] >> shift) & 3
                    out.append(d * scales[s] * (low - (0 if hmask[half + l] & m else 4)))
                s += 1
            m <<= 1
    return out


def scale_min_k4(j, q):
    if j < 4:
        return q[j] & 63, q[j + 4] & 63
    return (q[j + 4] & 15) | (q[j - 4] >> 6) << 4, (q[j + 4] >> 4) | (q[j] >> 6) << 4


def q45_k(b, five):
    d, dmin, packed = half_at(b, 0), half_at(b, 2), b[4:16]
    qh = b[16:48] if five else bytes(32)
    qs = b[48:176] if five else b[16:144]
    out = []
    for g in range(4):
        for part in (0, 1):
            sc, mn = scale_min_k4(2 * g + part, packed)
            for l in range(32):
                q = (qs[32 * g + l] >> (4 * part)) & 15
                if (qh[l] >> (2 * g + part)) & 1:
                    q += 16
                out.append(d * sc * q - dmin * mn)
    return out


def q6_k(b):
    ql, qh, d = b[0:128], b[128:192], half_at(b, 208)
    sc = [v - 256 if v > 127 else v for v in b[192:208]]
    out = [0] * 256
    for n in range(2):
        for l in range(32):
            i = l // 16
            lo, hi = ql[64 * n + l], ql[64 * n + 32 + l]
            h = qh[32 * n + l]
            quads = ((lo & 15) | (h & 3) << 4, (hi & 15) | (h >> 2 & 3) << 4,
                     (lo >> 4) | (h >> 4 & 3) << 4, (hi >> 4) | (h >> 6 & 3) << 4)
            for part, q in enumerate(quads):
                out[128 * n + 32 * part + l] = d * sc[8 * n + i + 2 * part] * (q - 32)
    return out


def iq4_nl(b):
    d, qs = half_at(b, 0), b[2:18]
    return [d * IQ4[v & 15] for v in qs] + [d * IQ4[v >> 4] for v in qs]


def iq4_xs(b):
    d, high, low, qs = half_at(b, 0), b[2] | b[3] << 8, b[4:8], b[8:136]
    out = []
    for ib in range(8):
        ls = (low[ib // 2] >> (4 * (ib % 2)) & 15) | ((high >> (2 * ib)) & 3) << 4
        part = qs[16 * ib:16 * ib + 16]
        out += [d * (ls - 32) * IQ4[v & 15] for v in part] + [d * (ls - 32) * IQ4[v >> 4] for v in part]
    return out


DECODE = {"Q2_K": q2_k, "Q3_K": q3_k, "Q4_K": lambda b: q45_k(b, False),
          "Q5_K": lambda b: q45_k(b, True), "Q6_K": q6_k, "IQ4_NL": iq4_nl, "IQ4_XS": iq4_xs}

# Scales: subnormal, small, ordinary, negative, the largest finite half.
HALVES = (0x0001, 0x03ff, 0x1234, 0x2c00, 0x3555, 0x3c00, 0xb800, 0x4a3f, 0x7bff, 0xfbff, 0x0400, 0x2e66)


def block_bytes(name, index):
    _, _, size, halves = FORMATS[name]
    state = 977 * (index + 1) + sum(map(ord, name))
    out = bytearray()
    for _ in range(size):
        state = (state * 1103515245 + 12345) % 2147483648
        out.append((state >> 16) & 255)
    for k, at in enumerate(halves):
        h = HALVES[(3 * index + 5 * k + len(name)) % len(HALVES)]
        out[at], out[at + 1] = h & 255, h >> 8
    return bytes(out)


def vectors():
    result = {}
    for name in FORMATS:
        kind, values, size, _ = FORMATS[name]
        data, bits = b"", []
        for index in range(BLOCKS.get(name, 2)):
            b = block_bytes(name, index)
            decoded = DECODE[name](b)
            assert len(decoded) == values and len(b) == size
            data += b
            bits += [f32_bits(v) for v in decoded]
        result[name] = (kind, data, bits)
    return result


def checks():
    assert f32_bits(1 << 24) == 0x3f800000 and f32_bits(-(1 << 23)) == 0xbf000000
    assert f32_bits(1) == 0x33800000 and f32_bits((1 << 24) + 1) == 0x3f800000
    assert f32_bits((1 << 25) + 3) == 0x40000001 and f32_bits((1 << 25) + 2) == 0x40000000 and f32_bits((1 << 25) + 6) == 0x40000002
    assert f32_bits((1 << 26) - 1) == 0x40800000
    for bits in range(0x7c00):
        if bits & 1023 == 0 or bits < 1024:
            units = half_units(bits)
            assert struct.unpack("<f", struct.pack("<I", f32_bits(units)))[0] == units / 2 ** 24 or units == 0
    # One hand block per family, the same blocks the chapter decodes by hand.
    b = bytearray(84)
    b[0], b[16], b[80], b[81], b[82], b[83] = 0x21, 0b11100100, 0x00, 0x38, 0x00, 0x3c
    v = q2_k(bytes(b))
    assert [x / 2 ** 24 for x in v[:4]] == [-2.0, -2.0, -2.0, -2.0] and v[16] == 0
    assert struct.calcsize("<" + "B" * 84) == FORMATS["Q2_K"][2]


def fixture(result):
    lines = ["// Generated by psionic/tests/kquant_oracle.py from ggml's block structs; exact",
             "// integers in 2^-24 units rounded to binary32 by integer arithmetic. Do not edit."]
    for name, (kind, data, bits) in result.items():
        tag = name.replace("_", "")
        lines.append(f"KQV_{tag}_KIND :: {kind};")
        lines.append(f"KQV_{tag}_BYTES :: [{len(data)}]u8.[")
        for start in range(0, len(data), 24):
            lines.append("    " + ",".join(str(c) for c in data[start:start + 24]) + ",")
        lines.append("];")
        lines.append(f"KQV_{tag}_BITS :: [{len(bits)}]u32.[")
        for start in range(0, len(bits), 8):
            lines.append("    " + ",".join(f"0x{c:08x}" for c in bits[start:start + 8]) + ",")
        lines.append("];")
    return "\n".join(lines + [""])


def main():
    checks()
    result = vectors()
    expected = fixture(result)
    if sys.argv[1:] == ["--write"]:
        TARGET.write_text(expected)
        return
    assert not sys.argv[1:], "usage: kquant_oracle.py [--write]"
    assert TARGET.read_text() == expected, "K-quant fixture drifted"
    total = sum(len(bits) for _, _, bits in result.values())
    print(f"K-quant oracle: {len(result)} formats, {total} decoded values match the fixture")


if __name__ == "__main__":
    main()
