# 6. RIPEMD-160 and HASH160

Bitcoin hashes public keys and scripts to 20 bytes with HASH160, which is
RIPEMD-160 applied to the SHA-256 of the input:

- A P2PKH output script checks `OP_HASH160 <20 bytes> OP_EQUALVERIFY`
  against the spender's public key (chapters 29 and 36).
- A P2SH address holds the HASH160 of a redeem script, and a P2WPKH
  witness program is the HASH160 of a compressed public key (chapter 29).
- A BIP-32 extended key records the first four bytes of its parent key's
  HASH160 as the parent fingerprint (chapter 32).

This chapter builds RIPEMD-160 from the paper by Dobbertin, Bosselaers and
Preneel: the padding, the five boolean functions, the two parallel lines of
80 steps and a streaming interface over state that you own. It works out the
first step of each line for `abc` by hand, builds HASH160 on chapter 4's
SHA-256, and checks both against the paper's test values and the
fingerprints in BIP-32.

## What this chapter teaches

- How RIPEMD-160's padding matches SHA-256's except for byte order: the
  words and the length are little-endian.
- What the five boolean functions do and which steps use each one.
- How the left and right lines process the same block with different word
  orders, rotations and constants, and how they combine at the end.
- How HASH160 composes two hashes, and how a BIP-32 fingerprint comes from
  it.
- How to read test values out of a pinned paper and check the code against
  them and against another implementation.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [ripemd160.coax](../../coax/lib/crypto/ripemd160.coax) | `R_rol`, `R_sum2`–`R_sum4`, `R_f` | Rotation, sums modulo 2^32 and the five boolean functions |
| | `R_compress(h, block)` | One 64-byte block into the five chaining words |
| | `ripemd160_init`, `ripemd160_update_checked`, `ripemd160_final_checked` | The streaming hash over a 100-byte state |
| | `ripemd160_hash_checked` | The one-shot hash |
| [hash160.coax](../../coax/lib/bitcoin/hash160.coax) | `hash160_checked` | RIPEMD-160 of the SHA-256 of the input |

The tables come from the paper's Appendix A and live at the top of
`ripemd160.coax`: `RMD_IV` is the initial chaining value, `RMD_R` and
`RMD_RP` pick the message word for each step of the left and right lines,
and `RMD_S` and `RMD_SP` give each step's rotation.

## Padding

RIPEMD-160 pads like MD4, and so like SHA-256 (chapter 4), with one
difference: it reads everything little-endian. The padded message is:

1. The input bytes.
2. One `0x80` byte.
3. Zero bytes until the length is 8 bytes short of a multiple of 64.
4. The input's length in bits, as a little-endian 64-bit number.

An input of 55 bytes fits in one block; an input of 56 bytes needs two. The
compression function reads each block as 16 little-endian 32-bit words,
`X[0]` to `X[15]`.

### Worked example: padding `abc`

`abc` is the bytes `61 62 63`, which is 24 bits.

1. 3 is less than 56, so the padded message is one block.
2. Bytes 0–3 are `61 62 63 80`.
3. Bytes 4–55 are `00`.
4. Bytes 56–63 hold 24 as a little-endian 64-bit number:
   `18 00 00 00 00 00 00 00`.

Reading bytes 0–3 little-endian puts byte 3 in the high position, so
`X[0]` is `0x80636261`. `X[14]` is bytes 56–59, which is `0x00000018`. Every
other word is 0. SHA-256 reads the same bytes as `0x61626380`, and puts the
length in its last word instead of `X[14]`.

## Words and the five functions

Every operation works on 32-bit words. The steps add modulo 2^32, rotate
left, and apply one of five boolean functions of three words:

| Steps `j` | `f(j, x, y, z)` | What it computes |
| --- | --- | --- |
| 0–15 | `x ^ y ^ z` | Parity |
| 16–31 | `(x & y) \| (~x & z)` | Each bit from `y` where `x` is 1, from `z` where it is 0 |
| 32–47 | `(x \| ~y) ^ z` | |
| 48–63 | `(x & z) \| (y & ~z)` | Each bit from `x` where `z` is 1, from `y` where it is 0 |
| 64–79 | `x ^ (y \| ~z)` | |

The code writes `~x` as `x ^ RMD_ONES`, which flips every bit:

<!-- source: coax/lib/crypto/ripemd160.coax words -->
```jai
// Word arithmetic. Sums widen to u64 and keep the low 32 bits, so they wrap
// the same way in checked and native builds. `x ^ RMD_ONES` is NOT x.
R_rol :: (x: u32, n: int) -> u32 {
  if n == 0 { return x; }
  return (x << n) | (x >> (32 - n));
}
R_sum2 :: (a: u32, b: u32) -> u32 { return cast,trunc(u32) (cast(u64) a + cast(u64) b); }
R_sum3 :: (a: u32, b: u32, c: u32) -> u32 { return cast,trunc(u32) (cast(u64) a + cast(u64) b + cast(u64) c); }
R_sum4 :: (a: u32, b: u32, c: u32, d: u32) -> u32 { return cast,trunc(u32) (cast(u64) a + cast(u64) b + cast(u64) c + cast(u64) d); }
// The five boolean functions. The step number j picks one; it is public.
R_f :: (j: int, x: u32, y: u32, z: u32) -> u32 {
  if j < 16 { return x ^ y ^ z; }
  if j < 32 { return (x & y) | ((x ^ RMD_ONES) & z); }
  if j < 48 { return (x | (y ^ RMD_ONES)) ^ z; }
  if j < 64 { return (x & z) | (y & (z ^ RMD_ONES)); }
  return x ^ (y | (z ^ RMD_ONES));
}
```

The step number `j` picks the function. It is public, so the branch in
`R_f` doesn't depend on the message.

## The compression function

The compression function updates five chaining words, `h[0]` to `h[4]`. It
copies them into two sets of working variables, `A` to `E` for the left
line and `A'` to `E'` for the right line, and runs 80 steps on each. One
step of the left line is:

```text
T = rol_s(j)(A + f(j, B, C, D) + X[r(j)] + K(j)) + E
A = E; E = D; D = rol_10(C); C = B; B = T
```

The right line runs the same step with its own tables: `r'(j)` for the
word, `s'(j)` for the rotation, `K'(j)` for the constant, and the boolean
functions in reverse order, `f(79 − j, …)`. The constants change every 16
steps:

| Steps | `K(j)` | `K'(j)` |
| --- | --- | --- |
| 0–15 | `0x00000000` | `0x50a28be6` |
| 16–31 | `0x5a827999` | `0x5c4dd124` |
| 32–47 | `0x6ed9eba1` | `0x6d703ef3` |
| 48–63 | `0x8f1bbcdc` | `0x7a6d76e9` |
| 64–79 | `0xa953fd4e` | `0x00000000` |

The left constants are the integer parts of 2^30 times the square roots of
2, 3, 5 and 7; the right constants use cube roots. After the 80 steps, each
new chaining word adds one word from each line to a different old word:

```text
T = h1 + C + D'; h1 = h2 + D + E'; h2 = h3 + E + A'
h3 = h4 + A + B'; h4 = h0 + B + C'; h0 = T
```

The code names the primed variables `ap` to `ep`:

<!-- source: coax/lib/crypto/ripemd160.coax compress -->
```jai
// One 64-byte block into the five chaining words `h`. The left and right
// lines run the same 80 steps with different tables, then cross-combine.
R_compress :: (h: [] u32, block: [] u8) -> bool {
  if h.count != 5 || block.count != 64 { return false; }
  k := [5] u32.[0x00000000, 0x5a827999, 0x6ed9eba1, 0x8f1bbcdc, 0xa953fd4e];
  kp := [5] u32.[0x50a28be6, 0x5c4dd124, 0x6d703ef3, 0x7a6d76e9, 0x00000000];
  // Sixteen little-endian message words.
  x: [16] u32;
  for i: 0..<16 {
    p := i * 4;
    x[i] = cast(u32) block[p] | (cast(u32) block[p + 1] << 8) | (cast(u32) block[p + 2] << 16) | (cast(u32) block[p + 3] << 24);
  }
  a := h[0]; b := h[1]; c := h[2]; d := h[3]; e := h[4];
  ap := h[0]; bp := h[1]; cp := h[2]; dp := h[3]; ep := h[4];
  for j: 0..<80 {
    t := R_sum2(R_rol(R_sum4(a, R_f(j, b, c, d), x[RMD_R[j]], k[j / 16]), cast(int) RMD_S[j]), e);
    a = e; e = d; d = R_rol(c, 10); c = b; b = t;
    t = R_sum2(R_rol(R_sum4(ap, R_f(79 - j, bp, cp, dp), x[RMD_RP[j]], kp[j / 16]), cast(int) RMD_SP[j]), ep);
    ap = ep; ep = dp; dp = R_rol(cp, 10); cp = bp; bp = t;
  }
  t := R_sum3(h[1], c, dp);
  h[1] = R_sum3(h[2], d, ep); h[2] = R_sum3(h[3], e, ap);
  h[3] = R_sum3(h[4], a, bp); h[4] = R_sum3(h[0], b, cp); h[0] = t;
  return true;
}
```

