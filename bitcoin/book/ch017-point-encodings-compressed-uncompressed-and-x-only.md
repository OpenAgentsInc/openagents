# 17. Point encodings: compressed, uncompressed and x-only

A public key is a point on secp256k1. Bitcoin writes it as bytes in three
ways:

- **Compressed** (SEC 1), 33 bytes: a prefix `02` when y is even or `03`
  when y is odd, then x. P2PKH and P2WPKH outputs commit to the HASH160 of
  this form (chapters 6 and 29), and every BIP-32 extended key holds one
  (chapter 32).
- **Uncompressed** (SEC 1), 65 bytes: a prefix `04`, then x and y. The
  Bitcoin wallet wrote only this form until version 0.6 (2012), so old P2PK
  and P2PKH outputs still commit to it. Bitcoin Core's policy refuses it in
  segwit v0 scripts (BIP-143).
- **x-only** (BIP-340), 32 bytes: x alone, standing for the point with even
  y. Taproot output keys, BIP-340 public keys and Nostr keys use it
  (chapters 23, 41 and 45).

The compressed and x-only forms drop y because x fixes it up to sign: if
`(x, y)` is on the curve, so is `(x, p − y)`, and exactly one of the two is
even. Decoding means solving `y^2 = x^3 + 7` for y and picking the root by
parity. This chapter builds one codec for the three forms on chapter 15's
group law and checks it against keys published by Bitcoin Core, Wycheproof
and BIP-32, BIP-340 and BIP-341.

## What this chapter teaches

- How each encoding lays out its bytes, and where Bitcoin uses each one.
- How to decompress a key: `x^3 + 7`, its square root, and the parity bit.
- Why an x-only key loses the parity bit, and how Taproot carries it.
- Which encodings a strict decoder refuses: the hybrid prefixes `06` and
  `07`, a coordinate at least p, a point off the curve, the point at
  infinity and wrong lengths.
- How to check a codec against published keys and against a reference
  written in another language.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [secp_encode.coax](../../coax/lib/crypto/secp_encode.coax) | `se_decode(bytes)` | A 33- or 65-byte SEC 1 key to a point |
| | `se_decode_xonly(bytes)` | A 32-byte BIP-340 key to the point with even y |
| | `se_encode(P, form, out)` | A point to 32, 33 or 65 bytes |
| | `se_parity(P)` | 1 when y is odd, 0 when it is even |
| | `se_recode(bytes, form, out)` | Decode, then encode in another form |
| [secp_group.coax](../../coax/lib/crypto/secp_group.coax) | `sg_lift_x_public`, `sg_decode_public`, `sg_encode_public` | Chapter 15's `lift_x` and compressed form, which the codec builds on |

`secp_encode.coax` is the one codec. Chapter 15's `sg_lift_x_public` is
the x-only decoder, and `sg_decode_public` and `sg_encode_public` are the
compressed form; the codec adds the uncompressed form, the length and
prefix rules, and refuses infinity.

Before this chapter, the BIP-32 code wrote compressed keys with
`sg_encode_public` and then checked the length itself, and the Nostr wallet
code (`coax/lib/nostr/nipkeys.coax`) did the same to read and write extended
public keys. Both now call `se_encode` and `se_decode`. One caller stays as
it was: the signer in `coax/lib/nostr/nipkeys_sign.coax` writes the prefix
from its constant-time point type, which keeps the key out of the general
point code.

## The three forms

| Form | Length | Bytes | Decodes to |
| --- | --- | --- | --- |
| Compressed | 33 | `02` or `03`, x | the point with x whose y has that parity |
| Uncompressed | 65 | `04`, x, y | `(x, y)`, if it is on the curve |
| x-only | 32 | x | the point with x and even y |

x and y are 32-byte big-endian numbers below
`p = 2^256 − 2^32 − 977`. The parity of y is its lowest bit. Because p is
odd, y and `p − y` have different parities: when one is even, the other is
odd.

SEC 1 defines two more encodings that this codec refuses:

- **Infinity** is the single byte `00`. A public key is never infinity, so
  a key decoder that accepts it only lets a broken key through. Chapter 15's
  `sg_decode_public` decodes it for group tools; `se_decode` does not.
