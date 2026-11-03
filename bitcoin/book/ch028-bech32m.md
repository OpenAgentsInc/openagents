# 28. Bech32m

Taproot addresses such as `bc1p…` are Bech32m strings. Bech32m is
[chapter 27](ch027-bech32.md)'s Bech32 with one change: the constant that
the checksum ends on is `0x2bc830a3` instead of 1. This chapter shows the
flaw that the change fixes, adds the second constant to chapter 27's code
rather than writing a second encoder, and builds segwit address encoding
and decoding on top: the network's human-readable part, the witness
version, the witness program and BIP-350's rule for which checksum goes
with which version. It checks the result against every BIP-350 and
BIP-173 vector and against BIP-350's reference Python implementation.

## What this chapter teaches

- The Bech32 weakness: when a string ends in `p`, `q` characters inserted
  or deleted just before that `p` leave the checksum valid.
- The Bech32m constant `0x2bc830a3`, and why changing only the constant
  is enough.
- BIP-350's rule: witness version 0 uses Bech32, and versions 1 to 16 use
  Bech32m. A decoder refuses an address whose checksum is the other one.
- Segwit address rules: the HRPs `bc`, `tb` and `bcrt`, the witness
  version, program lengths (20 or 32 bytes for version 0, 2 to 40 bytes
  for versions 1 to 16), the 90-character limit and mixed case.
- A Taproot address, worked out by hand.
- Twelve refusals, each with a code of its own.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [bech32-core.coax](../../coax/lib/nostr/bech32-core.coax) | `b32_scan(input, extended)` | Checks length, characters, case and separator; returns the polymod result as `residue` |
| | `b32_check(input, extended, constant)` | `b32_scan`, then refuses a residue other than `constant` |
| | `b32_encode`, `b32_decode` | Encode and decode with a given constant |
| | `bech32m_encode_checked`, `bech32m_decode_checked` | Bech32m only; the `bech32_` procedures stay Bech32 only |
| | `b32_encode_words(hrp, words, out, constant)` | 5-bit words under the 90-character limit, for the segwit encoder |
| [segwit.coax](../../coax/lib/bitcoin/segwit.coax) | `segwit_variant_vartime(text)` | `SW_BECH32`, `SW_BECH32M`, or why the string is neither |
| | `segwit_decode_vartime(text, program_out)` | The network, the witness version and the program of an address |
| | `segwit_encode_vartime(network, version, program, out)` | The address for a network, a version and a program |

Chapter 27's code took the constant 1 in two places: `b32_inspect`
compared the polymod result with it, and the encoder XORed it in. This
chapter makes the constant a parameter in both places. `b32_inspect`,
`bech32_encode_checked` and `bech32_decode_checked` pass 1, so NIP-19 and
everything else that uses them behaves as before.

The procedures in `segwit.coax` return a count (for the decoder, in
`Segwit_Address.length`) or one negative `SW_*` refusal, and a refusal
writes nothing. Addresses are public, so these procedures branch
and index on their input; the `_vartime` suffix says so, as in
[chapter 26](ch026-base58check.md).

## Why Bech32 needed fixing

After BIP-173 was deployed, its checksum turned out to miss one simple
mistake. If a valid Bech32 string ends in `p`, then inserting any number of
`q` characters just before that `p`, or deleting `q` characters from there,
gives another valid string. BIP-350 calls this the insertion weakness.

Chapter 27's polymod explains it. `b32_step(state, value)` shifts the state
by one word, reduces it by the generator and XORs in the value. For a fixed
value, it is a one-to-one function of the state: the shift and reduction
multiply by `x` modulo the generator polynomial, and that multiplication
can be undone because the polynomial's constant term is not zero. Two
facts about the zero state follow:

- `b32_step(0, 1) = 1`. `p` is the word 1, so the only state from which
  `p` reaches 1 is 0.
- `b32_step(0, 0) = 0`. `q` is the word 0, so from state 0 a `q` stays at 0.

A valid Bech32 string ending in `p` therefore reaches state 0 just before
its `p`. Insert a `q` there, and the state stays 0; the `p` still takes it
to 1. Delete a `q` from in front of the final `p`, and the state before
the `p` is still 0, because the state before that `q` went to 0 on a 0
word, and only 0 does that.

One string in 32 ends in `p`, and one inserted character in 32 is `q`, so
a random insertion in the second-to-last position goes undetected with
probability 2⁻¹⁰. A good 30-bit checksum misses any error with probability
about 2⁻³⁰.

Version 0 addresses are safe from this by accident: their programs are
exactly 20 or 32 bytes, so an address with a `q` added or removed has the
wrong length. Versions 1 to 16 allow any program from 2 to 40 bytes. A
Taproot address with a `q` inserted can decode to a valid version 1
address with a 33-byte program, and coins sent there are lost.

## The new constant

BIP-350 keeps everything about Bech32 except the constant. A valid string
must end with the polymod result `0x2bc830a3`, and an encoder XORs
`0x2bc830a3` into the polymod in place of 1.

With that constant, a string ending in `p` reaches a nonzero state `s`
before its `p`, where `b32_step(s, 1) = 0x2bc830a3`. A `q` takes `s` to
`b32_step(s, 0) = 0x2bc830a3 XOR 1 = 0x2bc830a2`, and the `p` then takes
that to `0x36d938ba`, not `0x2bc830a3`: the string refuses. BIP-350 chose
the constant by searching for one that keeps Bech32's guarantees (any
4 substituted characters are detected) and makes every other common error
pattern fail at least as often as a random 30-bit hash would.

