# 22. Public key recovery and signed messages

An ECDSA signature and its hash pin down the public key almost completely.
Given `(r, s)`, the hash and two extra bits, anyone can compute the one key
the signature verifies under. Bitcoin Core uses this for signed messages:
`signmessage` proves control of a P2PKH address without a transaction, and
the signature carries no key, only a header byte with those two bits.
`verifymessage` recovers the key, hashes it and compares the hash with the
address. This chapter builds recovery, extends chapter 20's signer to
return the two bits, and builds Bitcoin Core's signed messages on top,
checked against libsecp256k1's recovery module, tiny-secp256k1's 2,020
rows and Bitcoin Core's own test cases.

## What this chapter teaches

- Why a signature names up to four candidate nonce points `R`, and how
  the recovery ID (0 to 3) picks one: the parity of `y(R)` and whether
  `x(R)` was at least `n`.
- The recovery equation `Q = r^-1 · (s·R - z·G)`, and why it follows from
  the verification equation of chapter 19.
- The rare case `x(R) >= n`, where `x(R) = r + n`, and why `r + n` must
  stay below `p`.
- How the signer computes the recovery ID with masks, from values it
  already holds, without breaking chapter 3's rules.
- Bitcoin Core's signed messages: the magic text, compact-size prefixes,
  SHA-256d, the 65-byte compact signature with header bytes 27 to 34, and
  base64.
- Where this verifier is stricter than Bitcoin Core, and what each refusal
  code means.

## Where Bitcoin uses recovery

Bitcoin's consensus rules never recover a key: script always supplies the
key next to the signature. Recovery appears in two places outside
consensus:

- **Signed messages.** Bitcoin Core's `signmessage`,
  `signmessagewithprivkey` and `verifymessage` RPCs, and the wallet's
  "Sign Message" dialog, prove that the holder of a P2PKH address's key
  agreed to some text. Exchanges, auditors and proof-of-reserve reports
  use them. Since the address is a hash of the key, the signature must
  bring the key with it, or let the verifier recover it. Recovery saves 33
  bytes.
- **Compact signatures elsewhere.** Lightning's BOLT 11 invoices carry a
  65-byte recoverable signature, so a payer learns the payee's node key
  from the invoice itself (chapter 117).

BIP-137 extends the header byte to segwit addresses: 35 to 38 for
P2SH-P2WPKH and 39 to 42 for P2WPKH. Bitcoin Core doesn't implement it,
and this chapter doesn't either. BIP-322 replaces signed messages with a
virtual transaction that any script can sign, Taproot included. It needs
the transaction, script and signature-hash chapters (34 to 45), so it is
out of scope here; a later chapter can build it on them.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [ecdsa_recover.coax](../../coax/lib/crypto/ecdsa_recover.coax) | `ecdsa_recover_vartime(hash, signature, id, form, out)` | the key a signature and ID recover to, compressed or uncompressed |
| | `er_recover_vartime` | `Q = u1·G + u2·R` with `u1 = -z/r`, `u2 = s/r` |
| | `er_x_vartime`, `er_below_vartime` | `R`'s compressed encoding from `r` and the ID |
| [ecdsa_sign.coax](../../coax/lib/crypto/ecdsa_sign.coax) | `ecdsa_sign_recoverable(key, hash, w, out, overlap)` | chapter 20's signer, with the recovery ID in a 65th byte |
| | `ec_sign`, `ec_high`, `ec_over`, `ec_recid` | the shared signer body and the ID's three bits, all masks |
| [signmessage.coax](../../coax/lib/bitcoin/signmessage.coax) | `signed_message_hash(message, out)` | the hash a signed message signs |
| | `signed_message_sign(key, compressed, message, w, out)` | `signmessage`: 88 characters of base64 |
| | `signed_message_recover_vartime(signature, message, out)` | the key a base64 signature recovers to |
| | `verify_message_vartime(address, signature, message, testnet)` | `verifymessage` for a P2PKH address |

Recovery reads only the hash, the signature and the ID, which are all
public, so it runs in variable time and its names end in `_vartime`.
Signing keeps chapter 20's constant-time rules.

## The algorithm

### Four candidate points

Chapter 19's verifier accepts `(r, s)` for the hash `z` and key `Q` when

```text
R = s^-1·z·G + s^-1·r·Q      and      x(R) mod n = r
```

Multiply by `s` and solve for `Q`:

```text
s·R = z·G + r·Q      so      Q = r^-1 · (s·R - z·G)
```

The verifier doesn't know `R`, only `r = x(R) mod n`. Two things are
lost:

1. **The sign of `y`.** For each `x` on the curve there are two points,
   `(x, y)` and `(x, p - y)`. Exactly one of the two `y` values is odd,
   because `p` is odd.
2. **The reduction mod `n`.** `x(R)` is a field element below `p`, and
   `n < p`. If `x(R) >= n`, the signer published `r = x(R) - n`, so
   `x(R) = r + n`. That needs `r + n < p`, which means `r < p - n`, and
   `p - n` is about `2^128`.

The recovery ID records both: bit 0 is the parity of `y(R)` and bit 1 says
`x(R) >= n`. Each ID names at most one `R`, and each `R` gives one `Q`.
Different IDs give different keys, so the ID matters.

For secp256k1, a random `x(R)` is at least `n` with probability about
`2^-128`. IDs 2 and 3 never occur in an honest signature that anyone will
find, but a verifier must handle them, because an attacker picks the ID.
The toy curve of chapters 19 and 20 shows them often: there `n = 31` and
`p = 43`, so 12 of the 43 values of `x` are at least `n`, and 406 of
the toy curve's 870 signatures need ID 2 or 3.

### Recovery

Recovering the key from the hash `h`, the signature `(r, s)` and the ID:

1. **Lengths.** The hash is 32 bytes and the signature 64.
2. **The ID.** It is 0, 1, 2 or 3.
3. **The scalars.** `r` and `s` are in `[1, n-1]`, the same check as
   chapter 19's `ev_scalars_vartime`.
4. **`x(R)`.** It is `r`, or `r + n` when bit 1 is set. Refuse if
   `r + n >= p`.
5. **`R`.** Lift `x` to the curve and take the `y` whose parity is bit 0:
   that's chapter 17's decoder applied to the compressed encoding
   `02 || x` or `03 || x`. Refuse if no point has that `x`.
6. **`Q`.** `z = h mod n`, `u1 = -z·r^-1` and `u2 = s·r^-1`, then
   `Q = u1·G + u2·R` with chapter 16's `ecmult_vartime`. Refuse if `Q` is
   the point at infinity.

`er_x_vartime` does steps 4 and 5's encoding. The bound is a 32-byte
comparison with `p - n`, and the addition carries byte by byte:

<!-- source: coax/lib/crypto/ecdsa_recover.coax x -->
```jai
// R in compressed form into x (33 bytes): the prefix 02 or 03 from bit 0
// of the ID, then x(R), which is r, or r + n when bit 1 is set. False when
// r is not 32 bytes, or when r + n would reach p, exactly when r >= p - n.
er_x_vartime :: (r: [] u8, id: int, x: [] u8) -> bool {
  if r.count != 32 { return false; }
  if (id & 2) != 0 && !er_below_vartime(r, ER_P_MINUS_N) { return false; }
  x[0] = cast(u8) (2 | (id & 1));
  carry := 0;
  for j: 0..<32 {
    i := 31 - j;
    sum := cast(int) r[i] + carry;
    if (id & 2) != 0 { sum += cast(int) ER_N[i]; }
    x[1 + i] = cast(u8) (sum & 255);
    carry = sum >> 8;
  }
  return true;   // r < p - n, so r + n < p < 2^256: no carry is left
}
// a < b for two 32-byte big-endian numbers: the first differing byte decides.
er_below_vartime :: (a: [] u8, b: [32] u8) -> bool {
  for i: 0..<32 {
    if a[i] < b[i] { return true; }
    if a[i] > b[i] { return false; }
  }
  return false;                                          // equal
}
```

`er_recover_vartime` is the whole algorithm, one refusal code per step:

<!-- source: coax/lib/crypto/ecdsa_recover.coax recover -->
```jai
// The key Q with s·R = z·G + r·Q, for the hash z and the R that the ID
// names: Q = r^-1 · (s·R - z·G) = u1·G + u2·R with u1 = -z/r, u2 = s/r.
er_recover_vartime :: (hash: [] u8, signature: [] u8, id: int) -> ErResult {
  if hash.count != 32 || signature.count != 64 { return ErResult.{code=ER_BAD_LENGTH}; }
  if id < 0 || id > 3 { return ErResult.{code=ER_BAD_ID}; }
  scalars := ev_scalars_vartime(signature);              // r and s in [1, n-1] (chapter 19)
  if !scalars.ok { return ErResult.{code=ER_BAD_SCALAR}; }
  x: [33] u8;
  if !er_x_vartime(signature[0..<32], id, x[..]) { return ErResult.{code=ER_NO_X}; }
  big_r := se_decode(x[..]);                             // lift_x and the parity (chapter 17)
  if !big_r.ok { return ErResult.{code=ER_NO_POINT}; }
  z := sc_reduce_bytes(hash).value;                      // a hash at least n is reduced
  rinv := sc_inverse(scalars.r).value;
  u1 := sc_negate(sc_mul(z, rinv)); u2 := sc_mul(scalars.s, rinv);
  q := ecmult_vartime(u1, big_r.value, u2);              // u1·G + u2·R (chapter 16)
  if sg_to_affine(q.value).value.infinity != 0 { return ErResult.{code=ER_INFINITY}; }
  return ErResult.{key=q.value};
}
```

The entry encodes the key in the caller's form. Each refusal has its own
negative code, and a refusal writes nothing:

<!-- source: coax/lib/crypto/ecdsa_recover.coax entry -->
```jai
// The public key that the compact signature r || s of a 32-byte hash
// recovers to with the ID, written to out in form (SE_COMPRESSED, 33
// bytes, or SE_UNCOMPRESSED, 65). Returns the bytes written, or one of the
// ER_ refusals, which write nothing. Any recovered key verifies the
// signature; whether it is the right key is the caller's question.
ecdsa_recover_vartime :: (hash: [] u8, signature: [] u8, id: int, form: int, out: [] u8) -> int #export_c {
  if (form != SE_COMPRESSED && form != SE_UNCOMPRESSED) || out.count < form { return ER_BAD_FORM; }
  result := er_recover_vartime(hash, signature, id);
  if result.code != 0 { return result.code; }
  return se_encode(result.key, form, out);
}
```

| Code | Meaning |
| --- | --- |
| `ER_BAD_LENGTH` (-1) | the hash isn't 32 bytes or the signature isn't 64 |
| `ER_BAD_ID` (-2) | a recovery ID outside 0 to 3 |
| `ER_BAD_SCALAR` (-3) | `r` or `s` is 0 or at least `n` |
| `ER_NO_X` (-4) | ID 2 or 3, and `r + n` is at least `p` |
| `ER_NO_POINT` (-5) | no curve point has that `x` |
| `ER_INFINITY` (-6) | the recovered key is the point at infinity |
| `ER_BAD_FORM` (-7) | the form isn't 33 or 65 bytes, or `out` is too short |

Recovery doesn't check low-S. `(r, n - s)` recovers too, with the other
parity, to the same key: negating `s` negates `s·R`, which is the same as
using `-R`. Bitcoin Core's `verifymessage` accepts both halves, and so does
this code. A recovered key always verifies the signature, but recovery
alone proves nothing: every signature recovers to some key. What proves
something is comparing the key with one the verifier already trusts, here
an address.

`Q` is infinity only when `s·R = z·G`, that is, when the "signer" used the
key 0. libsecp256k1's tests sign with key 0 to reach that case, and the
example shows it.

### Worked example: the generator comes back

Chapter 20 signed `h = SHA-256("Satoshi Nakamoto")` with the key 1:
`r = 934b1ea1...210ee3d8`. The signer computed `R = k·G` with even `y`
and a high `s`, then published `n - s`, which signs with `-R`, whose `y` is
odd. So the ID is 1, and the example's transcript below shows each step:

1. `x(R) = r`, below `n`; the prefix `03` says odd `y`, so
   `R = 03 934b1ea1...210ee3d8`.
2. `u1 = -z/r = fd6baba0...e5445636` and `u2 = s/r = 938432d2...67c287e3`,
   both mod `n`.
3. `u1·G + u2·R = 02 79be667e...16f81798`, which is `G`: the key 1.

With ID 0 the same signature recovers to another valid key, and IDs 2 and
3 refuse because `r` is far above `p - n`. With the high `s` and ID 0,
recovery gives `G` again.

## Recoverable signing

The signer knows `R` and whether it flipped `s`, so it can report the ID
for free. libsecp256k1's `secp256k1_ecdsa_sign_recoverable` returns it
this way, and Bitcoin Core's `CKey::SignCompact` uses that.

`ecdsa_sign_recoverable` is chapter 20's signer with a 65th output byte.
Both entries check their views and call one body, `ec_sign`, so there is
one signer, not two. The body now keeps the low-S decision in the
workspace as a bit, `high`:

<!-- source: coax/lib/crypto/ecdsa_sign.coax low -->
```jai
// 1 when s > (n-1)/2, else 0. For s < n, that holds exactly when 2s >= n,
// so compute 2s in nine limbs, subtract n, and read the borrow.
ec_high :: (s: ScScalar) -> u64 {
  wide: [9] u32; carry: u32;
  for i: 0..<8 { wide[i] = (s.limbs[i] << 1) | carry; carry = s.limbs[i] >> 31; }
  wide[8] = carry;
  return sc_subtract_n(wide).borrow ^ 1;
}
// n - s when s is in the high half, selected on that bit.
ec_low :: (s: ScScalar) -> ScScalar {
  return sc_select(ec_high(s), sc_negate(s), s);
}
```

<!-- source: coax/lib/crypto/ecdsa_sign.coax ecdsa -->
```jai
// The signature of a 32-byte message hash under a 32-byte key, with RFC
// 6979's nonce, into out: r || s (32 bytes each, s in the low half), then
// the recovery ID when out has 65 bytes. The caller checks the views.
ec_sign :: (key: [] u8, hash: [] u8, w: [] EcdsaWork, out: [] u8) -> bool {
  #assert ss_key(key, w[0].key[..], false);              // d, or 1 with valid = 0
  ec_candidates(key, hash, w);
  ec_pick(w);                                            // k
  #assert sp_base(w[0].k, w[0].point[..]) && sp_x(w[0].point[..], w[0].x[..]);
  w[0].r = sc_reduce_bytes(w[0].x[..]).value;            // r = x(kG) mod n
  z := sc_reduce_bytes(hash).value;                      // z = h mod n
  w[0].kinv = sc_inverse(w[0].k).value;
  w[0].s = sc_mul(w[0].kinv, sc_add(z, sc_mul(w[0].r, w[0].key[0].safe)));   // s = (z + r*d) / k
  w[0].high = ec_high(w[0].s);
  w[0].s = sc_select(w[0].high, sc_negate(w[0].s), w[0].s);                // the low half
  valid := w[0].key[0].valid & w[0].valid & w[0].point[0].finite & ec_nonzero(w[0].r) & ec_nonzero(w[0].s);
  if !ec_declassify(valid) { return ec_clear(w); }      // the one fact the signer reveals
  for i: 0..<8 {
    #assert cb_store_be32(out, 28-4*i, w[0].r.limbs[i]) && cb_store_be32(out, 60-4*i, w[0].s.limbs[i]);
  }
  if out.count == 65 { out[64] = ec_recid(w); }         // a public length decides
  return !ec_clear(w);
}
// The low-S ECDSA signature r || s (32 bytes each). Views are stable and
// disjoint (overlap is the host's truthful observation). out is written
// only when the result is true; the workspace is cleared either way.
ecdsa_sign :: (key: [] u8, hash: [] u8, w: [] EcdsaWork, out: [] u8, overlap: bool) -> bool #export_c {
  if !ec_views(key, hash, w, out, overlap, 64) { return false; }
  return ec_sign(key, hash, w, out);
}
```