- **Hybrid** keys are 65 bytes with prefix `06` (y even) or `07` (y odd),
  then x and y. They carry y and its parity, so they add nothing. OpenSSL
  accepted them, so Bitcoin's original rules did too, and old scripts can
  hold them. Bitcoin Core's `STRICTENC` policy refuses them with
  `PUBKEYTYPE`, and its `script_tests.json` has the cases.

## Decompressing a key

To decompress `02 || x` or `03 || x`:

1. Refuse x when it is p or more.
2. Compute `y^2 = x^3 + 7 mod p`.
3. Compute a square root. p is `3 mod 4`, so a square root of a is
   `a^((p + 1) / 4)` when one exists (chapter 13).
4. Square the root. If it isn't `x^3 + 7`, x has no point: refuse.
5. Keep the root if its parity matches the prefix; otherwise use
   `p − root`.

An x-only key is step 1 to 4 and then the even root: it is decompression
with the prefix `02`. BIP-340 calls it `lift_x`.

### Worked example: a toy curve

The same steps on `y^2 = x^3 + 7` over the integers mod 23. 23 is
`3 mod 4`, so a square root of a is `a^((23 + 1) / 4) = a^6`.

For x = 1:

1. `x^3 + 7 = 8`.
2. `8^2 = 64 = 18`, `8^4 = 18^2 = 324 = 2`, and `8^6 = 2 · 18 = 36 = 13`
   (all mod 23).
3. `13^2 = 169 = 8`, so 13 is a root, and x = 1 has a point.
4. 13 is odd. Prefix `03` gives y = 13, and prefix `02` gives
   y = 23 − 13 = 10. Check: `10^2 = 100 = 8`.

For x = 3:

1. `x^3 + 7 = 34 = 11`.
2. `11^2 = 121 = 6`, `11^4 = 36 = 13` and `11^6 = 13 · 6 = 78 = 9`.
3. `9^2 = 81 = 12`, which is not 11. 11 is not a square mod 23, so x = 3
   has no point, and both `02 || 3` and `03 || 3` refuse.

### Worked example: decompressing G

The generator's x is:

```text
79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798
```

The example prints each step for secp256k1's p:

```text
x^3 + 7: 4866d6a5ab41ab2c6bcc57ccd3735da5f16f80a548e5e20a44e4e9b8118c26f2
its square root (even): 483ada7726a3c4655da4fbfc0e1108a8fd17b448a68554199c47d08ffb10d4b8
p minus the root (odd): b7c52588d95c3b9aa25b0403f1eef75702e84bb7597aabe663b82f6f04ef2777
```

The even root is G's y, which ends in `b8`, so G's compressed form starts
with `02`. `03 || x` decodes to `−G`, whose y is `p` minus G's y and ends in
`77`.

## Decoding

<!-- source: coax/lib/crypto/secp_encode.coax decode -->
```jai
// A SEC 1 public key: 33 bytes with prefix 02 or 03, or 65 bytes with
// prefix 04. Anything else, including infinity's one-byte 00 and the hybrid
// prefixes 06 and 07, refuses with ok false and the value infinity.
se_decode :: (bytes: [] u8) -> SgResult #export_c {
  // Compressed: lift_x, then negate y when the prefix's low bit says odd.
  if bytes.count == SE_COMPRESSED { return sg_decode_public(bytes); }
  r: SgResult; r.value = sg_infinity();
  if bytes.count != SE_UNCOMPRESSED || bytes[0] != 4 { return r; }
  // Uncompressed: both coordinates below p, and y^2 = x^3 + 7.
  x := sf_load_strict(bytes[1..<33]); y := sf_load_strict(bytes[33..<65]);
  if !x.ok || !y.ok { return r; } // mutant: equivalent (a refused load is zero, and no point has x = 0 or y = 0)
  seven: SecpField; seven.limb[0] = 7;
  rhs := sf_add(sf_multiply(sf_square(x.value), x.value), seven);
  if sf_equal_mask(sf_square(y.value), rhs) != CB_MAX64 { return r; }
  r.value.x = x.value; r.value.y = y.value;
  r.value.z = sg_generator().z;   // z = 1: the point is affine
  r.ok = true; return r;
}
// A BIP-340 x-only public key: 32 bytes of x, lifted to the point with
// even y. It refuses x >= p and an x with no point.
se_decode_xonly :: (bytes: [] u8) -> SgResult #export_c {
  return sg_lift_x_public(bytes);
}
```

