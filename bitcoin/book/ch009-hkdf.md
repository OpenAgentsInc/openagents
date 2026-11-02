# 9. HKDF

A shared secret is not a key yet. An ECDH secret (Nostr chapter 14) is 32 bytes
of a curve point's x-coordinate, and its bits are not uniformly random. You
also need more than one key from it. HKDF, the HMAC-based key derivation
function of RFC 5869, solves both problems in two steps:

- **Extract** turns input keying material into one uniformly random
  32-byte pseudorandom key (PRK).
- **Expand** stretches a PRK into as many output bytes as you ask for, up
  to 8,160, bound to a context string called `info`.

BIP-324 derives the v2 transport's keys with HKDF-SHA256: extract with the
salt `bitcoin_v2_shared_secret` followed by the network's magic bytes, then
one expand per key (chapter 67). BOLT 8's Noise handshake calls HKDF with the
chaining key as salt and an empty `info`, taking 64 bytes (chapter 105).
NIP-44 v2 uses both halves too. Extract makes the conversation key between
two people, and expand makes three keys for every message (Nostr chapter 20). This
chapter builds HKDF from the HMAC of chapter 8, works out the first output
blocks of RFC 5869 test case 1 by hand, and checks the NIP-44 split against
a published vector.

## What this chapter teaches

- Extract: `PRK = HMAC(salt, IKM)`.
- Expand: `T(i) = HMAC(PRK, T(i-1) || info || i)`, with `T(0)` empty, at
  most 255 blocks.
- Why an absent salt and a salt of 32 zero bytes give the same PRK.
- Why expand output is limited to `255 * 32` bytes, and how a request for
  one more byte is refused.
- NIP-44 v2: `conversation_key = HKDF-extract(salt = "nip44-v2", IKM =
  shared_x)`, and `HKDF-expand(conversation_key, nonce, 76)` split into
  32 + 12 + 32 bytes.

## Where the code is

[coax/lib/crypto/hkdf.coax](../../coax/lib/crypto/hkdf.coax) has
`hkdf_extract_checked` and `hkdf_expand_checked`, plus the preflight
`hkdf_expand_request` and the block count `hkdf_blocks`. They return the
SHA-256 status codes from chapter 4: `SHA_OK`, `SHA_BAD_INPUT`,
`SHA_BAD_OUTPUT`, `SHA_ALIAS` and `SHA_LENGTH`.
[coax/lib/crypto/hkdf-memory.coax](../../coax/lib/crypto/hkdf-memory.coax)
has the C-exported `hkdf_extract` and `hkdf_expand`, which compute the
overlap fact from the views' addresses.

## Extract

Extract is one HMAC call with the salt as the key and the input keying
material (IKM) as the message:

```jai
PRK = HMAC-SHA256(salt, IKM)
```

HMAC mixes every bit of the IKM into a 32-byte tag, so the PRK is
uniformly random whenever the IKM has enough entropy. The salt does not
need to be secret. NIP-44 uses the 8-byte ASCII salt `nip44-v2`, the bytes
`6e 69 70 34 34 2d 76 32`, so its keys never collide with another
protocol's HKDF keys from the same secret.

RFC 5869 says that a missing salt means 32 zero bytes. You don't need a
special case for that. HMAC pads a short key with zeros to 64 bytes
(chapter 8), so an empty key and a 32-byte zero key make the same padded
block and the same PRK. `hkdf_extract_checked` passes the salt to HMAC as
it is.

## Expand

Expand makes 32-byte blocks and joins them:

1. `T(1) = HMAC(PRK, info || 0x01)`.
2. `T(2) = HMAC(PRK, T(1) || info || 0x02)`.
3. Each later block hashes the block before it, `info` and the next
   counter byte.
4. The output is the first `L` bytes of `T(1) || T(2) || ...`.

Two facts follow from that construction:

- A request for `L` bytes needs `ceil(L / 32)` blocks. `hkdf_blocks`
  computes `(L + 31) / 32`.
