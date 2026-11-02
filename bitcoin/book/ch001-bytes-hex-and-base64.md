# 1. Bytes, hex and base64

Every Bitcoin and Nostr value starts as bytes:

- A hash, a txid and an x-only public key are 32 bytes.
- A BIP-340 signature is 64 bytes.
- A ChaCha20 nonce is 12 bytes.

Transactions store integers little-endian; hashes and keys are written as
lowercase hex. Nostr writes the same values as text too: events and filters
use lowercase hex, and NIP-44 payloads and NIP-98 headers use base64. Bitcoin
uses base64 too: PSBTs travel as base64 (chapter 94), and so do signed
messages (chapter 22). This chapter shows how Coax
holds bytes, and how it reads and writes them as hex and base64. Every later
chapter uses these functions.

## What this chapter teaches

- How each codec writes to a buffer that you own and returns a status.
- How to load and store big-endian and little-endian words with bounds
  checks.
- How strict Nostr hex differs from RFC 4648 hex.
- How canonical padded base64 works, and which inputs it refuses.
- How to check a codec with published vectors, a round-trip `#forall` and
  mutation testing.

## The rules every codec follows

The codecs are in three files:

- [coax/lib/nostr/codecs-common.coax](../../coax/lib/nostr/codecs-common.coax)
  defines the result type and the error codes.
- [coax/lib/nostr/hex.coax](../../coax/lib/nostr/hex.coax) is RFC 4648
  base16.
- [coax/lib/nostr/base64.coax](../../coax/lib/nostr/base64.coax) is RFC 4648
  base64 with the standard alphabet and required padding.

Each encode and decode function takes the input, an output slice that you
own, and an `overlap` flag. It returns an `NC_Result` with two fields:
`written`, the number of bytes written, and `error`, one of these codes:

| Code | Name | Meaning |
|---|---|---|
| 0 | `NC_OK` | Success. `written` bytes are in the output. |
| 1 | `NC_INVALID` | The input isn't valid for this encoding. |
| 2 | `NC_CAPACITY` | The output slice is too small. |
| 3 | `NC_LIMIT` | The input is larger than `NC_MAX_BYTES` (1 MiB). |
| 4 | `NC_ALIAS` | The caller said the input and output overlap. |

Two rules hold for every function:

1. The function checks the whole input and the output size before it
   writes anything. When it returns an error, the output is unchanged.
2. The function never allocates. You choose the buffer, so you also choose
   where a secret lives and when you erase it (chapter 3).

## Endian loads and stores

SHA-256 reads big-endian words, and ChaCha20 reads little-endian words.
[coax/lib/crypto/bytes.coax](../../coax/lib/crypto/bytes.coax) has one
function for each width and order: `cb_load_be32`, `cb_load_le32`,
`cb_load_be64`, `cb_load_le64` and the matching `cb_store_*` functions.

A load returns a `CbU32` or `CbU64` with a `value` and an `ok` flag. The
flag is `false` when the word doesn't fit in the slice at that offset. The
bounds check is `cb_span(total, at, n)`, which uses only subtraction, so a
large offset can't overflow:

```jai
at >= 0 && n >= 0 && at <= total && n <= total - at
```

A store returns `false` and writes nothing when the word doesn't fit.

## Strict Nostr hex

RFC 4648 base16 accepts uppercase and lowercase letters. `hex_decode_checked`
follows the RFC, so it accepts `"fF00"`.

NIP-01 is stricter. An event's `id`, `pubkey` and `sig` are lowercase hex of
a fixed size: 64 characters for an ID or key, and 128 for a signature. A
relay that accepted uppercase would store two spellings of the same key, and
filters that match on the text would miss one of them.

The example in this chapter adds the Nostr rule on top of the RFC decoder.
The event parser in [coax/lib/nostr/event.coax](../../coax/lib/nostr/event.coax)
uses the same check in `ne_lower_hex`:

1. Refuse the text unless it has exactly twice as many characters as the
   output has bytes.
2. Refuse the text if any character has ASCII bit 5 clear. The digits `0`–`9`
   (0x30–0x39) and the letters `a`–`f` (0x61–0x66) all have bit 5 set. The
   letters `A`–`F` (0x41–0x46) don't.
3. Decode with `hex_decode_checked`, which refuses every other non-hex byte.

### Worked example: one key

The x coordinate of the secp256k1 generator point is a valid public key:

```text
79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798
```

Each pair of characters is one byte. The first pair is `7` and `9`, so the
first byte is 7 × 16 + 9 = 121. The last pair is `9` and `8`, so the last
byte is 9 × 16 + 8 = 152.

The first four bytes are `79 be 66 7e`:

- Read big-endian, they're the number 0x79be667e, which is 2,042,521,214.
- Read little-endian, the bytes reverse to 0x7e66be79, which is
  2,120,662,649.

## Canonical base64

Base64 turns every 3 bytes into 4 characters, 6 bits each. When the input
length isn't a multiple of 3, the last group is padded with `=`:

- 1 byte left: 2 characters and `==`.
- 2 bytes left: 3 characters and `=`.

### Worked example: `"f"`

`"f"` is the byte 0x66, or `01100110` in binary.

1. Split it into 6-bit groups, padding the second group with zero bits:
   `011001` and `10` + `0000`.
2. `011001` is 25, the letter `Z`. `100000` is 32, the letter `g`.
3. One byte was left, so add `==`.

The result is `Zg==`.

The four zero bits added in step 1 must be zero. If you change `g` (32,
`100000`) to `h` (33, `100001`), the string `Zh==` still has the right
shape, but its last four bits are `0001`. A lenient decoder returns `"f"`
for both `Zg==` and `Zh==`. That gives one value two encodings, so a
signature or a hash computed over the text no longer identifies the value.
`base64_decode_checked` refuses `Zh==`.

The decoder refuses these inputs:

- A length that isn't a multiple of 4, such as `Zg` or `Zg=`.
- `=` anywhere except the last one or two characters.
- Non-zero padding bits, such as `Zh==`.
- Whitespace, line breaks and the base64url characters `-` and `_`.
- Any other byte outside the alphabet.

## The example

The example is
[bitcoin/examples/bytes-hex-base64.coax](../examples/bytes-hex-base64.coax). It decodes the key, reads its first word both ways, encodes it
again, and shows the refusals. Then it prints the RFC 4648 base64 vectors
and decodes four non-canonical strings.

<!-- source: bitcoin/examples/bytes-hex-base64.coax example -->
```jai
#load "../../coax/lib/crypto/bytes.coax";
#load "../../coax/lib/nostr/hex.coax";
#load "../../coax/lib/nostr/base64.coax";
#import "Basic";

// Nostr IDs, public keys and signatures are lowercase hex of a fixed size.
// The RFC 4648 decoder accepts both cases, so check the case first.
// Every lowercase hex digit has ASCII bit 5 set; uppercase digits don't.
nostr_hex :: (text: string, out: [] u8) -> u32 {
  if text.count != out.count * 2 { return NC_INVALID; }
  for c: text { if (c & 32) == 0 { return NC_INVALID; } }
  return hex_decode_checked(text, out, false).error;
}

ERRORS :: [5] string.["ok", "invalid", "no room", "over the limit", "overlaps"];

main :: () {
  // The x coordinate of the secp256k1 generator, a valid 32-byte public key.
  key := "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
  raw: [32] u8;
  print("decode the key: %\n", ERRORS[nostr_hex(key, raw[..])]);
  print("first byte %, last byte %\n", raw[0], raw[31]);
  print("first 4 bytes as big-endian u32: %\n", cb_load_be32(raw[..], 0).value);
  print("first 4 bytes as little-endian u32: %\n", cb_load_le32(raw[..], 0).value);
  text: [64] u8;
  r := hex_encode_checked(cast(string) raw[..], text[..], false);
  print("encode it again: % characters, same text %\n", r.written, cast(string) text[..] == key);

  // Refusals. Each one writes nothing to the output.
  upper := "79BE667EF9DCBBAC55A06295CE870B07029BFCDB2DCE28D959F2815B16F81798";
  print("uppercase key: %\n", ERRORS[nostr_hex(upper, raw[..])]);
  print("63 characters: %\n", ERRORS[nostr_hex(key[0..<63], raw[..])]);
  print("a g in the key: %\n", ERRORS[nostr_hex("g9be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798", raw[..])]);
  small: [16] u8;
  print("a 16-byte output: %\n", ERRORS[hex_decode_checked(key, small[..], false).error]);

  // RFC 4648 section 10 test vectors.
  plain := [7] string.["", "f", "fo", "foo", "foob", "fooba", "foobar"];
  for p: plain {
    out: [8] u8;
    e := base64_encode_checked(p, out[..], false);
    print("base64(\"%\") = \"%\"\n", p, cast(string) out[0..<e.written]);
  }
  // Non-canonical base64 is refused.
  bad := [4] string.["Zg", "Zg=", "Zh==", "Z g="];
  for s: bad {
    out: [4] u8;
    print("decode \"%\": %\n", s, ERRORS[base64_decode_checked(s, out[..], false).error]);
  }
}
```

Run it from the repository root:

```shell
coax/bin/coax run bitcoin/examples/bytes-hex-base64.coax
```