The ID's three inputs are all secret until the signature is published:
`y(R)`'s parity comes from the nonce, `high` from `s`, and `x(R) >= n` from
the nonce. Chapter 3's rules apply until the call returns, so each is a
0-or-1 value combined with `^`, `&`, `|` and `<<`, never a branch:

- **Parity.** `sp_base` already computes `y(R)`'s low bit as `odd` when it
  converts `R` to affine coordinates (chapter 24's signer uses it).
- **The flip.** Low-S replaces `s` with `n - s`, which is the signature for
  `-R`, so the parity flips: `odd ^ high`.
- **`x(R) >= n`.** `ec_over` loads the 32 bytes of `x(R)` into nine limbs,
  subtracts `n` and keeps the borrow, the same way `ec_high` tests `2s`.

<!-- source: coax/lib/crypto/ecdsa_sign.coax recoverable -->
```jai
// 1 when the 32 bytes of x(R) are at least n, else 0: the borrow of x - n.
ec_over :: (x: [] u8) -> u64 {
  wide: [9] u32;
  for i: 0..<8 { wide[7-i] = cb_load_be32(x, 4*i).value; }   // big-endian words; wide[8] stays 0
  return sc_subtract_n(wide).borrow ^ 1;
}
// The recovery ID: bit 0 is the parity of y(R), flipped when low-S
// replaced s with n - s (that signs with -R), and bit 1 says x(R) >= n.
ec_recid :: (w: [] EcdsaWork) -> u8 {
  return cast,trunc(u8) (((w[0].point[0].odd ^ w[0].high) & 1) | (ec_over(w[0].x[..]) << 1));   // no range check on a secret
}
// ecdsa_sign with the recovery ID in out[64]: out holds 65 bytes,
// r || s || id. Everything else is ecdsa_sign's.
ecdsa_sign_recoverable :: (key: [] u8, hash: [] u8, w: [] EcdsaWork, out: [] u8, overlap: bool) -> bool #export_c {
  if !ec_views(key, hash, w, out, overlap, 65) { return false; }
  return ec_sign(key, hash, w, out);
}
```

The branch `if out.count == 65` depends on a view length, which is public.
The ID is published with the signature, so writing it into `out` reveals
nothing new. The Memcheck probe of chapter 20 now signs with both entries,
and still finds only the `ec_declassify` decision (see "Secret flow"
below).

## Signed messages

### The message hash

Bitcoin Core's `MessageHash` serializes two strings into a hash writer:
the magic text `"Bitcoin Signed Message:\n"` and the message. Bitcoin
serializes a string as its length in compact-size form, then its bytes.
The hash is SHA-256d (chapter 5):

```text
SHA-256d( 18 || "Bitcoin Signed Message:\n" || compact_size(len(m)) || m )
```

The magic is 24 bytes, so its prefix is the single byte `0x18`. A compact
size is one byte for values below `0xfd`; otherwise it is the byte `fd`,
`fe` or `ff` followed by the value in 2, 4 or 8 little-endian bytes. The
magic exists so that a signed message can never be a transaction's
signature hash: no transaction serializes to bytes that start this way.

<!-- source: coax/lib/bitcoin/signmessage.coax hash -->
```jai
// Bitcoin's compact size: n in one byte below 0xfd, else the byte fd, fe
// or ff and n in 2, 4 or 8 little-endian bytes. Returns the bytes written.
sm_compact_size :: (n: int, out: [] u8) -> int {
  size := 9; first := 0xff;
  if n < 0xfd { out[0] = cast(u8) n; return 1; }
  if n <= 0xffff { size = 3; first = 0xfd; }
  else if n <= 0xffffffff { size = 5; first = 0xfe; }
  out[0] = cast(u8) first;
  for i: 1..<size { out[i] = cast(u8) ((n >> (8 * (i - 1))) & 255); }
  return size;
}
// Absorb the bytes of text, one at a time (SHA-256 buffers them). True
// when every update succeeds.
sm_absorb :: (state: [] u8, text: string) -> bool {
  one: [1] u8; ok := true;
  for i: 0..<text.count {
    one[0] = text[i];
    ok = ok && sha256_update_checked(state, one[..], false) == SHA_OK;
  }
  return ok;
}
// The hash a signed message signs: SHA-256d of compact_size(24), the magic,
// compact_size(len(message)) and the message, into out (32 bytes).
signed_message_hash :: (message: string, out: [] u8) -> bool #export_c {
  if out.count != 32 { return false; }
  state: [SHA_STATE_BYTES] u8; magic: [9] u8; length: [9] u8; first: [32] u8;
  m := sm_compact_size(SM_MAGIC.count, magic[..]);
  l := sm_compact_size(message.count, length[..]);
  ok := sha256_init(state[..]) == SHA_OK;
  ok = ok && sha256_update_checked(state[..], magic[0..<m], false) == SHA_OK && sm_absorb(state[..], SM_MAGIC);
  ok = ok && sha256_update_checked(state[..], length[0..<l], false) == SHA_OK && sm_absorb(state[..], message);
  ok = ok && sha256_final_checked(state[..], first[..], false) == SHA_OK;
  return ok && sha256_hash_checked(first[..], out, false) == SHA_OK;
}
```

### The compact signature

A signed message's signature is 65 bytes, then base64 (RFC 4648 with
padding, chapter 1), 88 characters:

| Byte | Content |
| --- | --- |
| 0 | the header: `27 + id`, plus 4 when the address hashes the compressed key |
| 1–32 | `r`, big-endian |
| 33–64 | `s`, big-endian |

So headers 27 to 30 mean an uncompressed key, and 31 to 34 a compressed
one. The compressed flag matters because a P2PKH address is the HASH160 of
the key's encoding: the compressed and uncompressed forms of one key give
two different addresses. The signer must say which one it signs for.

<!-- source: coax/lib/bitcoin/signmessage.coax sign -->
```jai
// signmessage: the base64 signature (88 characters) of message under a
// 32-byte key, into out. compressed says which form of the key the
// address hashes; it sets bit 2 of the header. Returns 88 or a refusal.
signed_message_sign :: (key: [] u8, compressed: bool, message: string, w: [] EcdsaWork, out: [] u8) -> int #export_c {
  if out.count < SM_SIGNATURE_TEXT { return SM_CAPACITY; }
  hash: [32] u8; signed: [65] u8; compact: [65] u8;
  ok := signed_message_hash(message, hash[..]) && ecdsa_sign_recoverable(key, hash[..], w, signed[..], false);
  if !ok { return SM_BAD_KEY; }
  flag := 0; if compressed { flag = 4; }
  compact[0] = cast(u8) (27 + cast(int) signed[64] + flag);   // the header: 27 + ID + 4·compressed
  for i: 0..<64 { compact[1 + i] = signed[i]; }
  return base64_encode_checked(cast(string) compact[..], out[0..<SM_SIGNATURE_TEXT], false).written;
}
```

### Verification

`verifymessage` takes an address, a base64 signature and the message.
`verify_message_vartime` follows Bitcoin Core's `MessageVerify` step by
step, and returns the first refusal in Core's order:

1. Decode the address (chapter 26). Not an address of this network:
   `SM_INVALID_ADDRESS`. A P2SH address names a script, not a key:
   `SM_ADDRESS_NO_KEY`.
2. Decode the base64. Not canonical padded base64: `SM_MALFORMED_SIGNATURE`.
3. Recover the key in the form the header names. No key:
   `SM_NOT_RECOVERED`.
4. Compare the key's HASH160 (chapter 6) with the address's hash:
   `SM_NOT_SIGNED` or `SM_OK`.

