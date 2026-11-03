# 47. Block headers and proof of work

Every Bitcoin block starts with an 80-byte header. The header names the
block before it, commits to the block's transactions through chapter 46's
Merkle root, and carries a number, `nBits`, that says how hard the header
was to make. Its hash, read as a 256-bit number, must not be above the
target `nBits` encodes. Finding such a header takes, on average, a number
of hashes that you can compute from the target alone: the header's
*work*. The chain with the most work is the one nodes follow.

This chapter builds the header and its hash, 256-bit unsigned numbers in
the limbs of chapter 2, `nBits` with every quirk of Bitcoin Core's
`SetCompact` and `GetCompact`, Core's proof-of-work check and the work of
one header. It checks them on the genesis block, eleven headers around the
first change of difficulty in 2009, three later mainnet headers, Bitcoin
Core's own tests and a Python transliteration of Core, natively and in
Wasm.

## What this chapter teaches

- The six fields of the 80-byte header, their byte order, and why the
  version is signed.
- How the block hash is computed and why explorers print it reversed.
- 256-bit unsigned arithmetic as Core's `arith_uint256` does it: eight
  32-bit limbs, shifts that drop bits, and division by shift and subtract.
- The compact form `nBits`: a size byte and a three-byte mantissa, its
  sign bit, how Core decodes values that are negative or too large, and
  why encoding is not always the inverse of decoding.
- Core's proof-of-work check, its four refusals of a target in Core's
  order, and the mainnet `powLimit`.
- The work of a header, `2^256 / (target + 1)`, and how Core computes it
  without a 257-bit number.

## Where Bitcoin uses headers and proof of work

| Where | What it uses | Chapter |
| --- | --- | --- |
| `CheckBlockHeader`: "high-hash" | the hash against the target `nBits` names | this chapter |
| The chain's links | each header's hash of the one before | this chapter, 59 |
| Retargeting: "bad-diffbits" | the target every 2,016 blocks, from the time they took | 48 |
| Time and version rules | median time past, the two-hour limit, deployments | 51 |
| Chain selection | the sum of every header's work, `nChainWork` | 59 |
| Headers-first sync | 80-byte headers before blocks | 72 |

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [pow.coax](../../coax/lib/bitcoin/pow.coax) | `U256`, `u256_small`, `u256_from_hash_vartime`, `u256_to_hash_vartime` | Core's `arith_uint256`, and the bytes of a hash as a number |
| | `u256_compare_vartime`, `u256_is_zero`, `u256_bits_vartime` | comparison, zero and the bit length |
| | `u256_shl_vartime`, `u256_shr_vartime`, `u256_add`, `u256_sub`, `u256_not` | shifts, wrapping addition and subtraction, complement |
| | `u256_div_vartime(a, b)` | Core's `operator/=` |
| | `pow_compact_decode_vartime(bits)`, `pow_compact_encode_vartime(target, negative)` | Core's `SetCompact` and `GetCompact` |
| | `pow_target_vartime(bits, limit)` | Core's `DeriveTarget`: the target, or the reason it is refused |
| | `pow_check_vartime(hash, bits, limit)` | Core's `CheckProofOfWorkImpl` |
| | `pow_work_vartime(bits)` | Core's `GetBitsProof`: the work of one header |
| | `pow_header_read_vartime`, `pow_header_write_vartime` | the 80 bytes and the six fields |
| | `pow_block_hash_vartime`, `pow_header_check_vartime`, `pow_links_vartime` | the hash, Core's `CheckBlockHeader`, and whether one header follows another |
| [hash.coax](../../coax/lib/bitcoin/hash.coax) | `sha256d_checked`, `bitcoin_reverse32` | SHA-256d and the display order (chapter 5) |

Headers, targets and work are public, so every procedure that decides
runs in variable time and its name ends in `_vartime` (chapter 3). The
arithmetic that never branches on a value, such as addition, keeps a
plain name.

Every procedure that checks returns one of these codes:

<!-- source: coax/lib/bitcoin/pow.coax codes -->
```jai
// Result codes. Core refuses every failure of the check, 1 to 5, with one
// reject reason: "high-hash", "proof of work failed".
POW_OK :: 0;
POW_NEGATIVE :: 1;     // nBits sets the sign bit over a nonzero mantissa
POW_ZERO :: 2;         // nBits decodes to a target of zero
POW_OVERFLOW :: 3;     // nBits names a target of more than 256 bits
POW_ABOVE_LIMIT :: 4;  // the target is easier than the chain's powLimit
POW_HIGH_HASH :: 5;    // the header's hash is above its target
POW_CAPACITY :: 6;     // a header is not 80 bytes, or a view is the wrong size

POW_HEADER :: 80;
```

## The 80-byte header

A header is six fields, 80 bytes in all. Every number is little-endian,
and both hashes are in hash order, the bytes SHA-256d produced:

| Bytes | Field | Type | What it holds |
| --- | --- | --- | --- |
| 0–3 | `nVersion` | signed 32-bit | the block's version; since BIP-9 its bits signal deployments |
| 4–35 | `hashPrevBlock` | 32-byte hash | the hash of the block before, zero for the genesis block |
| 36–67 | `hashMerkleRoot` | 32-byte hash | the root of the block's txids (chapter 46) |
| 68–71 | `nTime` | unsigned 32-bit | seconds since 1970, as the miner claims it |
| 72–75 | `nBits` | unsigned 32-bit | the target, in compact form |
| 76–79 | `nNonce` | unsigned 32-bit | the number a miner changes to find a hash below the target |

Core's `CBlockHeader` declares the version as `int32_t`, so a version with
its top bit set is negative. No valid mainnet block has one, but a reader
must still give every 80 bytes the meaning Core gives them, and a writer
must write them back unchanged. The reader returns `POW_CAPACITY` for
anything other than 80 bytes; the writer refuses a version outside the
signed 32-bit range.

<!-- source: coax/lib/bitcoin/pow.coax header -->
```jai
// An 80-byte header: version (a signed 32-bit number), the previous
// block's hash and the Merkle root (hash order), then time, nBits and
// nonce, every number little-endian. code is POW_OK or POW_CAPACITY.
BlockHeader :: struct {
  code: int; version: int;
  prev: [32] u8; merkle_root: [32] u8;
  time: u32; bits: u32; nonce: u32;
}
pw_le32 :: (b: [] u8, at: int) -> u32 {
  return cast(u32) b[at] | (cast(u32) b[at+1] << 8) | (cast(u32) b[at+2] << 16) | (cast(u32) b[at+3] << 24);
}
pow_header_read_vartime :: (b: [] u8) -> BlockHeader #export_c {
  h: BlockHeader;
  if b.count != POW_HEADER { h.code = POW_CAPACITY; return h; }
  v := cast(int) pw_le32(b, 0);
  h.version = ifx v >= 0x80000000 then v - 0x100000000 else v;
  for i: 0..<32 { h.prev[i] = b[4+i]; h.merkle_root[i] = b[36+i]; }
  h.time = pw_le32(b, 68); h.bits = pw_le32(b, 72); h.nonce = pw_le32(b, 76);
  return h;
}
pow_header_write_vartime :: (h: BlockHeader, out: [] u8) -> bool #export_c {
  if out.count != POW_HEADER || h.version < -0x80000000 || h.version > 0x7fffffff { return false; }
  v := cast,trunc(u32) h.version;    // the low 32 bits: two's complement
  words := [4] u32.[v, h.time, h.bits, h.nonce];
  at := [4] int.[0, 68, 72, 76];
  for k: 0..<4 { for j: 0..<4 { out[at[k]+j] = cast,trunc(u8) (words[k] >> (8 * cast(u32) j)); } }
  for i: 0..<32 { out[4+i] = h.prev[i]; out[36+i] = h.merkle_root[i]; }
  return true;
}
```