The output matches the values worked out by hand above, and the base64
lines match RFC 4648 section 10:

```text
decode the key: ok
first byte 121, last byte 152
first 4 bytes as big-endian u32: 2042521214
first 4 bytes as little-endian u32: 2120662649
encode it again: 64 characters, same text true
uppercase key: invalid
63 characters: invalid
a g in the key: invalid
a 16-byte output: no room
base64("") = ""
base64("f") = "Zg=="
base64("fo") = "Zm8="
base64("foo") = "Zm9v"
base64("foob") = "Zm9vYg=="
base64("fooba") = "Zm9vYmE="
base64("foobar") = "Zm9vYmFy"
decode "Zg": invalid
decode "Zg=": invalid
decode "Zh==": invalid
decode "Z g=": invalid
```

The book's checks run this program in debug and release builds and compare
the output with
[bitcoin/fixtures/bytes-hex-base64.out](../fixtures/bytes-hex-base64.out).

## How the code is checked

The codec files carry their own checks:

- **Published vectors.** `hex.coax` and `base64.coax` check all seven RFC
  4648 section 10 vectors in both directions.
- **Every byte.** A `#forall` over all 256 byte values checks each hex digit
  and each base64 character against an independent table, and checks that
  changing a padding bit makes the decoder refuse.
- **Refusals leave the output alone.** Each refusal test fills the output
  with 77 first and checks that every byte is still 77 afterwards.
- **Native oracles.** The verification record
  [coax-nostr-codecs-2026-10-28.md](../../docs/verification/coax-nostr-codecs-2026-10-28.md)
  compares the codecs with independent C implementations under the address
  and undefined-behaviour sanitizers.

The example adds a round-trip check over every length from 0 to 256 bytes:

<!-- source: bitcoin/examples/bytes-hex-base64.coax roundtrip -->
```jai
// Round trips for every length from 0 to 256 bytes. Each byte depends on the
// length and its position, so no two lengths share a pattern.
#forall "hex and base64 round-trip every length up to 256 bytes" n: 0..<257 {
  raw: [256] u8;
  for i: 0..<n { raw[i] = cast(u8) ((i * 151 + n * 7) % 256); }
  hex: [512] u8; back: [256] u8;
  h := hex_encode_checked(cast(string) raw[0..<n], hex[..], false);
  #assert h.error == NC_OK && h.written == n * 2;
  #assert nostr_hex(cast(string) hex[0..<h.written], back[0..<n]) == NC_OK;
  for i: 0..<n { #assert back[i] == raw[i]; }
  b64: [344] u8;
  e := base64_encode_checked(cast(string) raw[0..<n], b64[..], false);
  #assert e.error == NC_OK && e.written == (n + 2) / 3 * 4;
  d := base64_decode_checked(cast(string) b64[0..<e.written], back[..], false);
  #assert d.error == NC_OK && d.written == n;
  for i: 0..<n { #assert back[i] == raw[i]; }
}
```

The 1 MiB limit itself is checked at its edges: `hex_encode_size`,
`base64_encode_size` and `base64_payload_size` accept exactly `NC_MAX_BYTES`
and refuse one byte more.

To run all the checks:

```shell
coax/bin/coax test bitcoin/examples/bytes-hex-base64.coax
coax/bin/coax mutate coax/lib/nostr/hex.coax
coax/bin/coax mutate coax/lib/nostr/base64.coax
```

`coax test` runs 19 checks, including the checks in the loaded files.
`coax mutate` changes each operator and constant in the file, one at a time,
and runs the checks again. A mutant that every check still passes is a
survivor. The codec files have zero survivors.

## Summary

- Each codec writes to your buffer, checks everything before it writes, and
  returns a status code.
- Endian loads and stores check their bounds with subtraction only.
- Nostr hex is RFC 4648 hex plus two rules: an exact length and lowercase
  letters only.
- Canonical base64 has exactly one encoding for each value. The decoder
  refuses missing padding, misplaced padding and non-zero padding bits.

## Dependencies

- [Coax chapter 5, Structs, enums and
  unions](../../coax/book/ch05-structs-enums-unions.md)
- [Coax chapter 6, Arrays, strings, memory and the
  context](../../coax/book/ch06-memory.md)

## Sources

- [RFC 4648](https://www.rfc-editor.org/rfc/rfc4648), sections 4, 8 and 10.
- [NIP-01](../../nostr/nips/official/01.md), the event fields `id`, `pubkey` and `sig`.
- [NIP-44](../../nostr/nips/official/44.md) and [NIP-98](../../nostr/nips/official/98.md), which use base64.
- The pinned vector sources:
  [coax/tests/vectors/SOURCES.md](../../coax/tests/vectors/SOURCES.md).
