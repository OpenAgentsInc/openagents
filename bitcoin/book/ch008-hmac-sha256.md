# 8. HMAC-SHA256

A hash proves nothing about who made a message: anyone can hash anything.
HMAC adds a key. Only someone who has the key can compute the tag, so a
correct tag shows that the message came from a key holder and wasn't
changed. RFC 6979 ECDSA nonces (chapter 20) and BIP-32 keys (chapter 32,
with SHA-512) are built on HMAC. Nostr uses HMAC-SHA256 in two places:

- NIP-44 authenticates every encrypted payload with an HMAC-SHA256 tag
  (Nostr chapter 20).
- HKDF derives keys with HMAC-SHA256 (chapter 9).

This chapter builds HMAC from two SHA-256 hashes, works out its padded keys
by hand, and checks it against the RFC 4231 test cases.

## What this chapter teaches

- The construction: `SHA256((K ^ opad) || SHA256((K ^ ipad) || m))`.
- How a key is made exactly 64 bytes: short keys are padded with zeros, and
  keys over 64 bytes are hashed first.
- How a streaming HMAC keeps two SHA-256 states.
- Why you compare a received tag with a comparison that reads every byte.
- The truncated tag in RFC 4231 test case 5.

## Where the code is

[coax/lib/crypto/hmac.coax](../../coax/lib/crypto/hmac.coax) has the
streaming calls `hmac_sha256_init_checked`, `hmac_sha256_update_checked`
and `hmac_sha256_final_checked`, and the one-shot `hmac_sha256_checked`. They
return the same status codes as SHA-256 in chapter 4, such as `SHA_OK`,
`SHA_BAD_OUTPUT` and `SHA_PHASE`.
[coax/lib/crypto/hmac-memory.coax](../../coax/lib/crypto/hmac-memory.coax)
has wrappers that compute the overlap fact from the views' addresses.

## The construction

HMAC works on the block size of its hash, 64 bytes for SHA-256:

1. Make the key 64 bytes, `K`:
   - A key of 64 bytes or fewer is copied and padded with zero bytes.
   - A longer key is replaced by its 32-byte SHA-256, then padded.
2. XOR every byte of `K` with `0x36` to get the inner pad, and with `0x5c`
   to get the outer pad.
3. The inner hash is `SHA256(inner pad || message)`.
4. The tag is `SHA256(outer pad || inner hash)`.

The inner and outer pads differ in every byte, so the two hashes never
start from the same block, even with the same key.

### Worked example: the pads for `Jefe`

RFC 4231 test case 2 uses the key `Jefe`, the bytes `4a 65 66 65`.

1. The key is 4 bytes, so `K` is `4a 65 66 65` followed by 60 zero bytes.
2. The inner pad XORs each byte with `0x36`:
   - `0x4a ^ 0x36`: `0100 1010 ^ 0011 0110` is `0111 1100`, or `0x7c`.
   - `0x65 ^ 0x36` is `0x53`, and `0x66 ^ 0x36` is `0x50`.
   - Each zero byte becomes `0x36`.

   So the inner pad starts `7c 53 50 53 36 36`.
3. The outer pad XORs each byte with `0x5c`:
   - `0x4a ^ 0x5c`: `0100 1010 ^ 0101 1100` is `0001 0110`, or `0x16`.
   - `0x65 ^ 0x5c` is `0x39`, and `0x66 ^ 0x5c` is `0x3a`.
   - Each zero byte becomes `0x5c`.

   So the outer pad starts `16 39 3a 39 5c 5c`.

The message is `what do ya want for nothing?`, 28 bytes. The inner hash
covers 64 + 28 = 92 bytes, and the outer hash covers 64 + 32 = 96 bytes. RFC
4231 gives the tag:

```text
5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843
```

## Streaming HMAC

An HMAC state is 224 bytes: an inner SHA-256 state (bytes 0–111) and an
outer one (bytes 112–223).

1. `hmac_sha256_init_checked(state, key, overlap)` builds both pads,
   starts both SHA-256 states and feeds one pad into each.
2. `hmac_sha256_update_checked(state, input, overlap)` feeds the input into
   the inner state only.