### Worked example: the genesis header

The genesis header is in Bitcoin Core's `chainparams.cpp`, built from its
fields by `CreateGenesisBlock(1231006505, 2083236893, 0x1d00ffff, 1, ...)`.
The example reads it and prints each field:

```text
genesis header, 80 bytes
  version: 1
  previous block: 0000000000000000000000000000000000000000000000000000000000000000
  Merkle root: 4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b
  time: 1231006505
  nBits: 1d00ffff
  nonce: 2083236893
```

The time is 3 January 2009, 18:15:05 UTC. Writing the six fields back
gives the same 80 bytes.

## The block hash

A block's hash is SHA-256d of its 80-byte header, nothing else: the
transactions reach the hash only through the Merkle root. Explorers and
RPCs print the 32 bytes reversed, so the zeros a miner worked for appear
at the front (chapter 5).

<!-- source: coax/lib/bitcoin/pow.coax hash -->
```jai
// The block hash: SHA-256d of the 80 bytes, in hash order. Explorers show
// it reversed (chapter 5).
pow_block_hash_vartime :: (header: [] u8, out: [] u8) -> int #export_c {
  if header.count != POW_HEADER || out.count != 32 { return POW_CAPACITY; }
  #assert sha256d_checked(header, out, false) == SHA_OK;
  return POW_OK;
}
// Core's CheckBlockHeader: the header's hash must meet its own nBits under
// the chain's limit; any failure is "high-hash".
pow_header_check_vartime :: (header: [] u8, limit: U256) -> int #export_c {
  h := pow_header_read_vartime(header);
  if h.code != POW_OK { return h.code; }
  hash: [32] u8;
  #assert pow_block_hash_vartime(header, hash[..]) == POW_OK;
  return pow_check_vartime(hash[..], h.bits, limit);
}
// Does header name prev as the block before it? Its bytes 4 to 36 must be
// prev's hash.
pow_links_vartime :: (header: [] u8, prev: [] u8) -> bool #export_c {
  if header.count != POW_HEADER || prev.count != POW_HEADER { return false; }
  hash: [32] u8;
  #assert pow_block_hash_vartime(prev, hash[..]) == POW_OK;
  for i: 0..<32 { if header[4+i] != hash[i] { return false; } }
  return true;
}
```

```text
hash, as SHA-256d gives it: 6fe28c0ab6f1b372c1a6a246ae63f74f931e8365e15a089c68d6190000000000
hash, as explorers show it: 000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f
```

The second line is `consensus.hashGenesisBlock` in `chainparams.cpp`.
`pow_links_vartime` compares a header's bytes 4 to 36 with the hash of
the header before it. A chain of headers is a list in which every
neighbouring pair links; chapter 59 builds the index that holds them.

## 256-bit numbers

The proof-of-work check compares the hash with the target as 256-bit
numbers, and the work needs a 256-bit division. Coax has no 256-bit type,
so `pow.coax` uses chapter 2's representation for scalars: eight 32-bit
limbs, lowest first, in `u32`. This is the layout of Core's
`arith_uint256` (`base_uint<256>`, `pn[0]` lowest). Unlike chapter 2's
scalars, these numbers have no modulus: they are plain unsigned integers
below `2^256`.

A hash becomes a number by reading its 32 bytes as one little-endian
integer, Core's `UintToArith256`. That is why a hash with many zero bytes
at the *end*, in hash order, is a small number.

<!-- source: coax/lib/bitcoin/pow.coax u256 -->
```jai
// A 256-bit unsigned number: Core's arith_uint256, eight 32-bit limbs,
// limb[0] lowest.
U256 :: struct { limb: [8] u32; }
// A number below 2^64.
u256_small :: (v: u64) -> U256 #export_c {
  r: U256;
  r.limb[0] = cast,trunc(u32) v;
  r.limb[1] = cast,trunc(u32) (v >> 32);
  return r;
}
// The low 64 bits: Core's GetLow64.
u256_low64 :: (a: U256) -> u64 #export_c {
  return cast(u64) a.limb[0] | (cast(u64) a.limb[1] << 32);
}
// Core's UintToArith256: a hash's 32 bytes as a little-endian number. A
// view of another size is zero.
u256_from_hash_vartime :: (hash: [] u8) -> U256 #export_c {
  r: U256;
  if hash.count != 32 { return r; }
  for i: 0..<8 {
    r.limb[i] = cast(u32) hash[4*i] | (cast(u32) hash[4*i+1] << 8) | (cast(u32) hash[4*i+2] << 16) | (cast(u32) hash[4*i+3] << 24);
  }
  return r;
}
// Core's ArithToUint256: the number as 32 little-endian bytes.
u256_to_hash_vartime :: (a: U256, out: [] u8) -> bool #export_c {
  if out.count != 32 { return false; }
  for i: 0..<8 { for j: 0..<4 { out[4*i+j] = cast,trunc(u8) (a.limb[i] >> (8 * cast(u32) j)); } }
  return true;
}
// -1, 0 or 1 as a is below, equal to or above b, from the top limb down.
u256_compare_vartime :: (a: U256, b: U256) -> int #export_c {
  for k: 0..<8 {
    i := 7 - k;
    if a.limb[i] < b.limb[i] { return -1; }
    if a.limb[i] > b.limb[i] { return 1; }
  }
  return 0;
}
u256_is_zero :: (a: U256) -> bool #export_c {
  for i: 0..<8 { if a.limb[i] != 0 { return false; } }
  return true;
}
// Core's bits(): the position of the highest set bit plus one; 0 for zero.
u256_bits_vartime :: (a: U256) -> int #export_c {
  for k: 0..<8 {
    i := 7 - k;
    if a.limb[i] != 0 {
      n := 0;
      while (a.limb[i] >> cast(u32) n) > 1 { n += 1; }
      return 32 * i + n + 1;
    }
  }
  return 0;
}
```

Shifts follow Core's `operator<<=` and `operator>>=`. A shift moves whole
limbs by `shift / 32` and bits by `shift % 32`. The bits that leave the
top or the bottom are lost, so a shift by 256 or more gives zero. The
compact decoder relies on this: an `nBits` with a size byte of 255 asks
for a shift of 2,016 bits.

<!-- source: coax/lib/bitcoin/pow.coax shift -->
```jai
// Core's operator<<= and operator>>=: bits that leave the 256 are lost, so
// a shift of 256 or more gives zero. A negative shift is zero as well.
u256_shl_vartime :: (a: U256, shift: int) -> U256 #export_c {
  r: U256;
  if shift < 0 { return r; }
  k := shift / 32; s := cast(u64) (shift % 32);
  for i: 0..<8 {
    wide := cast(u64) a.limb[i] << s;
    if i + k < 8 { r.limb[i+k] = r.limb[i+k] | cast,trunc(u32) wide; }
    if i + k + 1 < 8 { r.limb[i+k+1] = r.limb[i+k+1] | cast,trunc(u32) (wide >> 32); }
  }
  return r;
}
u256_shr_vartime :: (a: U256, shift: int) -> U256 #export_c {
  r: U256;
  if shift < 0 { return r; }
  k := shift / 32; s := cast(u64) (shift % 32);
  for i: 0..<8 {
    wide := (cast(u64) a.limb[i] << 32) >> s;
    if i - k >= 0 { r.limb[i-k] = r.limb[i-k] | cast,trunc(u32) (wide >> 32); }
    if i - k - 1 >= 0 { r.limb[i-k-1] = r.limb[i-k-1] | cast,trunc(u32) wide; }
  }
  return r;
}
```

Addition and subtraction carry and borrow through the limbs and wrap at
`2^256`, as Core's do. The complement flips every bit, which is
`2^256 - 1 - a`.