`se_decode` takes the length as the form. A 33-byte key goes to chapter
15's `sg_decode_public`, which checks the prefix, lifts x and negates y for
`03`. A 65-byte key must start with `04`, have both coordinates below p
(`sf_load_strict` refuses a number that is p or more instead of reducing
it), and satisfy the curve equation. Every refusal returns `ok` false and
the value infinity, so a caller that forgets to check `ok` gets a point that
every later operation refuses.

The one marked line is a mutation that `coax mutate` can't kill: a
coordinate that fails `sf_load_strict` loads as zero, and no point on
secp256k1 has x = 0 or y = 0, so the curve check would refuse it anyway.
The explicit check stays because it states the rule.

## Encoding

<!-- source: coax/lib/crypto/secp_encode.coax encode -->
```jai
// Writes a in one of the three forms (SE_XONLY, SE_COMPRESSED or
// SE_UNCOMPRESSED) to out[0..<form] and returns form. Returns 0, and writes
// nothing, for an unknown form, a short output, an invalid point or
// infinity, which has no encoding in any of the three forms.
se_encode :: (a: SgPoint, form: int, out: [] u8) -> int #export_c {
  if form != SE_XONLY && form != SE_COMPRESSED && form != SE_UNCOMPRESSED { return 0; }
  if out.count < form { return 0; }
  // Prefix 02 or 03, then x; a length of 1 is infinity.
  c := sg_encode_public(a);
  if !c.ok || c.length != SE_COMPRESSED { return 0; }
  if form == SE_XONLY { for i: 0..<32 { out[i] = c.bytes[1 + i]; } return form; }
  if form == SE_COMPRESSED { for i: 0..<33 { out[i] = c.bytes[i]; } return form; }
  affine := sg_to_affine(a);
  out[0] = 4;
  for i: 0..<32 { out[1 + i] = c.bytes[1 + i]; }
  #assert sf_store(affine.value.y, out[33..<65]);
  return form;
}
// 1 when a's y is odd, 0 when it is even, and -1 for an invalid point or
// infinity. An x-only key drops this bit; Taproot carries it separately.
se_parity :: (a: SgPoint) -> int #export_c {
  c := sg_encode_public(a);
  if !c.ok || c.length != SE_COMPRESSED { return -1; }
  return cast(int) (c.bytes[0] & 1);
}
```

`se_encode` starts from chapter 15's compressed form, which converts the
point to affine coordinates and writes the parity prefix. x-only drops the
prefix. Uncompressed replaces the prefix with `04` and appends y.
Infinity has no encoding in any form, so it returns 0 and writes nothing.

### The parity bit and Taproot

An x-only key keeps x and drops the parity. Encoding a point with odd y as
x-only and decoding it again gives its negation:

```text
-G parity: 1, x-only equals G's: true, lifts to G: true
```

BIP-340 makes this safe by working only with points that have even y: a
signer whose public point has odd y negates its secret key first, so the
point becomes even (chapter 24). Taproot needs the bit back in one place:
when a script path spends an output, the control block's first byte holds
the output key's parity, so the verifier can check that the internal key
tweaked by the script tree gives exactly that point (chapter 45).
`se_parity` reads the bit.

## Secrets

These calls take variable time. They branch on lengths, prefixes and on
whether a point is valid, and `se_encode` returns early for infinity.
Public keys are public data: a script, a transaction or a Nostr event shows
them to everyone, so their timing reveals nothing. Chapter 15's masked
`sg_lift_x_public` and `sg_encode_public` do the field work underneath, so
the arithmetic itself has no branch on the coordinates.

