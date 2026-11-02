# 10. SipHash-2-4

SipHash is a keyed hash with a 64-bit output, built for short inputs. With a
secret key, an attacker can't predict which inputs collide. Bitcoin uses
SipHash-2-4 in three places:

- BIP-152 compact blocks name each transaction by a 6-byte short ID cut from
  a SipHash of its wtxid (chapter 56).
- BIP-158 block filters hash each script to a number with SipHash before
  they Golomb-code the set (chapter 57).
- Bitcoin Core keys the hash tables of its mempool and coins cache with
  SipHash and a random salt, so a peer can't choose txids that all land in
  one bucket.

This chapter builds SipHash-2-4 from the paper by Aumasson and Bernstein:
the key and the initial state, the SipRound, the last word that carries the
length, and finalization. It works out the start of the first SipRound by
hand and checks the code against the 64 vectors of the reference
implementation.

## What this chapter teaches

- How SipHash turns a 16-byte key into four 64-bit state words.
- What one SipRound does: additions, rotations and XORs, and nothing that
  depends on the data's value.
- What the "2" and the "4" in SipHash-2-4 count.
- How the last word encodes the input length, so that `00` and the empty
  input hash differently.
- How BIP-152, BIP-158 and Bitcoin Core choose their keys, and which of
  those keys are secret.
- How to check the code against a pinned reference table and against a
  second implementation.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [siphash.coax](../../coax/lib/crypto/siphash.coax) | `sip_init`, `sip_round` | The initial state and one SipRound over the four words in a `SipState` |
| | `sip_last_word` | The last 0–7 input bytes with the length in the top byte |
| | `siphash24(k0, k1, input)` | The hash, with the key as two 64-bit words |
| | `siphash24_checked(key, input, out, overlap)` | The hash with a 16-byte key and an 8-byte tag, checked first |

## Where Bitcoin uses it

Each use takes the 64-bit key as two words, `k0` and `k1`, and hashes a
short input:

| Use | Key | Input | Result |
| --- | --- | --- | --- |
| BIP-152 short ID | The first two little-endian words of SHA-256(block header ‖ nonce) | A wtxid, 32 bytes | The low 6 bytes of the tag |
| BIP-158 filter element | The first 16 bytes of the block hash | A script, any length | The tag times N·M, shifted right by 64 bits |
| Bitcoin Core hash tables | Random, chosen once per process | A txid, or a txid and an output index | The tag, used as a bucket index |

The BIP-152 and BIP-158 keys are public: anyone with the block can compute
them. Their job is to make the hash different for every block, so that an
attacker can't prepare collisions in advance. The Bitcoin Core salt is a
secret. If a peer learns it, it can make transactions whose hashes share a
bucket, and every lookup in that bucket gets slower. So the code treats the
key as a secret (chapter 3).

## The key and the initial state

The key is 16 bytes, read as two little-endian 64-bit words. The state is
four words, `v0` to `v3`. Each starts as one key word XOR a constant, and
the four constants are the ASCII of `somepseudorandomlygeneratedbytes`, eight
bytes each, read big-endian:

| Word | Starts as | Constant |
| --- | --- | --- |
| `v0` | `k0 ^ c0` | `0x736f6d6570736575` (`somepseu`) |
| `v1` | `k1 ^ c1` | `0x646f72616e646f6d` (`dorandom`) |
| `v2` | `k0 ^ c2` | `0x6c7967656e657261` (`lygenera`) |
| `v3` | `k1 ^ c3` | `0x7465646279746573` (`tedbytes`) |

## The SipRound

One SipRound mixes the four words with three operations: addition modulo
2^64, rotation left by a fixed distance and XOR. In Coax, `add_wrap` adds
modulo 2^64 in every build mode, and `rotl` rotates:

<!-- source: coax/lib/crypto/siphash.coax round -->
```jai
// The initial state: the key XOR the ASCII of "somepseudorandomlygeneratedbytes".
sip_init :: (k0: u64, k1: u64) -> SipState {
  return SipState.{v0 = k0 ^ 0x736f6d6570736575, v1 = k1 ^ 0x646f72616e646f6d,
    v2 = k0 ^ 0x6c7967656e657261, v3 = k1 ^ 0x7465646279746573};
}
// One SipRound: additions modulo 2^64, rotations by fixed distances and XORs.
sip_round :: (s: SipState) -> SipState {
  v0 := s.v0; v1 := s.v1; v2 := s.v2; v3 := s.v3;
  v0 = add_wrap(v0, v1); v1 = rotl(v1, 13); v1 ^= v0; v0 = rotl(v0, 32);
  v2 = add_wrap(v2, v3); v3 = rotl(v3, 16); v3 ^= v2;
  v0 = add_wrap(v0, v3); v3 = rotl(v3, 21); v3 ^= v0;
  v2 = add_wrap(v2, v1); v1 = rotl(v1, 17); v1 ^= v2; v2 = rotl(v2, 32);
  return SipState.{v0 = v0, v1 = v1, v2 = v2, v3 = v3};
}
```

The two halves of the round mirror each other. The left half updates `v0`
and `v1`, the right half updates `v2` and `v3`, and then each half feeds the
other. The rotation distances are 13, 16, 21, 17 and 32 twice. A rotation by
32 swaps the two 32-bit halves of a word.

`sip_round` takes the state and returns a new one. It has no branch and no
table lookup.

## Compression and finalization

SipHash reads the input as 64-bit little-endian words. For each word `m`:

1. XOR `m` into `v3`.
2. Run 2 SipRounds. This is the "2" in SipHash-2-4.
3. XOR `m` into `v0`.

The last word holds the 0–7 bytes after the last full word, little-endian,
and the input length modulo 256 in its top byte. Every input has a last
word, even an empty one or one whose length is a multiple of 8. Without the
length, `00` and the empty input would both give a last word of 0.

After the last word, finalization XORs `0xff` into `v2`, runs 4 SipRounds
(the "4") and returns `v0 ^ v1 ^ v2 ^ v3`:

<!-- source: coax/lib/crypto/siphash.coax hash -->
```jai
// The last word: the 0-7 bytes after the last full word, little-endian, with
// the input length mod 256 in the top byte.
sip_last_word :: (input: [] u8, total: int) -> u64 {
  b := cast(u64) (total & 255) << 56;
  tail := input.count - input.count % 8;
  for i: tail..<input.count { b |= cast(u64) input[i] << ((i - tail) * 8); }
  return b;
}
// Each 8-byte word goes into v3, through two SipRounds (the "2"), then into
// v0. Finalization runs four SipRounds (the "4").
siphash24 :: (k0: u64, k1: u64, input: [] u8) -> u64 {
  s := sip_init(k0, k1);
  words := input.count / 8;
  for w: 0..<words + 1 {
    m := ifx w < words then cb_load_le64(input, w * 8).value else sip_last_word(input, input.count);
    s.v3 ^= m;
    s = sip_round(s); s = sip_round(s);
    s.v0 ^= m;
  }
  s.v2 ^= 255;
  s = sip_round(s); s = sip_round(s); s = sip_round(s); s = sip_round(s);
  return s.v0 ^ s.v1 ^ s.v2 ^ s.v3;
}
```

The loop runs once per input word, and the input length is public. The
`ifx` chooses between a full word and the last word by the word number, not
by the data.

### Worked example: the first SipRound for the message `00`

The reference vectors use the key `00 01 02 … 0f` and the inputs `""`,
`00`, `00 01`, and so on. Take vector 1, the one-byte input `00`.

1. The key words are `k0 = 0x0706050403020100` and
   `k1 = 0x0f0e0d0c0b0a0908`. Byte 0 of the key is the low byte of `k0`.
2. `v0 = k0 ^ 0x736f6d6570736575`. Byte by byte from the top: `07 ^ 73` is
   `74`, `06 ^ 6f` is `69`, and so on, which gives `0x7469686173716475`.
   In the same way, `v1 = 0x6b617f6d656e6665`, `v2 = 0x6b7f62616d677361`
   and `v3 = 0x7b6b696e727e6c7b`.
