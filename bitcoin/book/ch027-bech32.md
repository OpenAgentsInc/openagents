# 27. Bech32

Segwit v0 addresses are Bech32 strings such as `bc1q…`, and NIP-19 writes
Nostr keys and references as Bech32 strings such as `npub1…`. This
chapter builds the BIP-173 encoding: the 5-bit conversion, the BCH checksum
and the rules for case and length. Chapter 28 adds Bech32m for Taproot
addresses, and Nostr chapter 16 builds the NIP-19 entities on top of it.

## What this chapter teaches

- The human-readable part, the separator `1` and the data part.
- Converting 8-bit bytes to 5-bit groups with padding rules.
- The polymod checksum with constant 1, and why Bech32m (constant
  `0x2bc830a3`) is refused.
- Mixed case refuses; output is always lowercase.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [bech32-core.coax](../../coax/lib/nostr/bech32-core.coax) | `b32_word(data, eight, at)` | One 5-bit word, from bytes (`eight` true) or from words |
| | `b32_step`, `b32_prefix` | One polymod step; the polymod state after the HRP |
| | `b32_inspect(input, extended)` | Checks length, characters, case, separator and checksum without writing anything |
| | `bech32_encode_checked`, `bech32_decode_checked` | Encode to and decode from caller buffers |
| [nip19-core.coax](../../coax/lib/nostr/nip19-core.coax) | `n19_byte_count(words, last)` | The byte count of `words` 5-bit words, or -1 when the padding is wrong |
| [bech32-memory.coax](../../coax/lib/nostr/bech32-memory.coax) | `bech32_encode`, `bech32_decode` | The exported entry points; they add the buffer-overlap check |

Every procedure that writes takes caller-owned buffers. On any error it
returns a code (`NC_INVALID`, `NC_CAPACITY`, `NC_LIMIT` or `NC_ALIAS`, from
[codecs-common.coax](../../coax/lib/nostr/codecs-common.coax)) and writes
nothing. `extended` raises the length limit from 90 to NIP-19's 5,000
characters; this chapter uses the BIP-173 limit.

## The parts of a Bech32 string

A Bech32 string has three parts:

1. The human-readable part (HRP): 1 to 83 characters, each from `!` (33) to
   `~` (126). It names the network or the type: `bc` for mainnet segwit
   addresses, `tb` for testnet, and `npub` or `nsec` for Nostr keys.
2. The separator: the last `1` in the string. The HRP may itself contain
   `1`, so the decoder takes the last one.
3. The data part: characters from the 32-character alphabet
   `qpzry9x8gf2tvdw0s3jn54khce6mua7l`, each one 5-bit word. The last six are
   the checksum.

The whole string is 8 to 90 characters. The alphabet leaves out `1`, `b`,
`i` and `o`, which are easy to confuse with other characters.

A string is all lowercase or all uppercase. A mixed-case string refuses.
The decoder lowercases an uppercase string, and the encoder always writes
lowercase, so every value has one canonical text.

## From bytes to 5-bit words

A key is bytes, and the data part is 5-bit words. The conversion reads the
bytes as one bit string, most significant bit first, and cuts it into
groups of 5. If the last group is short, zero bits fill it. `b32_word`
computes word `at` directly from the bits it covers:

<!-- source: coax/lib/nostr/bech32-core.coax word -->
```jai
b32_word :: (data: string, eight: bool, at: int) -> u8 {
  if !eight { return data[at]; }
  value: u32 = 0;
  for k: 0..<5 {
    value = value << 1;
    bit := at * 5 + k;
    if bit < data.count * 8 { value = value | ((cast(u32) data[bit / 8] >> (7 - bit % 8)) & 1); }
  }
  return cast(u8) value;
}
```

`n` bytes give `ceil(8n / 5)` words. A 32-byte key is 256 bits, so it gives
52 words (260 bits), and the last 4 bits are padding.

### Worked example: 7e 7e

`7e 7e` is `01111110 01111110`, 16 bits, so it gives 4 words:

| Word | Bits | Value | Character |
| --- | --- | ---: | --- |
| 0 | `01111` | 15 | `0` |
| 1 | `11001` | 25 | `e` |
| 2 | `11111` | 31 | `l` |
| 3 | `0` + `0000` (padding) | 0 | `q` |