No string is valid under both constants: a Bech32 string and a Bech32m
string of the same length differ in at least 3 characters. `b32_scan`
returns the polymod result, and the caller decides which constant it
wants.

### Worked example: a Taproot address

BIP-350's Taproot vector has the program
`79be667e…16f81798`, the x coordinate of the generator G
([chapter 17](ch017-point-encodings-compressed-uncompressed-and-x-only.md))
used as an output key. Its address is built from three parts:

1. The HRP `bc`, expanded as in chapter 27. `b` is 98 (`011 00010`) and
   `c` is 99 (`011 00011`), so the expansion is 3, 3, 0, 2, 3.
2. The data: the witness version 1 as one word, then the 32-byte program
   regrouped into 52 words. 1 is `p`. The program starts `79 be 66`, which
   is `01111 00110 11111 00110 01100 11111 …`, so its first words are 15,
   6, 31, 6, 12, 31: `0xlxvl`.
3. The checksum: run the polymod over the expanded HRP, the 53 data words
   and six zero words, then XOR in the constant.

The polymod over those 64 values is `0x2ea27aec`. XOR with `0x2bc830a3`
gives `0x056a4a4f`, whose six 5-bit groups are 2, 22, 20, 18, 18 and 15:
`zk5jj0`. The address is `bc` + `1` + `p` + 52 key characters +
`zk5jj0`, 62 characters:

```text
bc1p0xlxvlhemja6c4dqv22uapctqupfhlxm9h8z3k2e72q4k9hcz7vqzk5jj0
```

XOR with 1 instead gives `0x2ea27aed`, which is `h2y7hd`. The string that
ends in `h2y7hd` is BIP-350's example of a Taproot address with the wrong
checksum.

## Which checksum goes with which version

BIP-350 assigns one checksum to each witness version:

| Witness version | Outputs | Checksum |
| --- | --- | --- |
| 0 | P2WPKH (20-byte program) and P2WSH (32-byte program) | Bech32, constant 1 |
| 1 | P2TR (32-byte program) | Bech32m, constant `0x2bc830a3` |
| 2 to 16 | not yet defined | Bech32m |

An address with the other checksum refuses. Accepting both for a version
would cost a bit of checksum: with two valid residues, a random string
passes with probability 2⁻²⁹ instead of 2⁻³⁰. For versions 1 to 16 it
would also accept Bech32 addresses, which are the ones with the insertion
weakness. A wallet written before BIP-350 that encodes a Taproot address
with Bech32 therefore makes an address that current wallets refuse, so no
coins go to it.

Versions 2 to 16 decode and encode even though no soft fork defines them.
BIP-350 asks for this so that a future output type works in today's
wallets. A wallet must not create such an address for itself, since no
rule protects the coins.

## Segwit addresses

A segwit address is a Bech32 or Bech32m string with these rules:

- The HRP names the network: `bc` for mainnet, `tb` for testnet, testnet4
  and signet, and `bcrt` for regtest.
- The first data word is the witness version, 0 to 16. The version is one
  5-bit word, not regrouped with the program.
- The remaining data words are the witness program, regrouped from 5 to 8
  bits with chapter 27's padding rule: fewer than 5 spare bits, all zero.
- The program is 2 to 40 bytes, and exactly 20 or 32 bytes for version 0.
- The string is at most 90 characters, all lowercase or all uppercase. An
  encoder writes lowercase. Uppercase is allowed because QR codes store it
  more compactly.

The networks, the variants and the refusals:

<!-- source: coax/lib/bitcoin/segwit.coax networks -->
```jai
// The networks and their human-readable parts. Testnet, testnet4 and
// signet share "tb". Network 0 is unused.
SW_MAINNET :: 1; SW_TESTNET :: 2; SW_REGTEST :: 3;
SW_HRPS :: [4] string.["", "bc", "tb", "bcrt"];
// The two checksum variants.
SW_BECH32 :: 1; SW_BECH32M :: 2;
```

<!-- source: coax/lib/bitcoin/segwit.coax refusals -->
```jai
SW_TOO_LONG :: -1;       // over 90 characters
SW_MIXED_CASE :: -2;     // upper and lower case letters together
SW_BAD_FORMAT :: -3;     // otherwise not Bech32: a byte outside 33-126, no separator,
                         // an empty HRP, a short checksum or a character outside the alphabet
SW_BAD_CHECKSUM :: -4;   // the polymod result is neither 1 nor 0x2bc830a3
SW_UNKNOWN_HRP :: -5;    // not bc, tb or bcrt; for encoding, a network outside 1-3
SW_NO_VERSION :: -6;     // no data before the checksum, so no witness version
SW_BAD_VERSION :: -7;    // a witness version over 16
SW_WRONG_VARIANT :: -8;  // version 0 with Bech32m, or version 1-16 with Bech32
SW_BAD_PADDING :: -9;    // 5 or more spare bits, or a spare bit that is 1
SW_BAD_LENGTH :: -10;    // a witness program outside 2-40 bytes
SW_BAD_V0_LENGTH :: -11; // a version 0 program that is not 20 or 32 bytes
SW_CAPACITY :: -12;      // the output buffer is too small
```

### Bech32 or Bech32m

`segwit_variant_vartime` gives the string's variant, or why it has none:

<!-- source: coax/lib/bitcoin/segwit.coax variant -->
```jai
// A scanned string: its variant (SW_BECH32 or SW_BECH32M) or a refusal,
// and the HRP length and data word count that b32_scan found.
Segwit_Scan :: struct { variant: int; hrp: int; words: int; }
sw_scan :: (text: string) -> Segwit_Scan {
  h := b32_scan(text, false);
  if h.error != NC_OK {
    // b32_scan refuses an overlong or mixed-case string too; give those
    // two their own codes.
    if text.count > 90 { return Segwit_Scan.{variant = SW_TOO_LONG}; }
    upper := false; lower := false;
    for c: text {
      if c >= 'A' && c <= 'Z' { upper = true; }
      if c >= 'a' && c <= 'z' { lower = true; }
    }
    if upper && lower { return Segwit_Scan.{variant = SW_MIXED_CASE}; }
    return Segwit_Scan.{variant = SW_BAD_FORMAT};
  }
  variant := SW_BAD_CHECKSUM;
  if h.residue == B32_BECH32 { variant = SW_BECH32; }
  if h.residue == B32_BECH32M { variant = SW_BECH32M; }
  return Segwit_Scan.{variant = variant, hrp = h.hrp, words = h.words};
}
// Bech32 or Bech32m, or the reason the string is neither.
segwit_variant_vartime :: (text: string) -> int #export_c { return sw_scan(text).variant; }
```

`b32_scan` runs once. It refuses an overlong or mixed-case string as
`NC_INVALID`, like any other malformed string, so `sw_scan` looks again
only after a refusal, to give those two cases codes of their own. The
decoder calls `sw_scan` too, and keeps the HRP length and word count it
returns.

### The rules

<!-- source: coax/lib/bitcoin/segwit.coax rules -->
```jai
// The network for an HRP, compared without case, or SW_UNKNOWN_HRP.
sw_network :: (hrp: string) -> int {
  for network: SW_MAINNET..<SW_REGTEST + 1 {
    name := SW_HRPS[network];
    if name.count == hrp.count {
      same := true;
      for i: 0..<hrp.count { if b32_lower(hrp[i]) != name[i] { same = false; } }
      if same { return network; }
    }
  }
  return SW_UNKNOWN_HRP;
}
// 0 when a witness version and program length make an address, or the refusal.
sw_program_rule :: (version: int, length: int) -> int {
  if version < 0 || version > 16 { return SW_BAD_VERSION; }
  if length < 2 || length > 40 { return SW_BAD_LENGTH; }
  if version == 0 && length != 20 && length != 32 { return SW_BAD_V0_LENGTH; }
  return 0;
}
```

### Decoding

<!-- source: coax/lib/bitcoin/segwit.coax decode -->
```jai
segwit_decode_vartime :: (text: string, program_out: [] u8) -> Segwit_Address #export_c {
  h := sw_scan(text);
  if h.variant < SW_BECH32 { return Segwit_Address.{length = h.variant}; }
  network := sw_network(text[0..<h.hrp]);
  if network < SW_MAINNET { return Segwit_Address.{length = network}; }
  if h.words == 0 { return Segwit_Address.{length = SW_NO_VERSION}; }
  version := b32_digit(b32_lower(text[h.hrp + 1]));
  if version > 16 { return Segwit_Address.{length = SW_BAD_VERSION}; }
  if (version == 0) != (h.variant == SW_BECH32) { return Segwit_Address.{length = SW_WRONG_VARIANT}; }
  // The program is the words after the version, regrouped 5 to 8 bits.
  last := b32_digit(b32_lower(text[h.hrp + h.words]));
  length := n19_byte_count(h.words - 1, last);
  if length < 0 { return Segwit_Address.{length = SW_BAD_PADDING}; }
  rule := sw_program_rule(version, length);
  if rule < 0 { return Segwit_Address.{length = rule}; }
  if program_out.count < length { return Segwit_Address.{length = SW_CAPACITY}; }
  source := N19_Source.{input = text, start = h.hrp + 2, count = length, encoded = true};
  for i: 0..<length { program_out[i] = n19_byte(source, i); }
  return Segwit_Address.{length = length, network = network, version = version};
}
```

The checks run in a fixed order, and the first that fails names the
refusal:

1. the string itself: length, case, format and checksum;
2. the HRP;
3. an empty data part, which has no version;
4. a version over 16;
5. the variant against the version;
6. the padding;
7. the program length for the version;
8. the room in `program_out`.

The variant check comes before the length checks, so an address with the
wrong checksum refuses as `SW_WRONG_VARIANT` whatever its program. The
decoder reuses two pieces of `nip19-core.coax`: `n19_byte_count`, which
applies the padding rule (chapter 27), and `n19_byte`, which reads one
program byte from the text's 5-bit words. `N19_Source` starts two
characters after the HRP, past the separator and the version word.

### Encoding

<!-- source: coax/lib/bitcoin/segwit.coax encode -->
```jai
segwit_encode_vartime :: (network: int, version: int, program: [] u8, out: [] u8) -> int #export_c {
  if network < SW_MAINNET || network > SW_REGTEST { return SW_UNKNOWN_HRP; }
  rule := sw_program_rule(version, program.count);
  if rule < 0 { return rule; }
  // The version is one word, then the program regrouped 8 to 5 bits.
  words: [65] u8;
  count := b32_word_count(program.count, true);
  words[0] = cast(u8) version;
  for i: 0..<count { words[i + 1] = b32_word(cast(string) program, true, i); }
  constant := ifx version == 0 then B32_BECH32 else B32_BECH32M;
  r := b32_encode_words(SW_HRPS[network], cast(string) words[0..<count + 1], out, constant);
  if r.error != NC_OK { return SW_CAPACITY; }
  return r.written;
}
```

The encoder checks the network and the program rules, puts the version
word in front of the regrouped program, and picks the constant from the
version. `b32_encode` refuses only when `out` is too small, because every
address it is given here is at most 76 characters.

## The example