<!-- source: coax/lib/bitcoin/signmessage.coax recover -->
```jai
// The key a base64 signature recovers for message, in the form its header
// names (33 bytes compressed, 65 uncompressed), into out. Returns the
// bytes written or a refusal.
signed_message_recover_vartime :: (signature: string, message: string, out: [] u8) -> int #export_c {
  bytes: [65] u8;
  decoded := base64_decode_checked(signature, bytes[..], false);   // canonical padded base64 (chapter 1)
  if decoded.error == NC_INVALID { return SM_MALFORMED_SIGNATURE; }
  if decoded.error != NC_OK || decoded.written != SM_SIGNATURE_BYTES { return SM_BAD_LENGTH; }
  header := cast(int) bytes[0];
  if header < 27 || header > 34 { return SM_BAD_HEADER; }
  form := ifx header >= 31 then SE_COMPRESSED else SE_UNCOMPRESSED;
  if out.count < form { return SM_CAPACITY; }
  hash: [32] u8;
  ok := signed_message_hash(message, hash[..]);
  n := ecdsa_recover_vartime(hash[..], bytes[1..<65], (header - 27) & 3, form, out);
  if !ok || n != form { return SM_NOT_RECOVERED; }
  return form;
}
```

<!-- source: coax/lib/bitcoin/signmessage.coax verify -->
```jai
// verifymessage: does signature sign message under the key of address, a
// P2PKH address of mainnet, or of the test networks when testnet is set?
// Returns SM_OK or the first refusal, in Bitcoin Core's order.
verify_message_vartime :: (address: string, signature: string, message: string, testnet: bool) -> int #export_c {
  want: [20] u8;
  kind := base58_address_decode_vartime(address, want[..]);   // chapter 26
  p2pkh := ifx testnet then B58_TEST_P2PKH else B58_MAIN_P2PKH;
  p2sh := ifx testnet then B58_TEST_P2SH else B58_MAIN_P2SH;
  if kind == p2sh { return SM_ADDRESS_NO_KEY; }
  if kind != p2pkh { return SM_INVALID_ADDRESS; }
  key: [65] u8;
  n := signed_message_recover_vartime(signature, message, key[..]);
  if n < SE_COMPRESSED { return n; }                     // a refusal
  got: [20] u8;
  ok := hash160_checked(key[0..<n], got[..], false) == RMD_OK;   // chapter 6
  if !ok || cb_ct_equal(got[..], want[..]) != CB_MAX64 { return SM_NOT_SIGNED; }
  return SM_OK;
}
```

### Where this verifier differs from Bitcoin Core

Two refusals are this verifier's own:

- **`SM_BAD_LENGTH`.** Base64 that decodes to other than 65 bytes. Core
  returns `ERR_PUBKEY_NOT_RECOVERED` for this; the result is the same
  refusal under a more specific name.
- **`SM_BAD_HEADER`.** A header byte outside 27 to 34. Core's
  `CPubKey::RecoverCompact` doesn't check the range. It computes
  `(header - 27) & 3` for the ID and `(header - 27) & 4` for the flag, so
  every one of the 256 header values means something. Header 35, BIP-137's
  first segwit value, reads as ID 0 with an uncompressed key, for example.
  So one signature has many header bytes that Core accepts. The native
  harness below counts them: for one signature, Core accepts 31 other
  header bytes. This verifier accepts only the eight that a signer writes,
  and refuses the rest.

Bitcoin Core's own `message_verify` test includes one case where this
matters: an all-zero signature. Core answers `ERR_PUBKEY_NOT_RECOVERED`
because `r = 0`; this verifier answers `SM_BAD_HEADER` because the header
byte is 0. Both refuse. Every other case gives Core's result.

Two more differences are scope, not policy. Core decodes Bech32 and
Bech32m addresses too and answers `ERR_ADDRESS_NO_KEY` for them; this
verifier reads Base58 addresses only, so it answers `SM_INVALID_ADDRESS`
for them. Both refuse; answering `SM_ADDRESS_NO_KEY` would take chapter
28's segwit decoder, which this verifier doesn't load yet. And Core's node checks the
address against its own network; here the caller says which with
`testnet`, which covers testnet, signet and regtest, whose P2PKH and P2SH
version bytes are the same.

## The example

The example is
[bitcoin/examples/signed-messages.coax](../examples/signed-messages.coax).
It recovers every key on the toy curve, recovers chapter 20's worked
example by hand with each ID, checks every 20th pinned row, shows the
refusals and libsecp256k1's edge cases, and runs Bitcoin Core's
signed-message cases. Its vector table,
[coax/tests/bitcoin/ecdsa_recover_vectors.coax](../../coax/tests/bitcoin/ecdsa_recover_vectors.coax),
is copied from the pinned tiny-secp256k1 and Bitcoin Core files by
[make_ecdsa_recover_vectors.py](../../coax/tests/bitcoin/make_ecdsa_recover_vectors.py),
which refuses a file whose SHA-256 differs from its pin. The keys are
published test keys.