3. The input has no full word. The last word holds the byte `00` in byte 0
   and the length, 1, in byte 7: `m = 0x0100000000000000`.
4. `v3 ^= m` flips the low bit of the top byte: `v3 = 0x7a6b696e727e6c7b`.
5. `v0 += v1`. The low halves give `0x73716475 + 0x656e6665 = 0xd8dfcada`
   with no carry, and the high halves give
   `0x74696861 + 0x6b617f6d = 0xdfcae7ce`. So `v0 = 0xdfcae7ced8dfcada`.
6. `v1 = rotl(v1, 13)`. The word moves up 13 bits, and its top 13 bits,
   `0x0d6c`, come round to the bottom: `v1 = 0x2fedacadccccad6c`.
7. `v1 ^= v0` gives `0xf0274b63141367b6`.
8. `v0 = rotl(v0, 32)` swaps the halves of `0xdfcae7ced8dfcada`:
   `v0 = 0xd8dfcadadfcae7ce`.

The rest of the round continues the same way. After the whole first round
the state is:

```text
v0 = 0x6564848c93698d85
v1 = 0x40d43f143c94a7dc
v2 = 0xf3f94792d6121732
v3 = 0x5352f75825981d12
```

A second SipRound, `v0 ^= m`, finalization and the XOR of the four words
give `0x74f839c593dc67fd`. Written as 8 little-endian bytes, that's
`fd 67 dc 93 c5 39 f8 74`, the tag in row 1 of `vectors.h`.

## The byte interface

`siphash24_checked` takes the key and writes the tag as bytes:

<!-- source: coax/lib/crypto/siphash.coax checked -->
```jai
// The byte interface: a 16-byte key read as two little-endian words, and the
// 8-byte tag written little-endian. The checks run first, so a refusal writes
// nothing. `overlap` says whether the output shares memory with the key or
// the input.
siphash24_checked :: (key: [] u8, input: [] u8, out: [] u8, overlap: bool) -> u32 #export_c {
  status := sip_request(key.count, input.count, out.count, overlap);
  if status != SIP_OK { return status; }
  tag := siphash24(cb_load_le64(key, 0).value, cb_load_le64(key, 8).value, input);
  if !cb_store_le64(out, 0, tag) { return SIP_BAD_OUTPUT; }
  return SIP_OK;
}
```

It checks everything before it hashes, and returns a status:

| Status | Name | Meaning |
| --- | --- | --- |
| 0 | `SIP_OK` | Success. The first 8 bytes of `out` hold the tag. |
| 1 | `SIP_BAD_KEY` | The key isn't exactly 16 bytes |
| 2 | `SIP_BAD_INPUT` | The input is negative or larger than 1 MiB |
| 3 | `SIP_BAD_OUTPUT` | The output has fewer than 8 bytes |
| 4 | `SIP_ALIAS` | The output overlaps the key or the input |

A refusal writes nothing. A tag is 8 bytes; a longer output keeps its bytes
after the eighth. As in chapter 4, `overlap` is a fact you supply about your
buffers, not something the procedure can see.

Bitcoin Core passes the key as two `uint64_t` words and reads the tag as a
`uint64_t`. `siphash24(k0, k1, input)` matches that form. The reference
implementation's `vectors.h` lists each tag as 8 bytes, which is the same
number written little-endian.

## Secrets

SipHash follows chapter 3's rules for the key:

- **No branch on the key.** The only branches test the word number and the
  input length, which are public.
- **No index from the key.** SipHash has no table. The code indexes the
  input by public positions.
- **No variable-time operation on the key.** The state changes only by
  `add_wrap`, `rotl` by fixed distances, and XOR.

The source audit of chapter 3 doesn't cover this code: its profile accepts
`u32`, `u64` and `bool` values, and SipHash uses a struct, a byte view and a
loop. This chapter claims the rules from reading the code, not from the
audit.