- The counter is one byte and starts at 1, so there are at most 255
  blocks. The longest output is `255 * 32 = 8160` bytes
  (`HKDF_MAX_OUTPUT`). A request for 8,161 bytes returns `SHA_LENGTH`.

A shorter output is always a prefix of a longer one with the same PRK and
`info`. That is why NIP-44 must put a fresh nonce in `info` for each
message: the nonce, not the length, makes the keys different.

### Worked example: the blocks of RFC 5869 test case 1

Test case 1 asks for `L = 42` bytes with a 10-byte `info`,
`f0 f1 ... f9`.

1. Count the blocks: `(42 + 31) / 32 = 73 / 32 = 2`. Two blocks give 64
   bytes, and you keep 42.
2. The message for `T(1)` is `info || 01`: 10 + 1 = 11 bytes.
3. The message for `T(2)` is `T(1) || info || 02`: 32 + 10 + 1 = 43 bytes.
4. The output is all 32 bytes of `T(1)` and the first 10 bytes of
   `T(2)`: 32 + 10 = 42.

The RFC gives the output, which starts with `T(1)` and ends with the first
10 bytes of `T(2)`:

```text
3cb25f25faacd57a90434f64d0362f2a2d2d0a90cf1a5a4c5db02d56ecc4c5bf34007208d5b887185865
```

### Worked example: the NIP-44 message keys

NIP-44 expands the 32-byte conversation key with the message's 32-byte
nonce as `info` to `L = 76` bytes:

1. Count the blocks: `(76 + 31) / 32 = 107 / 32 = 3`. Three blocks give
   96 bytes, and you keep 76.
2. Split the 76 bytes in order:
   - bytes 0–31: the ChaCha20 key, 32 bytes;
   - bytes 32–43: the ChaCha20 nonce, 12 bytes;
   - bytes 44–75: the HMAC key, 32 bytes.
3. Check the sum: 32 + 12 + 32 = 76.

## Bounds and refusals

`hkdf_expand_checked` calls `hkdf_expand_request` before it writes
anything. The request is refused when:

- the PRK is shorter than 32 bytes (`SHA_BAD_INPUT`);
- `info` is longer than 1,024 bytes, a local bound (`SHA_BAD_INPUT`);
- the length is negative or over 8,160 bytes (`SHA_LENGTH`);
- the output view is shorter than the length (`SHA_BAD_OUTPUT`);
- the output overlaps the PRK or `info` (`SHA_ALIAS`).

Expand computes every block into a fixed 8,160-byte staging buffer and
copies only the requested prefix to your output at the end. A refused call
leaves your output unchanged, and the bytes after the requested prefix are
never written.

The PRK, the staged blocks and the output are secret. Erase them when you
are done (chapter 3).

## The example

The example is [bitcoin/examples/hkdf.coax](../examples/hkdf.coax). It counts
blocks, runs RFC 5869 test case 1 and builds its two expand blocks by hand,
shows that an empty salt equals 32 zero bytes, expands the NIP-44
conversation key from the official vector into its three keys, and shows
five refusals.