The example is [bitcoin/examples/bech32m.coax](../examples/bech32m.coax). It
builds the Taproot address by hand, encodes and decodes it with the
library, decodes BIP-350's P2WPKH address, shows the `q` insertion on a
Bech32 string and a Bech32m string that both end in `p`, and gives one
refusal for each code.

<!-- source: bitcoin/examples/bech32m.coax example -->
```jai
#load "../../coax/lib/bitcoin/segwit.coax";
#load "../../coax/lib/nostr/hex.coax";
#import "Basic";

// Print a label and bytes as lowercase hex.
show :: (label: string, bytes: [] u8) {
  text: [100] u8;
  r := hex_encode_checked(cast(string) bytes, text[..], false);
  print("%: %\n", label, cast(string) text[0..<r.written]);
}
// A 30-bit checksum state as 8 hex digits.
show_state :: (label: string, state: u32) {
  b: [4] u8;
  for i: 0..<4 { b[i] = cast(u8)((state >> (8 * (3 - i))) & 255); }
  show(label, b[..]);
}
// The six checksum words of a final polymod value, and their characters.
show_checksum :: (label: string, value: u32) {
  chars: [6] u8;
  print("%:", label);
  for i: 0..<6 {
    word := (value >> (5 * (5 - i))) & 31;
    print(" %", word);
    chars[i] = B32_ALPHABET[word];
  }
  print(" = %\n", cast(string) chars[..]);
}
// The name of a refusal or variant.
code_name :: (code: int) -> string {
  names := [12] string.["too long", "mixed case", "bad format", "bad checksum", "unknown HRP",
                        "no version", "bad version", "wrong variant", "bad padding", "bad length",
                        "bad v0 length", "capacity"];
  if code < 0 && code >= -12 { return names[-code - 1]; }
  if code == SW_BECH32 { return "Bech32"; }
  if code == SW_BECH32M { return "Bech32m"; }
  return "?";
}
// Decode an address and print what it holds, or its refusal.
try_decode :: (label: string, text: string) {
  program: [40] u8;
  a := segwit_decode_vartime(text, program[..]);
  if a.length < 0 { print("%: %\n", label, code_name(a.length)); return; }
  print("%: %, version %, % bytes\n", label, SW_HRPS[a.network], a.version, a.length);
}
// The polymod state over a string's expanded HRP and its data words, up to
// but not including the last `skip` characters.
state_before :: (text: string, skip: int) -> u32 {
  at := text.count - 1;
  while text[at] != '1' { at -= 1; }
  state := b32_prefix(text[0..<at]);
  for k: at + 1..<text.count - skip { state = b32_step(state, cast(u8) b32_digit(b32_lower(text[k]))); }
  return state;
}

// The x coordinate of the generator G, as a Taproot output key.
TAPROOT_KEY :: "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798";
TAPROOT_ADDRESS :: "bc1p0xlxvlhemja6c4dqv22uapctqupfhlxm9h8z3k2e72q4k9hcz7vqzk5jj0";
// The same key with a Bech32 checksum: BIP-350's invalid vector.
TAPROOT_BECH32 :: "bc1p0xlxvlhemja6c4dqv22uapctqupfhlxm9h8z3k2e72q4k9hcz7vqh2y7hd";
// A version 1 program whose Bech32 checksum ends in 'p': the key with its
// last byte changed to 0x0c, as a wallet written before BIP-350 would
// encode it. The Bech32m form of the key with last byte 0x2b also ends in 'p'.
ENDS_IN_P_BECH32 :: "bc1p0xlxvlhemja6c4dqv22uapctqupfhlxm9h8z3k2e72q4k9hczuxqwrttsp";
ENDS_IN_P_BECH32M :: "bc1p0xlxvlhemja6c4dqv22uapctqupfhlxm9h8z3k2e72q4k9hczu4se9yxfp";

main :: () {
  // The constant and the HRP.
  show_state("Bech32m constant", B32_BECH32M);
  print("expanded HRP of bc:");
  for c: "bc" { print(" %", c >> 5); }
  print(" 0");
  for c: "bc" { print(" %", c & 31); }
  print("\n");

  // The Taproot address by hand: the version word 1, then 52 words of key,
  // then the checksum.
  key: [32] u8;
  #assert hex_decode_checked(TAPROOT_KEY, key[..], false).error == NC_OK;
  print("version word: 1 = p\n");
  print("first key words:");
  for i: 0..<6 { print(" %", b32_word(cast(string) key[..], true, i)); }
  print("\nkey words: %\n", b32_word_count(32, true));
  state := b32_prefix("bc");
  state = b32_step(state, 1);
  for i: 0..<52 { state = b32_step(state, b32_word(cast(string) key[..], true, i)); }
  for i: 0..<6 { state = b32_step(state, 0); }
  show_state("polymod with six zeros", state);
  show_state("xor 0x2bc830a3", state ^ B32_BECH32M);
  show_checksum("Bech32m checksum", state ^ B32_BECH32M);
  show_state("xor 1", state ^ B32_BECH32);
  show_checksum("Bech32 checksum", state ^ B32_BECH32);

  // The library does the same.
  text: [90] u8;
  n := segwit_encode_vartime(SW_MAINNET, 1, key[..], text[..]);
  address := cast(string) text[0..<n];
  print("address: %\n", address);
  print("equals BIP-350's vector: %\n", address == TAPROOT_ADDRESS);
  print("variant: %\n", code_name(segwit_variant_vartime(address)));
  program: [40] u8;
  a := segwit_decode_vartime(address, program[..]);
  print("decode: %, version %, % bytes\n", SW_HRPS[a.network], a.version, a.length);
  show("program", program[0..<a.length]);
  n = segwit_encode_vartime(SW_REGTEST, 1, key[..], text[..]);
  print("regtest: %\n", cast(string) text[0..<n]);
  print("variant of the Bech32 form: %\n", code_name(segwit_variant_vartime(TAPROOT_BECH32)));
  try_decode("the Bech32 form", TAPROOT_BECH32);

  // Version 0 keeps Bech32.
  try_decode("P2WPKH BC1QW508D6...", "BC1QW508D6QEJXTDG4Y5R3ZARVARY0C5XW7KV8F3T4");
  a = segwit_decode_vartime("BC1QW508D6QEJXTDG4Y5R3ZARVARY0C5XW7KV8F3T4", program[..]);
  n = segwit_encode_vartime(a.network, a.version, program[0..<a.length], text[..]);
  print("re-encoded: %\n", cast(string) text[0..<n]);

  // The weakness: a Bech32 string ending in 'p' takes or loses 'q's in
  // front of that 'p' and still passes.
  print("Bech32, ends in p\n");
  show_state("  state before the p", state_before(ENDS_IN_P_BECH32, 1));
  inserted: [70] u8;
  for extra: 0..<3 {
    m := ENDS_IN_P_BECH32.count - 1;
    for i: 0..<m { inserted[i] = ENDS_IN_P_BECH32[i]; }
    for i: 0..<extra { inserted[m + i] = 'q'; }
    inserted[m + extra] = 'p';
    s := cast(string) inserted[0..<m + extra + 1];
    // BIP-173 took Bech32 for every version: what it would have read.
    h := b32_scan(s, false);
    bytes := n19_byte_count(h.words - 1, b32_digit(s[h.hrp + h.words]));
    print("  % q inserted: %, BIP-173 program ", extra, code_name(segwit_variant_vartime(s)));
    if bytes < 0 { print("bad padding\n"); } else { print("% bytes\n", bytes); }
  }
  print("Bech32m, ends in p\n");
  show_state("  state before the p", state_before(ENDS_IN_P_BECH32M, 1));
  m := ENDS_IN_P_BECH32M.count - 1;
  for i: 0..<m { inserted[i] = ENDS_IN_P_BECH32M[i]; }
  inserted[m] = 'q'; inserted[m + 1] = 'p';
  s := cast(string) inserted[0..<m + 2];
  show_state("  1 q inserted, state before the p", state_before(s, 1));
  print("  1 q inserted: %\n", code_name(segwit_variant_vartime(s)));

  // Refusals, one for each code.
  try_decode("91 characters", "bc1p0xlxvlhemja6c4dqv22uapctqupfhlxm9h8z3k2e72q4k9hcz7v8n0nx0muaewav253zgeavqqqqqqqqqqqqqqq");
  try_decode("tb1p...q47Zagq", "tb1p0xlxvlhemja6c4dqv22uapctqupfhlxm9h8z3k2e72q4k9hcz7vq47Zagq");
  try_decode("'o' in bc1p38j9...jow4", "bc1p38j9r5y49hruaue7wxjce0updqjuyyx0kh56v8s25huc6995vvpql3jow4");
  try_decode("last character changed", "bc1p0xlxvlhemja6c4dqv22uapctqupfhlxm9h8z3k2e72q4k9hcz7vqzk5jj2");
  try_decode("tc1p...", "tc1p0xlxvlhemja6c4dqv22uapctqupfhlxm9h8z3k2e72q4k9hcz7vq5zuyut");
  try_decode("bc1gmk9yu", "bc1gmk9yu");
  try_decode("version 17", "BC130XLXVLHEMJA6C4DQV22UAPCTQUPFHLXM9H8Z3K2E72Q4K9HCZ7VQ7ZWS8R");
  try_decode("v0 with Bech32m", "bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kemeawh");
  try_decode("5 spare bits", "bc1p0xlxvlhemja6c4dqv22uapctqupfhlxm9h8z3k2e72q4k9hcz7v07qwwzcrf");
  try_decode("a spare bit set", "tb1p0xlxvlhemja6c4dqv22uapctqupfhlxm9h8z3k2e72q4k9hcz7vpggkg4j");
  try_decode("1-byte program", "bc1pw5dgrnzv");
  try_decode("41-byte program", "bc1p0xlxvlhemja6c4dqv22uapctqupfhlxm9h8z3k2e72q4k9hcz7v8n0nx0muaewav253zgeav");
  try_decode("16-byte v0 program", "BC1QR508D6QEJXTDG4Y5R3ZARVARYV98GJ9P");
  small: [31] u8;
  print("32 bytes into 31: %\n", code_name(segwit_decode_vartime(TAPROOT_ADDRESS, small[..]).length));
  print("encode version 17: %\n", code_name(segwit_encode_vartime(SW_MAINNET, 17, key[..], text[..])));
  print("encode v0, 32 bytes: %\n", segwit_encode_vartime(SW_MAINNET, 0, key[..], text[..]));
  print("encode v0, 31 bytes: %\n", code_name(segwit_encode_vartime(SW_MAINNET, 0, key[0..<31], text[..])));
}
```