`siphash24` keeps the state in locals and passes it by value, so copies of
key-derived words reach registers and the stack, where chapter 3's erasure
can't reach. The key itself stays in your buffer. In Bitcoin Core the salt
lives as long as the process. When you retire a key, erase its buffer with
chapter 3's scope.

## The example

The example is [bitcoin/examples/siphash.coax](../examples/siphash.coax).
It prints the key words, the initial state and the steps worked out above,
the state after the first SipRound, six reference tags and Bitcoin Core's
salted-hash case. Then it flips one key bit and shows four refusals.

<!-- source: bitcoin/examples/siphash.coax example -->
```jai
#load "../../coax/lib/crypto/siphash.coax";
#load "../../coax/lib/bitcoin/hash.coax";
#import "Basic";

// Print a label and bytes as lowercase hex.
show_bytes :: (label: string, bytes: [] u8) {
  digits := "0123456789abcdef";
  text: [128] u8; n := 0;
  for b: bytes { text[n] = digits[b >> 4]; text[n + 1] = digits[b & 15]; n += 2; }
  print("%: %\n", label, cast(string) text[0..<n]);
}
// Print a label and one 64-bit word as 16 hex digits, most significant first.
show_word :: (label: string, x: u64) {
  b: [8] u8;
  if cb_store_be64(b[..], 0, x) { show_bytes(label, b[..]); }
}
// Print the four state words.
show_state :: (label: string, s: SipState) {
  print("%:\n", label);
  show_word("  v0", s.v0); show_word("  v1", s.v1); show_word("  v2", s.v2); show_word("  v3", s.v3);
}
STATUS :: [5] string.["ok", "bad key", "bad input", "bad output", "overlap"];

main :: () {
  // The reference key is the bytes 00 01 ... 0f, read as two little-endian
  // words.
  key: [16] u8;
  for i: 0..<16 { key[i] = cast(u8) i; }
  k0 := cb_load_le64(key[..], 0).value; k1 := cb_load_le64(key[..], 8).value;
  show_word("k0", k0); show_word("k1", k1);
  s := sip_init(k0, k1);
  show_state("initial state", s);

  // The one-byte message 00 is one last word: the byte, then the length 1
  // in the top byte.
  message := [1] u8.[0];
  m := sip_last_word(message[..], 1);
  show_word("last word m", m);
  s.v3 ^= m;
  show_word("v3 ^= m", s.v3);
  // The first four steps of the first SipRound, then the whole round.
  v0 := add_wrap(s.v0, s.v1); show_word("v0 += v1", v0);
  v1 := rotl(s.v1, 13); show_word("v1 rotl 13", v1);
  v1 ^= v0; show_word("v1 ^= v0", v1);
  show_word("v0 rotl 32", rotl(v0, 32));
  show_state("after the first SipRound", sip_round(s));

  // The tag is 8 bytes: the 64-bit result, little-endian.
  tag: [8] u8;
  lengths := [6] int.[0, 1, 7, 8, 15, 63];
  labels := [6] string.["vector 0 (empty)", "vector 1 (00)", "vector 7 (00..06)", "vector 8 (00..07)",
    "vector 15 (00..0e)", "vector 63 (00..3e)"];
  input: [63] u8;
  for i: 0..<63 { input[i] = cast(u8) i; }
  for k: 0..<6 {
    if siphash24_checked(key[..], input[0..<lengths[k]], tag[..], false) != SIP_OK { print("unexpected refusal\n"); }
    show_bytes(labels[k], tag[..]);
  }

  // Bitcoin Core's case: k0 = 1 and k1 = 2 over the SHA-256d of an empty
  // version-1 transaction (version, no inputs, no outputs, lock time 0).
  tx := [10] u8.[1, 0, 0, 0, 0, 0, 0, 0, 0, 0];
  txid: [32] u8;
  if sha256d_checked(tx[..], txid[..], false) != SHA_OK { print("sha256d refused\n"); }
  show_bytes("SHA-256d of the empty transaction", txid[..]);
  show_word("SipHash(k0 = 1, k1 = 2)", siphash24(1, 2, txid[..]));

  // A broken case: one flipped key bit gives an unrelated tag, and a
  // constant-time comparison with the expected tag fails.
  expected := [8] u8.[0xfd, 0x67, 0xdc, 0x93, 0xc5, 0x39, 0xf8, 0x74];
  if siphash24_checked(key[..], message[..], tag[..], false) != SIP_OK { print("unexpected refusal\n"); }
  print("tag of 00 matches vector 1: %\n", cb_ct_equal(tag[..], expected[..]) == CB_MAX64);
  key[0] ^= 1;
  if siphash24_checked(key[..], message[..], tag[..], false) != SIP_OK { print("unexpected refusal\n"); }
  show_bytes("tag of 00 with key bit 0 flipped", tag[..]);
  print("matches vector 1: %\n", cb_ct_equal(tag[..], expected[..]) == CB_MAX64);

  // Refusals. Each one returns a status and leaves the output unchanged.
  for i: 0..<8 { tag[i] = 0; }
  print("15-byte key: %\n", STATUS[siphash24_checked(key[0..<15], message[..], tag[..], false)]);
  print("7-byte output: %\n", STATUS[siphash24_checked(key[..], message[..], tag[0..<7], false)]);
  print("output overlaps the input: %\n", STATUS[siphash24_checked(key[..], message[..], tag[..], true)]);
  print("1 MiB + 1 byte: %\n", STATUS[sip_request(16, 1048577, 8, false)]);
  show_bytes("output after the refusals", tag[..]);
}
```