3. `hmac_sha256_final_checked(state, out, overlap)` finalizes the inner
   hash, feeds it into the outer state, finalizes that and writes 32 bytes.
   Calling it again returns the same tag.

Each call checks that the two halves are a matching pair: both active or
both final, the inner state has hashed at least its 64-byte pad, and the
outer one has hashed exactly 64 bytes (active) or 96 bytes (final). A plain
SHA-256 state passed as an HMAC state is refused.

After initialization, the state holds the key's pads, so it is secret. Erase
it when you are done (chapter 3). Initializing it again overwrites it; that
isn't an erasure.

## Comparing tags

When you receive a message and a tag, you compute the tag yourself and
compare. Compare with `cb_ct_equal` from chapter 3, which reads every byte
and returns a mask. A comparison that stops at the first difference tells an
attacker how many leading bytes of a forged tag were right, and they can
find a valid tag one byte at a time.

NIP-44 checks the tag before it decrypts anything (Nostr chapter 20). A payload
with a wrong tag is refused without running ChaCha20 on it.

## Truncated tags

RFC 4231 test case 5 publishes only the first 16 bytes of its tag. A
protocol can send fewer bytes than the full 32, and the receiver compares
only those. HMAC still computes all 32 bytes; truncation is the caller's
choice. NIP-44 uses the full 32 bytes.

## The example

The example is [bitcoin/examples/hmac-sha256.coax](../examples/hmac-sha256.coax).
It builds the `Jefe` tag by hand from two SHA-256 hashes, computes it with
the library, streams it in two updates, runs RFC 4231 test cases 5 and 6,
refuses a tag with one flipped bit, and shows four refusals.

<!-- source: bitcoin/examples/hmac-sha256.coax example -->
```jai
#load "../../coax/lib/crypto/hmac.coax";
#load "../../coax/lib/nostr/hex.coax";
#import "Basic";

// Print a label and bytes as lowercase hex.
show :: (label: string, bytes: [] u8) {
  text: [128] u8;
  r := hex_encode_checked(cast(string) bytes, text[..], false);
  print("%: %\n", label, cast(string) text[0..<r.written]);
}
// Every call below this point must succeed; say so if one doesn't.
must :: (status: u32) { if status != SHA_OK { print("unexpected status %\n", status); } }
// Copy a string's bytes into a buffer and return the filled part.
bytes_of :: (text: string, out: [] u8) -> [] u8 {
  for i: 0..<text.count { out[i] = text[i]; }
  return out[0..<text.count];
}

STATUS :: [8] string.["ok", "bad state", "bad input", "bad output",
  "already final", "overlap", "too long", "bad tag"];

main :: () {
  // RFC 4231 test case 2: the key "Jefe" and a 28-byte message.
  kb: [4] u8; mb: [28] u8;
  key := bytes_of("Jefe", kb[..]);
  message := bytes_of("what do ya want for nothing?", mb[..]);

  // HMAC by hand, from SHA-256: pad the key to 64 bytes, then hash twice.
  block: [64] u8;
  for i: 0..<key.count { block[i] = key[i]; }
  inner_pad: [64] u8; outer_pad: [64] u8;
  for i: 0..<64 { inner_pad[i] = block[i] ^ 0x36; outer_pad[i] = block[i] ^ 0x5c; }
  show("key ^ ipad, first 6 bytes", inner_pad[0..<6]);
  show("key ^ opad, first 6 bytes", outer_pad[0..<6]);
  state: [112] u8; inner: [32] u8; by_hand: [32] u8;
  must(sha256_init(state[..]));
  must(sha256_update_checked(state[..], inner_pad[..], false));
  must(sha256_update_checked(state[..], message, false));
  must(sha256_final_checked(state[..], inner[..], false));
  show("inner hash", inner[..]);
  must(sha256_init(state[..]));
  must(sha256_update_checked(state[..], outer_pad[..], false));
  must(sha256_update_checked(state[..], inner[..], false));
  must(sha256_final_checked(state[..], by_hand[..], false));
  show("outer hash", by_hand[..]);

  // The library computes the same tag in one call.
  tag: [32] u8;
  print("hmac: %\n", STATUS[hmac_sha256_checked(key, message, tag[..], false)]);
  show("HMAC-SHA256(\"Jefe\", message)", tag[..]);
  print("same as by hand: %\n", cb_ct_equal(tag[..], by_hand[..]) != 0);

  // The same tag again, streamed in two updates over a 224-byte state.
  hs: [224] u8; streamed: [32] u8;
  must(hmac_sha256_init_checked(hs[..], key, false));
  must(hmac_sha256_update_checked(hs[..], message[0..<10], false));
  must(hmac_sha256_update_checked(hs[..], message[10..<28], false));
  must(hmac_sha256_final_checked(hs[..], streamed[..], false));
  print("streamed in two updates, same tag: %\n", cb_ct_equal(tag[..], streamed[..]) != 0);

  // RFC 4231 test case 6: a 131-byte key is hashed to 32 bytes first.
  long_key: [131] u8;
  for i: 0..<131 { long_key[i] = 0xaa; }
  hashed: [32] u8;
  must(sha256_hash_checked(long_key[..], hashed[..], false));
  show("SHA-256 of the 131-byte key", hashed[..]);
  lb: [54] u8;
  long_message := bytes_of("Test Using Larger Than Block-Size Key - Hash Key First", lb[..]);
  must(hmac_sha256_checked(long_key[..], long_message, tag[..], false));
  show("HMAC-SHA256, case 6", tag[..]);

  // RFC 4231 test case 5 publishes only the first 16 bytes of the tag.
  short_key: [20] u8;
  for i: 0..<20 { short_key[i] = 0x0c; }
  tb: [20] u8;
  must(hmac_sha256_checked(short_key[..], bytes_of("Test With Truncation", tb[..]), tag[..], false));
  show("HMAC-SHA256, case 5, first 16 bytes", tag[0..<16]);

  // A tag that differs in one bit is refused, with a comparison that reads
  // every byte (chapter 3).
  must(hmac_sha256_checked(key, message, tag[..], false));
  forged := tag;
  forged[31] ^= 1;
  print("received tag, last bit flipped: accepted %\n", cb_ct_equal(tag[..], forged[..]) != 0);

  // Refusals. Each one returns a status and leaves the state and output
  // unchanged.
  print("update after final: %\n", STATUS[hmac_sha256_update_checked(hs[..], message, false)]);
  small: [31] u8;
  print("31-byte output: %\n", STATUS[hmac_sha256_checked(key, message, small[..], false)]);
  print("1 MiB + 1 byte key: %\n", STATUS[hmac_sha256_request(1048577, 0, 32, false)]);
  print("a SHA-256 state as an HMAC state: %\n", STATUS[hmac_sha256_info(state[..]).error]);
}
```