<!-- source: coax/lib/bitcoin/pow.coax arith -->
```jai
// a + b and a - b, wrapping at 2^256 as Core's do.
u256_add :: (a: U256, b: U256) -> U256 #export_c {
  r: U256; carry := cast(u64) 0;
  for i: 0..<8 {
    wide := cast(u64) a.limb[i] + cast(u64) b.limb[i] + carry;
    r.limb[i] = cast,trunc(u32) wide; carry = wide >> 32;
  }
  return r;
}
u256_sub :: (a: U256, b: U256) -> U256 #export_c {
  r: U256; borrow := cast(u64) 0;
  for i: 0..<8 {
    wide := (cast(u64) 1 << 32) + cast(u64) a.limb[i] - cast(u64) b.limb[i] - borrow;
    r.limb[i] = cast,trunc(u32) wide; borrow = 1 - (wide >> 32);
  }
  return r;
}
// ~a: every bit flipped, which is 2^256 - 1 - a.
u256_not :: (a: U256) -> U256 #export_c {
  r: U256;
  for i: 0..<8 { r.limb[i] = a.limb[i] ^ 0xffffffff; }
  return r;
}
```

Division is the schoolbook method in base 2, as Core writes it. Shift the
divisor left until its top bit lines up with the dividend's top bit. Then,
for each position from there down to zero, subtract the divisor if it
fits and set that bit of the quotient, and shift the divisor right by one.
It takes at most 256 steps.

<!-- source: coax/lib/bitcoin/pow.coax divide -->
```jai
// Core's operator/=, shift and subtract: align the divisor's top bit with
// the dividend's, then take one quotient bit per step. A zero divisor is
// not ok (Core throws).
U256Quotient :: struct { ok: bool; q: U256; }
u256_div_vartime :: (a: U256, b: U256) -> U256Quotient #export_c {
  r: U256Quotient;
  num_bits := u256_bits_vartime(a); div_bits := u256_bits_vartime(b);
  if div_bits == 0 { return r; }
  r.ok = true;
  if div_bits > num_bits { return r; }
  shift := num_bits - div_bits;
  num := a; div := u256_shl_vartime(b, shift);
  while shift >= 0 {
    if u256_compare_vartime(num, div) >= 0 {
      num = u256_sub(num, div);
      r.q.limb[shift/32] = r.q.limb[shift/32] | (cast(u32) 1 << cast(u32) (shift % 32));
    }
    div = u256_shr_vartime(div, 1);
    shift -= 1;
  }
  return r;
}
```

## Compact targets

A target is a 256-bit number, but a header has 32 bits for it. `nBits`
stores it like a floating-point number in base 256:

- the top byte is the *size*: how many bytes the number takes;
- the low 23 bits are the *mantissa*: the number's top three bytes;
- bit 23, `0x00800000`, is a sign bit, a leftover of OpenSSL's MPI format
  that Satoshi's first client used.

The target is `mantissa × 256^(size − 3)`. For the genesis block's
`1d00ffff`, the size is `0x1d`, 29, and the mantissa `0x00ffff`, so the
target is `0xffff × 256^26`: four zero bytes, `ffff`, then 26 zero bytes.

Core decodes `nBits` with shifts, not multiplication, and reports two
flags. *Negative* means the sign bit is set over a nonzero mantissa.
*Overflow* means the number does not fit in 256 bits: a size above 34, a
mantissa above `0xff` with a size above 33, or above `0xffff` with a size
above 32. The flags use the mantissa *after* a size below 3 has shifted
it right, so `01803456` is not negative: its mantissa `003456`, shifted
right by two bytes, is zero.

Encoding picks the shortest size that holds the number and takes its top
three bytes. If the mantissa's top bit is set it would read as the sign,
so the encoder shifts the mantissa right by one byte and adds one to the
size. `0x80` encodes as `02008000`, not `01800000`.

<!-- source: coax/lib/bitcoin/pow.coax compact -->
```jai
// nBits decoded: the target and Core's two flags. negative: the sign bit
// (0x00800000) is set over a nonzero mantissa. overflow: the number needs
// more than 256 bits.
Compact :: struct { target: U256; negative: bool; overflow: bool; }
// Core's SetCompact. The top byte is the size in bytes; the low 23 bits
// are the mantissa, the number's top three bytes. A size below 3 shifts
// the mantissa right and loses bytes; a larger size shifts it left, and
// bits past 256 are lost, which the overflow flag reports. Both flags look
// at the mantissa after a short size has shifted it, as Core's do.
pow_compact_decode_vartime :: (bits: u32) -> Compact #export_c {
  size := cast(int) (bits >> 24);
  word := bits & 0x007fffff;
  c: Compact;
  if size <= 3 {    // mutant: equivalent (at size 3 both branches shift by zero)
    word = word >> (8 * cast(u32) (3 - size));
    c.target = u256_small(cast(u64) word);
  } else {
    c.target = u256_shl_vartime(u256_small(cast(u64) word), 8 * (size - 3));
  }
  c.negative = word != 0 && (bits & 0x00800000) != 0;
  c.overflow = word != 0 && (size > 34 || (word > 0xff && size > 33) || (word > 0xffff && size > 32));
  return c;
}
// Core's GetCompact: the shortest size that holds the number, its top
// three bytes as the mantissa, and one byte more when the mantissa's top
// bit would read as the sign. negative sets the sign bit, except on zero.
pow_compact_encode_vartime :: (target: U256, negative: bool) -> u32 #export_c {
  size := (u256_bits_vartime(target) + 7) / 8;
  compact: u64;
  if size <= 3 {    // mutant: equivalent (at size 3 both branches shift by zero)
    compact = u256_low64(target) << (8 * cast(u64) (3 - size));
  } else {
    compact = u256_low64(u256_shr_vartime(target, 8 * (size - 3)));
  }
  if (compact & 0x00800000) != 0 { compact = compact >> 8; size += 1; }
  compact = compact | (cast(u64) size << 24);
  if negative && (compact & 0x007fffff) != 0 { compact = compact | 0x00800000; }
  return cast,trunc(u32) compact;
}
```

### Worked example: Core's quirks

The example decodes the three mainnet values it uses and the cases from
Core's `arith_uint256_tests.cpp` and `pow_tests.cpp`, then encodes each
target again:

```text
compact targets
1d00ffff: size 29, negative false, overflow false, encodes back as 1d00ffff
  target: 00000000ffff0000000000000000000000000000000000000000000000000000
1d00d86a: size 29, negative false, overflow false, encodes back as 1d00d86a
  target: 00000000d86a0000000000000000000000000000000000000000000000000000
17514a49: size 23, negative false, overflow false, encodes back as 17514a49
  target: 000000000000000000514a490000000000000000000000000000000000000000
01123456: size 1, negative false, overflow false, encodes back as 01120000
  target: 0000000000000000000000000000000000000000000000000000000000000012
01fedcba: size 1, negative true, overflow false, encodes back as 01fe0000
  target: 000000000000000000000000000000000000000000000000000000000000007e
01803456: size 1, negative false, overflow false, encodes back as 00000000
  target: 0000000000000000000000000000000000000000000000000000000000000000
04923456: size 4, negative true, overflow false, encodes back as 04923456
  target: 0000000000000000000000000000000000000000000000000000000012345600
05009234: size 5, negative false, overflow false, encodes back as 05009234
  target: 0000000000000000000000000000000000000000000000000000000092340000
21012345: size 33, negative false, overflow true, encodes back as 20234500
  target: 2345000000000000000000000000000000000000000000000000000000000000
ff123456: size 255, negative false, overflow true, encodes back as 00000000
  target: 0000000000000000000000000000000000000000000000000000000000000000
0x80 encodes as 02008000, not 01800000
```

