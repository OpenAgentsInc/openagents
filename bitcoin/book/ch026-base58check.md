# 26. Base58Check

Legacy Bitcoin addresses such as `1FsSia9r…` and `3…`, private keys in
wallet import format (WIF) such as `5Hp…` and `Kw…`, and BIP-32 extended
keys such as `xpub661M…` are all Base58Check strings. This chapter builds
Base58, the 4-byte SHA-256d checksum of chapter 5 that turns it into
Base58Check, and the table of version bytes that says what a payload is. It
decodes public strings in variable time and WIF keys in constant time, and
checks the result against Bitcoin Core's Base58 and key I/O vectors.

## What this chapter teaches

- The 58-character alphabet, and why it leaves out `0`, `O`, `I` and `l`.
- Base58 as a big number in base 58, with one `1` for each leading zero
  byte, and why every text has exactly one decoding.
- Base58Check: the payload, then the first 4 bytes of its SHA-256d.
- The version bytes: `0x00` and `0x05` for P2PKH and P2SH on mainnet,
  `0x6f` and `0xc4` on the test networks, and `0x80` and `0xef` for WIF,
  with the `0x01` flag for a compressed key.
- The refusals: a character outside the alphabet, a wrong checksum, a
  payload of the wrong length, an unknown version and a bad compression
  flag.
- Why a WIF decoder must run in constant time and an address decoder need
  not, and what this chapter's constant-time code does and doesn't claim.

## Where Bitcoin uses Base58Check

| String | Payload | Chapter |
| --- | --- | --- |
| P2PKH address, `1…` (`m…` or `n…` on the test networks) | version, then HASH160 of a public key | 6, 29 |
| P2SH address, `3…` (`2…` on the test networks) | version, then HASH160 of a redeem script | 6, 29 |
| WIF private key, `5…`, `K…` or `L…` (`9…` or `c…` on the test networks) | version, the 32-byte key, and `0x01` for a compressed key | 18 |
| BIP-32 extended key, `xpub…` or `xprv…` | 4 version bytes and 74 bytes of key data | 32 |

Segwit addresses (`bc1…`) use Bech32 and Bech32m instead (chapters 27 and
28). This chapter stops at the encoding and the version table. Chapter 29
turns a decoded hash into a scriptPubKey, and chapter 32 reads the fields
of an extended key.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [base58.coax](../../coax/lib/bitcoin/base58.coax) | `base58_encode_vartime(data, out)` | bytes to Base58 text |
| | `base58_decode_vartime(text, out)` | Base58 text to bytes |
| | `base58check_encode_vartime(payload, out)` | adds the checksum and encodes |
| | `base58check_decode_vartime(text, out)` | decodes and checks the checksum |
| | `base58_address_encode_vartime(kind, hash, out)` | a P2PKH or P2SH address for a 20-byte hash |
| | `base58_address_decode_vartime(text, hash_out)` | the kind and hash of an address |
| | `wif_encode_ct(key, compressed, testnet, work, out)` | a WIF key, in constant time |
| | `wif_decode_ct(text, work, key_out)` | the key and kind of a WIF, in constant time |
| [hash.coax](../../coax/lib/bitcoin/hash.coax) | `bitcoin_checksum_checked`, `bitcoin_checksum_matches` | the first 4 bytes of SHA-256d (chapter 5) |

Every procedure writes into a caller-owned buffer and returns the number of
bytes or characters written, a kind, or one negative refusal:
`B58_BAD_CHAR`, `B58_BAD_CHECKSUM`, `B58_BAD_LENGTH`,
`B58_UNKNOWN_VERSION`, `B58_BAD_FLAG`, `B58_CAPACITY` (the output buffer is
too small) or `B58_TOO_LONG` (over 1,024 bytes). A refusal writes nothing
to the output.

## The alphabet

Base58 writes a number with 58 digits:

```text
123456789ABCDEFGHJKLMNPQRSTUVWXYZabcdefghijkmnopqrstuvwxyz
```

These are the ten digits, the 26 capital letters and the 26 small letters,
without four characters that look alike in many fonts: `0` (zero), `O`
(capital o), `I` (capital i) and `l` (small L). Base58 also has no `+`, `/`
or `=`, unlike base64 (chapter 1), so an address is one word that a double
click selects whole, and it survives a URL or an email line break. The
digits stay in ASCII order, so the digit `1` is 0, `A` is 9, `a` is 33 and
`z` is 57.

The comment at the top of Bitcoin Core's `base58.h`, from the first Bitcoin
release, gives these reasons: characters that look alike, no punctuation
that breaks a line, and a double click that selects the whole string.

## Base58: bytes as a number

Base58 reads its input as one big-endian number and writes that number in
base 58, most significant digit first. A number has no leading zeros, so
the leading zero bytes would be lost. Base58 keeps them: each leading zero
byte becomes one `1`, the digit for 0. Only leading zeros get this
treatment; a zero byte after the first nonzero byte is part of the number.

### Worked example: 01 00 and 00 00 01

The two bytes `01 00` are the number 256. Divide by 58:

```text
256 = 4 * 58 + 24
```

The digits are 4 and 24, most significant first. The alphabet's 4th
character (from 0) is `5`, and its 24th is `R`, so `01 00` is `5R`.

The three bytes `00 00 01` are two zero bytes, then the number 1. The zero
bytes give `11`, and 1 gives the digit `2`, so the text is `112`. Without
the rule, `00 00 01`, `00 01` and `01` would all be `2`.

### Encoding

The encoder skips the leading zero bytes, then builds the base-58 digits
of the rest one byte at a time: for each byte it multiplies the digits so
far by 256, adds the byte, and carries in base 58. This is the method
Bitcoin Core's `EncodeBase58` uses, and it takes time proportional to the
square of the length.

<!-- source: coax/lib/bitcoin/base58.coax encode -->
```jai
// Base58 of data into out: one '1' for each leading zero byte, then the
// rest of data as a base-58 number, most significant digit first. Returns
// the number of characters written.
base58_encode_vartime :: (data: [] u8, out: [] u8) -> int #export_c {
  if data.count > B58_MAX_BYTES { return B58_TOO_LONG; }
  zeros := 0;
  while zeros < data.count && data[zeros] == 0 { zeros += 1; }
  digits: [B58_MAX_TEXT] u8; used := 0;           // base-58 digits, least significant first
  for i: zeros..<data.count {
    carry := cast(int) data[i];                   // digits = digits * 256 + data[i]
    for k: 0..<used {
      carry += cast(int) digits[k] * 256;
      digits[k] = cast(u8) (carry % 58);
      carry /= 58;
    }
    while carry > 0 { digits[used] = cast(u8) (carry % 58); used += 1; carry /= 58; }
  }
  if out.count < zeros + used { return B58_CAPACITY; }
  for i: 0..<zeros { out[i] = '1'; }
  for k: 0..<used { out[zeros + k] = B58_ALPHABET[digits[used - 1 - k]]; }
  return zeros + used;
}
```