<!-- source: bitcoin/examples/signed-messages.coax example -->
```jai
#load "../../coax/lib/bitcoin/signmessage.coax";
#load "../../coax/lib/nostr/hex.coax";
#load "../../coax/tests/bitcoin/ecdsa_sign_vectors.coax";
#load "../../coax/tests/bitcoin/ecdsa_recover_vectors.coax";
#import "Basic";

// Print a label and bytes as lowercase hex.
show :: (label: string, bytes: [] u8) {
  text: [160] u8;
  r := hex_encode_checked(cast(string) bytes, text[..], false);
  print("%: %\n", label, cast(string) text[0..<r.written]);
}
show_scalar :: (label: string, s: ScScalar) {
  b: [32] u8;
  if sc_store(s, b[..]) { show(label, b[..]); }
}
// Decode hex into out and return the filled part. Every literal here and
// in the vector tables is valid hex, so a decoding error stops the program.
decode :: (text: string, out: [] u8) -> [] u8 {
  r := hex_decode_checked(text, out, false);
  #assert r.error == 0;
  return out[0..<r.written];
}
bytes_same :: (a: [] u8, b: [] u8) -> bool {
  if a.count != b.count { return false; }
  for i: 0..<a.count { if a[i] != b[i] { return false; } }
  return true;
}
// The compressed public key d·G.
compressed_key :: (d: [] u8, out: [] u8) -> [] u8 {
  #assert se_encode(smb_multiply_web(sc_load(d).value), SE_COMPRESSED, out) == SE_COMPRESSED;
  return out[0..<33];
}
// The name of a recovery or signed-message result.
recover_name :: (code: int) -> string {
  if code == ER_BAD_LENGTH { return "bad length"; }
  if code == ER_BAD_ID { return "recovery ID out of range"; }
  if code == ER_BAD_SCALAR { return "r or s not in [1, n-1]"; }
  if code == ER_NO_X { return "r + n is at least p"; }
  if code == ER_NO_POINT { return "no point has that x"; }
  if code == ER_INFINITY { return "the key is infinity"; }
  if code == ER_BAD_FORM { return "bad output form"; }
  return "recovered";
}
message_name :: (code: int) -> string {
  if code == SM_OK { return "OK"; }
  if code == SM_INVALID_ADDRESS { return "ERR_INVALID_ADDRESS"; }
  if code == SM_ADDRESS_NO_KEY { return "ERR_ADDRESS_NO_KEY"; }
  if code == SM_MALFORMED_SIGNATURE { return "ERR_MALFORMED_SIGNATURE"; }
  if code == SM_NOT_RECOVERED { return "ERR_PUBKEY_NOT_RECOVERED"; }
  if code == SM_NOT_SIGNED { return "ERR_NOT_SIGNED"; }
  if code == SM_BAD_LENGTH { return "bad length"; }
  if code == SM_BAD_HEADER { return "bad header byte"; }
  if code == SM_BAD_KEY { return "bad key"; }
  return "capacity";
}

// ---- the toy curve of chapters 19 and 20: y^2 = x^3 + 7 over F_43, 31
// points, n = 31 < p = 43, so x(R) >= n happens for 12 of the 43 values of x.
TOY_P :: 43;
TOY_N :: 31;
Toy :: struct { x: int; y: int; }
TOY_G :: Toy.{x=2,y=12};
TOY_INFINITY :: Toy.{x=-1,y=-1};
toy_mod :: (a: int, m: int) -> int { r := a % m; return ifx r < 0 then r + m else r; }
toy_inverse :: (a: int, m: int) -> int {
  r := 1;
  for i: 0..<m-2 { r = toy_mod(r * a, m); }
  return r;
}
toy_add :: (a: Toy, b: Toy) -> Toy {
  if a.x < 0 { return b; }
  if b.x < 0 { return a; }
  if a.x == b.x && toy_mod(a.y + b.y, TOY_P) == 0 { return TOY_INFINITY; }
  slope := ifx a.x == b.x then toy_mod(3 * a.x * a.x * toy_inverse(2 * a.y, TOY_P), TOY_P)
           else toy_mod((b.y - a.y) * toy_inverse(toy_mod(b.x - a.x, TOY_P), TOY_P), TOY_P);
  x := toy_mod(slope * slope - a.x - b.x, TOY_P);
  return Toy.{x=x,y=toy_mod(slope * (a.x - x) - a.y, TOY_P)};
}
toy_times :: (k: int, p: Toy) -> Toy {
  r := TOY_INFINITY;
  for i: 0..<toy_mod(k, TOY_N) { r = toy_add(r, p); }
  return r;
}
ToySig :: struct { r: int; s: int; id: int; }
// A signature with its recovery ID: bit 0 is y(R) odd, bit 1 is x(R) >= n.
// (No low-S on the toy curve.)
toy_sign :: (z: int, d: int, k: int) -> ToySig {
  big_r := toy_times(k, TOY_G);
  r := big_r.x % TOY_N;
  id := (big_r.y % 2) + 2 * (ifx big_r.x >= TOY_N then 1 else 0);
  return ToySig.{r=r,s=toy_mod(toy_inverse(k, TOY_N) * (z + r * d), TOY_N),id=id};
}
// Q = r^-1 (s·R - z·G) for the R the ID names, or infinity when no point fits.
toy_recover :: (z: int, sig: ToySig) -> Toy {
  x := sig.r + TOY_N * (sig.id / 2);
  if x >= TOY_P { return TOY_INFINITY; }
  for y: 0..<TOY_P {
    if y % 2 != sig.id % 2 || toy_mod(y * y - x * x * x - 7, TOY_P) != 0 { continue; }
    big_r := Toy.{x=x,y=y};
    minus_z := toy_times(toy_mod(-z, TOY_N), TOY_G);
    return toy_times(toy_inverse(sig.r, TOY_N), toy_add(toy_times(sig.s, big_r), minus_z));
  }
  return TOY_INFINITY;
}

// libsecp256k1's edge cases (ecdsa_recover.coax's checks), ID 4 included.
EDGE_SIGS :: [3] string.[ER_T_SIG, ER_T_FOUR, ER_T_ONE];
EDGE_NAMES :: [3] string.["signed by key 0", "(4, 4)", "(1, 1)"];
KEY_ONE :: "0000000000000000000000000000000000000000000000000000000000000001";
SATOSHI_HASH :: "a0dc65ffca799873cbea0ac274015b9526505daaaed385155425f7337704883e";

main :: () {
  // The toy curve: every key comes back from its signature and ID.
  print("toy curve y^2 = x^3 + 7 mod 43, n = 31, G = (2, 12)\n");
  sig := toy_sign(5, 7, 10);
  q := toy_recover(5, sig);
  print("d = 7, k = 10, z = 5: (r, s) = (%, %), R = 10G = (42, 7), ID %\n", sig.r, sig.s, sig.id);
  print("x = r + n = %, y odd, Q = (%, %) = 7G: %\n", sig.r + TOY_N, q.x, q.y, q.x == toy_times(7, TOY_G).x && q.y == toy_times(7, TOY_G).y);
  high := 0; recovered := 0;
  for d: 1..<TOY_N { for k: 1..<TOY_N {
    t := toy_sign(5, d, k);
    if t.r == 0 || t.s == 0 { continue; }
    if t.id >= 2 { high += 1; }
    back := toy_recover(5, t); want := toy_times(d, TOY_G);
    if back.x == want.x && back.y == want.y { recovered += 1; }
  }}
  print("all keys and nonces: % recover their key, % with x(R) >= n\n", recovered, high);

  // secp256k1: the signature of chapter 20's worked example, by hand.
  print("secp256k1, key 1, SHA-256(\"Satoshi Nakamoto\")\n");
  key: [32] u8; hash: [32] u8; w: [1] EcdsaWork; signed: [65] u8;
  d := decode(KEY_ONE, key[..]); h := decode(SATOSHI_HASH, hash[..]);
  print("ecdsa_sign_recoverable: %, ID %\n", ecdsa_sign_recoverable(d, h, w[..], signed[..], false), signed[64]);
  show("r || s", signed[0..<64]);
  x: [33] u8;
  #assert er_x_vartime(signed[0..<32], 1, x[..]);
  show("R (ID 1: x = r, y odd)", x[..]);
  scalars := ev_scalars_vartime(signed[0..<64]);
  rinv := sc_inverse(scalars.r).value; z := sc_reduce_bytes(h).value;
  show_scalar("u1 = -z/r", sc_negate(sc_mul(z, rinv)));
  show_scalar("u2 = s/r", sc_mul(scalars.s, rinv));
  for id: 0..<4 {
    out: [33] u8;
    n := ecdsa_recover_vartime(h, signed[0..<64], id, SE_COMPRESSED, out[..]);
    print("Q, ID %: ", id);
    if n > 0 { show(recover_name(n), out[..]); } else { print("%\n", recover_name(n)); }
  }
  // n - s: the other half of the signature recovers the same key with the
  // other parity. Recovery has no low-S rule.
  flipped: [64] u8; back: [33] u8; g: [33] u8;
  for i: 0..<32 { flipped[i] = signed[i]; }
  #assert sc_store(sc_negate(scalars.s), flipped[32..<64]);
  #assert ecdsa_recover_vartime(h, flipped[..], 0, SE_COMPRESSED, back[..]) == SE_COMPRESSED;
  print("(r, n - s) with ID 0 gives G too: %\n", bytes_same(back[..], compressed_key(d, g[..])));

  // Every 20th pinned row: the signer gives the published ID, and recovery
  // with it gives d·G. A recovery costs about ten signatures.
  print("tiny-secp256k1 ecdsa.json, every 20th row\n");
  ids := 0; keys := 0; tried := 0;
  for i: 0..<ES_CASES.count {
    if i % 20 != 0 { continue; }
    c := ES_CASES[i]; kb: [32] u8; mb: [32] u8; got: [65] u8; mine: [33] u8; theirs: [33] u8;
    dk := decode(c.d, kb[..]); m := decode(c.m, mb[..]);
    #assert ecdsa_sign_recoverable(dk, m, w[..], got[..], false);
    tried += 1;
    if got[64] == ER_IDS[i] { ids += 1; }
    if ecdsa_recover_vartime(m, got[0..<64], cast(int) ER_IDS[i], SE_COMPRESSED, mine[..]) == SE_COMPRESSED &&
       bytes_same(mine[..], compressed_key(dk, theirs[..])) { keys += 1; }
  }
  print("% of % give the published ID, % recover the signing key\n", ids, tried, keys);

  // The pinned refusals and libsecp256k1's edge cases, with their codes.
  print("refusals\n");
  for i: 0..<ER_REFUSE.count {
    c := ER_REFUSE[i]; mb: [33] u8; sb: [64] u8; out: [33] u8;
    print("%: %\n", c.why, recover_name(ecdsa_recover_vartime(decode(c.m, mb[..]), decode(c.sig, sb[..]), c.id, SE_COMPRESSED, out[..])));
  }
  for which: 0..<3 {
    text := EDGE_SIGS[which]; name := EDGE_NAMES[which];
    mb: [32] u8; sb: [64] u8;
    for id: 0..<5 {
      out: [33] u8;
      n := ecdsa_recover_vartime(decode(ER_T_MSG, mb[..]), decode(text, sb[..]), id, SE_COMPRESSED, out[..]);
      print("%, ID %: %\n", name, id, recover_name(n));
    }
  }
  // Wycheproof tcId 346: r = p - n - 3, so ID 2 puts x(R) = p - 3.
  wh: [32] u8; ws: [64] u8; wk: [33] u8;
  wsig := "000000000000000000000000000000014551231950b75fc4402da1722fc9baeb0000000000000000000000000000000000000000000000000000000000000003";
  #assert ecdsa_recover_vartime(decode(EV_T_W_HASH, wh[..]), decode(wsig, ws[..]), 2, SE_COMPRESSED, wk[..]) == SE_COMPRESSED;
  show("Wycheproof tcId 346, ID 2", wk[..]);

  // Bitcoin Core's signed messages.
  print("signed messages\n");
  mh: [32] u8;
  #assert signed_message_hash("Trust no one", mh[..]);
  show("message hash of \"Trust no one\"", mh[..]);
  ck: [32] u8; text: [88] u8;
  n := signed_message_sign(decode(SM_CORE_SIGN[0], ck[..]), true, SM_CORE_SIGN[1], w[..], text[..]);
  print("signmessage: %\n", cast(string) text[0..<n]);
  print("equals Core's message_sign: %\n", cast(string) text[0..<n] == SM_CORE_SIGN[2]);
  for i: 0..<SM_CORE_VERIFY.count {
    c := SM_CORE_VERIFY[i];
    print("verify % (Core: %): %\n", c.message, c.result, message_name(verify_message_vartime(c.address, c.sig, c.message, false)));
  }
  // The functional test, on a test network, from a WIF key (chapter 26).
  wif_work: [WIF_WORK_BYTES] u8; fk: [32] u8;
  kind := wif_decode_ct(SM_CORE_FUNCTIONAL[0], wif_work[..], fk[..]);
  print("WIF: testnet compressed %\n", kind == B58_TEST_WIF_COMPRESSED);
  n = signed_message_sign(fk[..], kind == B58_TEST_WIF_COMPRESSED, SM_CORE_FUNCTIONAL[1], w[..], text[..]);
  print("signmessagewithprivkey equals Core's: %\n", cast(string) text[0..<n] == SM_CORE_FUNCTIONAL[2]);
  print("verify with %: %\n", SM_CORE_FUNCTIONAL[3], message_name(verify_message_vartime(SM_CORE_FUNCTIONAL[3], SM_CORE_FUNCTIONAL[2], SM_CORE_FUNCTIONAL[1], true)));
  // Its P2SH-P2WPKH address, from HASH160(00 14 HASH160(key)).
  print("verify with 2MyK4SVW4pMoiYobfMnLfkF56u2UjcZ7YTi: %\n", message_name(verify_message_vartime("2MyK4SVW4pMoiYobfMnLfkF56u2UjcZ7YTi", SM_CORE_FUNCTIONAL[2], SM_CORE_FUNCTIONAL[1], true)));
  print("verify with invalid_addr: %\n", message_name(verify_message_vartime("invalid_addr", SM_CORE_FUNCTIONAL[2], SM_CORE_FUNCTIONAL[1], true)));
  print("verify invalid_sig: %\n", message_name(verify_message_vartime(SM_CORE_FUNCTIONAL[3], "invalid_sig", SM_CORE_FUNCTIONAL[1], true)));
  print("on mainnet: %\n", message_name(verify_message_vartime(SM_CORE_FUNCTIONAL[3], SM_CORE_FUNCTIONAL[2], SM_CORE_FUNCTIONAL[1], false)));
  // One header byte outside 27 to 34 at each end, and a short signature.
  print("header 26: %\n", message_name(verify_message_vartime(SM_T_ADDRESS, "GvojfrX2dfPnH26UegfbGQQLrdK844DlHq5157/P6h57WyuS/Qsl+h/WSVGDF4MUi4rWSswW38oimDYfNNUBUOk=", "Trust no one", false)));
  print("header 35: %\n", message_name(verify_message_vartime(SM_T_ADDRESS, "I/ojfrX2dfPnH26UegfbGQQLrdK844DlHq5157/P6h57WyuS/Qsl+h/WSVGDF4MUi4rWSswW38oimDYfNNUBUOk=", "Trust no one", false)));
  print("64 bytes: %\n", message_name(verify_message_vartime(SM_T_ADDRESS, "+iN+tfZ18+cfbpR6B9sZBAut0rzjgOUernXnv8/qHntbK5L9CyX6H9ZJUYMXgxSLitZKzBbfyiKYNh801QFQ6Q==", "Trust no one", false)));
}
```