<!-- source: bitcoin/examples/hkdf.coax example -->
```jai
#load "../../coax/lib/crypto/hkdf.coax";
#load "../../coax/lib/nostr/hex.coax";
#import "Basic";

// Print a label and bytes as lowercase hex.
show :: (label: string, bytes: [] u8) {
  text: [192] u8;
  r := hex_encode_checked(cast(string) bytes, text[..], false);
  print("%: %\n", label, cast(string) text[0..<r.written]);
}
// Every call below this point must succeed; say so if one doesn't.
must :: (status: u32) { if status != SHA_OK { print("unexpected status %\n", status); } }
// Decode lowercase hex that the program itself supplies.
from_hex :: (text: string, out: [] u8) -> [] u8 {
  r := hex_decode_checked(text, out, false);
  if r.error != 0 { print("bad hex\n"); }
  return out[0..<r.written];
}
// One expand block by hand: T(i) = HMAC(PRK, T(i-1) || info || i).
// `previous` is empty for the first block. Returns the HMAC status.
block_by_hand :: (prk: [] u8, previous: [] u8, info: [] u8, counter: int, out: [] u8) -> u32 {
  message: [80] u8;
  n := 0;
  for i: 0..<previous.count { message[n] = previous[i]; n += 1; }
  for i: 0..<info.count { message[n] = info[i]; n += 1; }
  message[n] = cast(u8) counter; n += 1;
  return hmac_sha256_checked(prk, message[0..<n], out, false);
}

STATUS :: [8] string.["ok", "bad state", "bad input", "bad output",
  "already final", "overlap", "too long", "bad tag"];

main :: () {
  // Expand makes 32-byte blocks and keeps a prefix of the last one.
  print("blocks for 42, 76, 82 and 8160 bytes: % % % %\n",
    hkdf_blocks(42), hkdf_blocks(76), hkdf_blocks(82), hkdf_blocks(8160));

  // RFC 5869 test case 1. Extract: PRK = HMAC(salt, IKM).
  sb: [13] u8; ib: [22] u8; fb: [10] u8;
  salt := from_hex("000102030405060708090a0b0c", sb[..]);
  ikm := from_hex("0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b0b", ib[..]);
  info := from_hex("f0f1f2f3f4f5f6f7f8f9", fb[..]);
  prk: [32] u8;
  print("extract: %\n", STATUS[hkdf_extract_checked(salt, ikm, prk[..], false)]);
  show("PRK", prk[..]);

  // Expand by hand: two blocks, then keep the first 42 bytes.
  t1: [32] u8; t2: [32] u8;
  must(block_by_hand(prk[..], t1[0..<0], info, 1, t1[..]));
  must(block_by_hand(prk[..], t1[..], info, 2, t2[..]));
  show("T(1) = HMAC(PRK, info || 01)", t1[..]);
  show("T(2) = HMAC(PRK, T(1) || info || 02)", t2[..]);
  by_hand: [42] u8;
  for i: 0..<32 { by_hand[i] = t1[i]; }
  for i: 0..<10 { by_hand[32 + i] = t2[i]; }

  // The library computes the same 42 bytes in one call.
  okm: [42] u8;
  print("expand: %\n", STATUS[hkdf_expand_checked(prk[..], info, okm[..], 42, false)]);
  show("OKM", okm[..]);
  print("same as by hand: %\n", cb_ct_equal(okm[..], by_hand[..]) != 0);

  // RFC 5869 test case 3 has no salt. An empty HMAC key is padded with
  // zeros, so it equals the RFC's 32 zero bytes.
  no_salt: [32] u8; zero_salt: [32] u8; zeros: [32] u8;
  must(hkdf_extract_checked(zeros[0..<0], ikm, no_salt[..], false));
  must(hkdf_extract_checked(zeros[..], ikm, zero_salt[..], false));
  show("case 3 PRK, empty salt", no_salt[..]);
  print("empty salt equals 32 zero bytes: %\n", cb_ct_equal(no_salt[..], zero_salt[..]) != 0);

  // NIP-44 v2: the conversation key is a PRK, made by extract with the
  // salt "nip44-v2". Each message expands it with its 32-byte nonce as
  // info to 76 bytes: ChaCha20 key, ChaCha20 nonce, HMAC key.
  nip44_salt := [8] u8.[110, 105, 112, 52, 52, 45, 118, 50];
  show("salt \"nip44-v2\"", nip44_salt[..]);
  cb: [32] u8; nb: [32] u8;
  conversation_key := from_hex("a1a3d60f3470a8612633924e91febf96dc5366ce130f658b1f0fc652c20b3b54", cb[..]);
  nonce := from_hex("e1e6f880560d6d149ed83dcc7e5861ee62a5ee051f7fde9975fe5d25d2a02d72", nb[..]);
  keys: [76] u8;
  print("expand to 76 bytes: %\n", STATUS[hkdf_expand_checked(conversation_key, nonce, keys[..], 76, false)]);
  show("chacha_key, bytes 0-31", keys[0..<32]);
  show("chacha_nonce, bytes 32-43", keys[32..<44]);
  show("hmac_key, bytes 44-75", keys[44..<76]);
  vb: [76] u8;
  vector := from_hex("f145f3bed47cb70dbeaac07f3a3fe683e822b3715edb7c4fe310829014ce7d76c4ad129bb01180c0933a160c027c1db445f05e2eee864a0975b0ddef5b7110583c8c192de3732571ca5838c4", vb[..]);
  print("matches the NIP-44 vector: %\n", cb_ct_equal(keys[..], vector) != 0);

  // Refusals. Each returns a status and leaves the output unchanged.
  out: [8161] u8;
  for i: 0..<8161 { out[i] = 0xa5; }
  print("8161 bytes (255 * 32 + 1): %\n", STATUS[hkdf_expand_checked(prk[..], info, out[..], 8161, false)]);
  print("31-byte PRK: %\n", STATUS[hkdf_expand_checked(prk[0..<31], info, out[..], 42, false)]);
  print("42 bytes into 41: %\n", STATUS[hkdf_expand_checked(prk[..], info, out[0..<41], 42, false)]);
  print("1025-byte info: %\n", STATUS[hkdf_expand_checked(prk[..], out[0..<1025], out[1025..<1067], 42, false)]);
  print("output overlaps the PRK: %\n", STATUS[hkdf_expand_checked(prk[..], info, out[..], 42, true)]);
  untouched := true;
  for i: 0..<8161 { if out[i] != 0xa5 { untouched = false; } }
  print("output unchanged: %\n", untouched);
}
```