Don't use this codec on a point whose value is secret, such as an ECDH
shared point or a nonce point before it is published. Encode those with
the constant-time code that made them (chapter 16).

## The example

The example is
[bitcoin/examples/point-encodings.coax](../examples/point-encodings.coax).
It decompresses on the toy curve, encodes G three ways and decompresses it
step by step, encodes BIP-340 vector 0's key 3G, shows the parity bit,
converts a Bitcoin Core key between its two forms, round-trips 32 points
and shows eleven refusals.

<!-- source: bitcoin/examples/point-encodings.coax example -->
```jai
#load "../../coax/lib/crypto/secp_encode.coax";
#load "../../coax/lib/nostr/hex.coax";
#import "Basic";

// Print a label and bytes as lowercase hex.
show :: (label: string, bytes: [] u8) {
  text: [130] u8;
  r := hex_encode_checked(cast(string) bytes, text[..], false);
  print("%: %\n", label, cast(string) text[0..<r.written]);
}
// Print a field element as 32 big-endian bytes.
show_field :: (label: string, a: SecpField) {
  b: [32] u8;
  if sf_store(a, b[..]) { show(label, b[..]); }
}
// Decode hex into out and return the filled view.
bytes_of :: (text: string, out: [] u8) -> [] u8 {
  r := hex_decode_checked(text, out, false);
  if r.error != 0 { print("bad hex\n"); }
  return out[0..<r.written];
}
same_point :: (a: SgPoint, b: SgPoint) -> bool { return sg_equal_mask(a, b) == CB_MAX64; }
same_bytes :: (a: [] u8, b: [] u8) -> bool { return cb_ct_equal(a, b) == CB_MAX64; }
// Decode hex as SEC 1, or as x-only when it is 32 bytes, and say whether it
// decoded.
decodes :: (text: string) -> bool {
  b: [65] u8; v := bytes_of(text, b[..]);
  return ifx v.count == SE_XONLY then se_decode_xonly(v).ok else se_decode(v).ok;
}
// a^e mod m for small numbers: the toy curve's arithmetic.
power :: (a: int, e: int, m: int) -> int {
  r := 1; for i: 0..<e { r = (r * a) % m; }
  return r;
}
// Decompress x on y^2 = x^3 + 7 over F_23, where 23 = 3 mod 4, so a square
// root of a is a^((23 + 1) / 4) = a^6.
toy :: (x: int) {
  rhs := (x * x * x + 7) % 23; root := power(rhs, 6, 23);
  print("toy x = %: x^3 + 7 = %, % ^ 6 = %, % ^ 2 = %", x, rhs, rhs, root, root, (root * root) % 23);
  if (root * root) % 23 != rhs { print(": no point, refused\n"); return; }
  even := ifx root % 2 == 0 then root else 23 - root;
  print(": prefix 02 gives y = %, prefix 03 gives y = %\n", even, 23 - even);
}

main :: () {
  // The worked example on a toy curve.
  toy(1);
  toy(3);

  // G: the three encodings.
  g := sg_generator();
  c: [33] u8; u: [65] u8; x: [32] u8;
  print("encode G: % bytes, % bytes, % bytes\n", se_encode(g, SE_COMPRESSED, c[..]),
    se_encode(g, SE_UNCOMPRESSED, u[..]), se_encode(g, SE_XONLY, x[..]));
  show("G compressed", c[..]);
  show("G uncompressed", u[..]);
  show("G x-only", x[..]);

  // Decompressing G by hand: y^2 = x^3 + 7, the root, its parity.
  gx := sf_load_strict(x[..]).value; seven: SecpField; seven.limb[0] = 7;
  rhs := sf_add(sf_multiply(sf_square(gx), gx), seven);
  root := sf_sqrt(rhs);
  show_field("x^3 + 7", rhs);
  show_field("its square root (even)", root.value);
  show_field("p minus the root (odd)", sf_negate(root.value));
  print("02 || x decodes to G: %\n", same_point(se_decode(c[..]).value, g));
  c[0] = 3; print("03 || x decodes to -G: %\n", same_point(se_decode(c[..]).value, sg_negate(g).value));
  c[0] = 2;

  // BIP-340 test vector 0: secret key 3, so the public key is 3G.
  three := sg_add(sg_double(g).value, g).value; t: [65] u8;
  print("3G parity: %\n", se_parity(three));
  if se_encode(three, SE_COMPRESSED, t[..]) == 33 { show("3G compressed", t[0..<33]); }
  if se_encode(three, SE_UNCOMPRESSED, t[..]) == 65 { show("3G uncompressed", t[..]); }
  if se_encode(three, SE_XONLY, t[..]) == 32 { show("3G x-only", t[0..<32]); }
  row0: [32] u8;
  print("matches BIP-340 vector 0's public key: %\n",
    same_bytes(t[0..<32], bytes_of("f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9", row0[..])));

  // x-only drops the parity bit: -G and G have the same x-only key, which
  // lifts to the point with even y.
  minus := sg_negate(g).value; mx: [32] u8;
  if se_encode(minus, SE_XONLY, mx[..]) != 32 { print("encode refused\n"); }
  print("-G parity: %, x-only equals G's: %, lifts to G: %\n", se_parity(minus),
    same_bytes(mx[..], x[..]), same_point(se_decode_xonly(mx[..]).value, g));

  // A key from Bitcoin Core's script_tests.json in both of its forms.
  k1: [33] u8; k2: [65] u8; out: [65] u8;
  kc := bytes_of("038282263212c609d9ea2a6e3e172de238d8c39cabd5ac1ca10646e23fd5f51508", k1[..]);
  ku := bytes_of("048282263212c609d9ea2a6e3e172de238d8c39cabd5ac1ca10646e23fd5f5150811f8a8098557dfe45e8256e830b60ace62d613ac2f7b17bed31b6eaff6e26caf", k2[..]);
  print("script_tests 0382...: recoded uncompressed matches 0482...: %\n",
    se_recode(kc, SE_UNCOMPRESSED, out[..]) == 65 && same_bytes(out[..], ku));
  print("script_tests 0482...: recoded compressed matches 0382...: %\n",
    se_recode(ku, SE_COMPRESSED, out[..]) == 33 && same_bytes(out[0..<33], kc));

  // Round trips of 32 points: P(0) = G, P(i + 1) = 2 P(i) + G.
  p := g; agree := [3] int.[0, 0, 0]; odd := 0;
  forms := [3] int.[SE_COMPRESSED, SE_UNCOMPRESSED, SE_XONLY];
  for i: 0..<32 {
    odd += se_parity(p);
    for f: 0..<3 {
      e: [65] u8; n := se_encode(p, forms[f], e[..]);
      back := ifx n == SE_XONLY then se_decode_xonly(e[0..<n]) else se_decode(e[0..<n]);
      // x-only gives back P when y is even and -P when it is odd.
      want := ifx n == SE_XONLY && se_parity(p) == 1 then sg_negate(p).value else p;
      if back.ok && same_point(back.value, want) { agree[f] += 1; }
    }
    p = sg_add(sg_double(p).value, g).value;
  }
  print("32 points (% with odd y): compressed %, uncompressed %, x-only % round-trip\n", odd, agree[0], agree[1], agree[2]);

  // Refusals.
  u[0] = 6; print("hybrid 06 || G: decodes %\n", se_decode(u[..]).ok);
  u[0] = 7; print("hybrid 07 || G: decodes %\n", se_decode(u[..]).ok);
  u[0] = 4; print("04 prefix in 33 bytes: decodes %\n", se_decode(u[0..<33]).ok);
  u[0] = 2; print("02 prefix in 65 bytes: decodes %\n", se_decode(u[..]).ok); u[0] = 4;
  zero: [1] u8; print("infinity's 00: decodes %\n", se_decode(zero[..]).ok);
  u[64] += 1; print("G with y + 1: decodes %\n", se_decode(u[..]).ok); u[64] -= 1;
  print("x = p (Wycheproof tcId 487): decodes %\n", decodes("04fffffffffffffffffffffffffffffffffffffffffffffffffffffffefffffc2f0000000000000000000000000000000000000000000000000000000000000000"));
  print("BIP-32 vector 5's 02 || 00...07: decodes %\n", decodes("020000000000000000000000000000000000000000000000000000000000000007"));
  print("BIP-340 vector 5's x-only key: decodes %\n", decodes("eefdea4cdb677750a420fee807eacf21eb9898ae79b9768766e4faa04a2d4a34"));
  print("BIP-340 vector 14's x = p + 1: decodes %\n", decodes("fffffffffffffffffffffffffffffffffffffffffffffffffffffffefffffc30"));
  print("encode infinity: % bytes\n", se_encode(sg_infinity(), SE_COMPRESSED, out[..]));
}
```