Run it from the repository root:

```shell
coax/bin/coax run bitcoin/examples/bech32m.coax
```

The states, checksums and address are the ones worked out by hand above,
and the address is BIP-350's vector:

```text
Bech32m constant: 2bc830a3
expanded HRP of bc: 3 3 0 2 3
version word: 1 = p
first key words: 15 6 31 6 12 31
key words: 52
polymod with six zeros: 2ea27aec
xor 0x2bc830a3: 056a4a4f
Bech32m checksum: 2 22 20 18 18 15 = zk5jj0
xor 1: 2ea27aed
Bech32 checksum: 23 10 4 30 23 13 = h2y7hd
address: bc1p0xlxvlhemja6c4dqv22uapctqupfhlxm9h8z3k2e72q4k9hcz7vqzk5jj0
equals BIP-350's vector: true
variant: Bech32m
decode: bc, version 1, 32 bytes
program: 79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798
regtest: bcrt1p0xlxvlhemja6c4dqv22uapctqupfhlxm9h8z3k2e72q4k9hcz7vqc8gma6
variant of the Bech32 form: Bech32
the Bech32 form: wrong variant
P2WPKH BC1QW508D6...: bc, version 0, 20 bytes
re-encoded: bc1qw508d6qejxtdg4y5r3zarvary0c5xw7kv8f3t4
Bech32, ends in p
  state before the p: 00000000
  0 q inserted: Bech32, BIP-173 program 32 bytes
  1 q inserted: Bech32, BIP-173 program 33 bytes
  2 q inserted: Bech32, BIP-173 program bad padding
Bech32m, ends in p
  state before the p: 165d86d5
  1 q inserted, state before the p: 2bc830a2
  1 q inserted: bad checksum
91 characters: too long
tb1p...q47Zagq: mixed case
'o' in bc1p38j9...jow4: bad format
last character changed: bad checksum
tc1p...: unknown HRP
bc1gmk9yu: no version
version 17: bad version
v0 with Bech32m: wrong variant
5 spare bits: bad padding
a spare bit set: bad padding
1-byte program: bad length
41-byte program: bad length
16-byte v0 program: bad v0 length
32 bytes into 31: capacity
encode version 17: bad version
encode v0, 32 bytes: 62
encode v0, 31 bytes: bad v0 length
```