Run it from the repository root:

```shell
coax/bin/coax run bitcoin/examples/siphash.coax
```

The output matches the values worked out by hand above:

```text
k0: 0706050403020100
k1: 0f0e0d0c0b0a0908
initial state:
  v0: 7469686173716475
  v1: 6b617f6d656e6665
  v2: 6b7f62616d677361
  v3: 7b6b696e727e6c7b
last word m: 0100000000000000
v3 ^= m: 7a6b696e727e6c7b
v0 += v1: dfcae7ced8dfcada
v1 rotl 13: 2fedacadccccad6c
v1 ^= v0: f0274b63141367b6
v0 rotl 32: d8dfcadadfcae7ce
after the first SipRound:
  v0: 6564848c93698d85
  v1: 40d43f143c94a7dc
  v2: f3f94792d6121732
  v3: 5352f75825981d12
vector 0 (empty): 310e0edd47db6f72
vector 1 (00): fd67dc93c539f874
vector 7 (00..06): 37d1018bf50002ab
vector 8 (00..07): 6224939a79f5f593
vector 15 (00..0e): e545be4961ca29a1
vector 63 (00..3e): 724506eb4c328a95
SHA-256d of the empty transaction: 43ec7a579f5561a42a7e9637ad4156672735a658be2752181801f723ba3316d2
SipHash(k0 = 1, k1 = 2): 79751e980c2a0a35
tag of 00 matches vector 1: true
tag of 00 with key bit 0 flipped: 7511c2b0ba840578
matches vector 1: false
15-byte key: bad key
7-byte output: bad output
output overlaps the input: overlap
1 MiB + 1 byte: bad input
output after the refusals: 0000000000000000
```

The six tags are rows 0, 1, 7, 8, 15 and 63 of `vectors_sip64` in the
reference implementation's `vectors.h`. Rows 7 and 8 sit on either side of
the first full word: 7 bytes fit in the last word, and 8 bytes make one full
word and a last word that holds only the length. Row 15 is the paper's own
test value in its Appendix A.

The Bitcoin Core case comes from its `hash_tests.cpp`. It serializes an
empty version-1 transaction (`01000000`, no inputs, no outputs, lock time
0), hashes it with SHA-256d (chapter 5) and hashes the 32-byte result with
`k0 = 1` and `k1 = 2`. Core expects `0x79751e980c2a0a35`, and the example
prints the same value.

### A broken key and the refusals