NIP-19's second example key starts with `7e7e9c`, and its npub starts
`npub10el`: the first three words are the same. The fourth differs because
in the full key the next bits come from `9c`, not from padding.

### The padding rule on the way back

Decoding goes the other way: 52 words are 260 bits, which is 32 bytes and 4
spare bits. The checksum covers words, not bytes, so the spare bits are a
rule of the conversion back to bytes. BIP-173 states it for segwit
addresses, and NIP-19 uses the same conversion: there are at most 4 spare
bits, and they are zero. BIP-173's invalid vectors include a string for
each case.
`n19_byte_count` applies them:

<!-- source: coax/lib/nostr/nip19-core.coax padding -->
```jai
n19_byte_count :: (words: int, last: int) -> int {
  if !n19_word_count(words) || last < 0 || last > 31 { return -1; }
  bits := words * 5;
  unused := bits % 8;
  if unused >= 5 { return -1; }
  if unused != 0 && (last & ((1 << unused) - 1)) != 0 { return -1; }
  return bits / 8;
}
```

Without the second rule, 16 different strings would decode to the same 32
bytes. With it, each key has exactly one npub.

## The checksum

The checksum is a BCH code over 5-bit words. The state is a 30-bit number
that starts at 1. Each step shifts the state left by 5 bits, adds the next
word, and folds the 5 bits that left the top back in with five fixed
generator values:

<!-- source: coax/lib/nostr/bech32-core.coax polymod -->
```jai
b32_step :: (state: u32, value: u8) -> u32 {
  top := state >> 25;
  next := ((state & 0x1ffffff) << 5) ^ cast(u32) value;
  generators := [5] u32.[0x3b6a57b2,0x26508e6d,0x1ea119fa,0x3d4233dd,0x2a1462b3];
  for generator: generators { if (top & 1) != 0 { next = next ^ generator; } top = top >> 1; }
  return next;
}
b32_prefix :: (hrp: string) -> u32 {
  state: u32 = 1;
  for c: hrp { state = b32_step(state, b32_lower(c) >> 5); }
  state = b32_step(state, 0);
  for c: hrp { state = b32_step(state, b32_lower(c) & 31); }
  return state;
}
```

The HRP goes in first, expanded to words: the high 3 bits of each
character, a 0, then the low 5 bits of each character. This makes the
checksum cover the HRP, so a `tb` address can't pass as `bc`, and `npub`
data can't pass as `nsec`.

To encode, run the polymod over the expanded HRP, the data words and six
zero words, then XOR the result with 1. The six 5-bit groups of that value,
most significant first, are the checksum words. To check a string, run the
polymod over the expanded HRP and every data word, including the checksum.
The string is valid when the result is 1.

### Worked example: the checksum of "a"

The BIP-173 vector `a12uel5l` has HRP `a` and no data. `a` is 97, which is
`011 00001`: high bits 3, low bits 1. The polymod runs over 9 values:
3, 0, 1 (the expanded HRP), then six zeros.

The first five steps start from a small state, so the top 5 bits are 0 and
no generator applies. Each step is `state · 32 + value`:

| Step | Value | Top bits | State |
| ---: | ---: | --- | --- |
| 1 | 3 | `00000` | `1 · 32 + 3 = 0x00000023` |
| 2 | 0 | `00000` | `0x00000460` |
| 3 | 1 | `00000` | `0x00008c01` |
| 4 | 0 | `00000` | `0x00118020` |
| 5 | 0 | `00000` | `0x02300400` |

Step 6 is the first that folds. `0x02300400 >> 25 = 1`, so only generator
0 applies. The low 25 bits are `0x0300400`; shifted left by 5 they are
`0x06008000`, and `0x06008000 XOR 0x3b6a57b2 = 0x3d6ad7b2`. The last three
steps fold in the same way:

| Step | Value | Top bits (generator 4 to 0) | State |
| ---: | ---: | --- | --- |
| 6 | 0 | `00001` | `0x3d6ad7b2` |
| 7 | 0 | `11110` | `0x02fd30b9` |
| 8 | 0 | `00001` | `0x24cc4092` |
| 9 | 0 | `10010` | `0x15ccfe9e` |