Run it from the repository root. The signer's fixed-base tables are
computed at compile time and need more than the default compile-time
budget:

```shell
coax/bin/coax run bitcoin/examples/signed-messages.coax --run-steps 250000000
```

The output:

```text
toy curve y^2 = x^3 + 7 mod 43, n = 31, G = (2, 12)
d = 7, k = 10, z = 5: (r, s) = (11, 2), R = 10G = (42, 7), ID 3
x = r + n = 42, y odd, Q = (25, 18) = 7G: true
all keys and nonces: 870 recover their key, 406 with x(R) >= n
secp256k1, key 1, SHA-256("Satoshi Nakamoto")
ecdsa_sign_recoverable: true, ID 1
r || s: 934b1ea10a4b3c1757e2b0c017d0b6143ce3c9a7e6a4a49860d7a6ab210ee3d82442ce9d2b916064108014783e923ec36b49743e2ffa1c4496f01a512aafd9e5
R (ID 1: x = r, y odd): 03934b1ea10a4b3c1757e2b0c017d0b6143ce3c9a7e6a4a49860d7a6ab210ee3d8
u1 = -z/r: fd6baba07ef1b6f8beb07d943afa20404b4acf04fe1c76a29711b0bde5445636
u2 = s/r: 938432d275e07c34023c426ed09444d50b73edd6714dafbbd7e4fc6767c287e3
Q, ID 0: recovered: 0354d58835e07b996d6378eafa930ae9d2211e767381f1d52aedc67f48c8886f0b
Q, ID 1: recovered: 0279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798
Q, ID 2: r + n is at least p
Q, ID 3: r + n is at least p
(r, n - s) with ID 0 gives G too: true
tiny-secp256k1 ecdsa.json, every 20th row
101 of 101 give the published ID, 101 recover the signing key
refusals
Invalid r, s values (== 0): r or s not in [1, n-1]
Invalid r value (== 0): r or s not in [1, n-1]
Invalid s value (== 0): r or s not in [1, n-1]
Invalid r value (>= n): r or s not in [1, n-1]
Invalid s value (>= n): r or s not in [1, n-1]
Invalid r value for r (should be less than p - order): r + n is at least p
Invalid X coordinate value for r: no point has that x
Invalid hash length: bad length
signed by key 0, ID 0: the key is infinity
signed by key 0, ID 1: recovered
signed by key 0, ID 2: r + n is at least p
signed by key 0, ID 3: r + n is at least p
signed by key 0, ID 4: recovery ID out of range
(4, 4), ID 0: recovered
(4, 4), ID 1: recovered
(4, 4), ID 2: recovered
(4, 4), ID 3: recovered
(4, 4), ID 4: recovery ID out of range
(1, 1), ID 0: recovered
(1, 1), ID 1: recovered
(1, 1), ID 2: no point has that x
(1, 1), ID 3: no point has that x
(1, 1), ID 4: recovery ID out of range
Wycheproof tcId 346, ID 2: 0207310f90a9eae149a08402f54194a0f7b4ac427bf8d9bd6c7681071dc47dc362
signed messages
message hash of "Trust no one": aa8215d723ecd2f14867eeb7e19f192be7bc15a2352a24b991d4f5870cbaf6e8
signmessage: IPojfrX2dfPnH26UegfbGQQLrdK844DlHq5157/P6h57WyuS/Qsl+h/WSVGDF4MUi4rWSswW38oimDYfNNUBUOk=
equals Core's message_sign: true
verify message too (Core: ERR_INVALID_ADDRESS): ERR_INVALID_ADDRESS
verify message too (Core: ERR_ADDRESS_NO_KEY): ERR_ADDRESS_NO_KEY
verify message should be irrelevant (Core: ERR_MALFORMED_SIGNATURE): ERR_MALFORMED_SIGNATURE
verify message should be irrelevant (Core: ERR_PUBKEY_NOT_RECOVERED): bad header byte
verify I never signed this (Core: ERR_NOT_SIGNED): ERR_NOT_SIGNED
verify Trust no one (Core: OK): OK
verify Trust me (Core: OK): OK
WIF: testnet compressed true
signmessagewithprivkey equals Core's: true
verify with mpLQjfK79b7CCV4VMJWEWAj5Mpx8Up5zxB: OK
verify with 2MyK4SVW4pMoiYobfMnLfkF56u2UjcZ7YTi: ERR_ADDRESS_NO_KEY
verify with invalid_addr: ERR_INVALID_ADDRESS
verify invalid_sig: ERR_MALFORMED_SIGNATURE
on mainnet: ERR_INVALID_ADDRESS
header 26: bad header byte
header 35: bad header byte
64 bytes: bad length
```