Four things to notice:

- A size below 3 drops mantissa bytes: `01123456` is just `0x12`, and it
  encodes back as `01120000`. Many `nBits` values name the same target;
  only the encoder's output is canonical.
- The sign bit makes a value negative only when the mantissa that remains
  is nonzero. `01fedcba` is negative 126; `01803456` is zero.
- An overflowing value still decodes to a number: the bits that fit.
  `21012345` asks for `0x012345 × 256^30`, which needs 257 bits; the top
  bit is lost, and what remains encodes as `20234500`. Core's
  `GetBitsProof_test` has this case to show that the overflow flag, not
  the value, makes its work zero.
- The size byte is unsigned. `80010203` is a size of 128, which shifts the
  mantissa out of the number entirely: zero, with the overflow flag set.

The `nBits` of every mainnet header in this chapter is canonical: encoding
its target gives the same 32 bits back.

## The proof-of-work check

Core's `CheckProofOfWorkImpl` decodes `nBits` with `DeriveTarget`, which
refuses a target that is negative, zero, overflows or is above the chain's
`powLimit`, in one condition with that order. The codes here name the
first that holds. Then it reads the hash as a number and refuses a hash
above the target. A hash equal to the target passes.

`powLimit` is the easiest target a chain allows. On mainnet it is
`2^224 − 1`, 32 zero bits followed by 224 one bits. Its compact form is
the genesis block's `1d00ffff`, which names the slightly smaller
`0xffff × 256^26`, because the encoding keeps only three bytes.
Chapter 50 gives the limits of testnet4, signet and regtest.

<!-- source: coax/lib/bitcoin/pow.coax limit -->
```jai
// The mainnet powLimit (chainparams.cpp): 2^224 - 1, the easiest target a
// mainnet header may claim. Chapter 50 gives the other networks'.
POW_LIMIT_MAINNET :: U256.{limb=[8]u32.[0xffffffff, 0xffffffff, 0xffffffff, 0xffffffff, 0xffffffff, 0xffffffff, 0xffffffff, 0]};
```

<!-- source: coax/lib/bitcoin/pow.coax check -->
```jai
// A target from nBits, or the reason it is refused: Core's DeriveTarget,
// which tests negative, zero, overflow and the limit in that order.
PowTarget :: struct { code: int; target: U256; }
pow_target_vartime :: (bits: u32, limit: U256) -> PowTarget #export_c {
  c := pow_compact_decode_vartime(bits);
  r := PowTarget.{target=c.target};
  if c.negative { r.code = POW_NEGATIVE; }
  else if u256_is_zero(c.target) { r.code = POW_ZERO; }
  else if c.overflow { r.code = POW_OVERFLOW; }
  else if u256_compare_vartime(c.target, limit) > 0 { r.code = POW_ABOVE_LIMIT; }
  return r;
}
// Core's CheckProofOfWorkImpl: the hash, in the order SHA-256d gives it,
// read as a little-endian number, must not be above the target.
pow_check_vartime :: (hash: [] u8, bits: u32, limit: U256) -> int #export_c {
  if hash.count != 32 { return POW_CAPACITY; }
  t := pow_target_vartime(bits, limit);
  if t.code != POW_OK { return t.code; }
  if u256_compare_vartime(u256_from_hash_vartime(hash), t.target) > 0 { return POW_HIGH_HASH; }
  return POW_OK;
}
// Core's GetBitsProof: the expected number of hashes to meet the target,
// 2^256 / (target + 1). 2^256 does not fit, so Core computes
// ~target / (target + 1) + 1, which is equal. A target that is negative,
// zero or overflows proves no work.
pow_work_vartime :: (bits: u32) -> U256 #export_c {
  c := pow_compact_decode_vartime(bits);
  if c.negative || c.overflow || u256_is_zero(c.target) { return U256.{}; }
  q := u256_div_vartime(u256_not(c.target), u256_add(c.target, u256_small(1)));
  #assert q.ok;    // target + 1 is never zero: a target that fits is below 2^256 - 1
  return u256_add(q.q, u256_small(1));
}
```

`CheckBlockHeader` in `validation.cpp` calls this check on the header's
hash, and every failure has one reject reason: "high-hash", "proof of
work failed". The codes say which part failed. Core marks such a header
invalid (`BLOCK_INVALID_HEADER`): unlike chapter 46's mutated block, the
header's own bytes are wrong, so no other copy of the same header can be
valid.

### Worked example: the refusals

The example checks the hash `1`, the smallest possible, against seven
`nBits` values, then a hash equal to the genesis target and one more:

```text
refusals
  hash 1 against 1d00ffff: ok
  hash 1 against 1d80ffff: negative
  hash 1 against 01003456: zero
  hash 1 against 80010203: zero
  hash 1 against 21012345: overflow
  hash 1 against 1d01ffff: above the limit
  hash 1 against 1e00ffff: above the limit
  a hash equal to the genesis target: ok
  one more: high hash
```

`80010203` is both zero and an overflow. Core refuses both in one
condition that names zero first, so the code here is `POW_ZERO`. Core's
`CheckProofOfWork_test_overflow_target` uses `~0x00800000`, `ff7fffff`,
which is the same kind of case: a zero target with the overflow flag. `1d01ffff` and `1e00ffff`
are well-formed targets that are easier than mainnet allows.

### Worked example: one changed bit

The header's hash covers every one of its 640 bits. Changing any of them
gives a hash that is, in effect, a fresh random number, and a random
number is below the genesis target with probability about `2^-32`. The
example flips the lowest bit of the nonce:

```text
with the nonce's low bit flipped: 8cdc343dc1ca87b0fae7d09a6537a536fec9784a3c4541b770bfb5881c5d66d9
check: high hash
```

The library's checks flip each of the genesis header's 640 bits in turn,
and the example's checks do the same for block 900,000: every one fails.
A flipped bit in `nBits` itself fails too: the new target is refused, or
the new hash is above it.

## Work and chainwork

A hash is below the target `T` with probability `(T + 1) / 2^256`, so a
miner needs `2^256 / (T + 1)` hashes on average to find a valid header.
That number is the header's work. Core computes it in `GetBitsProof`. A
target that is negative, zero or overflows proves no work.

`2^256` needs 257 bits, so Core does not compute it. Instead it uses

```text
2^256 / (T + 1) = (2^256 − T − 1) / (T + 1) + 1 = ~T / (T + 1) + 1
```

which holds for integer division because `2^256` is at least `T + 1`.
`~T` is the complement, `2^256 − 1 − T`, and fits in 256 bits.

For the genesis target, `0xffff × 256^26`, the work is `0x100010001`,
4,295,032,833: a little over `2^32`, because the target is a little under
`2^224`. The difficulty that explorers show is the genesis target divided
by the block's, so the work is about the difficulty times `2^32`.

A node adds the work of every header from the genesis block to get
`nChainWork`, and follows the chain with the most. Chapter 59 builds that
index; this chapter adds the work of a short chain.

### Worked example: blocks 32,250 to 32,260

Blocks 0 to 32,255 all have the genesis target. The first retarget
that changed it came at block 32,256, on 30 December 2009, when the
target fell to `1d00d86a`: difficulty 1.18. The example reads eleven
headers across that change, checks that each names the one before, and
adds their work:

```text
blocks 32,250 to 32,260
  32250 000000002931f8c7… nBits 1d00ffff work 4295032833 links true check ok
  32251 00000000360d826e… nBits 1d00ffff work 4295032833 links true check ok
  32252 000000003d5eb232… nBits 1d00ffff work 4295032833 links true check ok
  32253 000000000e27ee2a… nBits 1d00ffff work 4295032833 links true check ok
  32254 000000006baebaa7… nBits 1d00ffff work 4295032833 links true check ok
  32255 00000000984f9621… nBits 1d00ffff work 4295032833 links true check ok
  32256 000000004f2886a1… nBits 1d00d86a work 5080592338 links true check ok
  32257 0000000078bb40a0… nBits 1d00d86a work 5080592338 links true check ok
  32258 000000006e35246a… nBits 1d00d86a work 5080592338 links true check ok
  32259 000000008a5b32a0… nBits 1d00d86a work 5080592338 links true check ok
  32260 0000000007bfbe7e… nBits 1d00d86a work 5080592338 links true check ok
  total work: 51173158688
  32,251 after 32,252: links false
```

Why the target changed at 32,256, and to that value, is chapter 48's
retargeting rule: 32,256 is a multiple of 2,016, and the 2,016 blocks
before it took less than two weeks.

Later headers have far smaller targets, so far more work each:

```text
later blocks
block 100000 nBits 1b04864c: ok
  hash: 000000000003ba27aa200b1cecaad478d2b00432346c3f1f3986da1afd33e506
  target: 000000000004864c000000000000000000000000000000000000000000000000
  work: 62209952899966
block 514999 nBits 17514a49: ok
  hash: 00000000000000000008fda0e5bb3105dab72605332085684c42dceb0ea61097
  target: 000000000000000000514a490000000000000000000000000000000000000000
  work: 00000000000000000000000000000000000000000000032632c22a59ba6d1d63
block 900000 nBits 17023774: ok
  hash: 000000000000000000010538edbfd2d5b809a33dd83f284aeea41c6d0d96968a
  target: 0000000000000000000237740000000000000000000000000000000000000000
  work: 00000000000000000000000000000000000000000000737dd17d8cead8884cdc
```

Block 514,999's work is about `1.49 × 10^22` hashes and block 900,000's
about `5.45 × 10^23`. The genesis target divided by block 514,999's is
3,462,542,391,191.56, the difficulty mempool.space publishes for it, and
`make_pow_vectors.py` checks it for blocks 100,000 and 514,999.

## The result codes

| Code | Core's reject reason | Returned by |
| --- | --- | --- |
| `POW_OK` | none | all |
| `POW_NEGATIVE` | "high-hash" | the target, the check |
| `POW_ZERO` | "high-hash" | the target, the check |
| `POW_OVERFLOW` | "high-hash" | the target, the check |
| `POW_ABOVE_LIMIT` | "high-hash" | the target, the check |
| `POW_HIGH_HASH` | "high-hash" | the check |
| `POW_CAPACITY` | none: the caller's views | the reader, the hash, the check |

A refusal writes nothing to the caller's output.

## The audit

Two places in the library already touched block headers or work:

- [nip03.coax](../../coax/lib/nostr/nip03.coax) (Nostr's OpenTimestamps
  attestations) replays a proof up to a Merkle root and leaves the
  comparison with a block header to the caller, because this stack does
  not fetch headers. With this chapter the caller can check the header's
  proof of work and its root; fetching it is still the caller's job.
- Nostr's NIP-13 proof of work counts the leading zero bits of an event
  ID. It has no target, no compact form and no work sum, and shares
  nothing with this chapter.

Nothing else read a header, decoded `nBits` or did 256-bit unsigned
arithmetic, so `pow.coax` is new. It builds on chapter 5's SHA-256d.

## The example

The example is
[bitcoin/examples/block-headers.coax](../examples/block-headers.coax). It
reads the genesis header field by field, hashes it and checks it, decodes
and encodes Core's compact cases, shows the check's refusals, follows
eleven headers across the first retarget and adds their work, and checks
three later mainnet headers.

<!-- source: bitcoin/examples/block-headers.coax example -->
```jai
#load "../../coax/lib/bitcoin/pow.coax";
#load "../../coax/lib/nostr/hex.coax";
#import "Basic";

// Print a label and bytes as lowercase hex.
show :: (label: string, bytes: [] u8) {
  text: [200] u8;
  r := hex_encode_checked(cast(string) bytes, text[..], false);
  print("%: %\n", label, cast(string) text[0..<r.written]);
}
// A hash as explorers show it: the digest reversed.
show_id :: (label: string, digest: [] u8) {
  shown: [32] u8;
  #assert bitcoin_reverse32(digest, shown[..]) == SHA_OK;
  show(label, shown[..]);
}
// A 256-bit number as 64 hex digits, most significant first, the way
// Core's GetHex prints it.
show_number :: (label: string, a: U256) {
  b: [32] u8;
  #assert u256_to_hash_vartime(a, b[..]);
  show_id(label, b[..]);
}
// Print a 32-bit number as 8 hex digits.
put32 :: (v: u32) {
  b: [4] u8; text: [8] u8;
  for i: 0..<4 { b[i] = cast,trunc(u8) (v >> (8 * cast(u32) (3 - i))); }
  r := hex_encode_checked(cast(string) b[..], text[..], false);
  print("%", cast(string) text[0..<r.written]);
}
// Decode hex into out and return the filled part. Every literal here and
// in the vector tables is valid lowercase hex, so an error stops the program.
decode :: (text: string, out: [] u8) -> [] u8 {
  r := hex_decode_checked(text, out, false);
  #assert r.error == 0;
  return out[0..<r.written];
}
// The name of each result code.
CODES :: [7] string.["ok", "negative", "zero", "overflow", "above the limit", "high hash", "capacity"];
// A number below 2^64 in decimal; larger ones print as hex.
show_work :: (label: string, w: U256) {
  if u256_bits_vartime(w) <= 63 { print("%: %\n", label, cast(int) u256_low64(w)); }
  else { show_number(label, w); }
}
// What nBits decodes to, with Core's flags and the canonical form.
explain :: (bits: u32) {
  c := pow_compact_decode_vartime(bits);
  put32(bits); print(": size %, ", bits >> 24);
  print("negative %, overflow %, encodes back as ", c.negative, c.overflow);
  put32(pow_compact_encode_vartime(c.target, c.negative)); print("\n");
  show_number("  target", c.target);
}

main :: () {
  b: [80] u8; x: [80] u8; c: [80 * 11] u8; hash: [32] u8;

  // The genesis header, field by field.
  g := decode(PW_GENESIS, b[..]);
  h := pow_header_read_vartime(g);
  print("genesis header, 80 bytes\n");
  print("  version: %\n", h.version);
  show_id("  previous block", h.prev[..]);
  show_id("  Merkle root", h.merkle_root[..]);
  print("  time: %\n", h.time);
  print("  nBits: "); put32(h.bits); print("\n");
  print("  nonce: %\n", h.nonce);
  #assert pow_block_hash_vartime(g, hash[..]) == POW_OK;
  show("hash, as SHA-256d gives it", hash[..]);
  show_id("hash, as explorers show it", hash[..]);
  t := pow_target_vartime(h.bits, POW_LIMIT_MAINNET);
  show_number("target", t.target);
  show_number("mainnet powLimit", POW_LIMIT_MAINNET);
  print("check: %\n", CODES[pow_header_check_vartime(g, POW_LIMIT_MAINNET)]);
  show_work("work", pow_work_vartime(h.bits));
  #assert pow_header_write_vartime(h, x[..]);
  same := true;
  for i: 0..<80 { if x[i] != g[i] { same = false; } }
  print("written back, the same 80 bytes: %\n", same);

  // One changed bit: the nonce's lowest.
  g[76] = g[76] ^ 1;
  #assert pow_block_hash_vartime(g, hash[..]) == POW_OK;
  show_id("with the nonce's low bit flipped", hash[..]);
  print("check: %\n", CODES[pow_header_check_vartime(g, POW_LIMIT_MAINNET)]);
  g[76] = g[76] ^ 1;

  // nBits, and Core's quirks.
  print("compact targets\n");
  explain(0x1d00ffff);
  explain(0x1d00d86a);
  explain(0x17514a49);
  explain(0x01123456);
  explain(0x01fedcba);
  explain(0x01803456);
  explain(0x04923456);
  explain(0x05009234);
  explain(0x21012345);
  explain(0xff123456);
  print("0x80 encodes as "); put32(pow_compact_encode_vartime(u256_small(0x80), false)); print(", not 01800000\n");

  // The check's refusals, in Core's order.
  print("refusals\n");
  one: [32] u8; one[0] = 1;
  checks := [7] u32.[0x1d00ffff, 0x1d80ffff, 0x01003456, 0x80010203, 0x21012345, 0x1d01ffff, 0x1e00ffff];
  for k: 0..<7 {
    print("  hash 1 against "); put32(checks[k]);
    print(": %\n", CODES[pow_check_vartime(one[..], checks[k], POW_LIMIT_MAINNET)]);
  }
  #assert u256_to_hash_vartime(t.target, hash[..]);
  print("  a hash equal to the genesis target: %\n", CODES[pow_check_vartime(hash[..], 0x1d00ffff, POW_LIMIT_MAINNET)]);
  #assert u256_to_hash_vartime(u256_add(t.target, u256_small(1)), hash[..]);
  print("  one more: %\n", CODES[pow_check_vartime(hash[..], 0x1d00ffff, POW_LIMIT_MAINNET)]);

  // Eleven headers from 32,250: each names the one before it.
  print("blocks 32,250 to 32,260\n");
  chain := decode(PW_CHAIN, c[..]);
  total := U256.{};
  for i: 0..<PW_CHAIN_COUNT {
    k := chain[80*i..<80*i+80];
    bits := pow_header_read_vartime(k).bits;
    #assert pow_block_hash_vartime(k, hash[..]) == POW_OK;
    shown: [32] u8;
    #assert bitcoin_reverse32(hash[..], shown[..]) == SHA_OK;
    text: [64] u8;
    r := hex_encode_checked(cast(string) shown[..], text[..], false);
    links := i == 0 || pow_links_vartime(k, chain[80*(i-1)..<80*i]);
    w := pow_work_vartime(bits);
    total = u256_add(total, w);
    print("  % %… nBits ", PW_CHAIN_FIRST + i, cast(string) text[0..<16]); put32(bits);
    print(" work % links % check %\n", cast(int) u256_low64(w), links, CODES[pow_header_check_vartime(k, POW_LIMIT_MAINNET)]);
  }
  show_work("  total work", total);
  // Swap two headers and the links break.
  print("  32,251 after 32,252: links %\n", pow_links_vartime(chain[80..<160], chain[160..<240]));

  // Later headers: smaller targets, more work.
  print("later blocks\n");
  texts := [3] string.[PW_HEADER_100000, PW_HEADER_514999, PW_HEADER_900000];
  heights := [3] int.[100000, 514999, 900000];
  for k: 0..<3 {
    hd := decode(texts[k], b[..]);
    bits := pow_header_read_vartime(hd).bits;
    #assert pow_block_hash_vartime(hd, hash[..]) == POW_OK;
    print("block % nBits ", heights[k]); put32(bits);
    print(": %\n", CODES[pow_header_check_vartime(hd, POW_LIMIT_MAINNET)]);
    show_id("  hash", hash[..]);
    show_number("  target", pow_target_vartime(bits, POW_LIMIT_MAINNET).target);
    show_work("  work", pow_work_vartime(bits));
  }
}
```