`R_compress` refuses, and changes nothing, unless `h` has five words and
the block has 64 bytes. Every index it uses comes from a public table, and
it has no branch on the data.

### Worked example: step 0 of each line for `abc`

At step 0, both lines start from the initial chaining value: `A` is
`0x67452301`, `B` is `0xefcdab89`, `C` is `0x98badcfe`, `D` is
`0x10325476` and `E` is `0xc3d2e1f0`.

The left line uses `f(0)`, `X[r(0)] = X[0]`, `K(0) = 0` and a rotation of
11:

1. `f(0, B, C, D) = B ^ C ^ D`. `0xefcdab89 ^ 0x98badcfe` is `0x77777777`,
   and `0x77777777 ^ 0x10325476` is `0x67452301`.
2. The sum is `0x67452301 + 0x67452301 + 0x80636261 + 0`, which is
   `0x14eeda863`. The carry out of bit 31 is dropped, so the sum is
   `0x4eeda863`.
3. Rotating left by 11 moves the top 11 bits, `01001110111` (`0x277`), to
   the bottom. The other 21 bits shift up to give `0x6d431800`, so the result
   is `0x6d431800 | 0x277 = 0x6d431a77`.
4. The new `B` is `0x6d431a77 + 0xc3d2e1f0`, which is `0x13115fc67`, so `B`
   is `0x3115fc67`.
5. The new `D` is `C` rotated left by 10: `0xeb73fa62`.

The right line uses `f(79)`, `X[r'(0)] = X[5]`, `K'(0) = 0x50a28be6` and a
rotation of 8:

1. `f(79, B', C', D') = B' ^ (C' | ~D')`. `~0x10325476` is `0xefcdab89`;
   `0x98badcfe | 0xefcdab89` is `0xffffffff`; and
   `0xefcdab89 ^ 0xffffffff` is `0x10325476`.
2. The sum is `0x67452301 + 0x10325476 + 0 + 0x50a28be6`, which is
   `0xc81a035d`.
3. Rotating left by 8 gives `0x1a035dc8`.
4. The new `B'` is `0x1a035dc8 + 0xc3d2e1f0`, which is `0xddd63fb8` after
   the carry is dropped.

After all 80 steps of both lines and the final combination, the digest of
`abc` is the paper's value:

```text
8eb208f7e05d987a9b044a8e98c6b087f15a0bfc
```

The digest is the five chaining words written little-endian, so it starts
with the low byte of `h[0]`.

## Streaming over your state

`ripemd160.coax` keeps the hash in a 100-byte state that you own:

| Bytes | Field |
| --- | --- |
| 0–3 | The magic bytes `R160` |
| 4 | The version, 1 |
| 5 | The phase: 1 active, 2 final |
| 6 | Zero |
| 7 | The number of buffered bytes, 0–63 |
| 8–15 | The total byte count, big-endian |
| 16–35 | The five chaining words, little-endian |
| 36–99 | The partial block |

The calls are the same as chapter 4's:

1. `ripemd160_init(state)` writes a fresh state.
2. `ripemd160_update_checked(state, input, overlap)` appends the input to
   the partial block and compresses each block as it fills.
3. `ripemd160_final_checked(state, out, overlap)` pads the partial block,
   compresses the last one or two blocks and writes 20 bytes to `out`.
   Calling it again returns the same digest.

<!-- source: coax/lib/crypto/ripemd160.coax stream -->
```jai
ripemd160_update_checked :: (state: [] u8, input: [] u8, overlap: bool) -> u32 {
  w := ripemd160_unpack(state);
  if !w.ok { return RMD_BAD_STATE; }
  if w.phase != RMD_ACTIVE { return RMD_PHASE; }
  status := ripemd160_update_limit(w.total, input.count);
  if status != RMD_OK { return status; }
  if input.count > 0 && overlap { return RMD_ALIAS; }
  for byte: input {
    w.block[w.used] = byte; w.used += 1;
    if w.used == 64 {
      if !R_compress(w.h[..], w.block[..]) { return RMD_BAD_STATE; }
      w.used = 0;
    }
  }
  w.total += cast(u64) input.count;
  return ripemd160_pack(w, state);
}
// Padding is MD4's: 0x80, zeros, and the bit length as a little-endian u64.
ripemd160_final_checked :: (state: [] u8, out: [] u8, overlap: bool) -> u32 {
  w := ripemd160_unpack(state);
  if !w.ok { return RMD_BAD_STATE; }
  if !cb_span(out.count, 0, RMD_DIGEST_BYTES) { return RMD_BAD_OUTPUT; }
  if overlap { return RMD_ALIAS; }
  if w.phase == RMD_FINAL { return ripemd160_emit(w.h[..], out); }
  w.block[w.used] = 128;
  for i: w.used + 1..<64 { w.block[i] = 0; }
  if w.used >= 56 {
    if !R_compress(w.h[..], w.block[..]) { return RMD_BAD_STATE; }
    for i: 0..<64 { w.block[i] = 0; }
  }
  bits := w.total * 8;
  for i: 0..<8 { w.block[56 + i] = cast(u8) ((bits >> (i * 8)) & 255); }
  if !R_compress(w.h[..], w.block[..]) { return RMD_BAD_STATE; }
  w.used = 0; w.phase = RMD_FINAL;
  status := ripemd160_emit(w.h[..], out);
  if status != RMD_OK { return status; }
  return ripemd160_pack(w, state);
}
```