### Decoding

The decoder runs the same loop the other way: it counts the leading `1`s,
then for each remaining character it multiplies the bytes so far by 58 and
adds the digit. It refuses a character outside the alphabet as soon as it
reaches one, and stops as soon as the value passes 1,024 bytes.

<!-- source: coax/lib/bitcoin/base58.coax decode -->
```jai
// The bytes that the Base58 text names into out: one zero byte for each
// leading '1', then the rest of text as a base-58 number in as few bytes
// as it needs. Every text has exactly one decoding and re-encodes to itself.
// Returns the number of bytes written. Work stops once the value passes
// B58_MAX_BYTES, so a long text costs no more than a short one.
base58_decode_vartime :: (text: string, out: [] u8) -> int #export_c {
  ones := 0;
  while ones < text.count && text[ones] == '1' { ones += 1; }
  bytes: [B58_MAX_BYTES] u8; used := 0;           // the value, least significant byte first
  for i: ones..<text.count {
    carry := b58_digit_vartime(text[i]);          // bytes = bytes * 58 + digit
    if carry < 0 { return B58_BAD_CHAR; }
    for k: 0..<used {
      carry += cast(int) bytes[k] * 58;
      bytes[k] = cast(u8) (carry & 255);
      carry >>= 8;
    }
    if carry > 0 {                                // below 58: one new byte at most
      if used == B58_MAX_BYTES { return B58_TOO_LONG; }
      bytes[used] = cast(u8) carry; used += 1;
    }
  }
  if ones + used > B58_MAX_BYTES { return B58_TOO_LONG; }
  if out.count < ones + used { return B58_CAPACITY; }
  for i: 0..<ones { out[i] = 0; }
  for k: 0..<used { out[ones + k] = bytes[used - 1 - k]; }
  return ones + used;
}
```

Every text has exactly one decoding, and every decoding re-encodes to the
text it came from. After the leading `1`s, the first digit is not 0, so
the number has no leading zero digit and its bytes have no leading zero
byte. The count of `1`s and the count of zero bytes are therefore the
same, both ways.

This decoder is stricter than Bitcoin Core's `DecodeBase58` in one way:
Core skips spaces before and after the text, and this decoder refuses
them as bad characters. Core's own tests check the skipping with the
string `" \t\n\v\f\r skip \r\f\v\n\t "`; this chapter leaves trimming to
the caller.

## The checksum

Base58Check appends the first 4 bytes of the payload's SHA-256d (chapter 5)
before encoding, and the decoder checks them before it returns the
payload. A text shorter than 4 bytes has no room for a checksum and
refuses as `B58_BAD_LENGTH`.

<!-- source: coax/lib/bitcoin/base58.coax check -->
```jai
// Base58Check: payload, then the first 4 bytes of its SHA-256d, in Base58.
base58check_encode_vartime :: (payload: [] u8, out: [] u8) -> int #export_c {
  if payload.count > B58_MAX_BYTES - 4 { return B58_TOO_LONG; }
  data: [B58_MAX_BYTES] u8;
  for i: 0..<payload.count { data[i] = payload[i]; }
  #assert bitcoin_checksum_checked(payload, data[payload.count..<payload.count + 4], false) == SHA_OK;
  return base58_encode_vartime(data[0..<payload.count + 4], out);
}
// The payload of a Base58Check text into out, after the checksum matches.
// Returns the payload's length.
base58check_decode_vartime :: (text: string, out: [] u8) -> int #export_c {
  data: [B58_MAX_BYTES] u8;
  n := base58_decode_vartime(text, data[..]);
  if n < 0 { return n; }
  if n < 4 { return B58_BAD_LENGTH; }
  if !bitcoin_checksum_matches(data[0..<n - 4], data[n - 4..<n]) { return B58_BAD_CHECKSUM; }
  if out.count < n - 4 { return B58_CAPACITY; }
  for i: 0..<n - 4 { out[i] = data[i]; }
  return n - 4;
}
```

A typing error changes the decoded bytes, and the changed payload's
checksum matches the changed checksum bytes with probability 2^-32, about
one in four billion. The checksum is a test against mistakes, not against
an attacker: anyone can compute a valid checksum for any payload.

### Worked example: an address

The first address in Bitcoin Core's `key_io_valid.json` is a P2PKH address
whose script holds the 20-byte hash `a31c06bd…6c080717`.

1. The payload is the version `0x00` followed by the hash:
   `00a31c06bd463e3923bc1aadbde48b16976c080717`, 21 bytes.
2. Its SHA-256d is `b874721228faecff…b65164fd`, so the checksum is
   `b8747212`.
3. The 25 bytes are `00a31c…0717b8747212`. The leading zero byte becomes
   `1`, and the remaining 24 bytes, as a number, give 33 more digits.
4. The address is `1FsSia9rv4NeEwvJ2GvXrX7LyxYspbN2mo`.

Every P2PKH address on mainnet starts with `1` for the same reason: its
version byte is zero. The example prints these values, and the checks
compare the SHA-256d with Python's `hashlib`.

## Version bytes

The first payload byte says what the rest is, and the payload's length
must match:

| Kind | Version | Payload | First character |
| --- | --- | --- | --- |
| P2PKH, mainnet | `0x00` | 21 bytes | `1` |
| P2SH, mainnet | `0x05` | 21 bytes | `3` |
| P2PKH, testnet, signet and regtest | `0x6f` | 21 bytes | `m` or `n` |
| P2SH, testnet, signet and regtest | `0xc4` | 21 bytes | `2` |
| WIF, mainnet | `0x80` | 33 bytes | `5` |
| WIF, mainnet, compressed | `0x80` | 34 bytes, the last `0x01` | `K` or `L` |
| WIF, testnet, signet and regtest | `0xef` | 33 bytes | `9` |
| WIF, testnet, signet and regtest, compressed | `0xef` | 34 bytes, the last `0x01` | `c` |

The test networks share their version bytes, so a Base58 string doesn't say
which test network it belongs to. The first character follows from the
version and the length: the example encodes each kind with an all-zero and
an all-`0xff` hash or key and prints the range between them.

BIP-32 extended keys use 4 version bytes and a 78-byte payload: `0488b21e`
(`xpub`) and `0488ade4` (`xprv`) on mainnet, and `043587cf` (`tpub`) and
`04358394` (`tprv`) on the test networks. This chapter decodes `xpub`
strings only as Base58Check payloads; chapter 32 reads their fields and
handles the private ones under the rules below.