The book's checks run this program in debug and release builds and compare
the output with
[bitcoin/fixtures/signed-messages.out](../fixtures/signed-messages.out).

## How the code is checked

### Checks in the example

The checks are in
[bitcoin/examples/checks/signed-messages.coax](../examples/checks/signed-messages.coax),
which loads the example:

- on the toy curve, for every key and nonce, the right ID recovers the key
  and the other three IDs don't, with `x(R) >= n` among the cases;
- the first 20 pinned rows in the interpreter: the published ID from the
  recoverable signer, and the row's key from recovery;
- the 8 pinned refusals, each with its own code;
- Bitcoin Core's `message_sign` and `message_verify` cases and the
  functional test's WIF key, signature and addresses.

<!-- source: bitcoin/examples/checks/signed-messages.coax check -->
```jai
// On the toy curve, for every key d and nonce k, the signature and its ID
// recover d·G, and every other ID recovers another point or none. Pairs
// where r = 0 or s = 0 are not signatures.
#forall "toy: the right ID recovers the key, the others don't" d: 1..<TOY_N #steps 400000000 {
  solved := 0; above := 0;
  for k: 1..<TOY_N {
    t := toy_sign(5, d, k); want := toy_times(d, TOY_G);
    if t.r == 0 || t.s == 0 { continue; }
    back := toy_recover(5, t);
    #assert back.x == want.x && back.y == want.y;
    for other: 0..<4 {
      if other == t.id { continue; }
      wrong := toy_recover(5, ToySig.{r=t.r,s=t.s,id=other});
      #assert wrong.x != want.x || wrong.y != want.y;
    }
    solved += 1;
    if t.id >= 2 { above += 1; }
  }
  #assert solved > 25 && above > 0;
}
// The first 20 pinned rows in the interpreter: the published ID from the
// recoverable signer, and the row's key from recovery. The example covers
// every 20th row; the native harness covers all 2,020.
#forall "tiny-secp256k1 rows: the ID and the key" i: 0..<20 #steps 4000000000 {
  c := ES_CASES[i]; kb: [32] u8; mb: [32] u8; sb: [64] u8; got: [65] u8; mine: [33] u8; theirs: [33] u8; w: [1] EcdsaWork;
  dk := decode(c.d, kb[..]); m := decode(c.m, mb[..]);
  #assert ecdsa_sign_recoverable(dk, m, w[..], got[..], false) && got[64] == ER_IDS[i];
  #assert bytes_same(got[0..<64], decode(c.sig, sb[..]));
  #assert ecdsa_recover_vartime(m, got[0..<64], cast(int) ER_IDS[i], SE_COMPRESSED, mine[..]) == SE_COMPRESSED;
  #assert bytes_same(mine[..], compressed_key(dk, theirs[..]));
}
// tiny-secp256k1's 8 recovery refusals, each with its own reason.
#test "the pinned refusals give their codes" #steps 600000000 {
  want := [8] int.[ER_BAD_SCALAR, ER_BAD_SCALAR, ER_BAD_SCALAR, ER_BAD_SCALAR, ER_BAD_SCALAR, ER_NO_X, ER_NO_POINT, ER_BAD_LENGTH];
  for i: 0..<8 {
    c := ER_REFUSE[i]; mb: [33] u8; sb: [64] u8; out: [33] u8; out[0] = 7;
    #assert ecdsa_recover_vartime(decode(c.m, mb[..]), decode(c.sig, sb[..]), c.id, SE_COMPRESSED, out[..]) == want[i] && out[0] == 7;
  }
}
// Every case of Core's message_verify, mapped to this verifier's codes.
#test "Bitcoin Core's message cases give Core's results" #steps 2000000000 {
  for i: 0..<SM_CORE_VERIFY.count {
    c := SM_CORE_VERIFY[i];
    got := verify_message_vartime(c.address, c.sig, c.message, false);
    // The all-zero signature: Core recovers nothing; this verifier stops at its header byte 0.
    expected := ifx got == SM_BAD_HEADER then "ERR_PUBKEY_NOT_RECOVERED" else message_name(got);
    #assert expected == c.result;
  }
  ck: [32] u8; w: [1] EcdsaWork; text: [88] u8;
  #assert signed_message_sign(decode(SM_CORE_SIGN[0], ck[..]), true, SM_CORE_SIGN[1], w[..], text[..]) == 88;
  #assert cast(string) text[..] == SM_CORE_SIGN[2];
}
#test "the functional test's key, signature and addresses" #steps 2000000000 {
  wif_work: [WIF_WORK_BYTES] u8; fk: [32] u8; w: [1] EcdsaWork; text: [88] u8;
  #assert wif_decode_ct(SM_CORE_FUNCTIONAL[0], wif_work[..], fk[..]) == B58_TEST_WIF_COMPRESSED;
  #assert signed_message_sign(fk[..], true, SM_CORE_FUNCTIONAL[1], w[..], text[..]) == 88;
  #assert cast(string) text[..] == SM_CORE_FUNCTIONAL[2];
  #assert verify_message_vartime(SM_CORE_FUNCTIONAL[3], SM_CORE_FUNCTIONAL[2], SM_CORE_FUNCTIONAL[1], true) == SM_OK;
  #assert verify_message_vartime("2MyK4SVW4pMoiYobfMnLfkF56u2UjcZ7YTi", SM_CORE_FUNCTIONAL[2], SM_CORE_FUNCTIONAL[1], true) == SM_ADDRESS_NO_KEY;
  #assert verify_message_vartime("invalid_addr", SM_CORE_FUNCTIONAL[2], SM_CORE_FUNCTIONAL[1], true) == SM_INVALID_ADDRESS;
  #assert verify_message_vartime(SM_CORE_FUNCTIONAL[3], "invalid_sig", SM_CORE_FUNCTIONAL[1], true) == SM_MALFORMED_SIGNATURE;
}
#test "every result has its own name" {
  #assert recover_name(ER_BAD_LENGTH) != recover_name(ER_BAD_ID) && recover_name(ER_NO_X) != recover_name(ER_NO_POINT);
  #assert recover_name(ER_INFINITY) != recover_name(ER_BAD_FORM) && recover_name(ER_BAD_SCALAR) != recover_name(SE_COMPRESSED);
  #assert recover_name(SE_COMPRESSED) == "recovered" && recover_name(ER_BAD_FORM) == "bad output form";
  #assert message_name(SM_OK) == "OK" && message_name(SM_CAPACITY) == "capacity" && message_name(SM_BAD_KEY) == "bad key";
  #assert message_name(SM_BAD_LENGTH) != message_name(SM_BAD_HEADER) && message_name(SM_NOT_SIGNED) != message_name(SM_NOT_RECOVERED);
}
```

```shell
coax/bin/coax test bitcoin/examples/checks/signed-messages.coax --run-steps 250000000
coax/bin/coax mutate bitcoin/examples/checks/signed-messages.coax --root-only --run-steps 250000000
```

On 2026-11-02, `coax test` ran 228 checks, including the loaded files'
own checks. The 20 pinned rows each sign and recover, about 100 million
interpreter steps a row, so they carry a budget of 4 billion together. The
checks file has no procedures of its own, so root-only mutation finds
nothing to mutate there; the example's own procedures were not mutated for
this chapter.

### The library's own checks

`ecdsa_recover.coax` checks libsecp256k1's edge cases with every ID,
Wycheproof's tcId 346, the `p - n` boundary byte by byte, every refusal
code, and chapter 20's signature with both halves of `s`.
`ecdsa_sign.coax` adds four recoverable signatures equal to libsecp256k1's,
one for each combination of parity and flip, and the ID's bits on a
synthetic workspace. `signmessage.coax` checks the message hash against
Python's `hashlib`, every compact-size boundary, Core's cases, both
networks, every header boundary and the recovered key's form.