The broken case flips bit 0 of the key. The tag of `00` becomes
`7511c2b0ba840578`, unrelated to `fd67dc93c539f874`, so the comparison
with vector 1 fails. A BIP-152 receiver with the wrong key computes short
IDs that match none of the sender's, so it can't rebuild the block from its
mempool. The example compares the tags with `cb_ct_equal` from chapter 3.

The four refusals are a 15-byte key, a 7-byte output, an output that
overlaps the input and an input of 1 MiB + 1 byte. Each returns its status
before it writes anything, so the output stays all zeros.

The book's checks run this program in debug and release builds and compare
the output with [bitcoin/fixtures/siphash.out](../fixtures/siphash.out).

## How the code is checked

### Published vectors and a second implementation

The reference implementation's `vectors.h` is pinned at commit `32d06760`
of `veorq/SipHash` as
[coax/tests/vectors/upstream/siphash/vectors.h](../../coax/tests/vectors/upstream/siphash/vectors.h),
and Bitcoin Core's test file as
[coax/tests/vectors/upstream/bitcoin-core/hash_tests.cpp](../../coax/tests/vectors/upstream/bitcoin-core/hash_tests.cpp).
[coax/tests/bitcoin/siphash.py](../../coax/tests/bitcoin/siphash.py)
compiles `siphash.coax` to C in checked and release builds and checks:

- All 64 tags of `vectors_sip64`. It first checks that they equal Bitcoin
  Core's `siphash_4_2_testvec` table, which copies them.
- Bitcoin Core's salted-hash case, with the transaction hash computed by
  Python's `hashlib`.
- 2,000 random keys and inputs of 0 to 300 bytes against a second SipHash,
  written in Python from the paper inside the test.
- Five refusals (a 15-byte and a 17-byte key, a negative input length, a
  7-byte output and an overlap), each leaving the output unchanged.

Then it compiles the same C to wasm32 and runs the 64 vectors, the Bitcoin
Core case and 500 of the random cases in Node:

```shell
python3 coax/tests/bitcoin/siphash.py
```

```text
checked: 64 reference vectors, 1 Bitcoin Core case, 2000 oracle comparisons, 5 refusals
release: 64 reference vectors, 1 Bitcoin Core case, 2000 oracle comparisons, 5 refusals
wasm checked: 565 cases in Node v24.13.1
wasm release: 565 cases in Node v24.13.1
```

### Checks in Coax

`siphash.coax` checks the worked SipRound, reference rows 0, 1, 8, 15 and
63 and the Bitcoin Core case through `siphash24`, the byte order of the
tag, the last word for lengths 0, 3, 15, 16 and 259, and that every
refusal leaves the output unchanged.

[bitcoin/examples/checks/siphash.coax](../examples/checks/siphash.coax)
checks all 64 reference vectors through the byte interface:

<!-- source: bitcoin/examples/checks/siphash.coax vectors -->
```jai
// The 64 SipHash-2-4 tags of the reference implementation's vectors.h
// (vectors_sip64), read as little-endian 64-bit words. The same table is
// Bitcoin Core's siphash_4_2_testvec in the pinned hash_tests.cpp;
// coax/tests/bitcoin/siphash.py reads both files and checks that they agree.
VECTORS :: [64] u64.[
  0x726fdb47dd0e0e31, 0x74f839c593dc67fd, 0x0d6c8009d9a94f5a, 0x85676696d7fb7e2d,
  0xcf2794e0277187b7, 0x18765564cd99a68d, 0xcbc9466e58fee3ce, 0xab0200f58b01d137,
  0x93f5f5799a932462, 0x9e0082df0ba9e4b0, 0x7a5dbbc594ddb9f3, 0xf4b32f46226bada7,
  0x751e8fbc860ee5fb, 0x14ea5627c0843d90, 0xf723ca908e7af2ee, 0xa129ca6149be45e5,
  0x3f2acc7f57c29bdb, 0x699ae9f52cbe4794, 0x4bc1b3f0968dd39c, 0xbb6dc91da77961bd,
  0xbed65cf21aa2ee98, 0xd0f2cbb02e3b67c7, 0x93536795e3a33e88, 0xa80c038ccd5ccec8,
  0xb8ad50c6f649af94, 0xbce192de8a85b8ea, 0x17d835b85bbb15f3, 0x2f2e6163076bcfad,
  0xde4daaaca71dc9a5, 0xa6a2506687956571, 0xad87a3535c49ef28, 0x32d892fad841c342,
  0x7127512f72f27cce, 0xa7f32346f95978e3, 0x12e0b01abb051238, 0x15e034d40fa197ae,
  0x314dffbe0815a3b4, 0x027990f029623981, 0xcadcd4e59ef40c4d, 0x9abfd8766a33735c,
  0x0e3ea96b5304a7d0, 0xad0c42d6fc585992, 0x187306c89bc215a9, 0xd4a60abcf3792b95,
  0xf935451de4f21df2, 0xa9538f0419755787, 0xdb9acddff56ca510, 0xd06c98cd5c0975eb,
  0xe612a3cb9ecba951, 0xc766e62cfcadaf96, 0xee64435a9752fe72, 0xa192d576b245165a,
  0x0a8787bf8ecb74b2, 0x81b3e73d20b49b6f, 0x7fa8220ba3b2ecea, 0x245731c13ca42499,
  0xb78dbfaf3a8d83bd, 0xea1ad565322a1a0b, 0x60e61c23a3795013, 0x6606d7e446282b93,
  0x6ca4ecb15c5f91e1, 0x9f626da15c9625f3, 0xe51b38608ef25f57, 0x958a324ceb064572];
#forall "every reference vector: key 00..0f, input 00..(n - 1)" n: 0..<64 {
  key: [16] u8; input: [63] u8; tag: [8] u8;
  for i: 0..<16 { key[i] = cast(u8) i; }
  for i: 0..<63 { input[i] = cast(u8) i; }
  #assert siphash24_checked(key[..], input[0..<n], tag[..], false) == SIP_OK;
  #assert cb_load_le64(tag[..], 0).value == VECTORS[n];
}
```

To run all the checks:

```shell
coax/bin/coax test coax/lib/crypto/siphash.coax
coax/bin/coax test bitcoin/examples/checks/siphash.coax
coax/bin/coax mutate coax/lib/crypto/siphash.coax --root-only
```

`coax mutate --root-only` kills all 80 mutants in `siphash.coax`.

## Summary

- SipHash-2-4 hashes a short input under a 16-byte key to 64 bits.
- The state is four words. Each input word goes through 2 SipRounds, and
  finalization runs 4.
- The last word carries the input length modulo 256 in its top byte.
- BIP-152 and BIP-158 derive public per-block keys. Bitcoin Core's salt is
  secret, so the key never decides a branch or an index.

## Dependencies

- [1. Bytes, hex and base64](ch001-bytes-hex-and-base64.md), for the
  little-endian loads and stores.
- [3. Constant-time rules and secrets](ch003-constant-time-rules.md), for
  the rules the key follows and `cb_ct_equal`.
- [5. SHA-256d and tagged hashes](ch005-sha-256d-and-tagged-hashes.md), for
  the transaction hash in Bitcoin Core's case.

## Sources

- J.-P. Aumasson and D. J. Bernstein,
  [SipHash: a fast short-input PRF](https://cr.yp.to/siphash/siphash-20120918.pdf),
  2012: section 2 for the definition and Appendix A for the worked example.
- The reference implementation,
  [veorq/SipHash](https://github.com/veorq/SipHash/tree/32d067603b93b47828700880649198e0bfbbcffa),
  for `vectors.h`.
- [BIP-152](https://github.com/bitcoin/bips/blob/master/bip-0152.mediawiki),
  "Short transaction IDs", and
  [BIP-158](https://github.com/bitcoin/bips/blob/master/bip-0158.mediawiki),
  "Hashing Data Objects".
- Bitcoin Core's `src/test/hash_tests.cpp`, test case `siphash`.
- Both test files are pinned in
  [coax/tests/vectors/SOURCES.md](../../coax/tests/vectors/SOURCES.md).