<!-- source: coax/lib/bitcoin/base58.coax versions -->
```jai
// The kinds of Base58Check payload, with their version byte and payload
// size (the version byte included). Testnet, signet and regtest share the
// test versions. Kind 0 is unused.
B58_MAIN_P2PKH :: 1; B58_MAIN_P2SH :: 2; B58_TEST_P2PKH :: 3; B58_TEST_P2SH :: 4;
B58_MAIN_WIF :: 5; B58_MAIN_WIF_COMPRESSED :: 6; B58_TEST_WIF :: 7; B58_TEST_WIF_COMPRESSED :: 8;
B58_VERSIONS :: [9] int.[-1, 0x00, 0x05, 0x6f, 0xc4, 0x80, 0x80, 0xef, 0xef];
B58_SIZES :: [9] int.[0, 21, 21, 21, 21, 33, 34, 33, 34];
// The address kind (1 to 4) for a version byte, or B58_UNKNOWN_VERSION.
b58_address_kind :: (version: int) -> int {
  for kind: B58_MAIN_P2PKH..<B58_MAIN_WIF { if B58_VERSIONS[kind] == version { return kind; } }
  return B58_UNKNOWN_VERSION;
}
```

The address entries check the kind first and the length second: a 21-byte
payload with version `0x80` refuses as an unknown version, and a 22-byte
payload with version `0x00` refuses as a bad length. An empty payload has
no version byte and refuses as an unknown version.

<!-- source: coax/lib/bitcoin/base58.coax address -->
```jai
// A P2PKH or P2SH address of kind 1 to 4 for a 20-byte hash. Returns the
// number of characters written.
base58_address_encode_vartime :: (kind: int, hash: [] u8, out: [] u8) -> int #export_c {
  if kind < B58_MAIN_P2PKH || kind > B58_TEST_P2SH { return B58_UNKNOWN_VERSION; }
  if hash.count != 20 { return B58_BAD_LENGTH; }
  payload: [21] u8;
  payload[0] = cast(u8) B58_VERSIONS[kind];
  for i: 0..<20 { payload[1 + i] = hash[i]; }
  return base58check_encode_vartime(payload[..], out);
}
// The 20-byte hash of a P2PKH or P2SH address into hash_out. Returns its
// kind, 1 to 4. The version byte decides the kind; the payload must then be
// 21 bytes.
base58_address_decode_vartime :: (text: string, hash_out: [] u8) -> int #export_c {
  if hash_out.count != 20 { return B58_CAPACITY; }
  payload: [B58_MAX_BYTES] u8;
  n := base58check_decode_vartime(text, payload[..]);
  if n < 0 { return n; }
  kind := b58_address_kind(ifx n > 0 then cast(int) payload[0] else -1);   // empty: no version
  if kind == B58_UNKNOWN_VERSION { return kind; }
  if n != B58_SIZES[kind] { return B58_BAD_LENGTH; }
  for i: 0..<20 { hash_out[i] = payload[1 + i]; }
  return kind;
}
```

## WIF private keys and the constant-time rules

A WIF string is a private key. Chapter 3's rules apply: no branch, no
memory index and no variable-time operation may depend on it, and every
copy must be erased. The variable-time decoder above breaks all three. It
looks up each character with a loop that stops at the match, it stops
early on a bad character, and the number of bytes it works on grows with
the value. So WIF has its own encoder and decoder.

Two facts make a fixed amount of work possible. The payload is 33 or 34
bytes and its first byte is `0x80` or `0xef`, at least `0x80`, so the value
lies between 58^50 and 58^51 for a 33-byte payload (37 bytes with the
checksum) and between 58^51 and 58^52 for a 34-byte one. A WIF string is
therefore always 51 or 52 characters, never starts with `1`, and its
length says which payload size it holds. The length and the two flags,
compressed and testnet, are public: wallets show them, and they don't
reveal the key.

The building blocks compare without branching, as chapter 3's
`cb_ct_equal` does, and read every alphabet entry for every character:

<!-- source: coax/lib/bitcoin/base58.coax ct -->
```jai
// All ones when a == b, else 0. a and b are below 256, so a ^ b is at most
// 255 and (x + 255) >> 8 is 1 exactly when x is not 0.
b58_eq_mask :: (a: u64, b: u64) -> u64 {
  x := a ^ b;
  return value_barrier(sub_wrap(cast(u64) 0, ((x + 255) >> 8) ^ 1));
}
// The digit of c in the low 8 bits, and bit 8 set when c is not in the
// alphabet. It reads all 58 entries and never branches on c.
b58_digit_ct :: (c: u8) -> u64 {
  digit: u64 = 0; found: u64 = 0;
  for k: 0..<58 {
    m := b58_eq_mask(cast(u64) c, cast(u64) B58_ALPHABET[k]);
    digit |= m & cast(u64) k; found |= m;
  }
  return digit | (((found & 1) ^ 1) << 8);
}
// The character of a digit below 58, reading all 58 entries.
b58_char_ct :: (digit: u64) -> u8 {
  c: u64 = 0;
  for k: 0..<58 { c |= b58_eq_mask(digit, cast(u64) k) & cast(u64) B58_ALPHABET[k]; }
  return cast(u8) c;
}
```

The encoder divides the 37- or 38-byte number by 58 once for each of the
51 or 52 characters, each time over every byte. It replaces the division
with a multiply and a shift: for every `x` below 58 · 256 = 14,848,
`(x · 565) >> 15` equals `x / 58`, which the checks confirm for all 14,848
values. Some processors take longer to divide some numbers, and a multiply
by a constant doesn't. The decoder multiplies by 58 and adds each digit
over every byte, for every character, and records a bad character, an
overflow, a checksum mismatch, a wrong version and a bad flag as masks. It
branches once, at the end, on the result.