```shell
coax/bin/coax test coax/lib/crypto/ecdsa_recover.coax --run-steps 250000000
coax/bin/coax mutate coax/lib/crypto/ecdsa_recover.coax --root-only --run-steps 250000000
coax/bin/coax test coax/lib/bitcoin/signmessage.coax --run-steps 250000000
coax/bin/coax mutate coax/lib/bitcoin/signmessage.coax --root-only --run-steps 250000000
coax/bin/coax mutate coax/lib/crypto/ecdsa_sign.coax --root-only --run-steps 250000000
```

On 2026-11-02, `coax test` passed all 90 checks of `ecdsa_recover.coax`,
all 220 of `signmessage.coax` and all 157 of `ecdsa_sign.coax`, each count
including the loaded files'. Root-only mutation killed all 107 mutants of
`ecdsa_recover.coax`, all 149 of `signmessage.coax` and all 283 of
`ecdsa_sign.coax`, with none marked equivalent. The first runs left
survivors, each fixed by a new check or by code with no equivalent
mutant: a byte comparison that returned `a[i] < b[i]` once the bytes
differed (so `<=` changed nothing), unreachable refusal branches after a
hash that can't fail, a 64-byte chunking loop whose boundary mutants
changed nothing, and the headers 27 and 31, which no check had accepted.

### libsecp256k1, Bitcoin Core's rules and Python, natively and in Wasm

[ecdsa_recover.py](../../coax/tests/bitcoin/ecdsa_recover.py) compiles
`signmessage.coax`, with the recovery and the signer, to C in checked and
release builds and runs:

- all 2,020 pinned rows: `ecdsa_sign_recoverable` equal to libsecp256k1's
  `secp256k1_ecdsa_sign_recoverable`, signature and ID, with the workspace
  cleared; and recovery with each ID from 0 to 4, compressed and
  uncompressed, against libsecp256k1's `secp256k1_ecdsa_recover` and a
  Python recovery that names its reason;
- the 8 pinned refusals and libsecp256k1's edge cases;
- 600 seeded signatures with small `r` (so IDs 2 and 3 recover), `r`
  around `p - n`, `r` and `s` at 0, 1, `n - 1` and `n`, and random values;
- Bitcoin Core's cases, then 60 seeded messages (lengths 0, 1, 252, 253,
  254, 65,535, 65,536 and 70,000 among them) under random keys, each
  signed equal to a Python `signmessage` over libsecp256k1 and verified
  against the right address, the other key form, the other network and a
  changed message; and all 256 header bytes on one signature, compared
  with Core's `MessageVerify` rules written in Python.

Then it builds the same C for wasm32 and runs the recoveries, the
recoverable signatures and the message cases again in Node. It needs
libsecp256k1 with its recovery module, which `build_oracle.py` now
enables:

```shell
python3 tools/coax-nostr/fuzz/build_oracle.py --fetch --reviewed   # once: libsecp256k1 v0.8.0
python3 coax/tests/bitcoin/ecdsa_recover.py
```

On 2026-11-02 its 6 tests passed in 25 minutes on a loaded machine,
against libsecp256k1 v0.8.0 built with its recovery module. Every random
recovery ID 0 to 3 recovered at least once. Core's masking accepted 31
header bytes outside 27 to 34 for one signature (0, 8, 16, 24, then every
multiple of 8 from 40 to 248), each refused here with `SM_BAD_HEADER`.
In Wasm (Node's WASI), 4,312 recoveries, 2,020 recoverable signatures, 507
verifications and 6 signed messages gave the native results, in checked
and release builds.

### Secret flow

Chapter 20's Memcheck probe,
[ecdsa_sign_secret_probe.sh](../../coax/tests/bitcoin/ecdsa_sign_secret_probe.sh),
now signs with both `ecdsa_sign` and `ecdsa_sign_recoverable`, with the
key marked undefined, and checks that the two signatures agree and that
the ID is 0 to 3:

```shell
coax/tests/bitcoin/ecdsa_sign_secret_probe.sh --host HOST
PLANT=1 coax/tests/bitcoin/ecdsa_sign_secret_probe.sh --host HOST   # must FAIL
```

The first run on 2026-11-02 failed, as it should have: it reported a
second site, the range check of a narrowing cast. `ec_recid` first
returned `cast(u8)` of the ID, and a checked cast compares the value with
255, a branch on a secret. It now uses `cast,trunc(u8)`, which has no
check. Then, on the same NixOS host as chapter 20 (Valgrind 3.26.0, GCC
15.2.0), Memcheck reported one site, the `ec_declassify` branch in
`ec_sign`, and the control branch on the key was flagged. With `PLANT=1`
the planted branch was reported too, and the probe failed, as it must.

Recovery handles no secret: its names end in `_vartime`, and it isn't
probed.

## Summary

- A signature, its hash and a 2-bit recovery ID give the one public key
  it verifies under: `Q = r^-1 · (s·R - z·G)`, with `R` from `r`, the
  parity bit and the `x(R) >= n` bit.
- `x(R) = r + n` is possible only when `r < p - n`, about `2^128`; honest
  signatures almost never need it, but a verifier must handle it.
- The signer returns the ID from values it already has, with masks; there
  is still one signer body and one revealed fact.
- Bitcoin Core's signed messages hash the magic text and the message with
  compact-size prefixes, sign recoverably, and put `27 + id + 4·compressed`
  in front. Verification compares the recovered key's HASH160 with a
  P2PKH address.
- This verifier refuses header bytes outside 27 to 34, which Core masks
  instead; otherwise every pinned case gives Core's result.

## Dependencies

- [19. Verify ECDSA](ch019-verify-ecdsa.md)
- [20. Sign with ECDSA](ch020-sign-with-ecdsa.md)
- [17. Point encodings: compressed, uncompressed and x-only](ch017-point-encodings-compressed-uncompressed-and-x-only.md)
- [26. Base58Check](ch026-base58check.md)
- [6. RIPEMD-160 and HASH160](ch006-ripemd-160-and-hash160.md)
- [1. Bytes, hex and base64](ch001-bytes-hex-and-base64.md)

## Sources

- [SEC 1 v2](https://www.secg.org/sec1-v2.pdf), section 4.1.6 (public key
  recovery).
- Bitcoin Core's
  [`src/common/signmessage.cpp`](https://github.com/bitcoin/bitcoin/blob/ddea9196493d3b8fd841a36a6c53efd85dac7a05/src/common/signmessage.cpp)
  and `CPubKey::RecoverCompact` in `src/pubkey.cpp`; its
  `src/test/util_tests.cpp` and
  `test/functional/rpc_signmessagewithprivkey.py`, pinned at revision
  `ddea9196` in
  [coax/tests/vectors/upstream/bitcoin-core/](../../coax/tests/vectors/upstream/bitcoin-core/util_tests.cpp),
  listed in [SOURCES.md](../../coax/tests/vectors/SOURCES.md).
- libsecp256k1's recovery module, release `v0.8.0`: its tests,
  `src/modules/recovery/tests_impl.h`, pinned in
  [coax/tests/vectors/upstream/secp256k1/](../../coax/tests/vectors/upstream/secp256k1/recovery/tests_impl.h);
  the library itself, pinned in
  [UPSTREAM.json](../../tools/coax-nostr/fuzz/UPSTREAM.json), in tests only.
- tiny-secp256k1's `tests/fixtures/ecdsa.json` (MIT), pinned at revision
  `e1fd7a77` in
  [coax/tests/vectors/upstream/tiny-secp256k1/](../../coax/tests/vectors/upstream/tiny-secp256k1/ecdsa.json).
- Wycheproof `ecdsa_secp256k1_sha256_bitcoin_test.json`, tcId 346
  (chapter 19's pin).
- [BIP-137](https://github.com/bitcoin/bips/blob/master/bip-0137.mediawiki)
  (signed messages for segwit addresses) and
  [BIP-322](https://github.com/bitcoin/bips/blob/master/bip-0322.mediawiki)
  (generic signed messages), both out of scope.