Run it from the repository root:

```shell
coax/bin/coax run bitcoin/examples/point-encodings.coax
```

The output:

```text
toy x = 1: x^3 + 7 = 8, 8 ^ 6 = 13, 13 ^ 2 = 8: prefix 02 gives y = 10, prefix 03 gives y = 13
toy x = 3: x^3 + 7 = 11, 11 ^ 6 = 9, 9 ^ 2 = 12: no point, refused
encode G: 33 bytes, 65 bytes, 32 bytes
G compressed: 0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798
G uncompressed: 0479be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798483ada7726a3c4655da4fbfc0e1108a8fd17b448a68554199c47d08ffb10d4b8
G x-only: 79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798
x^3 + 7: 4866d6a5ab41ab2c6bcc57ccd3735da5f16f80a548e5e20a44e4e9b8118c26f2
its square root (even): 483ada7726a3c4655da4fbfc0e1108a8fd17b448a68554199c47d08ffb10d4b8
p minus the root (odd): b7c52588d95c3b9aa25b0403f1eef75702e84bb7597aabe663b82f6f04ef2777
02 || x decodes to G: true
03 || x decodes to -G: true
3G parity: 0
3G compressed: 02f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9
3G uncompressed: 04f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9388f7b0f632de8140fe337e62a37f3566500a99934c2231b6cb9fd7584b8e672
3G x-only: f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9
matches BIP-340 vector 0's public key: true
-G parity: 1, x-only equals G's: true, lifts to G: true
script_tests 0382...: recoded uncompressed matches 0482...: true
script_tests 0482...: recoded compressed matches 0382...: true
32 points (15 with odd y): compressed 32, uncompressed 32, x-only 32 round-trip
hybrid 06 || G: decodes false
hybrid 07 || G: decodes false
04 prefix in 33 bytes: decodes false
02 prefix in 65 bytes: decodes false
infinity's 00: decodes false
G with y + 1: decodes false
x = p (Wycheproof tcId 487): decodes false
BIP-32 vector 5's 02 || 00...07: decodes false
BIP-340 vector 5's x-only key: decodes false
BIP-340 vector 14's x = p + 1: decodes false
encode infinity: 0 bytes
```