It prints:

```text
genesis header, 80 bytes
  version: 1
  previous block: 0000000000000000000000000000000000000000000000000000000000000000
  Merkle root: 4a5e1e4baab89f3a32518a88c31bc87f618f76673e2cc77ab2127b7afdeda33b
  time: 1231006505
  nBits: 1d00ffff
  nonce: 2083236893
hash, as SHA-256d gives it: 6fe28c0ab6f1b372c1a6a246ae63f74f931e8365e15a089c68d6190000000000
hash, as explorers show it: 000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f
target: 00000000ffff0000000000000000000000000000000000000000000000000000
mainnet powLimit: 00000000ffffffffffffffffffffffffffffffffffffffffffffffffffffffff
check: ok
work: 4295032833
written back, the same 80 bytes: true
with the nonce's low bit flipped: 8cdc343dc1ca87b0fae7d09a6537a536fec9784a3c4541b770bfb5881c5d66d9
check: high hash
compact targets
1d00ffff: size 29, negative false, overflow false, encodes back as 1d00ffff
  target: 00000000ffff0000000000000000000000000000000000000000000000000000
1d00d86a: size 29, negative false, overflow false, encodes back as 1d00d86a
  target: 00000000d86a0000000000000000000000000000000000000000000000000000
17514a49: size 23, negative false, overflow false, encodes back as 17514a49
  target: 000000000000000000514a490000000000000000000000000000000000000000
01123456: size 1, negative false, overflow false, encodes back as 01120000
  target: 0000000000000000000000000000000000000000000000000000000000000012
01fedcba: size 1, negative true, overflow false, encodes back as 01fe0000
  target: 000000000000000000000000000000000000000000000000000000000000007e
01803456: size 1, negative false, overflow false, encodes back as 00000000
  target: 0000000000000000000000000000000000000000000000000000000000000000
04923456: size 4, negative true, overflow false, encodes back as 04923456
  target: 0000000000000000000000000000000000000000000000000000000012345600
05009234: size 5, negative false, overflow false, encodes back as 05009234
  target: 0000000000000000000000000000000000000000000000000000000092340000
21012345: size 33, negative false, overflow true, encodes back as 20234500
  target: 2345000000000000000000000000000000000000000000000000000000000000
ff123456: size 255, negative false, overflow true, encodes back as 00000000
  target: 0000000000000000000000000000000000000000000000000000000000000000
0x80 encodes as 02008000, not 01800000
refusals
  hash 1 against 1d00ffff: ok
  hash 1 against 1d80ffff: negative
  hash 1 against 01003456: zero
  hash 1 against 80010203: zero
  hash 1 against 21012345: overflow
  hash 1 against 1d01ffff: above the limit
  hash 1 against 1e00ffff: above the limit
  a hash equal to the genesis target: ok
  one more: high hash
blocks 32,250 to 32,260
  32250 000000002931f8c7… nBits 1d00ffff work 4295032833 links true check ok
  32251 00000000360d826e… nBits 1d00ffff work 4295032833 links true check ok
  32252 000000003d5eb232… nBits 1d00ffff work 4295032833 links true check ok
  32253 000000000e27ee2a… nBits 1d00ffff work 4295032833 links true check ok
  32254 000000006baebaa7… nBits 1d00ffff work 4295032833 links true check ok
  32255 00000000984f9621… nBits 1d00ffff work 4295032833 links true check ok
  32256 000000004f2886a1… nBits 1d00d86a work 5080592338 links true check ok
  32257 0000000078bb40a0… nBits 1d00d86a work 5080592338 links true check ok
  32258 000000006e35246a… nBits 1d00d86a work 5080592338 links true check ok
  32259 000000008a5b32a0… nBits 1d00d86a work 5080592338 links true check ok
  32260 0000000007bfbe7e… nBits 1d00d86a work 5080592338 links true check ok
  total work: 51173158688
  32,251 after 32,252: links false
later blocks
block 100000 nBits 1b04864c: ok
  hash: 000000000003ba27aa200b1cecaad478d2b00432346c3f1f3986da1afd33e506
  target: 000000000004864c000000000000000000000000000000000000000000000000
  work: 62209952899966
block 514999 nBits 17514a49: ok
  hash: 00000000000000000008fda0e5bb3105dab72605332085684c42dceb0ea61097
  target: 000000000000000000514a490000000000000000000000000000000000000000
  work: 00000000000000000000000000000000000000000000032632c22a59ba6d1d63
block 900000 nBits 17023774: ok
  hash: 000000000000000000010538edbfd2d5b809a33dd83f284aeea41c6d0d96968a
  target: 0000000000000000000237740000000000000000000000000000000000000000
  work: 00000000000000000000000000000000000000000000737dd17d8cead8884cdc
```