What the lines show:

- The hand-built checksums are `zk5jj0` for Bech32m and `h2y7hd` for
  Bech32, and the library's encoder gives the same address as BIP-350.
  On regtest the same key gives a `bcrt1p…` address with a different
  checksum, because the HRP is part of it.
- The Bech32 form of the Taproot address is a valid Bech32 string, and
  the decoder refuses it as the wrong variant for version 1.
- The P2WPKH vector is uppercase. It decodes as version 0 with a 20-byte
  program and re-encodes in lowercase.
- The Bech32 string ending in `p` reaches state 0 before its `p`. With one
  `q` inserted, it is still valid Bech32, and BIP-173's rules, which took
  Bech32 for every version, would read a 33-byte version 1 program: a
  different output. With two, the padding rule catches it.
- The Bech32m string ending in `p` reaches `0x165d86d5`. One `q` takes it
  to `0x2bc830a2`, and the string refuses.
- Each refusal code appears once, with BIP-350's invalid vectors where
  there is one. The 91-character string is the 41-byte vector with
  15 `q`s added.

The book's checks run this program in debug and release builds and compare
the output with [bitcoin/fixtures/bech32m.out](../fixtures/bech32m.out).

## How the code is checked

### Checks in the example

The checks are in
[bitcoin/examples/checks/bech32m.coax](../examples/checks/bech32m.coax),
which loads the example and
[bech32m_vectors.coax](../../coax/tests/bitcoin/bech32m_vectors.coax).
[make_bech32m_vectors.py](../../coax/tests/bitcoin/make_bech32m_vectors.py)
writes that file by copying strings, scriptPubKeys and reasons from the
pinned BIP-350 and BIP-173 texts and Bitcoin Core's `key_io_valid.json`
and `key_io_invalid.json`, after checking each file's SHA-256. The checks:

- BIP-350's 7 valid Bech32m strings are Bech32m, decode and re-encode in
  lowercase, and refuse as Bech32;
- its 14 invalid Bech32m strings refuse;
- 71 segwit addresses: BIP-350's 8 valid and 15 invalid, BIP-173's 6
  valid and 10 invalid, and the 32 Bech32 and Bech32m addresses of
  `key_io_valid.json`. Each valid address decodes to the network, version
  and program its scriptPubKey or chain gives, and re-encodes in
  lowercase. Each invalid one refuses with the code for its BIP's reason;
- BIP-173 predates BIP-350, so its addresses for versions 1, 2 and 16 have
  Bech32 checksums. Under BIP-350 the three valid ones and three invalid
  ones refuse as `SW_WRONG_VARIANT`, and each valid one's program,
  re-encoded with Bech32m, decodes back;
- all 70 strings of `key_io_invalid.json` refuse;
- each of the Taproot address's 62 characters, replaced by each of the 31
  other alphabet characters, refuses as a bad checksum, or as bad format
  at the separator;
- 1 to 20 `q` characters inserted before the final `p` keep the Bech32
  string valid and break the Bech32m one;
- the worked example's states, from the pinned `segwit_addr.py`.