BIP-340 vector 0 has secret key 3, and its public key is
`f9308a01…e036f9`, the x-only form of 3G above. 3G's y ends in `72`, so it
is even and its compressed form starts with `02`. Bitcoin Core's
`script_tests.json` uses the key `0382…1508` in P2PKH tests and
`0482…6caf` in P2PK tests; the example shows that they are one point.

## Broken cases and their refusals

The last eleven lines are refusals. Each decoder returns `ok` false, and
the encoder returns 0:

| Input | Why it refuses |
| --- | --- |
| `06 \|\| x \|\| y` for G | Hybrid; Bitcoin Core's "P2PK with hybrid pubkey" case |
| `07 \|\| x \|\| y` for G | Hybrid, with the wrong parity besides |
| `04 \|\| x`, 33 bytes | Uncompressed prefix, compressed length |
| `02 \|\| x \|\| y`, 65 bytes | Compressed prefix, uncompressed length |
| `00` | Infinity is not a public key |
| G with y + 1 | Off the curve |
| x = p, Wycheproof `tcId` 487 | A coordinate is not below p |
| `02 \|\| 00…07`, BIP-32 vector 5 | `x^3 + 7` has no square root for x = 7 |
| BIP-340 vector 5's x-only key | No point has that x |
| BIP-340 vector 14's x = p + 1 | Not below p, though it reduces to 1, which lifts |
| Encoding infinity | No form encodes it |