Every call checks the state before it uses it: the magic bytes, the
version, the phase, a total of at most 2^61 − 1 bytes, and a buffered count
that equals the total modulo 64. Each call returns a status:

| Status | Name | Meaning |
| --- | --- | --- |
| 0 | `RMD_OK` | Success |
| 1 | `RMD_BAD_STATE` | The state has the wrong size or isn't a valid state |
| 2 | `RMD_BAD_INPUT` | The input is negative or larger than 1 MiB |
| 3 | `RMD_BAD_OUTPUT` | The output has fewer than 20 bytes |
| 4 | `RMD_PHASE` | An update after the hash was finalized |
| 5 | `RMD_ALIAS` | The state, input or output overlap |
| 6 | `RMD_LENGTH` | The total would pass 2^61 − 1 bytes |

A refusal leaves the state and the output unchanged. The `_checked`
procedures take the overlap fact as a `bool`, as in chapter 4.

## HASH160

HASH160 runs SHA-256 and then RIPEMD-160 on the 32-byte result:

<!-- source: coax/lib/bitcoin/hash160.coax hash160 -->
```jai
// HASH160(x) = RIPEMD160(SHA256(x)). The checks run before either hash, so
// a refusal writes nothing.
hash160_checked :: (input: [] u8, out: [] u8, overlap: bool) -> u32 #export_c {
  status := ripemd160_request(input.count, out.count, overlap);
  if status != RMD_OK { return status; }
  inner: [32] u8;
  if sha256_hash_checked(input, inner[..], false) != SHA_OK { return RMD_BAD_INPUT; }
  return ripemd160_hash_checked(inner[..], out, false);
}
```

`hash160_checked` checks the input size, the output size and the overlap
fact before it hashes anything, so a refusal writes nothing. It returns the
`RMD_` statuses.

Bitcoin has used HASH160 since its first release. The 20-byte result keeps
addresses and scripts short.

### Worked example: the fingerprint of BIP-32's master key

BIP-32 test vector 1 starts from the seed `000102030405060708090a0b0c0d0e0f`.
Its master extended public key, `xpub661MyMwAqRbc…`, holds this compressed
public key:

```text
0339a36013301597daef41fbe593a02cc513d0b55527ec2df1050e2e8ff49c85c2
```

1. SHA-256 of the 33 key bytes is
   `b766246eacc76f47810bbbdcfcdc0b4e54c5983de526c2b660c65a588468e0ab`.
2. RIPEMD-160 of those 32 bytes is
   `3442193e1bb70916e914552172cd4e2dbc9df811`.
3. The fingerprint is the first four bytes: `3442193e`.

The vector's child key `m/0H`, `xpub68Gmy5EdvgibQVfP…`, records `3442193e`
in bytes 5–8 as its parent fingerprint.

## Secrets

RIPEMD-160 has no table lookup or branch that depends on the message: the
step number selects the function, the word and the rotation, and the
arithmetic is additions, rotations and bitwise operations. So it follows
chapter 3's rules when the input is secret. In Bitcoin the input of HASH160
is almost always public: a public key or a script. When you hash a secret,
erase the state and the SHA-256 intermediate afterwards (chapter 3).
`ripemd160_init` overwrites the state; it is not an erasure.

## The example

The example is [bitcoin/examples/ripemd160.coax](../examples/ripemd160.coax).
It prints the padding and step 0 worked out above, compresses the `abc`
block, hashes the paper's test values, streams one million `a` bytes in
1,000 updates, checks every split of a 1,000-byte message, computes the
BIP-32 fingerprint, shows a broken key and shows five refusals.