<!-- source: bitcoin/examples/checks/bech32m.coax check -->
```jai
// BIP-350's valid Bech32m strings decode as Bech32m, re-encode in
// lowercase, and refuse as Bech32.
#forall "every valid BIP-350 string is Bech32m and only Bech32m" i: 0..<7 {
  s := SW_BECH32M_VALID[i];
  hrp: [83] u8; words: [90] u8; text: [90] u8;
  #assert segwit_variant_vartime(s) == SW_BECH32M;
  #assert bech32_decode_checked(s, hrp[..], words[..], false, false).error == NC_INVALID;
  r := bech32m_decode_checked(s, hrp[..], words[..], false, false);
  #assert r.error == NC_OK && r.hrp_written + 1 + r.written + 6 == s.count;
  q := bech32m_encode_checked(cast(string) hrp[0..<r.hrp_written], cast(string) words[0..<r.written], false, text[..], false, false);
  #assert q.error == NC_OK && q.written == s.count;
  for k: 0..<s.count { #assert text[k] == b32_lower(s[k]); }
}
// BIP-350's invalid Bech32m strings refuse. The three with a byte outside
// printable ASCII are built from their listed bytes.
#forall "every invalid BIP-350 string refuses" i: 0..<14 {
  s: [8] u8; text := "";
  if i < 11 {
    text = SW_BECH32M_INVALID[i];
  } else {
    s[0] = SW_BECH32M_LEAD[i - 11];
    for k: 0..<7 { s[k + 1] = SW_BECH32M_TAIL[i - 11][k]; }
    text = cast(string) s[..];
  }
  hrp: [83] u8; words: [90] u8;
  #assert bech32m_decode_checked(text, hrp[..], words[..], false, false).error == NC_INVALID;
  #assert segwit_variant_vartime(text) < 0;
}
// Every segwit address in BIP-350, BIP-173 and key_io_valid.json decodes
// to its network, version and program, or refuses with the code for its
// BIP's reason. A valid address re-encodes in lowercase. A BIP-173 address
// for version 1 or later refuses as the wrong variant, and its program
// re-encodes with Bech32m to a different string that decodes back.
#forall "every segwit address vector" i: 0..<71 #steps 200000000 {
  c := SW_CASES[i];
  program: [41] u8; want: [41] u8; text: [90] u8;
  a := segwit_decode_vartime(c.text, program[..]);
  if c.code != 0 {
    #assert a.length == c.code && a.network == 0 && a.version == 0;
  } else {
    p := sw_unhex(c.program, want[..]);
    #assert a.length == p.count && a.network == c.network && a.version == c.version;
    #assert sw_same(program[0..<a.length], cast(string) p);
    n := segwit_encode_vartime(a.network, a.version, program[0..<a.length], text[..]);
    #assert n == c.text.count;
    for k: 0..<n { #assert text[k] == b32_lower(c.text[k]); }
  }
  if c.code == SW_WRONG_VARIANT && c.program.count > 0 {
    p := sw_unhex(c.program, want[..]);
    n := segwit_encode_vartime(c.network, c.version, p, text[..]);
    #assert n == c.text.count && segwit_variant_vartime(cast(string) text[0..<n]) == SW_BECH32M;
    #assert segwit_variant_vartime(c.text) == SW_BECH32;
    #assert sw_decodes(cast(string) text[0..<n], c.network, c.version, c.program);
  }
}
#forall "every string in key_io_invalid.json refuses" i: 0..<70 {
  out: [41] u8;
  code := segwit_decode_vartime(SW_KEY_IO_INVALID[i], out[..]).length;
  #assert code < 0 && sw_refuses(SW_KEY_IO_INVALID[i], code);
}
// Each character of the Taproot address replaced by each other character
// of the alphabet: BCH codes detect any 4 substitutions, so each of the
// 1,922 changes fails the checksum, except at the separator, where the
// string loses its '1'.
#forall "every one-character change refuses" at: 0..<62, by: 0..<32 #steps 200000000 {
  s: [62] u8;
  for k: 0..<62 { s[k] = TAPROOT_ADDRESS[k]; }
  if s[at] != B32_ALPHABET[by] {
    s[at] = B32_ALPHABET[by];
    want := ifx at == 2 then SW_BAD_FORMAT else SW_BAD_CHECKSUM;
    #assert sw_refuses(cast(string) s[..], want);
  }
}
// The weakness and its fix: up to 20 'q's in front of the final 'p' keep a
// Bech32 string valid, and any number of them breaks a Bech32m string.
#forall "q insertion before a final p" extra: 1..<21 {
  s: [90] u8; t: [90] u8;
  m := ENDS_IN_P_BECH32.count - 1;
  for k: 0..<m { s[k] = ENDS_IN_P_BECH32[k]; t[k] = ENDS_IN_P_BECH32M[k]; }
  for k: 0..<extra { s[m + k] = 'q'; t[m + k] = 'q'; }
  s[m + extra] = 'p'; t[m + extra] = 'p';
  #assert state_before(ENDS_IN_P_BECH32, 1) == 0;
  #assert segwit_variant_vartime(cast(string) s[0..<m + extra + 1]) == SW_BECH32;
  #assert segwit_variant_vartime(cast(string) t[0..<m + extra + 1]) == SW_BAD_CHECKSUM;
}
// The worked example's values, computed with the pinned segwit_addr.py:
// bech32_polymod(bech32_hrp_expand("bc") + [1] + convertbits(key, 8, 5)
// + [0] * 6) is 0x2ea27aec.
#test "the worked example" {
  key: [32] u8;
  #assert hex_decode_checked(TAPROOT_KEY, key[..], false).error == NC_OK;
  state := b32_prefix("bc");
  #assert state == b32_step(b32_step(b32_step(b32_step(b32_step(1, 3), 3), 0), 2), 3);
  state = b32_step(state, 1);
  for i: 0..<52 { state = b32_step(state, b32_word(cast(string) key[..], true, i)); }
  for i: 0..<6 { state = b32_step(state, 0); }
  #assert state == 0x2ea27aec && (state ^ B32_BECH32M) == 0x056a4a4f && (state ^ 1) == 0x2ea27aed;
  first := [6] u8.[15, 6, 31, 6, 12, 31];
  for i: 0..<6 { #assert b32_word(cast(string) key[..], true, i) == first[i]; }
  #assert state_before(ENDS_IN_P_BECH32M, 1) == 0x165d86d5;
}
```

To run them:

```shell
coax/bin/coax test bitcoin/examples/checks/bech32m.coax
coax/bin/coax mutate bitcoin/examples/checks/bech32m.coax --root-only
```