<!-- source: coax/lib/bitcoin/base58.coax wif -->
```jai
// A WIF private key: Base58Check of a version byte (0x80 mainnet, 0xef the
// test networks), the 32-byte key and, for a key whose public key is
// compressed, 0x01. The payload is 33 or 34 bytes with a first byte of at
// least 0x80, so the text is always 51 or 52 characters and never starts
// with '1'. Every loop runs a fixed number of times and every lookup reads
// the whole table; only the key's length and the two flags, which are
// public, choose a path. The secret intermediates live in work, which the
// caller owns, can register for erasure, and gets back as zeros.
WIF_WORK_BYTES :: 42;
wif_encode_ct :: (key: [] u8, compressed: bool, testnet: bool, work: [] u8, out: [] u8) -> int #export_c {
  if key.count != 32 { return B58_BAD_LENGTH; }
  size := ifx compressed then 38 else 37;         // payload and checksum
  chars := size + 14;                             // 51 or 52
  if out.count < chars || work.count < WIF_WORK_BYTES { return B58_CAPACITY; }
  b := work[0..<size];
  b[0] = ifx testnet then 0xef else 0x80;
  for i: 0..<32 { b[1 + i] = key[i]; }
  if compressed { b[33] = 1; }
  #assert bitcoin_checksum_checked(b[0..<size - 4], b[size - 4..<size], false) == SHA_OK;
  // chars divisions by 58, each over every byte, q = cur / 58 by multiply
  // and shift. The remainders are the digits, least significant first; the
  // last division leaves b all zeros, because the value is below 58^chars.
  for j: 0..<chars {
    rem: u64 = 0;
    for i: 0..<size {
      cur := add_wrap(mul_wrap(rem, 256), cast(u64) b[i]);
      q := mul_wrap(cur, 565) >> 15;
      rem = sub_wrap(cur, mul_wrap(q, 58));
      b[i] = cast(u8) q;
    }
    out[chars - 1 - j] = cast(u8) rem;
  }
  for j: 0..<chars { out[j] = b58_char_ct(cast(u64) out[j]); }
  return chars;
}
// The 32-byte key of a WIF into key_out. Returns its kind, 5 to 8:
// B58_MAIN_WIF, B58_MAIN_WIF_COMPRESSED, B58_TEST_WIF or
// B58_TEST_WIF_COMPRESSED. The text's length (51 or 52) is public and
// decides the payload size. Each check becomes a mask; the procedure
// branches once, at the end, on the result.
wif_decode_ct :: (text: string, work: [] u8, key_out: [] u8) -> int #export_c {
  if key_out.count != 32 || work.count < WIF_WORK_BYTES { return B58_CAPACITY; }
  if text.count != 51 && text.count != 52 { return B58_BAD_LENGTH; }
  size := text.count - 14;
  b := work[0..<size]; sum := work[size..<size + 4];
  for i: 0..<size { b[i] = 0; }
  bad: u64 = 0; over: u64 = 0;
  for i: 0..<text.count {
    d := b58_digit_ct(text[i]);
    bad |= d >> 8;
    carry := d & 255;                             // b = b * 58 + digit
    for k: 0..<size {
      j := size - 1 - k;
      carry = add_wrap(carry, mul_wrap(cast(u64) b[j], 58));
      b[j] = cast(u8) (carry & 255);
      carry = carry >> 8;
    }
    over |= carry;                                // more than size bytes
  }
  #assert bitcoin_checksum_checked(b[0..<size - 4], sum, false) == SHA_OK;
  diff: u64 = 0;
  for i: 0..<4 { diff |= cast(u64) (sum[i] ^ b[size - 4 + i]); }
  mainnet := b58_eq_mask(cast(u64) b[0], 0x80); test := b58_eq_mask(cast(u64) b[0], 0xef);
  bad_flag: u64 = 0;                              // 1 when a compressed key's flag is not 0x01
  if size == 38 { bad_flag = (b58_eq_mask(cast(u64) b[33], 1) & 1) ^ 1; }
  result := B58_MAIN_WIF + (size - 37) + 2 * cast(int) (test & 1);
  if bad != 0 { result = B58_BAD_CHAR; }
  else if over != 0 { result = B58_BAD_LENGTH; }
  else if diff != 0 { result = B58_BAD_CHECKSUM; }
  else if (mainnet | test) == 0 { result = B58_UNKNOWN_VERSION; }
  else if bad_flag != 0 { result = B58_BAD_FLAG; }
  if result >= B58_MAIN_WIF { for i: 0..<32 { key_out[i] = b[1 + i]; } }
  for i: 0..<size + 4 { work[i] = 0; }
  return result;
}
```

The secret intermediates live in `work`, a 42-byte buffer the caller owns.
The caller can register it with chapter 3's `zeroize.coax`, and both
procedures leave the bytes they used as zeros. The encoder uses 37 or 38
bytes, and its last division leaves the number zero. The decoder uses 41
or 42, the payload and the computed checksum, and clears them before it
returns. It also clears the payload bytes first, so a buffer left dirty by
an earlier call doesn't change the result.

What these procedures don't claim:

- The result says which check failed. For a string that isn't a valid key,
  the refusal code tells a caller which step refused it. For a valid key
  the path is the same every time.
- `key_out` and the encoder's `out` hold the secret afterwards. Erasing them
  is the caller's job.
- The secret-path audit of chapter 3 doesn't follow arrays or loops, so it
  can't check these procedures, and this chapter hasn't run the timing
  harness or the machine-code check on them. The constant-time property
  rests on the source: fixed loop bounds, full-table reads, wrapping
  arithmetic and `value_barrier` on every mask.
- The SHA-256d of the payload runs in constant time for a fixed length, as
  chapter 4's SHA-256 does.
- The decoder doesn't check that the key is in `[1, n-1]`. Chapter 18's
  key checks do that, and Bitcoin Core's `DecodeSecret` does the same
  after decoding.

### Decoding public data

Addresses, P2SH hashes and extended public keys are public, so their
decoder may branch, look up and stop early on its input. Every procedure
that may do so ends in `_vartime`. Give these procedures only public
strings. The address decoder refuses a WIF as an unknown version, but by
then it has already read the key in variable time.

## The example

The example is [bitcoin/examples/base58check.coax](../examples/base58check.coax).
It works the two small numbers and the address by hand, prints the version
table, runs Bitcoin Core's three vector files and BIP-32's extended public
keys, changes each character of the address in turn, and then shows the
refusals. Its vector table,
[coax/tests/bitcoin/base58_vectors.coax](../../coax/tests/bitcoin/base58_vectors.coax),
is copied from the pinned files by
[make_base58_vectors.py](../../coax/tests/bitcoin/make_base58_vectors.py),
which refuses a file whose SHA-256 differs from its pin. The WIF keys in
those files are published test keys.