XOR with 1 gives `0x15ccfe9f`. Cut into six 5-bit groups it is 10, 28, 25,
31, 20, 31, which the alphabet writes as `2uel5l`. The string is `a`, `1`,
no data, `2uel5l`: `a12uel5l`, the published vector.

### Why Bech32m refuses

BIP-350 defines Bech32m, which is the same encoding with the constant
`0x2bc830a3` in place of 1. It fixes a weakness of Bech32: when the last
data character is `p`, inserting or deleting `q` characters just before it
can leave the checksum valid. Bitcoin uses Bech32m for SegWit version 1 and
later addresses.

BIP-173 is Bech32 with constant 1, and so is NIP-19, so this code uses
constant 1 only. Chapter 28 adds the other constant for Taproot addresses. A Bech32m string
leaves the polymod at `0x2bc830a3`, not 1, and refuses as a bad checksum.
There is no fallback that tries the other constant: one string has one
meaning.

## Checking a string

`b32_inspect` applies every rule before a decoder writes anything:

<!-- source: coax/lib/nostr/bech32-core.coax inspect -->
```jai
b32_inspect :: (input: string, extended: bool) -> B32_Header {
  if !b32_text_count(input.count, extended) { return B32_Header.{error = NC_INVALID}; }
  upper := false; lower := false; separator := 0;
  for i: 0..<input.count {
    c := input[i];
    if c < 33 || c > 126 { return B32_Header.{error = NC_INVALID}; }
    if c >= 'A' && c <= 'Z' { upper = true; }
    if c >= 'a' && c <= 'z' { lower = true; }
    if c == '1' { separator = i; }
  }
  if upper && lower { return B32_Header.{error = NC_INVALID}; }
  if separator < 1 || separator > 83 || input.count - separator < 7 { return B32_Header.{error = NC_INVALID}; }
  state := b32_prefix(input[0..<separator]);
  for i: separator + 1..<input.count {
    value := b32_digit(b32_lower(input[i]));
    if value < 0 { return B32_Header.{error = NC_INVALID}; }
    state = b32_step(state, cast(u8) value);
  }
  if state != 1 { return B32_Header.{error = NC_INVALID}; }
  return B32_Header.{hrp = separator, words = input.count - separator - 7};
}
```

In order, it refuses:

1. a length outside 8 to 90 (or 5,000 when `extended`);
2. a byte outside 33 to 126;
3. a mix of upper and lower case;
4. no separator, an empty HRP, an HRP over 83 characters, or fewer than six
   characters after the separator;
5. a data character outside the alphabet;
6. a polymod result other than 1.

`bech32_decode_checked` then copies the lowercase HRP and the data words,
without the checksum, to the caller's buffers.

## The example

The example is [bitcoin/examples/bech32.coax](../examples/bech32.coax). It
prints the polymod states for `a`, regroups `7e 7e`, encodes and decodes
NIP-19's example npub, re-encodes an uppercase string, and shows six
refusals.