`coax test` runs 29 checks, including the loaded files' own checks, in
about 7 seconds on 2026-11-02; the one-character changes take most of
it. The checks file has no procedures of its own, so root-only mutation
finds no sites in it; the example's procedures were not mutated.

### The library's own checks

`segwit.coax` has 8 checks of its own: the check helpers against
deliberate differences, the program rules at every boundary, the network
names in both cases, BIP-350's valid addresses and Bitcoin Core regtest
addresses both ways, one decoding refusal per code with nothing written,
the encoding refusals, the variants at the case and length boundaries, and
every version from 0 to 17 with every program length from 0 to 41 encoded,
decoded back, and refused as the wrong variant once the checksum is
recomputed with the other constant.

`bech32-core.coax` gained a check of its own for this chapter: BIP-350's
valid and invalid Bech32m strings through `b32_scan`, `b32_check` and the
`bech32m_` procedures, each valid string refused by the Bech32 decoder and
re-encoded with the other constant, and `b32_encode_words` at the
90-character limit. Its existing BIP-173 checks pass unchanged.

```shell
coax/bin/coax test coax/lib/bitcoin/segwit.coax
coax/bin/coax mutate coax/lib/bitcoin/segwit.coax --root-only
coax/bin/coax mutate coax/lib/nostr/bech32-core.coax --root-only
```

On 2026-11-02, root-only mutation kills all 200 mutants in `segwit.coax`
and all 248 in `bech32-core.coax`, with no survivor.

### BIP-350's reference implementation, natively and in Wasm

[segwit.py](../../coax/tests/bitcoin/segwit.py) compiles `segwit.coax` to
C in checked and release builds and runs its three exported procedures
against BIP-350's reference Python implementation, `segwit_addr.py`,
pinned byte for byte and loaded only after its SHA-256 matches. The
reference gives no reasons, so a short classifier in the script applies
this chapter's rules in this chapter's order to name each refusal, and it
must agree with the reference on every accept and every decoded program.
The cases are:

- every string in BIP-350's and BIP-173's test-vector sections, and every
  Bech32 or Bech32m entry and invalid string of Bitcoin Core's key I/O
  files;
- every witness version from 0 to 17, program length from 0 to 41 and
  network, with seeded programs, plus networks and versions out of range
  and output buffers one character short;
- each valid address with its checksum recomputed with the other
  constant;
- each string above with one seeded edit: a character replaced, inserted
  or removed, its case flipped, or two characters swapped;
- 1, 2, 3, 5 and 8 `q`s inserted before the final `p` of 200 Bech32 and
  Bech32m strings that end in `p`;
- program buffers one byte short.

Every result, network, version and output must match, and nothing may be
written past the output. It then builds the same C for wasm32 and runs
every case again in Node. On 2026-11-02, all 23,338 cases agree natively
and in Wasm, in checked and release builds, in about 16 seconds.

```shell
python3 coax/tests/bitcoin/segwit.py
```

## What this chapter leaves out

- Address types: which scriptPubKey a version and program make, and
  P2PKH, P2SH, P2WPKH, P2WSH and P2TR side by side. That is
  chapter 29 in the [plan](PLAN.md).
- Error location. BIP-350 describes how a wallet can point at the likely
  wrong characters by trying both constants. This code only detects.
- Bech32 strings longer than 90 characters, such as Lightning invoices.
  The core still takes `extended` for NIP-19's 5,000-character limit; the
  segwit procedures always use 90.

## Summary

- Bech32 misses `q` characters inserted or deleted before a final `p`,
  because the polymod state before that `p` is 0 and a `q` keeps it 0.
- Bech32m changes only the final constant, to `0x2bc830a3`, which closes
  that gap and keeps Bech32's other guarantees.
- Version 0 addresses use Bech32 and versions 1 to 16 use Bech32m; the
  other checksum refuses.
- A segwit address is an HRP (`bc`, `tb` or `bcrt`), a version word and a
  2- to 40-byte program (20 or 32 for version 0), at most 90 characters in
  one case.
- Every BIP-350 and BIP-173 vector and every Bech32 entry of Bitcoin
  Core's key I/O files gives its expected result, and the code agrees with
  BIP-350's reference Python implementation natively and in Wasm.

## Dependencies

- [27. Bech32](ch027-bech32.md)

## Sources

- [BIP-350](https://github.com/bitcoin/bips/blob/master/bip-0350.mediawiki),
  pinned in
  [coax/tests/vectors/upstream/bip350/](../../coax/tests/vectors/upstream/bip350/bip-0350.mediawiki).
- [BIP-173](https://github.com/bitcoin/bips/blob/master/bip-0173.mediawiki),
  pinned in
  [coax/tests/vectors/upstream/bip173/](../../coax/tests/vectors/upstream/bip173/bip-0173.mediawiki).
- BIP-350's reference Python implementation, `ref/python/segwit_addr.py`
  in [sipa/bech32](https://github.com/sipa/bech32), pinned in
  [coax/tests/vectors/upstream/sipa-bech32/](../../coax/tests/vectors/upstream/sipa-bech32/segwit_addr.py.txt).
- Bitcoin Core `key_io_valid.json` and `key_io_invalid.json`, pinned in
  [coax/tests/vectors/upstream/bitcoin-core/](../../coax/tests/vectors/upstream/bitcoin-core/key_io_valid.json).
- [BIP-141](https://github.com/bitcoin/bips/blob/master/bip-0141.mediawiki),
  for the version 0 program lengths, and
  [BIP-341](https://github.com/bitcoin/bips/blob/master/bip-0341.mediawiki),
  for version 1.