<!-- source: bitcoin/examples/base58check.coax example -->
```jai
#load "../../coax/lib/bitcoin/base58.coax";
#load "../../coax/lib/nostr/hex.coax";
#load "../../coax/tests/bitcoin/base58_vectors.coax";
#import "Basic";

// Print a label and bytes as lowercase hex.
show :: (label: string, bytes: [] u8) {
  text: [2100] u8;
  r := hex_encode_checked(cast(string) bytes, text[..], false);
  print("%: %\n", label, cast(string) text[0..<r.written]);
}
// Decode hex into out and return the filled part. Every literal here and
// in the vector table is valid lowercase hex, so an error stops the program.
decode :: (text: string, out: [] u8) -> [] u8 {
  r := hex_decode_checked(text, out, false);
  #assert r.error == 0;
  return out[0..<r.written];
}
equal_bytes :: (a: [] u8, b: [] u8) -> bool {
  if a.count != b.count { return false; }
  for i: 0..<a.count { if a[i] != b[i] { return false; } }
  return true;
}
// The name of a refusal.
REASONS :: [7] string.["bad character", "bad checksum", "bad length", "unknown version", "bad flag", "capacity", "too long"];
reason :: (status: int) -> string { return ifx status < 0 && status >= -7 then REASONS[-status - 1] else "accepted"; }
KINDS :: [9] string.["", "P2PKH", "P2SH", "testnet P2PKH", "testnet P2SH",
  "WIF", "WIF, compressed", "testnet WIF", "testnet WIF, compressed"];

main :: () {
  // Base58 by hand: 0x0100 is 256 = 4 * 58 + 24, the digits 4 and 24.
  print("Base58 by hand\n");
  bytes: [1024] u8; text: [1400] u8;
  n := base58_encode_vartime(decode("0100", bytes[..]), text[..]);
  print("0100 = 256 = 4 * 58 + 24 -> '%' '%' -> %\n", cast(string) B58_ALPHABET[4..<5], cast(string) B58_ALPHABET[24..<25], cast(string) text[0..<n]);
  n = base58_encode_vartime(decode("000001", bytes[..]), text[..]);
  print("000001: two zero bytes, then 1 -> %\n", cast(string) text[0..<n]);

  // Base58Check by hand: the first address in key_io_valid.json.
  print("Base58Check by hand\n");
  payload: [21] u8; digest: [32] u8;
  payload[0] = 0x00;
  hash := decode("a31c06bd463e3923bc1aadbde48b16976c080717", payload[1..<21]);
  show("version || hash", payload[..]);
  #assert sha256d_checked(payload[..], digest[..], false) == SHA_OK;
  show("SHA-256d", digest[..]);
  show("checksum", digest[0..<4]);
  n = base58check_encode_vartime(payload[..], text[..]);
  print("Base58Check: %\n", cast(string) text[0..<n]);
  out: [20] u8;
  kind := base58_address_decode_vartime(cast(string) text[0..<n], out[..]);
  print("decodes to: %, hash equal: %\n", KINDS[kind], equal_bytes(out[..], hash));

  // The version table, each kind with the hash or key of all 0x00 or all 0xff.
  print("version bytes\n");
  zeros: [32] u8; ones: [32] u8; work: [WIF_WORK_BYTES] u8;
  for i: 0..<32 { ones[i] = 0xff; }
  for k: 1..<9 {
    a: int; b: int;
    if k < B58_MAIN_WIF {
      a = base58_address_encode_vartime(k, zeros[0..<20], text[..]);
      b = base58_address_encode_vartime(k, ones[0..<20], text[700..<800]);
    } else {
      compressed := k % 2 == 0; testnet := k >= B58_TEST_WIF;
      a = wif_encode_ct(zeros[..], compressed, testnet, work[..], text[..]);
      b = wif_encode_ct(ones[..], compressed, testnet, work[..], text[700..<800]);
    }
    v := B58_VERSIONS[k]; digits := "0123456789abcdef"; vh: [2] u8; vh[0] = digits[v / 16]; vh[1] = digits[v % 16];
    print("0x% %: % bytes, % to %\n", cast(string) vh[..], KINDS[k], B58_SIZES[k],
      cast(string) text[0..<a], cast(string) text[700..<700 + b]);
  }

  // Bitcoin Core's base58_encode_decode.json, both ways.
  print("Bitcoin Core base58_encode_decode.json\n");
  agree := 0;
  for i: 0..<B58_CODEC.count {
    c := B58_CODEC[i];
    data := decode(c.hex, bytes[..]);
    e := base58_encode_vartime(data, text[..]);
    back: [1024] u8;
    d := base58_decode_vartime(c.text, back[..]);
    if e >= 0 && cast(string) text[0..<e] == c.text && d >= 0 && equal_bytes(back[0..<d], data) { agree += 1; }
  }
  print("% of % encode and decode exactly\n", agree, B58_CODEC.count);

  // key_io_valid.json: every P2PKH and P2SH address and every WIF.
  print("Bitcoin Core key_io_valid.json, Base58 entries\n");
  by_kind: [9] int; good: [9] int;
  for i: 0..<B58_VALID.count {
    v := B58_VALID[i]; test := v.chain != "main";
    want: [64] u8; got: [32] u8; ok := false; kind = 0;
    data := decode(v.data, want[..]);
    if v.key {
      kind = wif_decode_ct(v.text, work[..], got[..]);
      e := wif_encode_ct(got[..], v.compressed, test, work[..], text[..]);
      expected := B58_MAIN_WIF + ifx v.compressed then 1 else 0;
      if test { expected += 2; }
      ok = kind == expected && equal_bytes(got[..], data) && e > 0 && cast(string) text[0..<e] == v.text;
    } else {
      // A P2PKH script is 76 a9 14 <hash> 88 ac, a P2SH script a9 14 <hash> 87 (chapter 29).
      p2pkh := data.count == 25;
      script_hash := ifx p2pkh then data[3..<23] else data[2..<22];
      kind = base58_address_decode_vartime(v.text, got[0..<20]);
      expected := ifx p2pkh then B58_MAIN_P2PKH else B58_MAIN_P2SH;
      if test { expected += 2; }
      e := base58_address_encode_vartime(expected, script_hash, text[..]);
      ok = kind == expected && equal_bytes(got[0..<20], script_hash) && e > 0 && cast(string) text[0..<e] == v.text;
    }
    if kind > 0 { by_kind[kind] += 1; if ok { good[kind] += 1; } }
  }
  for k: 1..<9 { print("%: % of % decode and re-encode exactly\n", KINDS[k], good[k], by_kind[k]); }

  // key_io_invalid.json: every string refuses as an address and as a WIF.
  print("Bitcoin Core key_io_invalid.json\n");
  as_address: [8] int; as_wif: [8] int; refused := 0;
  for i: 0..<B58_INVALID.count {
    s := B58_INVALID[i]; got: [32] u8;
    a := base58_address_decode_vartime(s, got[0..<20]);
    w := wif_decode_ct(s, work[..], got[..]);
    if a < 0 && w < 0 { refused += 1; as_address[-a] += 1; as_wif[-w] += 1; }
  }
  for r: 1..<6 { print("%: % as an address, % as a WIF\n", REASONS[r - 1], as_address[r], as_wif[r]); }
  print("% of % refused by both decoders\n", refused, B58_INVALID.count);

  // BIP-32 extended public keys: 78-byte payloads, version 0488b21e.
  print("BIP-32 extended public keys\n");
  xpubs := 0;
  for i: 0..<B58_XPUBS.count {
    m := base58check_decode_vartime(B58_XPUBS[i], bytes[..]);
    e := base58check_encode_vartime(bytes[0..<78], text[..]);
    if m == 78 && bytes[0] == 0x04 && bytes[1] == 0x88 && bytes[2] == 0xb2 && bytes[3] == 0x1e && cast(string) text[0..<e] == B58_XPUBS[i] { xpubs += 1; }
  }
  print("% of % decode to 78 bytes with version 0488b21e and re-encode exactly\n", xpubs, B58_XPUBS.count);

  // One changed character anywhere in the address of the worked example.
  address := "1FsSia9rv4NeEwvJ2GvXrX7LyxYspbN2mo";
  changed: [8] int;
  for at: 0..<address.count {
    copy: [34] u8;
    for i: 0..<34 { copy[i] = address[i]; }
    d := b58_digit_vartime(copy[at]);
    copy[at] = B58_ALPHABET[(d + 1) % 58];
    changed[-base58_address_decode_vartime(cast(string) copy[..], out[..])] += 1;
  }
  print("one character changed, at each of % places: % bad checksum, % bad length\n", address.count, changed[2], changed[3]);

  // Refusals.
  print("refusals\n");
  print("0 in place of o: %\n", reason(base58_address_decode_vartime("1FsSia9rv4NeEwvJ2GvXrX7LyxYspbN2m0", out[..])));
  print("l in place of 1: %\n", reason(base58_address_decode_vartime("lFsSia9rv4NeEwvJ2GvXrX7LyxYspbN2mo", out[..])));
  print("last character changed: %\n", reason(base58_address_decode_vartime("1FsSia9rv4NeEwvJ2GvXrX7LyxYspbN2mp", out[..])));
  print("first character dropped: %\n", reason(base58_address_decode_vartime("FsSia9rv4NeEwvJ2GvXrX7LyxYspbN2mo", out[..])));
  print("a 3-byte value, too short for a checksum: %\n", reason(base58check_decode_vartime("zzzz", bytes[..])));
  print("an xpub as an address: %\n", reason(base58_address_decode_vartime(B58_XPUBS[0], out[..])));
  print("a WIF as an address: %\n", reason(base58_address_decode_vartime(WIF_ONE, out[..])));
  key: [32] u8;
  print("an address as a WIF: %\n", reason(wif_decode_ct(address, work[..], key[..])));
  print("version 0x81: %\n", reason(wif_decode_ct(WIF_81, work[..], key[..])));
  print("compressed flag 0x02: %\n", reason(wif_decode_ct(WIF_FLAG_2, work[..], key[..])));
  print("34 characters into a 33-character buffer: %\n", reason(base58_address_encode_vartime(1, hash, text[0..<33])));
}
```