<!-- source: bitcoin/examples/bech32.coax example -->
```jai
#load "../../coax/lib/nostr/nip19-core.coax";
#load "../../coax/lib/nostr/hex.coax";
#import "Basic";

// Print a label and bytes as lowercase hex.
show :: (label: string, bytes: [] u8) {
  text: [80] u8;
  r := hex_encode_checked(cast(string) bytes, text[..], false);
  print("%: %\n", label, cast(string) text[0..<r.written]);
}
// A 30-bit checksum state as 8 hex digits.
show_state :: (label: string, state: u32) {
  b: [4] u8;
  for i: 0..<4 { b[i] = cast(u8)((state >> (8 * (3 - i))) & 255); }
  show(label, b[..]);
}
// The name of a codec error code.
error_name :: (e: u32) -> string {
  if e == NC_OK { return "ok"; }
  if e == NC_INVALID { return "invalid"; }
  if e == NC_CAPACITY { return "capacity"; }
  if e == NC_LIMIT { return "limit"; }
  return "alias";
}
// Decode a Bech32 string and print its result, its HRP and its word count.
try_decode :: (label: string, text: string) {
  hrp: [83] u8; words: [90] u8;
  r := bech32_decode_checked(text, hrp[..], words[..], false, false);
  if r.error != NC_OK { print("%: %\n", label, error_name(r.error)); return; }
  print("%: ok, hrp %, % words\n", label, cast(string) hrp[0..<r.hrp_written], r.written);
}

NIP19_KEY  :: "3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d";
NIP19_NPUB :: "npub180cvv07tjdrrgpa0j7j7tmnyl2yr6yr7l8j4s3evf6u64th6gkwsyjh6w6";
// The same key with a Bech32m checksum (constant 0x2bc830a3).
NIP19_NPUB_M :: "npub180cvv07tjdrrgpa0j7j7tmnyl2yr6yr7l8j4s3evf6u64th6gkws3w8ktc";

main :: () {
  // The checksum of HRP "a" with no data, one polymod step per value:
  // the expanded HRP (3, 0, 1), then six zeros.
  values := [9] u8.[3, 0, 1, 0, 0, 0, 0, 0, 0];
  state: u32 = 1;
  for i: 0..<9 {
    state = b32_step(state, values[i]);
    print("step % (value %): ", i + 1, values[i]);
    show_state("state", state);
  }
  show_state("state xor 1", state ^ 1);
  print("checksum words:");
  for i: 0..<6 { print(" %", (state ^ 1) >> (5 * (5 - i)) & 31); }
  print("\n");
  text: [90] u8;
  e := bech32_encode_checked("a", "", false, text[..], false, false);
  print("encode \"a\", no data: %\n", cast(string) text[0..<e.written]);

  // Two bytes regrouped into 5-bit words.
  pair := [2] u8.[0x7e, 0x7e];
  print("words of 7e 7e:");
  for i: 0..<b32_word_count(2, true) { print(" %", b32_word(cast(string) pair[..], true, i)); }
  print("\n");

  // The NIP-19 example key as an npub, and back.
  key: [32] u8;
  #assert hex_decode_checked(NIP19_KEY, key[..], false).error == NC_OK;
  e = bech32_encode_checked("npub", cast(string) key[..], true, text[..], false, false);
  npub := cast(string) text[0..<e.written];
  print("npub: %\n", npub);
  print("equals NIP-19's example: %\n", npub == NIP19_NPUB);
  hrp: [83] u8; words: [90] u8;
  d := bech32_decode_checked(npub, hrp[..], words[..], false, false);
  print("decode: %, hrp %, % words, last word %\n", error_name(d.error),
        cast(string) hrp[0..<d.hrp_written], d.written, words[d.written - 1]);
  back: [32] u8;
  n := nip19_decode_checked(npub, back[..], false);
  print("bytes: %, % of them\n", error_name(n.error), n.written);
  show("key", back[0..<n.written]);

  // Uppercase is accepted, and output is always lowercase.
  d = bech32_decode_checked("A12UEL5L", hrp[..], words[..], false, false);
  e = bech32_encode_checked(cast(string) hrp[0..<d.hrp_written], "", false, text[..], false, false);
  print("A12UEL5L re-encodes as %\n", cast(string) text[0..<e.written]);

  // Refusals.
  try_decode("mixed case a12UEL5L", "a12UEL5L");
  try_decode("bad checksum a12uel5m", "a12uel5m");
  try_decode("no separator pzry9x0s0muk", "pzry9x0s0muk");
  try_decode("Bech32m a1lqfn3a", "a1lqfn3a");
  try_decode("Bech32m npub", NIP19_NPUB_M);
  print("Bech32m npub as NIP-19: %\n", error_name(nip19_decode_checked(NIP19_NPUB_M, back[..], false).error));

  // Bad padding: set the last word's low bit, one of the 4 unused bits, and
  // give the string a valid checksum. Bech32 accepts it; the 5-to-8
  // regrouping refuses it.
  d = bech32_decode_checked(npub, hrp[..], words[..], false, false);
  words[51] = words[51] | 1;
  padded: [90] u8;
  e = bech32_encode_checked("npub", cast(string) words[0..<52], false, padded[..], false, false);
  bad := cast(string) padded[0..<e.written];
  print("bad padding: %\n", bad);
  try_decode("bad padding as Bech32", bad);
  print("bad padding as bytes: %\n", error_name(nip19_decode_checked(bad, back[..], false).error));
  print("unused bits of 52 words, last word 17: byte count %\n", n19_byte_count(52, 17));
}
```