The book's checks run this program in debug and release builds and
compare the output with
[bitcoin/fixtures/block-headers.out](../fixtures/block-headers.out).

## How the code is checked

### Checks in the example

The checks are in
[bitcoin/examples/checks/block-headers.coax](../examples/checks/block-headers.coax),
which loads the example:

- `arith_uint256_tests.cpp`'s `bignum_SetCompact`, every case: the
  target, both flags and the encoding;
- `pow_tests.cpp`'s five `CheckProofOfWork` tests and
  `GetBitsProof_test`;
- the fifteen mainnet headers: each hash, each check, the links of the
  eleven, and the work, against `pow_reference.py`'s values;
- each of the 640 bits of block 900,000's header, flipped in turn;
- the example's verdicts.

<!-- source: bitcoin/examples/checks/block-headers.coax check -->
```jai
// A number from 64 hex digits, most significant first.
number :: (text: string) -> U256 {
  b: [32] u8; r: [32] u8;
  x := decode(text, b[..]);
  for i: 0..<32 { r[i] = x[31-i]; }
  return u256_from_hash_vartime(r[..]);
}
same :: (a: U256, b: U256) -> bool { return u256_compare_vartime(a, b) == 0; }
// arith_uint256_tests.cpp's bignum_SetCompact: each target, both flags and
// GetCompact with the case's sign.
#test "Core's bignum_SetCompact" {
  for i: 0..<PW_CORE_COUNT {
    c := pow_compact_decode_vartime(cast(u32) PW_CORE_COMPACT[i]);
    #assert c.negative == ((PW_CORE_FLAGS[i] & 1) != 0) && c.overflow == ((PW_CORE_FLAGS[i] & 2) != 0);
    if PW_CORE_ENCODE[i] >= 0 {
      #assert same(c.target, number(PW_CORE_TARGET[i]));
      #assert cast(int) pow_compact_encode_vartime(c.target, c.negative) == PW_CORE_ENCODE[i];
    }
  }
  #assert pow_compact_encode_vartime(u256_small(0x80), false) == 0x02008000;
}
// pow_tests.cpp's CheckProofOfWork tests and GetBitsProof_test.
#test "Core's pow_tests" {
  one: [32] u8; one[0] = 1; h: [32] u8;
  #assert pow_check_vartime(one[..], pow_compact_encode_vartime(POW_LIMIT_MAINNET, true), POW_LIMIT_MAINNET) != POW_OK;
  #assert pow_check_vartime(one[..], 0xff7fffff, POW_LIMIT_MAINNET) != POW_OK;
  twice := u256_shl_vartime(POW_LIMIT_MAINNET, 1);
  #assert pow_check_vartime(one[..], pow_compact_encode_vartime(twice, false), POW_LIMIT_MAINNET) != POW_OK;
  #assert u256_to_hash_vartime(twice, h[..]);
  #assert pow_check_vartime(h[..], pow_compact_encode_vartime(POW_LIMIT_MAINNET, false), POW_LIMIT_MAINNET) == POW_HIGH_HASH;
  zero: [32] u8;
  #assert pow_check_vartime(zero[..], pow_compact_encode_vartime(U256.{}, false), POW_LIMIT_MAINNET) == POW_ZERO;
  for i: 0..<6 { #assert u256_is_zero(pow_work_vartime(cast(u32) PW_CORE_ZERO_WORK[i])); }
  #assert same(pow_work_vartime(0x1d00ffff), u256_small(0x100010001));
}
// The genesis header, the eleven headers from 32,250 and blocks 100,000,
// 514,999 and 900,000: each hashes to its published hash and meets its
// target, the eleven link, and the work matches pow_reference.py's.
#test "mainnet headers" #steps 100000000 {
  b: [80] u8; c: [80 * 11] u8; hash: [32] u8;
  g := decode(PW_GENESIS, b[..]);
  #assert pow_header_check_vartime(g, POW_LIMIT_MAINNET) == POW_OK;
  #assert pow_block_hash_vartime(g, hash[..]) == POW_OK && id_text_is(hash[..], PW_GENESIS_HASH);
  // The comparison sees the first and the last byte shown.
  #assert !id_text_is(hash[..], "010000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f");
  #assert !id_text_is(hash[..], "000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26e");
  #assert same(pow_work_vartime(pow_header_read_vartime(g).bits), number(PW_WORK_GENESIS));
  chain := decode(PW_CHAIN, c[..]);
  total := U256.{};
  for i: 0..<PW_CHAIN_COUNT {
    k := chain[80*i..<80*i+80];
    #assert pow_header_check_vartime(k, POW_LIMIT_MAINNET) == POW_OK;
    #assert pow_block_hash_vartime(k, hash[..]) == POW_OK && id_text_is(hash[..], PW_CHAIN_HASHES[64*i..<64*i+64]);
    #assert i == 0 || pow_links_vartime(k, chain[80*(i-1)..<80*i]);
    total = u256_add(total, pow_work_vartime(pow_header_read_vartime(k).bits));
  }
  #assert same(total, number(PW_CHAIN_WORK));
  texts := [3] string.[PW_HEADER_100000, PW_HEADER_514999, PW_HEADER_900000];
  hashes := [3] string.[PW_HASH_100000, PW_HASH_514999, PW_HASH_900000];
  works := [3] string.[PW_WORK_100000, PW_WORK_514999, PW_WORK_900000];
  for k: 0..<3 {
    h := decode(texts[k], b[..]);
    #assert pow_header_check_vartime(h, POW_LIMIT_MAINNET) == POW_OK;
    #assert pow_block_hash_vartime(h, hash[..]) == POW_OK && id_text_is(hash[..], hashes[k]);
    #assert same(pow_work_vartime(pow_header_read_vartime(h).bits), number(works[k]));
  }
}
// Is the digest, shown reversed, this hex?
id_text_is :: (digest: [] u8, text: string) -> bool {
  want: [32] u8; shown: [32] u8;
  #assert bitcoin_reverse32(digest, shown[..]) == SHA_OK;
  w := decode(text, want[..]);
  for i: 0..<32 { if shown[i] != w[i] { return false; } }
  return true;
}
// One changed bit of block 900,000's header, anywhere in its 640, and the
// header fails the check.
#forall "one changed bit of block 900,000 fails" bit: 0..<640 #steps 250000000 {
  b: [80] u8;
  h := decode(PW_HEADER_900000, b[..]);
  h[bit/8] = h[bit/8] ^ cast,trunc(u8) (1 << (bit % 8));
  #assert pow_header_check_vartime(h, POW_LIMIT_MAINNET) != POW_OK;
}
// The example's verdicts: the flipped nonce, the refusals in Core's order
// and a hash equal to the target.
#test "the example's verdicts" #steps 20000000 {
  b: [80] u8; hash: [32] u8;
  g := decode(PW_GENESIS, b[..]);
  g[76] = g[76] ^ 1;
  #assert pow_header_check_vartime(g, POW_LIMIT_MAINNET) == POW_HIGH_HASH;
  one: [32] u8; one[0] = 1;
  want := [7] int.[POW_OK, POW_NEGATIVE, POW_ZERO, POW_ZERO, POW_OVERFLOW, POW_ABOVE_LIMIT, POW_ABOVE_LIMIT];
  checks := [7] u32.[0x1d00ffff, 0x1d80ffff, 0x01003456, 0x80010203, 0x21012345, 0x1d01ffff, 0x1e00ffff];
  for k: 0..<7 { #assert pow_check_vartime(one[..], checks[k], POW_LIMIT_MAINNET) == want[k]; }
  t := pow_target_vartime(0x1d00ffff, POW_LIMIT_MAINNET).target;
  #assert u256_to_hash_vartime(t, hash[..]) && pow_check_vartime(hash[..], 0x1d00ffff, POW_LIMIT_MAINNET) == POW_OK;
  #assert u256_to_hash_vartime(u256_add(t, u256_small(1)), hash[..]) && pow_check_vartime(hash[..], 0x1d00ffff, POW_LIMIT_MAINNET) == POW_HIGH_HASH;
  #assert pow_compact_encode_vartime(pow_compact_decode_vartime(0x21012345).target, false) == 0x20234500;
  #assert pow_compact_encode_vartime(pow_compact_decode_vartime(0x01803456).target, false) == 0;
}
```