Run it from the repository root:

```shell
coax/bin/coax run bitcoin/examples/base58check.coax
```

The output:

```text
Base58 by hand
0100 = 256 = 4 * 58 + 24 -> '5' 'R' -> 5R
000001: two zero bytes, then 1 -> 112
Base58Check by hand
version || hash: 00a31c06bd463e3923bc1aadbde48b16976c080717
SHA-256d: b874721228faecff56d7aea9498ea767498ae08530fa2748ff5c0b9cb65164fd
checksum: b8747212
Base58Check: 1FsSia9rv4NeEwvJ2GvXrX7LyxYspbN2mo
decodes to: P2PKH, hash equal: true
version bytes
0x00 P2PKH: 21 bytes, 1111111111111111111114oLvT2 to 1QLbz7JHiBTspS962RLKV8GndWFwi5j6Qr
0x05 P2SH: 21 bytes, 31h1vYVSYuKP6AhS86fbRdMw9XHieotbST to 3R2cuenjG5nFubqX9Wzuukdin2YfBbQ6Kw
0x6f testnet P2PKH: 21 bytes, mfWxJ45yp2SFn7UciZyNpvDKrzbhyfKrY8 to n4rZHAPGXCu8bYchjzJhK3V7VVreascJxe
0xc4 testnet P2SH: 21 bytes, 2MsFDzHRUAMpjHxKyoEHU3aMCMsVtMqs1PV to 2NGapyPiksYHc7PU4pecnXhcyzNkq1wvz5p
0x80 WIF: 33 bytes, 5HpHagT65TZzG1PH3CSu63k8DbpvD8s5ip4nEB3kEsreAbuatmU to 5Km2kuu7vtFDPpxywn4u3NLu8iSdrqhxWT8tUKjeEXs2f9yxoWz
0x80 WIF, compressed: 34 bytes, KwDiBf89QgGbjEhKnhXJuH7LrciVrZi3qYjgd9M7rFU73Nd2Mcv1 to L5oLkpV3aqBjhki6LmvChTCq73v9gyymzzMpBbhDLjDpKCuAXpsi
0xef testnet WIF: 33 bytes, 91avARGdfge8E4tZfYLoxeJ5sGBdNJQH4kvjJoQFacbgwi1C2GD to 93XfLeifX7KMMtUGa7xouxtrnNoM21F9rPzqYx69aGc5SDGXUKD
0xef testnet WIF, compressed: 34 bytes, cMahea7zqjxrtgAbB7LSGbcQUr1uX1ojuat9jZodMN87J7g8rY9t to cWALDjUu1tszsCBMjBjL4mhtjHDZMS5U52WHJ29iqqspZx2Er6J2
Bitcoin Core base58_encode_decode.json
21 of 21 encode and decode exactly
Bitcoin Core key_io_valid.json, Base58 entries
P2PKH: 3 of 3 decode and re-encode exactly
P2SH: 3 of 3 decode and re-encode exactly
testnet P2PKH: 8 of 8 decode and re-encode exactly
testnet P2SH: 8 of 8 decode and re-encode exactly
WIF: 2 of 2 decode and re-encode exactly
WIF, compressed: 2 of 2 decode and re-encode exactly
testnet WIF: 6 of 6 decode and re-encode exactly
testnet WIF, compressed: 6 of 6 decode and re-encode exactly
Bitcoin Core key_io_invalid.json
bad character: 35 as an address, 0 as a WIF
bad checksum: 12 as an address, 1 as a WIF
bad length: 7 as an address, 63 as a WIF
unknown version: 16 as an address, 3 as a WIF
bad flag: 0 as an address, 3 as a WIF
70 of 70 refused by both decoders
BIP-32 extended public keys
17 of 17 decode to 78 bytes with version 0488b21e and re-encode exactly
one character changed, at each of 34 places: 34 bad checksum, 0 bad length
refusals
0 in place of o: bad character
l in place of 1: bad character
last character changed: bad checksum
first character dropped: bad checksum
a 3-byte value, too short for a checksum: bad length
an xpub as an address: unknown version
a WIF as an address: unknown version
an address as a WIF: bad length
version 0x81: unknown version
compressed flag 0x02: bad flag
34 characters into a 33-character buffer: capacity
```

What the output shows:

- The hand calculations: `01 00` is `5R`, `00 00 01` is `112`, and the
  address's checksum is `b8747212`.
- The version table. The all-zero WIF keys and all-`0xff` keys are outside
  `[1, n-1]` and are not usable keys; they only bound the first
  characters.
- All 21 Base58 pairs of `base58_encode_decode.json`, both ways, including
  the empty string, 40 zero bytes as 40 `1`s, and the alphabet itself as
  the encoding of a 43-byte value.
- All 38 Base58 entries of `key_io_valid.json`: 22 addresses and 16 WIF
  keys on mainnet, testnet4, signet and regtest, each decoded to its kind
  and its script's hash or its key, and re-encoded exactly.
- All 70 strings of `key_io_invalid.json` refuse as an address and as a
  WIF. The table says which step refused each one. As an address, 35 have
  a character outside the alphabet (30 are Bech32 or Bech32m strings and 5
  look like them; each has a `0` or an `l`), 12 have a wrong checksum, 7
  are too short or have a known version with the wrong length, and 16 have
  a version that isn't an address version. As a WIF, 63 are not 51 or 52 characters long, and the
  rest have a wrong checksum, version or flag.