Run it from the repository root:

```shell
coax/bin/coax run bitcoin/examples/bech32.coax
```

The states and checksum words are the ones worked out by hand, and the npub
is the one NIP-19 gives for its example key:

```text
step 1 (value 3): state: 00000023
step 2 (value 0): state: 00000460
step 3 (value 1): state: 00008c01
step 4 (value 0): state: 00118020
step 5 (value 0): state: 02300400
step 6 (value 0): state: 3d6ad7b2
step 7 (value 0): state: 02fd30b9
step 8 (value 0): state: 24cc4092
step 9 (value 0): state: 15ccfe9e
state xor 1: 15ccfe9f
checksum words: 10 28 25 31 20 31
encode "a", no data: a12uel5l
words of 7e 7e: 15 25 31 0
npub: npub180cvv07tjdrrgpa0j7j7tmnyl2yr6yr7l8j4s3evf6u64th6gkwsyjh6w6
equals NIP-19's example: true
decode: ok, hrp npub, 52 words, last word 16
bytes: ok, 32 of them
key: 3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d
A12UEL5L re-encodes as a12uel5l
mixed case a12UEL5L: invalid
bad checksum a12uel5m: invalid
no separator pzry9x0s0muk: invalid
Bech32m a1lqfn3a: invalid
Bech32m npub: invalid
Bech32m npub as NIP-19: invalid
bad padding: npub180cvv07tjdrrgpa0j7j7tmnyl2yr6yr7l8j4s3evf6u64th6gkw3eyr0ng
bad padding as Bech32: ok, hrp npub, 52 words
bad padding as bytes: invalid
unused bits of 52 words, last word 17: byte count -1
```

The refusals:

- `a12UEL5L` mixes cases.
- `a12uel5m` changes the last checksum character from `l` to `m`; the
  polymod result is 5, not 1.
- `pzry9x0s0muk` has no separator.
- `a1lqfn3a` and the Bech32m npub have valid Bech32m checksums, so their
  polymod result is `0x2bc830a3`. `nip19_decode_checked` refuses the npub
  for the same reason, because it checks the string with `b32_inspect`
  first.
- The bad-padding npub has its last word changed from 16 (`10000`) to 17
  (`10001`) and a new, valid checksum. Bech32 accepts it: 52 words with
  HRP `npub`. The conversion to bytes refuses it, because one of the 4
  spare bits is 1.

The book's checks run this program in debug and release builds and compare
the output with [bitcoin/fixtures/bech32.out](../fixtures/bech32.out).

## How the code is checked

### Checks in the example

The checks are in
[bitcoin/examples/checks/bech32.coax](../examples/checks/bech32.coax), which
loads the example, so running the example doesn't run them. They compare
the code with published and hand-derived values:

- the 7 valid BIP-173 strings decode, and re-encoding gives each one in
  lowercase;
- the 12 invalid BIP-173 strings refuse, including the four with a byte
  outside printable ASCII (`0x20`, `0x7f`, `0x80` and a trailing `0xff`);
- the 7 valid BIP-350 Bech32m strings have polymod result `0x2bc830a3` and
  refuse;
- the nine polymod states of `a` match the hand trace above, which was
  also run through BIP-173's reference polymod in Python;
- NIP-19's two npub examples encode from their hex keys and decode back;
- `7e 7e` regroups to 15, 25, 31, 0.