<!-- source: bitcoin/examples/ripemd160.coax example -->
```jai
#load "../../coax/lib/bitcoin/hash160.coax";
#import "Basic";

// Print a label and a 20-byte digest as lowercase hex.
show :: (label: string, digest: [] u8) {
  text: [40] u8;
  if R_hex(digest, text[..]) { print("%: %\n", label, cast(string) text[..]); }
}
// Print a label and bytes as lowercase hex.
show_bytes :: (label: string, bytes: [] u8) {
  digits := "0123456789abcdef";
  text: [128] u8; n := 0;
  for b: bytes { text[n] = digits[b >> 4]; text[n + 1] = digits[b & 15]; n += 2; }
  print("%: %\n", label, cast(string) text[0..<n]);
}
// Print a label and one 32-bit word as 8 hex digits, most significant first.
show_word :: (label: string, x: u32) {
  b: [4] u8;
  if cb_store_be32(b[..], 0, x) { show_bytes(label, b[..]); }
}
// Every call below this point must succeed; say so if one doesn't.
must :: (status: u32) { if status != RMD_OK { print("unexpected status %\n", status); } }
// Copy a string into a byte buffer and return the filled view.
bytes_of :: (text: string, buffer: [] u8) -> [] u8 {
  for i: 0..<text.count { buffer[i] = text[i]; }
  return buffer[0..<text.count];
}

STATUS :: [7] string.["ok", "bad state", "bad input", "bad output",
  "already final", "overlap", "too long"];

main :: () {
  // Padding "abc": 0x80 after the message, and the bit length, 24, as a
  // little-endian 64-bit number in bytes 56-63.
  block: [64] u8;
  block[0] = 97; block[1] = 98; block[2] = 99; block[3] = 128; block[56] = 24;
  show_bytes("padded block bytes 0-3", block[0..<4]);
  show_bytes("padded block bytes 56-63", block[56..<64]);
  x0 := cb_load_le32(block[..], 0).value;
  x5 := cb_load_le32(block[..], 20).value;
  show_word("X[0]", x0); show_word("X[14]", cb_load_le32(block[..], 56).value);

  // Step 0 of each line, from the initial chaining value.
  iv := RMD_IV;
  f := R_f(0, iv[1], iv[2], iv[3]);
  sum := R_sum4(iv[0], f, x0, 0x00000000);
  show_word("left f(0, B, C, D)", f);
  show_word("left A + f + X[0] + K(0)", sum);
  show_word("left rol 11", R_rol(sum, 11));
  show_word("left new B", R_sum2(R_rol(sum, 11), iv[4]));
  show_word("left new D = rol 10 of C", R_rol(iv[2], 10));
  fp := R_f(79, iv[1], iv[2], iv[3]);
  sump := R_sum4(iv[0], fp, x5, 0x50a28be6);
  show_word("right f(79, B', C', D')", fp);
  show_word("right A' + f + X[5] + K'(0)", sump);
  show_word("right new B'", R_sum2(R_rol(sump, 8), iv[4]));

  // The whole block through the compression function gives the digest.
  h := RMD_IV;
  if !R_compress(h[..], block[..]) { print("compress refused\n"); }
  out: [20] u8;
  must(ripemd160_emit(h[..], out[..]));
  show("RIPEMD-160(\"abc\") from one block", out[..]);

  // The paper's test values, through the streaming interface.
  messages := [8] string.["", "a", "abc", "message digest", "abcdefghijklmnopqrstuvwxyz",
    "abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq",
    "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789",
    "12345678901234567890123456789012345678901234567890123456789012345678901234567890"];
  labels := [8] string.["\"\"", "\"a\"", "\"abc\"", "\"message digest\"", "\"a...z\"",
    "\"abcdbcde...nopq\"", "\"A...Za...z0...9\"", "8 times \"1234567890\""];
  for k: 0..<8 {
    text: [80] u8;
    must(ripemd160_hash_checked(bytes_of(messages[k], text[..]), out[..], false));
    show(labels[k], out[..]);
  }

  // One million "a" bytes, in 1,000 updates of 1,000 bytes.
  thousand: [1000] u8;
  for i: 0..<1000 { thousand[i] = 97; }
  state: [100] u8;
  must(ripemd160_init(state[..]));
  for i: 0..<1000 { must(ripemd160_update_checked(state[..], thousand[..], false)); }
  must(ripemd160_final_checked(state[..], out[..], false));
  show("1 million times \"a\"", out[..]);

  // Every split of a 1,000-byte message into two updates gives the digest
  // of one update.
  message: [1000] u8;
  for i: 0..<1000 { message[i] = cast(u8) ((i * 7 + 3) % 256); }
  whole: [20] u8;
  must(ripemd160_hash_checked(message[..], whole[..], false));
  agree := 0;
  for cut: 0..<1001 {
    split: [20] u8;
    must(ripemd160_init(state[..]));
    must(ripemd160_update_checked(state[..], message[0..<cut], false));
    must(ripemd160_update_checked(state[..], message[cut..<1000], false));
    must(ripemd160_final_checked(state[..], split[..], false));
    same := true;
    for i: 0..<20 { if split[i] != whole[i] { same = false; } }
    if same { agree += 1; }
  }
  print("splits of 1,000 bytes that match one update: % of 1001\n", agree);
  show("RIPEMD-160(1,000-byte message)", whole[..]);

  // HASH160 of BIP-32 test vector 1's master public key. Its first four
  // bytes are the fingerprint that every child of the master key records.
  key := [33] u8.[0x03, 0x39, 0xa3, 0x60, 0x13, 0x30, 0x15, 0x97, 0xda, 0xef, 0x41, 0xfb, 0xe5, 0x93, 0xa0, 0x2c, 0xc5,
    0x13, 0xd0, 0xb5, 0x55, 0x27, 0xec, 0x2d, 0xf1, 0x05, 0x0e, 0x2e, 0x8f, 0xf4, 0x9c, 0x85, 0xc2];
  inner: [32] u8;
  if sha256_hash_checked(key[..], inner[..], false) != SHA_OK { print("sha256 refused\n"); }
  show_bytes("SHA-256(master key)", inner[..]);
  must(hash160_checked(key[..], out[..], false));
  show("HASH160(master key)", out[..]);
  published := [4] u8.[0x34, 0x42, 0x19, 0x3e];
  show_bytes("fingerprint", out[0..<4]);
  print("matches the m/0H parent fingerprint: %\n", cb_ct_equal(out[0..<4], published[..]) == CB_MAX64);

  // A broken case: one flipped bit in the key gives an unrelated fingerprint.
  key[32] ^= 1;
  must(hash160_checked(key[..], out[..], false));
  show_bytes("fingerprint with the last bit flipped", out[0..<4]);
  print("matches the m/0H parent fingerprint: %\n", cb_ct_equal(out[0..<4], published[..]) == CB_MAX64);

  // Refusals. Each one returns a status and leaves the output unchanged.
  must(ripemd160_init(state[..]));
  must(ripemd160_final_checked(state[..], out[..], false));
  print("update after final: %\n", STATUS[ripemd160_update_checked(state[..], thousand[..], false)]);
  small: [19] u8;
  print("19-byte output: %\n", STATUS[hash160_checked(key[..], small[..], false)]);
  print("1 MiB + 1 byte in one call: %\n", STATUS[ripemd160_request(1048577, 20, false)]);
  short: [99] u8;
  print("99-byte state: %\n", STATUS[ripemd160_init(short[..])]);
  must(ripemd160_init(state[..]));
  state[7] = 5;
  print("5 bytes buffered but 0 counted: %\n", STATUS[ripemd160_update_checked(state[..], thousand[..], false)]);
}
```