Accepting any of them would let two byte strings name one key, or let a key
that isn't on secp256k1 reach the group law. A point off the curve lies on
another curve `y^2 = x^3 + b`, and an ECDH or signature check on it can leak
a secret key (Wycheproof calls this an invalid-curve attack).

## How the code is checked

### Published keys and a Python reference

[coax/tests/bitcoin/points.py](../../coax/tests/bitcoin/points.py) reads
public keys from five pinned files, after checking each file's SHA-256:

- Bitcoin Core's `script_tests.json` (newly pinned): G and the `0382…`
  key in both forms, a key a signature verifies against, and the four keys
  its `STRICTENC` cases refuse with `PUBKEYTYPE`: the hybrid `06` key, `04`
  in 33 bytes, `02` in 65 bytes and `06` in 33 bytes.
- Wycheproof's `ecdh_secp256k1_test.json`: the SEC 1 point inside each
  public key whose header names secp256k1. That gives 455 distinct valid
  uncompressed points, one valid compressed point, and 21 invalid ones: 16
  off the curve (including x = p and y = p), a modified point, a compressed
  x with no point, two low-order points on the twist and an empty point.
  Cases whose ASN.1 or curve parameters are what is wrong are left out.
- BIP-32: the key in each of the 17 extended public keys of vectors 1–4, and
  the three keys of vector 5 marked "invalid pubkey".
- BIP-340: the public key of each of the 19 rows; rows 5 and 14 must refuse.
- BIP-341: the internal and output key of each of the seven
  `scriptPubKey` cases.

It also has a reference codec in about 30 lines of Python, with its own
affine arithmetic, and asserts that the reference decodes each published key
exactly when the file says it is valid. Then it builds `secp_encode.coax` as
a C library in checked and release builds and checks that the codec agrees
with the reference on every published key, 300 random points in all nine
pairs of forms, and 3,000 corrupted encodings: every prefix byte, hybrid
keys, a flipped bit of y, the other y, random x (about half have a point),
x or y equal to p, x above p, and truncated and extended keys:

```shell
python3 coax/tests/bitcoin/points.py
```

It prints, for each build:

```text
checked: 539 published keys {'script_tests': 9, 'wycheproof': 477, 'bip32': 20, 'bip340': 19, 'bip341': 14} (30 refuse), 2 pairs, 300 random points x 9 form pairs, 3000 corrupted encodings (1005 decode)
release: 539 published keys {'script_tests': 9, 'wycheproof': 477, 'bip32': 20, 'bip340': 19, 'bip341': 14} (30 refuse), 2 pairs, 300 random points x 9 form pairs, 3000 corrupted encodings (1012 decode)
```

The two builds draw different random inputs from one seeded generator, so
their counts of corrupted encodings that decode differ.

Each count is of distinct keys within its file.

### Checks in Coax

`secp_encode.coax` checks G in all three forms, `−G` and its parity, 2G to
9G round-tripping between every pair of forms, every refusal in the table
above, and that each refusal writes nothing.

[bitcoin/examples/checks/point-encodings.coax](../examples/checks/point-encodings.coax)
runs the published keys in Coax. `points.py --write` copies them into
[coax/tests/bitcoin/point_vectors.coax](../../coax/tests/bitcoin/point_vectors.coax),
and `points.py` fails when that file is out of date. Wycheproof's valid
points stay in the Python check, apart from its first two, to keep the
Coax check fast.