Run it from the repository root:

```shell
coax/bin/coax run bitcoin/examples/hmac-sha256.coax
```

The output matches the pads worked out by hand and the RFC 4231 tags for
test cases 2, 5 and 6:

```text
key ^ ipad, first 6 bytes: 7c5350533636
key ^ opad, first 6 bytes: 16393a395c5c
inner hash: a2e485863d27f9d864ac8d802432a1ed477d8c4c6f349d16d4e7e917c629cad7
outer hash: 5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843
hmac: ok
HMAC-SHA256("Jefe", message): 5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843
same as by hand: true
streamed in two updates, same tag: true
SHA-256 of the 131-byte key: 45ad4b37c6e2fc0a2cfcc1b5da524132ec707615c2cae1dbbc43c97aa521db81
HMAC-SHA256, case 6: 60e431591ee0b67f0d8a26aacbf5b77f8e0bc6213728c5140546040f0ee37f54
HMAC-SHA256, case 5, first 16 bytes: a3b6167473100ee06e0c796c2955552b
received tag, last bit flipped: accepted false
update after final: already final
31-byte output: bad output
1 MiB + 1 byte key: bad input
a SHA-256 state as an HMAC state: bad state
```

Test case 6 uses a 131-byte key of `0xaa` bytes. That is over 64 bytes, so
HMAC hashes it to `45ad4b37…a521db81` and pads that to 64 bytes.

The four refusals are an update after the final call, a 31-byte output, a
key of 1 MiB + 1 byte and a SHA-256 state passed as an HMAC state. Each
returns its status before it writes anything.