Run it from the repository root:

```shell
coax/bin/coax run bitcoin/examples/ripemd160.coax
```

The output matches the values worked out by hand above:

```text
padded block bytes 0-3: 61626380
padded block bytes 56-63: 1800000000000000
X[0]: 80636261
X[14]: 00000018
left f(0, B, C, D): 67452301
left A + f + X[0] + K(0): 4eeda863
left rol 11: 6d431a77
left new B: 3115fc67
left new D = rol 10 of C: eb73fa62
right f(79, B', C', D'): 10325476
right A' + f + X[5] + K'(0): c81a035d
right new B': ddd63fb8
RIPEMD-160("abc") from one block: 8eb208f7e05d987a9b044a8e98c6b087f15a0bfc
"": 9c1185a5c5e9fc54612808977ee8f548b2258d31
"a": 0bdc9d2d256b3ee9daae347be6f4dc835a467ffe
"abc": 8eb208f7e05d987a9b044a8e98c6b087f15a0bfc
"message digest": 5d0689ef49d2fae572b881b123a85ffa21595f36
"a...z": f71c27109c692c1b56bbdceb5b9d2865b3708dbc
"abcdbcde...nopq": 12a053384a9c0c88e405a06c27dcf49ada62eb2b
"A...Za...z0...9": b0e20b6e3116640286ed3a87a5713079b21f5189
8 times "1234567890": 9b752e45573d4b39f4dbd3323cab82bf63326bfb
1 million times "a": 52783243c1697bdbe16d37f97f68f08325dc1528
splits of 1,000 bytes that match one update: 1001 of 1001
RIPEMD-160(1,000-byte message): 462fa67a8f19c1df2d98cff47379ba31d681b572
SHA-256(master key): b766246eacc76f47810bbbdcfcdc0b4e54c5983de526c2b660c65a588468e0ab
HASH160(master key): 3442193e1bb70916e914552172cd4e2dbc9df811
fingerprint: 3442193e
matches the m/0H parent fingerprint: true
fingerprint with the last bit flipped: ef09242e
matches the m/0H parent fingerprint: false
update after final: already final
19-byte output: bad output
1 MiB + 1 byte in one call: bad input
99-byte state: bad state
5 bytes buffered but 0 counted: bad state
```