- All 17 extended public keys of BIP-32's test vectors decode to 78-byte
  payloads with the version `0488b21e`.
- Each of the address's 34 characters changed to the next one in the
  alphabet: all 34 refuse on the checksum.
- The refusals: a `0` and an `l`, a changed last character, a dropped
  first character (the leading zero byte disappears, so the bytes and the
  checksum shift), a 3-byte value, an `xpub` and a WIF given to the address
  decoder, an address given to the WIF decoder, version `0x81`, the flag
  `0x02` and an output buffer one character short.

The book's checks run this program in debug and release builds and compare
the output with [bitcoin/fixtures/base58check.out](../fixtures/base58check.out).

## How the code is checked

### Checks in the example

The checks are in
[bitcoin/examples/checks/base58check.coax](../examples/checks/base58check.coax),
which loads the example:

- every Bitcoin Core Base58 pair, both ways, with nothing written past the
  value;
- every Base58 entry of `key_io_valid.json`, decoded to its kind and hash
  or key and re-encoded, with each WIF also decoded by the variable-time
  Base58Check path to the same payload, so the two implementations agree;
- every string of `key_io_invalid.json`, refused by both decoders with the
  output unwritten;
- every BIP-32 extended public key;
- each of the address's 34 characters replaced by each of the 57 other
  alphabet characters: all 1,938 strings refuse;
- the worked example's SHA-256d, from Python's `hashlib`;
- every byte value after zero to three zero bytes, with exactly one `1` per
  zero byte.

<!-- source: bitcoin/examples/checks/base58check.coax check -->
```jai
// Every pair in Bitcoin Core's base58_encode_decode.json encodes and
// decodes exactly, and decoding writes nothing past the value.
#forall "Bitcoin Core's Base58 pairs, both ways" i: 0..<21 #steps 200000000 {
  c := B58_CODEC[i]; data: [256] u8; text: [400] u8; back: [257] u8;
  d := decode(c.hex, data[..]);
  e := base58_encode_vartime(d, text[..]);
  #assert e == c.text.count && cast(string) text[0..<e] == c.text;
  back[d.count] = 7;
  #assert base58_decode_vartime(c.text, back[..]) == d.count && equal_bytes(back[0..<d.count], d) && back[d.count] == 7;
}
// The kind a key_io_valid.json entry must decode to, from its chain, its
// key flag and, for an address, its script's length.
expected_kind :: (v: B58Valid, script_bytes: int) -> int {
  test := ifx v.chain == "main" then 0 else 2;
  if v.key { return B58_MAIN_WIF + test + ifx v.compressed then 1 else 0; }
  return test + ifx script_bytes == 25 then B58_MAIN_P2PKH else B58_MAIN_P2SH;
}
#test "expected_kind maps chains, flags and scripts" {
  v := B58Valid.{chain="main"};
  #assert expected_kind(v, 25) == B58_MAIN_P2PKH && expected_kind(v, 23) == B58_MAIN_P2SH;
  v.chain = "signet"; #assert expected_kind(v, 25) == B58_TEST_P2PKH && expected_kind(v, 23) == B58_TEST_P2SH;
  v.key = true; #assert expected_kind(v, 0) == B58_TEST_WIF;
  v.compressed = true; #assert expected_kind(v, 0) == B58_TEST_WIF_COMPRESSED;
  v.chain = "main"; #assert expected_kind(v, 0) == B58_MAIN_WIF_COMPRESSED;
  v.compressed = false; #assert expected_kind(v, 0) == B58_MAIN_WIF;
}
// Every Base58 entry of key_io_valid.json decodes to its kind and its
// script's hash or its key, and re-encodes exactly. Each WIF also decodes
// through the public Base58Check path to the same payload, so the
// constant-time decoder and the variable-time one agree.
#forall "Bitcoin Core's valid addresses and keys" i: 0..<38 #steps 200000000 {
  v := B58_VALID[i]; want: [64] u8; got: [32] u8; text: [60] u8; work: [WIF_WORK_BYTES] u8;
  data := decode(v.data, want[..]);
  kind := expected_kind(v, data.count);
  if v.key {
    #assert wif_decode_ct(v.text, work[..], got[..]) == kind && equal_bytes(got[..], data);
    e := wif_encode_ct(got[..], v.compressed, kind >= B58_TEST_WIF, work[..], text[..]);
    #assert cast(string) text[0..<e] == v.text;
    payload: [64] u8;
    #assert base58check_decode_vartime(v.text, payload[..]) == B58_SIZES[kind];
    #assert cast(int) payload[0] == B58_VERSIONS[kind] && equal_bytes(payload[1..<33], data);
  } else {
    hash := ifx kind % 2 == 1 then data[3..<23] else data[2..<22];
    #assert base58_address_decode_vartime(v.text, got[0..<20]) == kind && equal_bytes(got[0..<20], hash);
    e := base58_address_encode_vartime(kind, hash, text[..]);
    #assert cast(string) text[0..<e] == v.text;
  }
}
// Every string in key_io_invalid.json refuses as an address and as a WIF,
// and neither decoder writes its output.
#forall "Bitcoin Core's invalid strings refuse" i: 0..<70 #steps 200000000 {
  out: [32] u8; work: [WIF_WORK_BYTES] u8;
  for k: 0..<32 { out[k] = 7; }
  #assert base58_address_decode_vartime(B58_INVALID[i], out[0..<20]) < 0;
  #assert wif_decode_ct(B58_INVALID[i], work[..], out[..]) < 0;
  for k: 0..<32 { #assert out[k] == 7; }
}
// Every BIP-32 extended public key is a 78-byte Base58Check payload with
// version 0488b21e, and re-encodes exactly.
#forall "BIP-32 xpubs" i: 0..<17 #steps 200000000 {
  payload: [80] u8; text: [120] u8;
  #assert base58check_decode_vartime(B58_XPUBS[i], payload[..]) == 78;
  #assert payload[0] == 0x04 && payload[1] == 0x88 && payload[2] == 0xb2 && payload[3] == 0x1e;
  e := base58check_encode_vartime(payload[0..<78], text[..]);
  #assert cast(string) text[0..<e] == B58_XPUBS[i];
}
// Each character of the worked example's address, replaced by every other
// character of the alphabet, fails: with 4 checksum bytes, a random change
// passes with probability 2^-32, and none of these 1,938 does.
#forall "every one-character change refuses" at: 0..<34 #steps 400000000 {
  address := "1FsSia9rv4NeEwvJ2GvXrX7LyxYspbN2mo"; copy: [34] u8; out: [20] u8;
  for i: 0..<34 { copy[i] = address[i]; }
  for d: 0..<58 {
    if B58_ALPHABET[d] == address[at] { continue; }
    copy[at] = B58_ALPHABET[d];
    #assert base58_address_decode_vartime(cast(string) copy[..], out[..]) < 0;
  }
}
// The worked example's SHA-256d, computed with Python's hashlib.
#test "the worked example's checksum" {
  payload: [21] u8; digest: [32] u8; want: [32] u8;
  #assert sha256d_checked(decode("00a31c06bd463e3923bc1aadbde48b16976c080717", payload[..]), digest[..], false) == SHA_OK;
  #assert equal_bytes(digest[..], decode("b874721228faecff56d7aea9498ea767498ae08530fa2748ff5c0b9cb65164fd", want[..]));
}
// Every byte value, after zero to three zero bytes, round-trips, and the
// text starts with one '1' per zero byte and no more.
#forall "single bytes after leading zeros round-trip" value: 0..<256, zeros: 0..<4 {
  data: [4] u8; text: [8] u8; back: [4] u8;
  data[zeros] = cast(u8) value;
  e := base58_encode_vartime(data[0..<zeros + 1], text[..]);
  ones := 0; while ones < e && text[ones] == '1' { ones += 1; }
  #assert ones == ifx value == 0 then zeros + 1 else zeros;
  #assert base58_decode_vartime(cast(string) text[0..<e], back[..]) == zeros + 1 && equal_bytes(back[0..<zeros + 1], data[0..<zeros + 1]);
}
#test "equal_bytes notices a change at either end and a different length" {
  a := [3] u8.[1, 2, 3]; b := a;
  #assert equal_bytes(a[..], b[..]) && !equal_bytes(a[..], b[0..<2]) && !equal_bytes(a[0..<2], b[..]);
  b[0] = 9; #assert !equal_bytes(a[..], b[..]);
  b = a; b[2] = 9; #assert !equal_bytes(a[..], b[..]);
}
```