<!-- source: bitcoin/examples/checks/bech32.coax check -->
```jai
// BIP-173's valid Bech32 strings, copied from the pinned
// coax/tests/vectors/upstream/bip173/bip-0173.mediawiki. Each decodes, and
// re-encoding its HRP and words gives the string in lowercase.
VALID :: [7] string.[
    "A12UEL5L", "a12uel5l",
    "an83characterlonghumanreadablepartthatcontainsthenumber1andtheexcludedcharactersbio1tt5tgs",
    "abcdef1qpzry9x8gf2tvdw0s3jn54khce6mua7lmqqqxw",
    "11qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqc8247j",
    "split1checkupstagehandshakeupstreamerranterredcaperred2y9e3w",
    "?1ezyfcl"];
#forall "every valid BIP-173 string decodes and re-encodes in lowercase" i: 0..<7 {
  s := VALID[i];
  hrp: [83] u8; words: [90] u8; text: [90] u8;
  r := bech32_decode_checked(s, hrp[..], words[..], false, false);
  #assert r.error == NC_OK && r.hrp_written + 1 + r.written + 6 == s.count;
  q := bech32_encode_checked(cast(string) hrp[0..<r.hrp_written], cast(string) words[0..<r.written], false, text[..], false, false);
  #assert q.error == NC_OK && q.written == s.count;
  for k: 0..<s.count { #assert text[k] == b32_lower(s[k]); }
}
// BIP-173's invalid strings. The four with a byte outside printable ASCII
// are built from their listed bytes.
INVALID :: [9] string.[
    "an84characterslonghumanreadablepartthatcontainsthenumber1andtheexcludedcharactersbio1569pvx",
    "pzry9x0s0muk", "1pzry9x0s0muk", "x1b4n0q5v", "li1dgmt3", "A1G7SGD8",
    "10a06t8", "1qzzfhee", "de1lg7wt"];
#forall "every invalid BIP-173 string refuses" i: 0..<12 {
  s: [9] u8; text := "";
  if i < 8 {
    text = INVALID[i];
  } else if i == 8 {
    for k: 0..<8 { s[k] = INVALID[8][k]; }
    s[8] = 0xff;  // "de1lg7wt" + 0xFF
    text = cast(string) s[..];
  } else {
    lead := [3] u8.[0x20, 0x7f, 0x80];
    tail := [3] string.["1nwldj5", "1axkwrx", "1eym55h"];
    s[0] = lead[i - 9];
    for k: 0..<7 { s[k + 1] = tail[i - 9][k]; }
    text = cast(string) s[0..<8];
  }
  hrp: [83] u8; words: [90] u8;
  #assert bech32_decode_checked(text, hrp[..], words[..], false, false).error == NC_INVALID;
}
// BIP-350's valid Bech32m strings: the same checksum with constant
// 0x2bc830a3 in place of 1. Every one refuses here.
BECH32M :: [7] string.[
    "A1LQFN3A", "a1lqfn3a",
    "an83characterlonghumanreadablepartthatcontainsthetheexcludedcharactersbioandnumber11sg7hg6",
    "abcdef1l7aum6echk45nj3s0wdvt2fg8x9yrzpqzd3ryx",
    "11llllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllllludsr8",
    "split1checkupstagehandshakeupstreamerranterredcaperredlc445v",
    "?1v759aa"];
// The polymod residue of a string: 1 for Bech32, 0x2bc830a3 for Bech32m.
residue :: (s: string) -> u32 {
  at := s.count - 1;
  while s[at] != '1' { at -= 1; }
  state := b32_prefix(s[0..<at]);
  for k: at + 1..<s.count { state = b32_step(state, cast(u8) b32_digit(b32_lower(s[k]))); }
  return state;
}
#forall "every BIP-350 Bech32m string refuses" i: 0..<7 {
  #assert residue(BECH32M[i]) == 0x2bc830a3;
  hrp: [83] u8; words: [90] u8;
  #assert bech32_decode_checked(BECH32M[i], hrp[..], words[..], false, false).error == NC_INVALID;
}
// The hand trace: HRP "a" with no data. The states were worked out by hand
// and with BIP-173's reference polymod in Python.
TRACE :: [9] u32.[0x23, 0x460, 0x8c01, 0x118020, 0x2300400, 0x3d6ad7b2,
                  0x2fd30b9, 0x24cc4092, 0x15ccfe9e];
#test "the polymod of \"a\" matches the hand trace" {
  values := [9] u8.[3, 0, 1, 0, 0, 0, 0, 0, 0];
  state: u32 = 1;
  for i: 0..<9 { state = b32_step(state, values[i]); #assert state == TRACE[i]; }
  #assert (state ^ 1) == 0x15ccfe9f;
  #assert residue("a12uel5l") == 1;
  #assert residue("a1") == TRACE[2];   // the expanded HRP alone: step 3
  #assert residue("a1q") == TRACE[3];  // then one zero word: step 4
}
// NIP-19's two npub examples, from the pinned
// coax/tests/vectors/upstream/nips/19.md, encode and decode both ways.
NPUBS :: [4] string.[
    "3bf0c63fcb93463407af97a5e5ee64fa883d107ef9e558472c4eb9aaaefa459d",
    "npub180cvv07tjdrrgpa0j7j7tmnyl2yr6yr7l8j4s3evf6u64th6gkwsyjh6w6",
    "7e7e9c42a91bfef19fa929e5fda1b72e0ebc1a4c1141673e2794234d86addf4e",
    "npub10elfcs4fr0l0r8af98jlmgdh9c8tcxjvz9qkw038js35mp4dma8qzvjptg"];
#forall "NIP-19's npub examples round-trip" i: 0..<2 {
  key: [32] u8; back: [32] u8; text: [90] u8;
  #assert hex_decode_checked(NPUBS[2 * i], key[..], false).error == NC_OK;
  e := bech32_encode_checked("npub", cast(string) key[..], true, text[..], false, false);
  #assert e.error == NC_OK && cast(string) text[0..<e.written] == NPUBS[2 * i + 1];
  n := nip19_decode_checked(NPUBS[2 * i + 1], back[..], false);
  #assert n.error == NC_OK && n.written == 32;
  for k: 0..<32 { #assert back[k] == key[k]; }
}
// 8-to-5 regrouping by hand: 7e 7e is 01111 11001 11111 0(0000).
#test "two bytes regroup into four words" {
  pair := [2] u8.[0x7e, 0x7e];
  want := [4] u8.[15, 25, 31, 0];
  #assert b32_word_count(2, true) == 4;
  for i: 0..<4 { #assert b32_word(cast(string) pair[..], true, i) == want[i]; }
}
```