The nine digests from `""` to one million `a` bytes are the paper's
Appendix B values. The HASH160 of the master key starts with `3442193e`,
the parent fingerprint recorded in BIP-32's `m/0H` key.

### A broken key and the refusals

The broken case flips the last bit of the master key. Its HASH160 starts
with `ef09242e`, so the comparison with the published fingerprint fails. A
wallet that matches a child key to its parent this way rejects the pair. The
example compares the four bytes with `cb_ct_equal` from chapter 3.

The five refusals are an update after the final call, a 19-byte output, one
update of 1 MiB + 1 byte, a 99-byte state and a state whose buffered count
(5) doesn't match its total (0). Each returns its status before it writes
anything.

The book's checks run this program in debug and release builds and compare
the output with [bitcoin/fixtures/ripemd160.out](../fixtures/ripemd160.out).

## How the code is checked

### Published vectors and another implementation

The paper is pinned byte for byte as
[coax/tests/vectors/upstream/ripemd160/AB-9601.pdf](../../coax/tests/vectors/upstream/ripemd160/AB-9601.pdf),
and BIP-32 as
[coax/tests/vectors/upstream/bip32/bip-0032.mediawiki](../../coax/tests/vectors/upstream/bip32/bip-0032.mediawiki).
[coax/tests/bitcoin/ripemd160.py](../../coax/tests/bitcoin/ripemd160.py)
compiles `hash160.coax` to C in checked and release builds and checks:

- The nine RIPEMD-160 values of Appendix B, which it reads from the PDF's
  compressed text, including one million `a` bytes.
- Every parent and child pair in BIP-32 test vectors 1 to 3. It decodes the
  extended public keys from the BIP text and checks that HASH160 of each
  parent's key starts with the child's parent fingerprint. There are 11
  pairs.
- Every input length from 0 to 300 bytes, and 20 random inputs of up to
  1 MiB, against Python's `hashlib` RIPEMD-160, which comes from OpenSSL.
  HASH160 is compared for the same 301 lengths.

```shell
python3 coax/tests/bitcoin/ripemd160.py
```

```text
checked: 9 paper values, 11 BIP-32 fingerprints, 321 hashlib comparisons
release: 9 paper values, 11 BIP-32 fingerprints, 321 hashlib comparisons
```

If the local Python has no RIPEMD-160 in `hashlib`, the script skips the
`hashlib` comparisons and says so; the paper and BIP-32 checks still run.

### Checks in Coax

- `ripemd160.coax` checks the paper's eight short values, the streaming
  path against one update for ten lengths around the block boundaries and
  three chunk sizes, every header byte of the state, the length limit, the
  five boolean functions, and that every refusal leaves the state and the
  output unchanged.
- `hash160.coax` checks the BIP-32 vector 1 fingerprint and HASH160 of the
  empty input.

[bitcoin/examples/checks/ripemd160.coax](../examples/checks/ripemd160.coax)
checks the 11 BIP-32 fingerprints in Coax:

<!-- source: bitcoin/examples/checks/ripemd160.coax fingerprints -->
```jai
// Every parent/child step of BIP-32 test vectors 1-3. The parent's public
// key and the child's parent fingerprint are decoded from the extended keys
// in the pinned coax/tests/vectors/upstream/bip32/bip-0032.mediawiki
// (coax/tests/bitcoin/ripemd160.py decodes them again from the text).
PARENTS :: [11] string.[
  "0339a36013301597daef41fbe593a02cc513d0b55527ec2df1050e2e8ff49c85c2",
  "035a784662a4a20a65bf6aab9ae98a6c068a81c52e4b032c0fb5400c706cfccc56",
  "03501e454bf00751f24b1b489aa925215d66af2234e3891c3b21a52bedb3cd711c",
  "0357bfe1e341d01c69fe5654309956cbea516822fba8a601743a012a7896ee8dc2",
  "02e8445082a72f29b75ca48748a914df60622a609cacfce8ed0e35804560741d29",
  "03cbcaa9c98c877a26977d00825c956a238e8dddfbd322cce4f74b0b5bd6ace4a7",
  "02fc9e5af0ac8d9b3cecfe2a888e2117ba3d089d8585886c9c826b6b22a98d12ea",
  "03c01e7425647bdefa82b12d9bad5e3e6865bee0502694b94ca58b666abc0a5c3b",
  "03a7d1d856deb74c508e05031f9895dab54626251b3806e16b4bd12e781a7df5b9",
  "02d2b36900396c9282fa14628566582f206a5dd0bcc8d5e892611806cafb0301f0",
  "03683af1ba5743bdfc798cf814efeeab2735ec52d95eced528e692b8e34c4e5669"];
FINGERPRINTS :: [11] string.["3442193e", "5c1bd648", "bef5a2f9", "ee7ab90c", "d880d7d8",
  "bd16bee5", "5a61ff8e", "d8ab4937", "78412e3a", "31a507b8", "41d63b50"];
nibble :: (c: u8) -> u8 { if c <= '9' { return c - '0'; } return c - 'a' + 10; }
#forall "HASH160 of each BIP-32 parent key starts with its child's parent fingerprint" i: 0..<11 {
  key: [33] u8;
  for k: 0..<33 { key[k] = (nibble(PARENTS[i][k * 2]) << 4) | nibble(PARENTS[i][k * 2 + 1]); }
  out: [20] u8; text: [40] u8;
  #assert hash160_checked(key[..], out[..], false) == RMD_OK;
  #assert R_hex(out[..], text[..]) && cast(string) text[0..<8] == FINGERPRINTS[i];
}
```

It also checks every split of a 130-byte message against a digest computed
by Python's `hashlib`:

<!-- source: bitcoin/examples/checks/ripemd160.coax splits -->
```jai
// Every split of a 130-byte message into two updates gives the digest that
// Python's hashlib computes. 130 bytes cross two block boundaries, so the
// cuts cover an empty first chunk, a full first block and an empty second
// chunk.
#forall "every split of a 130-byte message gives hashlib's digest" cut: 0..<131 {
  message: [130] u8;
  for i: 0..<130 { message[i] = cast(u8) ((i * 7 + 3) % 256); }
  state: [100] u8; split: [20] u8; text: [40] u8;
  #assert ripemd160_init(state[..]) == RMD_OK;
  #assert ripemd160_update_checked(state[..], message[0..<cut], false) == RMD_OK;
  #assert ripemd160_update_checked(state[..], message[cut..<130], false) == RMD_OK;
  #assert ripemd160_final_checked(state[..], split[..], false) == RMD_OK;
  // python3 -c 'import hashlib;print(hashlib.new("ripemd160",bytes((i*7+3)%256 for i in range(130))).hexdigest())'
  #assert R_hex(split[..], text[..]) && cast(string) text[..] == "076fb3c7147bd18eb31839c459836c98ed84315e";
}
```

To run all the checks:

```shell
coax/bin/coax test coax/lib/crypto/ripemd160.coax
coax/bin/coax test coax/lib/bitcoin/hash160.coax
coax/bin/coax test bitcoin/examples/checks/ripemd160.coax
coax/bin/coax mutate coax/lib/crypto/ripemd160.coax --root-only
coax/bin/coax mutate coax/lib/bitcoin/hash160.coax --root-only
```

`coax mutate --root-only` leaves zero survivors: 358 mutants in
`ripemd160.coax` and 6 in `hash160.coax`.

## Summary

- RIPEMD-160 pads like SHA-256 but reads words and the length
  little-endian.
- Two lines of 80 steps process each block with different word orders,
  rotations, constants and function orders, then cross-combine into the
  five chaining words.
- Every table index is public, so the hash has no data-dependent branch or
  lookup.
- HASH160 is RIPEMD-160 of SHA-256. Its first four bytes over a public key
  are the BIP-32 fingerprint.

## Dependencies

- [1. Bytes, hex and base64](ch001-bytes-hex-and-base64.md)
- [4. SHA-256](ch004-sha-256.md), for HASH160

## Sources

- H. Dobbertin, A. Bosselaers and B. Preneel,
  [RIPEMD-160: A Strengthened Version of RIPEMD](https://homes.esat.kuleuven.be/~bosselae/ripemd160/pdf/AB-9601/AB-9601.pdf),
  1996: Appendix A for the definition and Appendix B for the test values.
- [BIP-32](https://github.com/bitcoin/bips/blob/master/bip-0032.mediawiki),
  "Key identifiers" and test vectors 1 to 3.
- Both files are pinned in
  [coax/tests/vectors/SOURCES.md](../../coax/tests/vectors/SOURCES.md).