To run them:

```shell
coax/bin/coax test bitcoin/examples/checks/base58check.coax
coax/bin/coax mutate bitcoin/examples/checks/base58check.coax --root-only
```

`coax test` runs 71 checks, including the loaded files' own checks, in
about 20 seconds on 2026-11-02. Root-only mutation kills all 13 mutants in
the checks file; the example's own procedures were not mutated for this
chapter.

### The library's own checks

`base58.coax` has 59 checks of its own, including those of the files it
loads: the alphabet and its order, the constant-time digit and character
lookups against the table for all 256 byte values, the multiply-and-shift
division for all 14,848 inputs, encoding and decoding at every limit,
Base58Check and the address entries with each refusal, and the WIF entries
in all four forms, for the key 1 and the key `n - 1`, with every refusal
and the work buffer checked after each call.

```shell
coax/bin/coax test coax/lib/bitcoin/base58.coax
coax/bin/coax mutate coax/lib/bitcoin/base58.coax --root-only
```

Root-only mutation kills all 428 mutants on 2026-11-02, with no survivor.

### Python, natively and in Wasm

[base58.py](../../coax/tests/bitcoin/base58.py) compiles `base58.coax` to C
in checked and release builds and runs every procedure it exports against
a separate Python implementation, which uses Python integers and
`hashlib` and shares no code with Coax. Its 11,942 cases are:

- every pair, entry and string of the three Bitcoin Core files and every
  BIP-32 extended public key;
- the limits: 1,024 and 1,025 bytes in, and texts that decode to 1,024 and
  1,025 bytes;
- 300 seeded random byte strings with up to three leading zeros, encoded
  with and without the checksum;
- 200 seeded random keys in all four WIF forms, and the same keys with a
  random flag byte or a random version;
- each string above with one seeded edit, a character replaced, inserted
  or removed, including `0`, `O`, `I`, `l`, spaces and bytes outside ASCII,
  each through all four decoders.

Every result and output must match the Python implementation's, and the
WIF procedures must leave `work` zero. It then builds the same C for
wasm32 and runs every case again in Node. On 2026-11-02 all 11,942 cases
agreed in the native checked and release builds and the Wasm checked and
release builds; the run took 31 seconds.

```shell
python3 coax/tests/bitcoin/base58.py
```

## What this chapter leaves out

- Address types themselves: which script a P2PKH or P2SH hash goes into,
  and the segwit addresses. That is chapter 29.
- The fields of a BIP-32 extended key and the rules of BIP-32's test
  vector 5. That is chapter 32.
- The key range check, which is chapter 18's.
- Two older decoders in the Nostr library read Base58Check for their own
  purposes: `a3_base58` in
  [nipa3.coax](../../coax/lib/nostr/nipa3.coax), for a 25-byte address in
  a NIP-A3 payment target, and the extended-key reader in
  [nipkeys.coax](../../coax/lib/nostr/nipkeys.coax). Both use the same
  carry method as this chapter's decoder, refuse a bad character and check
  that the count of `1`s equals the count of zero bytes. They are not
  changed here; moving them onto `base58.coax` is later work.

## Summary

- Base58 writes bytes as a base-58 number with an alphabet that leaves out
  `0`, `O`, `I` and `l`, and writes each leading zero byte as `1`. Every
  text has one decoding.
- Base58Check appends the first 4 bytes of the payload's SHA-256d. A random
  change passes with probability 2^-32.
- The version byte says what the payload is: `0x00`, `0x05`, `0x6f` and
  `0xc4` for addresses, `0x80` and `0xef` for WIF keys, with `0x01` after a
  compressed key.
- Public strings decode in variable time. WIF keys encode and decode in
  constant time with a fixed length, full-table lookups and a caller-owned
  work buffer that returns as zeros.
- Bitcoin Core's Base58 and key I/O vectors and BIP-32's extended public
  keys pass, and 11,942 cases agree with Python, natively and in Wasm.

## Dependencies

- [5. SHA-256d and tagged hashes](ch005-sha-256d-and-tagged-hashes.md)

## Sources

- Bitcoin Core `src/base58.cpp` (`EncodeBase58`, `DecodeBase58` and the
  Base58Check pair) and `src/key_io.cpp` (`EncodeSecret`, `DecodeSecret`
  and the address encoders), and the version bytes in
  `src/kernel/chainparams.cpp`, pinned in
  [coax/tests/vectors/upstream/bitcoin-core/](../../coax/tests/vectors/upstream/bitcoin-core/chainparams.cpp).
- Bitcoin Core `base58_encode_decode.json`, `key_io_valid.json` and
  `key_io_invalid.json` at revision `ddea9196`, pinned in
  [coax/tests/vectors/upstream/bitcoin-core/](../../coax/tests/vectors/upstream/bitcoin-core/base58_encode_decode.json)
  and listed in [SOURCES.md](../../coax/tests/vectors/SOURCES.md).
- [BIP-13](https://github.com/bitcoin/bips/blob/master/bip-0013.mediawiki)
  (P2SH addresses) and
  [BIP-32](https://github.com/bitcoin/bips/blob/master/bip-0032.mediawiki)
  (extended keys and their version bytes), pinned in
  [coax/tests/vectors/upstream/bip32/](../../coax/tests/vectors/upstream/bip32/bip-0032.mediawiki).