Run it from the repository root:

```shell
coax/bin/coax run bitcoin/examples/hkdf.coax
```

The output matches the block count worked out by hand, the PRK and output
of RFC 5869 test cases 1 and 3, and the NIP-44 message keys:

```text
blocks for 42, 76, 82 and 8160 bytes: 2 3 3 255
extract: ok
PRK: 077709362c2e32df0ddc3f0dc47bba6390b6c73bb50f9c3122ec844ad7c2b3e5
T(1) = HMAC(PRK, info || 01): 3cb25f25faacd57a90434f64d0362f2a2d2d0a90cf1a5a4c5db02d56ecc4c5bf
T(2) = HMAC(PRK, T(1) || info || 02): 34007208d5b887185865b4b0a85a993b89b9b65683d60f0106d28fff039d0b6f
expand: ok
OKM: 3cb25f25faacd57a90434f64d0362f2a2d2d0a90cf1a5a4c5db02d56ecc4c5bf34007208d5b887185865
same as by hand: true
case 3 PRK, empty salt: 19ef24a32c717b167f33a91d6f648bdf96596776afdb6377ac434c1c293ccb04
empty salt equals 32 zero bytes: true
salt "nip44-v2": 6e697034342d7632
expand to 76 bytes: ok
chacha_key, bytes 0-31: f145f3bed47cb70dbeaac07f3a3fe683e822b3715edb7c4fe310829014ce7d76
chacha_nonce, bytes 32-43: c4ad129bb01180c0933a160c
hmac_key, bytes 44-75: 027c1db445f05e2eee864a0975b0ddef5b7110583c8c192de3732571ca5838c4
matches the NIP-44 vector: true
8161 bytes (255 * 32 + 1): too long
31-byte PRK: bad input
42 bytes into 41: bad output
1025-byte info: bad input
output overlaps the PRK: overlap
output unchanged: true
```

The 42-byte output is `T(1)` followed by the first 10 bytes of `T(2)`, as
the worked example says. The three NIP-44 keys equal the `chacha_key`,
`chacha_nonce` and `hmac_key` of the first `get_message_keys` vector in
[nip44.vectors.json](../../coax/tests/vectors/upstream/nip44/nip44.vectors.json).

The five refusals are an 8,161-byte request, a 31-byte PRK, a 42-byte
request into a 41-byte output, a 1,025-byte `info` and an output that
overlaps the PRK. Each returns its status before it writes anything, so
the output still holds its `0xa5` fill.

The book's checks run this program in debug and release builds and compare
the output with [bitcoin/fixtures/hkdf.out](../fixtures/hkdf.out).

## How the code is checked

### Published vectors