<!-- source: bitcoin/examples/checks/point-encodings.coax published -->
```jai
// Every published key in coax/tests/bitcoin/point_vectors.coax decodes or
// refuses as its source says, and a key that decodes re-encodes to the same
// bytes. The table is copied from the pinned files by
// coax/tests/bitcoin/points.py, which also runs Wycheproof's 455 valid
// points through the codec in C.
#forall "each published key decodes or refuses as stated and re-encodes exactly" i: 0..<PV_KEYS.count #steps 500000000 {
  k := PV_KEYS[i]; b: [65] u8; out: [65] u8;
  v := bytes_of(k.hex, b[..]);
  // The empty encoding asks for a compressed key back; it refuses first.
  n := se_recode(v, ifx v.count == 0 then SE_COMPRESSED else v.count, out[..]);
  #assert (n > 0) == k.ok;
  if k.ok { #assert same_bytes(out[0..<v.count], v); }
}
// Bitcoin Core's script_tests.json uses G and a second key in both forms.
#forall "a compressed key and its uncompressed form are one point" i: 0..<PV_PAIRS.count #steps 500000000 {
  c: [33] u8; u: [65] u8; out: [65] u8;
  cv := bytes_of(PV_PAIRS[i].compressed, c[..]); uv := bytes_of(PV_PAIRS[i].uncompressed, u[..]);
  #assert se_recode(cv, SE_UNCOMPRESSED, out[..]) == 65 && same_bytes(out[..], uv);
  #assert se_recode(uv, SE_COMPRESSED, out[..]) == 33 && same_bytes(out[0..<33], cv);
}
```

<!-- source: bitcoin/examples/checks/point-encodings.coax parity -->
```jai
// An x-only key is the compressed key without its prefix when y is even. A
// BIP-341 output key is x-only; its compressed form starts with 02.
#forall "x-only keys lift to the point whose compressed form is 02 || x" i: 0..<PV_KEYS.count #steps 500000000 {
  k := PV_KEYS[i]; b: [65] u8; out: [65] u8;
  v := bytes_of(k.hex, b[..]);
  if k.ok && v.count == SE_XONLY {
    #assert se_recode(v, SE_COMPRESSED, out[..]) == 33 && out[0] == 2 && same_bytes(out[1..<33], v);
  }
}
```

To run all the checks:

```shell
coax/bin/coax test bitcoin/examples/checks/point-encodings.coax
coax/bin/coax test coax/lib/crypto/secp_encode.coax
coax/bin/coax mutate coax/lib/crypto/secp_encode.coax --root-only
```

`coax mutate` leaves zero survivors: 132 mutants, 131 killed and the one
marked equivalent above.

The callers' checks pass unchanged: `coax test` on `bip32.coax`,
`coax/lib/nostr/nipkeys.coax`, `coax/lib/nostr/nipkeys_sign.coax` and
`nostr/examples/checks/seed-phrase.coax`, and the BIP-39, BIP-32 and NIP-06
vectors in `coax/tests/nostr/weblogin_vectors.coax`.

## Summary

- Compressed keys are `02` or `03` and x; uncompressed keys are `04`, x
  and y; x-only keys are x, with y even.
- Decompression computes `x^3 + 7`, takes the square root
  `a^((p + 1) / 4)`, checks it, and picks the root by parity.
- An x-only key drops the parity bit; BIP-340 keeps points even, and
  Taproot's control block carries the output key's bit.
- The codec refuses hybrid keys, infinity, coordinates at least p, points
  off the curve and wrong lengths, and it takes variable time because
  public keys are public.

## Dependencies

- [15. The secp256k1 group law](ch015-the-secp256k1-group-law.md), for the
  points, `lift_x` and the compressed form.
- [13. Field arithmetic mod p](ch013-field-arithmetic-mod-p.md), for the
  square root.

## Sources

- [SEC 1 version 2.0](https://www.secg.org/sec1-v2.pdf), sections 2.3.3
  and 2.3.4 (point to octet string and back).
- [SEC 2 version 2.0](https://www.secg.org/sec2-v2.pdf), section 2.4.1,
  for G.
- [BIP-340](https://github.com/bitcoin/bips/blob/master/bip-0340.mediawiki),
  `lift_x` and x-only keys, and
  [BIP-341](https://github.com/bitcoin/bips/blob/master/bip-0341.mediawiki),
  the output key and the control block's parity bit.
- Bitcoin Core's `script_tests.json`, Wycheproof's
  `ecdh_secp256k1_test.json`, and the BIP-32, BIP-340 and BIP-341 vector
  files, pinned in
  [coax/tests/vectors/SOURCES.md](../../coax/tests/vectors/SOURCES.md).