To run them:

```shell
coax/bin/coax test bitcoin/examples/checks/bech32.coax
coax/bin/coax mutate bitcoin/examples/checks/bech32.coax --root-only
```

`coax test` runs 19 checks, including the loaded files' own checks, in
under a second. On 2026-11-01, root-only mutation killed 12 of 12 mutants
in the checks file.

### The library's own checks

`bech32-core.coax` and `nip19-core.coax` carry their own `#test` and
`#forall` checks: the same BIP-173 vectors, every one of the 256 bytes as an
HRP character and as a data byte, length and capacity limits, and the rule
that a refused call writes nothing. Both files are `#authority`. The
record
[coax-nostr-bech32-2026-10-28.md](../../docs/verification/coax-nostr-bech32-2026-10-28.md)
gives the results on 2026-10-28:

- `coax mutate` killed 259 of 259 mutants in `bech32-core.coax` and 511 of
  511 in `nip19-core.coax` with the core loaded.
- [nostr_bech32.py](../../coax/tests/nostr_bech32.py) ran 57,214 vector and
  native span records through checked and optimized emitted C under clang
  ASan/UBSan, with no sanitizer errors.
- 10,000 generated NIP-19 entities agreed with `nostr-effect 0.0.13` and
  `@scure/base 1.2.4`.

Those runs were not repeated for this chapter. Since then the two library
files gained only the `// book:` comment markers that this chapter's
listings use.

## Summary

- A Bech32 string is an HRP, the separator `1` and 5-bit data words, the
  last six of which are the checksum.
- Bytes become words by regrouping bits 8 to 5, with zero padding; on the
  way back, the spare bits must be fewer than 5 and zero.
- The checksum is a BCH polymod over the expanded HRP and the data; a valid
  string leaves 1. Bech32m's constant `0x2bc830a3` refuses.
- Mixed case refuses, uppercase is accepted, and output is lowercase.

## Dependencies

- [1. Bytes, hex and base64](ch001-bytes-hex-and-base64.md)

## Sources

- [BIP-173](https://github.com/bitcoin/bips/blob/master/bip-0173.mediawiki),
  pinned in
  [coax/tests/vectors/upstream/bip173/](../../coax/tests/vectors/upstream/bip173/bip-0173.mediawiki).
- [BIP-350](https://github.com/bitcoin/bips/blob/master/bip-0350.mediawiki),
  for the Bech32m constant and its valid test strings.
- [NIP-19](https://github.com/nostr-protocol/nips/blob/master/19.md),
  pinned in
  [coax/tests/vectors/upstream/nips/19.md](../../coax/tests/vectors/upstream/nips/19.md).
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