To run them:

```shell
coax/bin/coax test bitcoin/examples/checks/block-headers.coax
coax/bin/coax mutate bitcoin/examples/checks/block-headers.coax --root-only
```

`coax test` runs 75 checks, including those of the files it loads, in about 9 seconds on 2026-11-02. Root-only mutation on 2026-11-02 makes 14 mutants and kills all 14.

### The library's own checks

`pow.coax` is an `#authority` file. Its own checks cover:

- numbers: bytes in both directions, comparison from the top limb, bits,
  shifts of 0 to 259 bits in both directions, carries across limbs, wrap
  at `2^256` and division with a zero, a larger and an equal divisor;
- the compact form: Core's cases, sizes 0 to 34 at the edges of each
  overflow rule, a round trip at every size from 1 to 32, and the sign;
- the target's refusals in Core's order, a target equal to the limit, and
  Core's `CheckProofOfWork` and `GetBitsProof` tests;
- the header: the genesis fields, its hash and work, every refusal of the
  reader and the writer, the largest and smallest versions, the fifteen
  mainnet headers and their links, a link broken in the first or the last
  byte of the previous hash, and all 640 one-bit changes of the genesis
  header.

```shell
coax/bin/coax test coax/lib/bitcoin/pow.coax
coax/bin/coax mutate coax/lib/bitcoin/pow.coax --root-only
```

`coax test` runs 67 checks in about 5 seconds on 2026-11-02; the 640 one-bit changes take most of it. Root-only mutation on 2026-11-02 makes 499 mutants: it kills 497, and two are marked equivalent in the source with their reason (`size <= 3` and `size < 3` differ only at size 3, where both branches shift by zero bytes). None survives.

### Core in Python, natively and in Wasm

[pow_reference.py](../../coax/tests/bitcoin/pow_reference.py) is a
transliteration of the pinned Core sources: `base_uint<256>`'s shifts,
`bits()` and `operator/=` from `arith_uint256.h` and `arith_uint256.cpp`,
`SetCompact` and `GetCompact`, `DeriveTarget` and `CheckProofOfWorkImpl`
from `pow.cpp`, `GetBitsProof` from `chain.cpp`, and the header layout
from `primitives/block.h`. Its numbers are lists of 32-bit limbs, so each
shift and carry happens as in Core. It shares no code with Coax, and its
own test compares it with Python's big integers.
[pow.py](../../coax/tests/bitcoin/pow.py) compiles `pow.coax` to C in
checked and release builds, through a flat test entry point
([pow_report.coax](../../coax/tests/bitcoin/pow_report.coax)). Its cases
are:

- `nBits`: Core's cases, every size byte from 0 to 255 with eight
  mantissas each and both signs, and 300 seeded words, under the mainnet
  and regtest limits. For each: the target, both flags, both encodings,
  the target's verdict and the work;
- encodings of numbers of every bit length from 0 to 256, with and
  without the sign;
- arithmetic on the edges (zero, one, `2^32`, `2^255`, `2^256 − 1`, the
  limit) and 300 seeded pairs, with shifts up to 299 bits;
- the check at, below and above seven targets, and 200 seeded hashes;
- the fifteen headers, all 640 one-bit changes of the genesis header and
  64 of each other header, wrong lengths and 60 seeded headers.

The report must equal the one built from the Python reference, and the
output buffer past it must be untouched. Then the harness builds the same
C for wasm32 and runs every case in Node.

```shell
python3 coax/tests/bitcoin/pow.py
```

On 2026-11-02, 7,317 cases agree with Python in native checked and release builds and in Wasm checked and release builds. The run takes about 20 seconds.

The vectors come from
[make_pow_vectors.py](../../coax/tests/bitcoin/make_pow_vectors.py),
which checks each pinned file's SHA-256, that each header hashes to the
hash in its file name, that the eleven headers link, that the genesis
fields are `chainparams.cpp`'s, that every Core case it copies appears in
Core's test file, and that the difficulty mempool.space publishes for
blocks 100,000 and 514,999 matches their targets. It writes
[pow_vectors.coax](../../coax/tests/bitcoin/pow_vectors.coax).

## What this chapter leaves out

- Retargeting: `GetNextWorkRequired`, `CalculateNextWorkRequired`, the
  factor-of-four clamp, `PermittedDifficultyTransition` and testnet's
  minimum-difficulty rule: chapter 48.
- The chain parameters of testnet4, signet and regtest, and signet's block
  signatures: chapter 50.
- The header's time and version rules: median time past, the two-hour
  future limit, the timewarp fix and deployment versions: chapter 51.
- The block index, `nChainWork` from genesis and the most-work chain:
  chapter 59.
- Mining: building a header and searching for a nonce, and block
  templates: chapter 88.

## Summary

- A header is 80 bytes: version, the previous hash, the Merkle root, time,
  `nBits` and nonce. Its hash is SHA-256d of those bytes, printed
  reversed.
- `nBits` is a size byte and a three-byte mantissa with a sign bit. Core
  decodes it with shifts and flags values that are negative or need more
  than 256 bits; many values name one target, and only the encoder's
  output is canonical.
- A header passes when its target is positive, fits, is not above
  `powLimit`, and its hash, as a little-endian number, is not above it.
  Every failure is "high-hash".
- The work of a header is `2^256 / (target + 1)`, computed as
  `~target / (target + 1) + 1`. The chain with the most total work wins.

## Dependencies

- [2. Big integers in fixed limbs](ch002-big-integers-in-fixed-limbs.md)
- [3. Constant-time rules and secrets](ch003-constant-time-rules.md)
- [5. SHA-256d and tagged hashes](ch005-sha-256d-and-tagged-hashes.md)

## Sources

- Bitcoin Core at revision `ddea9196`: `pow.cpp`, `arith_uint256.h`,
  `arith_uint256.cpp`, `chain.cpp`, `primitives/block.h`,
  `test/pow_tests.cpp` and `test/arith_uint256_tests.cpp`, pinned in
  [coax/tests/vectors/upstream/bitcoin-core/](../../coax/tests/vectors/upstream/bitcoin-core/pow.cpp),
  with `kernel/chainparams.cpp` and `validation.cpp` from chapters 5
  and 46.
- Mainnet headers from mempool.space: the genesis block, blocks 32,250 to
  32,260 and block 900,000, each pinned in
  [coax/tests/vectors/upstream/mainnet/](../../coax/tests/vectors/upstream/mainnet/000000000019d6689c085ae165831e934ff763ae46a2a6c172b3f1b60a8ce26f.header)
  under its block hash, and blocks 100,000 and 514,999 from chapter 46.
- All are listed in [SOURCES.md](../../coax/tests/vectors/SOURCES.md).