The book's checks run this program in debug and release builds and compare
the output with [bitcoin/fixtures/hmac-sha256.out](../fixtures/hmac-sha256.out).

## How the code is checked

### Published vectors

[coax/tests/nostr/hmac_rfc.coax](../../coax/tests/nostr/hmac_rfc.coax)
checks all seven RFC 4231 SHA-256 test cases, including the truncated case 5,
and that the bytes after the 32-byte tag are unchanged.
[coax/tests/nostr/hmac_hkdf.py](../../coax/tests/nostr/hmac_hkdf.py)
compiles HMAC to C and to wasm32 and compares it with Python's `hmac` module
for many key and message lengths:

```shell
python3 coax/tests/nostr/hmac_hkdf.py
```

### Checks in Coax

- `hmac.coax` checks the 63-, 64- and 65-byte key boundary against frozen
  values, streaming and repeated finalization, every refusal leaving the
  state and output unchanged, and that a pair of unkeyed SHA-256 states isn't
  accepted as an HMAC state.
- The example checks the library against the construction built by hand from
  SHA-256, for key lengths on both sides of 32 and 64 bytes:

<!-- source: bitcoin/examples/hmac-sha256.coax check -->
```jai
// The tag built by hand from SHA-256 equals the library's tag for key
// lengths on both sides of 32 and 64 bytes: short keys are padded, and keys
// over 64 bytes are hashed first.
#forall "HMAC equals the two-hash construction around the key boundaries" which: 0..<12 {
  lengths := [12] int.[0, 1, 31, 32, 33, 63, 64, 65, 66, 100, 128, 130];
  n := lengths[which];
  key: [130] u8;
  for i: 0..<n { key[i] = cast(u8) ((i * 13 + 5) % 256); }
  message := [3] u8.[97, 98, 99];
  effective: [64] u8;
  if n > 64 {
    #assert sha256_hash_checked(key[0..<n], effective[0..<32], false) == SHA_OK;
  } else {
    for i: 0..<n { effective[i] = key[i]; }
  }
  ipad: [64] u8; opad: [64] u8;
  for i: 0..<64 { ipad[i] = effective[i] ^ 0x36; opad[i] = effective[i] ^ 0x5c; }
  s: [112] u8; inner: [32] u8; expected: [32] u8;
  #assert sha256_init(s[..]) == SHA_OK && sha256_update_checked(s[..], ipad[..], false) == SHA_OK;
  #assert sha256_update_checked(s[..], message[..], false) == SHA_OK && sha256_final_checked(s[..], inner[..], false) == SHA_OK;
  #assert sha256_init(s[..]) == SHA_OK && sha256_update_checked(s[..], opad[..], false) == SHA_OK;
  #assert sha256_update_checked(s[..], inner[..], false) == SHA_OK && sha256_final_checked(s[..], expected[..], false) == SHA_OK;
  tag: [32] u8;
  #assert hmac_sha256_checked(key[0..<n], message[..], tag[..], false) == SHA_OK;
  #assert cb_ct_equal(tag[..], expected[..]) == CB_MAX64;
}
```

To run all the checks:

```shell
coax/bin/coax test bitcoin/examples/hmac-sha256.coax
coax/bin/coax mutate coax/lib/crypto/hmac.coax
```

`coax mutate` leaves zero survivors: 819 mutants in `hmac.coax` with the
files it loads, and 911 in the example with the files it loads. `coax test`
on the example runs 55 checks.

## Summary

- HMAC is two SHA-256 hashes: the inner hash over the inner pad and the
  message, and the outer hash over the outer pad and the inner hash.
- The key is made exactly 64 bytes. Keys over 64 bytes are hashed first.
- A streaming HMAC keeps an inner and an outer SHA-256 state.
- Compare tags with a comparison that reads every byte.

## Dependencies

- [4. SHA-256](ch004-sha-256.md)

## Sources

- [RFC 2104](https://www.rfc-editor.org/rfc/rfc2104), section 2.
- [RFC 4231](https://www.rfc-editor.org/rfc/rfc4231), section 4, pinned in
  [coax/tests/vectors/SOURCES.md](../../coax/tests/vectors/SOURCES.md).
- [NIP-44](../../nostr/nips/official/44.md), which authenticates payloads with
  HMAC-SHA256.