- [coax/tests/nostr/hkdf_rfc.coax](../../coax/tests/nostr/hkdf_rfc.coax)
  checks RFC 5869 SHA-256 test cases 1–3: the PRK, the output, and that the
  bytes after the requested length are unchanged.
- The example checks the NIP-44 v2 message keys against the official
  vector file.

### A reference implementation

[coax/tests/nostr/hmac_hkdf.py](../../coax/tests/nostr/hmac_hkdf.py)
compiles HKDF to C and to wasm32 and compares it with an HKDF built on
Python's `hmac` module: RFC 5869 cases 1–3, random PRKs and `info` at
twelve lengths from 0 to 8,160 bytes, and every refusal leaving the output
unchanged:

```shell
python3 coax/tests/nostr/hmac_hkdf.py
```

### Checks in Coax

- `hkdf.coax` checks every preflight bound on both sides (31 and 32 PRK
  bytes, 1,024 and 1,025 `info` bytes, 8,160 and 8,161 output bytes), the
  block count at 0, 1, 31, 32, 33 and 8,160 bytes, and that a zero-length
  request writes nothing.
- The example checks the library against the block-by-block construction
  for lengths on both sides of 32 and 64 bytes, and that a shorter output
  is a prefix of a longer one:

<!-- source: bitcoin/examples/hkdf.coax check -->
```jai
// The library's expand equals the block-by-block construction for lengths
// around the block size, and a shorter output is a prefix of a longer one.
#forall "HKDF expand equals T(1) || T(2) || ... cut to the length" which: 0..<10 {
  lengths := [10] int.[0, 1, 31, 32, 33, 63, 64, 65, 76, 96];
  n := lengths[which];
  prk: [32] u8; info: [5] u8;
  for i: 0..<32 { prk[i] = cast(u8) ((i * 29 + 3) % 256); }
  for i: 0..<5 { info[i] = cast(u8) (100 + i); }
  expected: [96] u8; t: [32] u8;
  for block: 0..<hkdf_blocks(n) {
    if block == 0 { #assert block_by_hand(prk[..], t[0..<0], info[..], 1, t[..]) == SHA_OK; }
    else { previous := t; #assert block_by_hand(prk[..], previous[..], info[..], block + 1, t[..]) == SHA_OK; }
    for i: 0..<32 { expected[block * 32 + i] = t[i]; }
  }
  out: [97] u8; longest: [96] u8;
  for i: 0..<97 { out[i] = 0x5a; }
  #assert hkdf_expand_checked(prk[..], info[..], out[..], n, false) == SHA_OK;
  #assert hkdf_expand_checked(prk[..], info[..], longest[..], 96, false) == SHA_OK;
  for i: 0..<n { #assert out[i] == expected[i] && out[i] == longest[i]; }
  for i: n..<97 { #assert out[i] == 0x5a; }
}
```

To run all the checks:

```shell
coax/bin/coax test bitcoin/examples/hkdf.coax
coax/bin/coax mutate coax/lib/crypto/hkdf.coax
```

`coax mutate` leaves zero survivors: 882 mutants in `hkdf.coax` with the
files it loads, and 991 in the example with the files it loads. `coax test`
on the example runs 61 checks.

## Summary

- Extract is `HMAC(salt, IKM)`. An empty salt equals 32 zero bytes.
- Expand chains HMAC blocks, each over the previous block, `info` and a
  counter byte, and keeps a prefix.
- The one-byte counter limits output to 255 blocks, 8,160 bytes.
- NIP-44 v2 extracts with the salt `nip44-v2` and expands with the nonce
  to 76 bytes: a 32-byte ChaCha20 key, a 12-byte nonce and a 32-byte HMAC
  key.

## Dependencies

- [8. HMAC-SHA256](ch008-hmac-sha256.md)

## Sources

- [RFC 5869](https://www.rfc-editor.org/rfc/rfc5869), sections 2.2, 2.3
  and appendix A, pinned in
  [coax/tests/vectors/SOURCES.md](../../coax/tests/vectors/SOURCES.md).
- [NIP-44](../../nostr/nips/official/44.md), which derives its conversation key and
  message keys with HKDF.
