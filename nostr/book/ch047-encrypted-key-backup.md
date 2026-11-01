# 47. Encrypted key backup

NIP-49 encrypts a private key with a password so that it can be stored or
moved as text. The result is an `ncryptsec` string: 91 bytes in Bech32 that
hold the scrypt cost, a salt, a nonce, a key-security byte and the
XChaCha20-Poly1305 ciphertext of the key. This chapter takes the string
apart, derives the key from the password, opens the box, and shows which
step refuses which mistake. The payload's `log_n` is untrusted input, so it
is bounded before any memory is set aside for scrypt.

## What this chapter teaches

- The 91-byte payload: version `0x02`, `log_n`, a 16-byte salt, a 24-byte
  nonce, the key-security byte, 32 bytes of ciphertext and a 16-byte tag.
- The password: normalized to Unicode NFKC, so that the same password typed
  on another system gives the same bytes.
- scrypt with `r = 8`, `p = 1` and `N = 2^log_n`: PBKDF2-HMAC-SHA256,
  ROMix over Salsa20/8 BlockMix, and PBKDF2 again, to a 32-byte key. ROMix
  needs `128 · r · N` bytes, so `log_n` is the memory cost.
- XChaCha20-Poly1305: an HChaCha20 subkey from the first 16 nonce bytes, a
  one-time Poly1305 key from block 0, and a tag over the associated data
  (the key-security byte) and the ciphertext.
- Poly1305, one block worked by hand.
- Which mistakes refuse where: Bech32, the version, `log_n`, the
  key-security byte, and the tag for a wrong password or any changed byte.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [scrypt_nip49.coax](../../coax/lib/crypto/scrypt_nip49.coax) | `nip49_scrypt_start`, `nip49_scrypt_step`, `nip49_scrypt_finish` | resumable scrypt with `r = 8`, `p = 1`, `log_n` 1 to 18, in caller memory |
| | `nip49_scrypt_params(password, salt, memory, log_n, overlap)` | admission: refuses before any work |
| [scrypt_romix.coax](../../coax/lib/crypto/scrypt_romix.coax) | `romix_step` | at most `ROMIX_MAX_STEP` (64) BlockMix steps per call |
| [xchacha20_poly1305.coax](../../coax/lib/crypto/xchacha20_poly1305.coax) | `xcp_seal`, `xcp_open` | the AEAD; `xcp_open` writes nothing unless the tag matches |
| [poly1305.coax](../../coax/lib/crypto/poly1305.coax) | `poly1305_checked(key, msg, out)` | the one-time MAC, in five 26-bit limbs |
| [nfkc.coax](../../coax/lib/text/nfkc.coax) | `un_nfkc(input, scratch, out)` | Unicode 18.0.0 NFKC, tables generated from the UCD |
| [bech32-core.coax](../../coax/lib/nostr/bech32-core.coax) | `bech32_decode_checked` | chapter 15's Bech32, with the extended length that 162 characters need |

The library has no `ncryptsec` import yet. The example assembles it from
these pieces in three procedures, `ncs_decode`, `ncs_parse` and `ncs_open`,
and a `derive` loop over the resumable scrypt.

## The payload

NIP-49's test vector is

```text
ncryptsec1qgg9947rlpvqu76pj5ecreduf9jxhselq2nae2kghhvd5g7dgjtcxfqtd67p9m0w57lspw8gsq6yphnm8623nsl8xn9j4jdzz84zm3frztj3z7s35vpzmqf6ksu8r89qk5z2zxfmu5gv8th8wclt0h4p
```

with the password `nostr`.

### Worked example: the bytes of the vector

The string has 162 characters: the HRP `ncryptsec`, the separator `1`, 146
data characters and 6 checksum characters. 162 is more than the 90
characters that BIP-173 allows, so the decoder runs in chapter 15's extended
mode.

146 words of 5 bits are 730 bits: 91 bytes and 2 bits left over, which must
be zero. The first two characters, `q` and `g`, are the words 0 and 8:

```text
q = 00000   g = 01000   ->   00000010 00...
```

so the first byte is `0x02`, the version. Reading on gives:

| Offset | Bytes | Field | Value in the vector |
| ---: | ---: | --- | --- |
| 0 | 1 | version | `02` |
| 1 | 1 | `log_n` | `10`, so `N = 2^16` |
| 2 | 16 | salt | `52d7c3f8580e7b41953381e5bc49646b` |
| 18 | 24 | nonce | `c33f02a7dcaac8bdd8da23cd449783240b6ebc12edeea7bf` |
| 42 | 1 | key-security byte | `00`: the key was handled insecurely |
| 43 | 32 | ciphertext | `b8e88034...022d813a` |
| 75 | 16 | Poly1305 tag | `b438719ca0b504a1193be510c3aee776` |

`1 + 1 + 16 + 24 + 1 + 48 = 91`. The key-security byte is `0x00` if the key
is known to have been handled insecurely, `0x01` if not, and `0x02` if the
client doesn't track it. Any other value refuses.

`ncs_decode` regroups the words and checks the padding. `ncs_parse` checks
the version, `log_n` and the key-security byte, then copies the fields:

<!-- source: nostr/examples/ncryptsec.coax layout -->
```jai
// The 91 bytes of a version 2 payload.
NCS_BYTES :: 91;
// Refusal codes.
NCS_OK :: 0; NCS_BECH32 :: 1; NCS_PREFIX :: 2; NCS_LENGTH :: 3;
NCS_VERSION :: 4; NCS_LOG_N :: 5; NCS_SECURITY :: 6; NCS_TAG :: 7;
Ncryptsec :: struct {
  log_n: int;
  salt: [16] u8;
  nonce: [24] u8;
  security: u8;     // 0 handled insecurely, 1 not known to be, 2 not tracked
  box: [48] u8;     // 32 ciphertext bytes, then the 16-byte Poly1305 tag
}
NcsParsed :: struct { error: int; value: Ncryptsec; }

// The bech32 text to its 91 payload bytes: HRP "ncryptsec", 146 five-bit
// words regrouped into bytes, and the 2 left-over bits zero.
ncs_decode :: (text: string, out: [] u8) -> int {
  hrp: [83] u8; words: [160] u8;
  r := bech32_decode_checked(text, hrp[..], words[..], true, false);
  if r.error != NC_OK { return NCS_BECH32; }
  if cast(string) hrp[0..<r.hrp_written] != "ncryptsec" { return NCS_PREFIX; }
  if r.written != 146 || out.count != NCS_BYTES { return NCS_LENGTH; }
  acc: u32; bits := 0; at := 0;
  for i: 0..<r.written {
    acc = ((acc << 5) | cast(u32) words[i]) & 0xfff; bits += 5;
    if bits >= 8 { bits -= 8; out[at] = cast(u8) ((acc >> bits) & 255); at += 1; }
  }
  if (acc & ((cast(u32) 1 << bits) - 1)) != 0 { return NCS_LENGTH; }
  return NCS_OK;
}

// The payload's fields. A log_n outside 1..18, the library's limit, refuses
// here, before anything is allocated for scrypt.
ncs_parse :: (payload: [] u8) -> NcsParsed {
  p: NcsParsed;
  if payload.count != NCS_BYTES { p.error = NCS_LENGTH; return p; }
  if payload[0] != 0x02 { p.error = NCS_VERSION; return p; }
  if nip49_scrypt_cost(cast(int) payload[1]) == 0 { p.error = NCS_LOG_N; return p; }
  if payload[42] > 2 { p.error = NCS_SECURITY; return p; }
  p.value.log_n = cast(int) payload[1];
  for i: 0..<16 { p.value.salt[i] = payload[2 + i]; }
  for i: 0..<24 { p.value.nonce[i] = payload[18 + i]; }
  p.value.security = payload[42];
  for i: 0..<48 { p.value.box[i] = payload[43 + i]; }
  return p;
}
```

`log_n` is checked with `nip49_scrypt_cost`, which returns 0 outside 1 to
18. That check reads one byte; nothing has been allocated yet.

## The password

NIP-49 normalizes the password to NFKC before scrypt. Its example password
is the four code points U+212B (ANGSTROM SIGN), U+2126 (OHM SIGN), U+1E9B
U+0323 (long s with dot above, then combining dot below), 11 bytes of
UTF-8. NFKC maps the Angstrom sign to U+00C5 and the Ohm sign to U+03A9, and
turns the long s with its two dots into U+1E69 (s with dot below and dot
above): 7 bytes, `c3 85 ce a9 e1 b9 a9`.
Without the normalization, a password typed with a different input method
gives different bytes and a different key.

## scrypt

`SYMMETRIC_KEY = scrypt(password, salt, N = 2^log_n, r = 8, p = 1)`, 32
bytes:

1. PBKDF2-HMAC-SHA256 with one iteration turns the password and salt into
   `128 · r = 1024` bytes, `B`.
2. ROMix fills `N` blocks of 1024 bytes: `V[i] = X`, then `X =
   BlockMix(X)`. BlockMix runs Salsa20/8 over the sixteen 64-byte pieces of
   `X`.
3. ROMix then runs `N` more steps. Each reads the last 64 bytes of `X` as
   an integer `j`, sets `X = BlockMix(X xor V[j mod N])`.
4. PBKDF2-HMAC-SHA256 with one iteration, the password and `X` as the
   salt, gives the 32-byte key.

Step 2 is why scrypt costs memory: `V` is `1024 · N` bytes, 64 MiB for
`log_n = 16` and 256 MiB for 18. The memory address in step 3 depends on
the password, so this is not a constant-time construction; NIP-49 accepts
that.

`nip49_scrypt_step` runs at most 64 BlockMix steps per call, so a caller
can spread the `2N` steps over frames and stop between calls. `derive`
runs them in a loop:

<!-- source: nostr/examples/ncryptsec.coax open -->
```jai
// Decrypt with the 32-byte scrypt key. The key-security byte is the
// associated data, so changing it also fails the tag. On a refusal the
// output is not written.
ncs_open :: (n: Ncryptsec, key: [] u8, out: [] u8) -> int {
  aad: [1] u8; aad[0] = n.security;
  nonce := n.nonce; box := n.box;
  if !xcp_open(key, nonce[..], aad[..], box[..], out, false) { return NCS_TAG; }
  return NCS_OK;
}

// scrypt with r = 8, p = 1 and N = 2^log_n, in the caller's memory. ROMix
// runs its 2N BlockMix steps at most 64 per call, so a caller could stop.
derive :: (password: [] u8, salt: [] u8, log_n: int, memory: [] u8, out: [] u8) -> bool {
  state: [1] Nip49Scrypt;
  if !nip49_scrypt_start(state[..], password, salt, memory, log_n, false) { return false; }
  while state[0].romix[0].phase != ROMIX_DONE && nip49_scrypt_step(state[..], memory, ROMIX_MAX_STEP, false) {}
  // A failed step leaves ROMix unfinished, and finish then refuses.
  return nip49_scrypt_finish(state[..], out, false);
}
```

### The limit on log_n

The payload chooses `log_n`, so a hostile string could ask for `log_n =
22`, which is 4 GiB. The library refuses anything outside 1 to 18 (256 MiB)
and refuses memory shorter than `1024 · 2^log_n`, both before the first
byte of work. NIP-49 leaves the limit to the implementer; 18 is this
library's.

## XChaCha20-Poly1305

ChaCha20 (chapter 18) takes a 12-byte nonce. XChaCha20 takes 24: HChaCha20
mixes the key with the first 16 nonce bytes into a subkey, and ChaCha20 runs
under the subkey with the last 8 nonce bytes. A random 24-byte nonce is
safe to pick without a counter.

To seal, `xcp_seal`:

1. runs keystream block 0 and keeps its first 32 bytes as the one-time
   Poly1305 key;
2. XORs the plaintext with the keystream from block 1;
3. computes the tag over the associated data padded to 16 bytes, the
   ciphertext padded to 16 bytes, and both lengths as 8-byte little-endian
   numbers.

`xcp_open` recomputes the tag, compares it in constant time, and decrypts
into the output only if they match. For NIP-49, the associated data is the
key-security byte, so changing that byte is caught like a changed
ciphertext byte.

## Poly1305

Poly1305 splits its 32-byte key into `r` (the first 16 bytes, with 22 bits
cleared, "clamped") and `s` (the last 16). For each 16-byte block, it
appends a `0x01` byte, reads the 17 bytes as a little-endian number, adds
it to the accumulator, and multiplies by `r` modulo `p = 2^130 - 5`. The
tag is `(accumulator + s) mod 2^128`, little-endian.

### Worked example: RFC 8439 section 2.5.2, block 1

The key is `85d6be78...4149f51b` and the message is `Cryptographic Forum
Research Group`, 34 bytes.

1. Clamping the first 16 bytes gives `r = 0x0806d5400e52447c036d555408bed685`.
   The last 16 bytes give `s = 0x1bf54941aff6bf4afdb20dfb8a800301`.
2. Block 1 is `Cryptographic Fo`. With `0x01` appended, read little-endian,
   it is `0x016f4620636968706172676f7470797243`.
3. The accumulator starts at 0, so `acc + block` is the block. Times `r`,
   modulo `p`, it is `0x2c88c77849d64ae9147ddeb88e69c83fc`, the value the
   RFC prints.
4. A 16-byte message would stop here. Its tag is the low 128 bits of `acc
   + s`, `0xe481c0c64d5b6ddc458ff984711c86fd`, which is
   `fd861c7184f98f45dc6d5b4dc6c081e4` as little-endian bytes.

Two more blocks (`rum Research Gro` and `up`, which is short and gets its
`0x01` after 2 bytes) give the RFC's tag `a8061dc1305136c6c22b8baf0c0127a9`.
The example prints both tags.

The code holds the accumulator in five limbs of 26 bits, so that a limb
times a limb of `r`, times 5 for the wrapped terms, fits in 64 bits. The
final `poly_reduce` subtracts `p` once with a mask, never with a branch.

## The example

The example is [nostr/examples/ncryptsec.coax](../examples/ncryptsec.coax).
It tags the RFC message, normalizes NIP-49's password, decodes and parses
the vector, runs scrypt at `log_n = 4`, shows the admission limits, opens
the vector, and shows ten refusals. Above the listings it defines `show`,
`bytes_of` and `unhex`, small print and copy helpers.

scrypt at the vector's `log_n = 16` needs a 64 MiB workspace. The library
never allocates it: `nip49_scrypt_start` and `nip49_scrypt_step` take `V`,
`128 · r · N` bytes, as a `[] u8` from the caller and refuse a view that is
too small. `coax0` has no heap, so a Coax program's arrays live on the
stack, 8 MiB by default on Linux and macOS, and macOS can't raise it past
64 MiB. A host allocates the workspace instead, with `malloc` in C
platform code on Linux or Apple. The book runs each example as a
plain Coax program, with no host to hand it a heap buffer, so this example
takes the 32-byte scrypt output for `nostr` and the vector's salt,
`KEY_NOSTR`, from OpenSSL's scrypt through Python's `hashlib.scrypt`, and
runs its own scrypt at `log_n = 4`, which needs 16 KiB. "How the code is
checked" shows the Coax scrypt deriving `KEY_NOSTR` itself, from a C host
that owns the 64 MiB.

<!-- source: nostr/examples/ncryptsec.coax example -->
```jai
error_name :: (e: int) -> string {
  if e == NCS_OK { return "ok"; }
  if e == NCS_BECH32 { return "bad bech32"; }
  if e == NCS_PREFIX { return "not ncryptsec"; }
  if e == NCS_LENGTH { return "bad length"; }
  if e == NCS_VERSION { return "unknown version"; }
  if e == NCS_LOG_N { return "log_n outside 1..18"; }
  if e == NCS_SECURITY { return "bad key-security byte"; }
  return "tag mismatch";
}
// Parse a payload, open it with a key, and say whether the output was written.
try_open :: (label: string, payload: [] u8, key: [] u8) {
  out: [32] u8;
  for i: 0..<32 { out[i] = 0xa5; }
  p := ncs_parse(payload);
  e := p.error;
  if e == NCS_OK { e = ncs_open(p.value, key, out[..]); }
  untouched := true;
  for v: out { if v != 0xa5 { untouched = false; } }
  print("%: %, output untouched %\n", label, error_name(e), untouched);
}

// NIP-49's published vector, password "nostr", log_n 16.
VECTOR :: "ncryptsec1qgg9947rlpvqu76pj5ecreduf9jxhselq2nae2kghhvd5g7dgjtcxfqtd67p9m0w57lspw8gsq6yphnm8623nsl8xn9j4jdzz84zm3frztj3z7s35vpzmqf6ksu8r89qk5z2zxfmu5gv8th8wclt0h4p";
// scrypt(password, the vector's salt, N = 2^16, r = 8, p = 1), 32 bytes,
// from OpenSSL through Python's hashlib.scrypt. N = 2^16 needs a 64 MiB
// workspace, more than the stack that coax0 gives a program; a host passes
// it from the heap (coax/tests/nostr/ncryptsec_vector.sh derives this key).
KEY_NOSTR :: "6d1e8b279f52b3a9ba60a2b43cab50804a67ac763ff4bafa782475efb0722aff";
KEY_WRONG :: "aaa80aa968b73767fbf6055b1aa804df774a66180d02d54b67cabded04055212"; // "Nostr"

main :: () {
  // One Poly1305 block by hand: RFC 8439 section 2.5.2.
  rfc: [32] u8;
  unhex("85d6be7857556d337f4452fe42d506a80103808afb0db2fd4abff6af4149f51b", rfc[..]);
  text: [34] u8; msg := bytes_of("Cryptographic Forum Research Group", text[..]);
  tag: [16] u8;
  #assert poly1305_checked(rfc[..], msg[0..<16], tag[..], false);
  show("Poly1305 tag of the first 16 bytes", tag[..]);
  #assert poly1305_checked(rfc[..], msg, tag[..], false);
  show("Poly1305 tag of all 34 bytes", tag[..]);

  // The password: NIP-49's NFKC example, U+212B U+2126 U+1E9B U+0323.
  scratch: [64] u32; normal: [32] u8; typed: [11] u8;
  unhex("e284abe284a6e1ba9bcca3", typed[..]);
  n := un_nfkc(cast(string) typed[..], scratch[..], normal[..], false);
  print("NFKC: error %, % bytes\n", n.error, n.written);
  show("NFKC bytes", normal[0..<n.written]);

  // The payload's layout.
  payload: [NCS_BYTES] u8;
  print("decode: %\n", error_name(ncs_decode(VECTOR, payload[..])));
  p := ncs_parse(payload[..]);
  salt := p.value.salt; nonce := p.value.nonce; box := p.value.box;
  print("version %, log_n %, key-security byte %\n", payload[0], p.value.log_n, p.value.security);
  show("salt", salt[..]);
  show("nonce", nonce[..]);
  show("ciphertext", box[0..<32]);
  show("tag", box[32..<48]);

  // scrypt at a small cost, against hashlib.scrypt.
  memory: [16384] u8; small: [32] u8; word: [5] u8;
  #assert derive(bytes_of("nostr", word[..]), salt[..], 4, memory[..], small[..]);
  show("scrypt(\"nostr\", salt, 2^4)", small[..]);
  print("log_n 16 admitted with 64 MiB: %\n", nip49_scrypt_params(5, 16, 67108864, 16, false));
  print("log_n 16 with one byte less: %\n", nip49_scrypt_params(5, 16, 67108863, 16, false));
  print("log_n 18, the limit, with 256 MiB: %\n", nip49_scrypt_params(5, 16, 268435456, 18, false));
  print("log_n 22 with 4 GiB: %\n", nip49_scrypt_params(5, 16, 4294967296, 22, false));

  // Decrypt the vector with the key scrypt gives for "nostr".
  key: [32] u8; secret: [32] u8;
  unhex(KEY_NOSTR, key[..]);
  print("open: %\n", error_name(ncs_open(p.value, key[..], secret[..])));
  show("private key", secret[..]);

  // Refusals.
  wrong: [32] u8; unhex(KEY_WRONG, wrong[..]);
  try_open("password \"Nostr\"", payload[..], wrong[..]);
  bad := payload; bad[0] = 0x01;
  try_open("version 1", bad[..], key[..]);
  bad = payload; bad[1] = 22;
  try_open("log_n 22", bad[..], key[..]);
  bad = payload; bad[1] = 0;
  try_open("log_n 0", bad[..], key[..]);
  bad = payload; bad[42] = 0x01;
  try_open("key-security byte 0 changed to 1", bad[..], key[..]);
  bad = payload; bad[42] = 0x03;
  try_open("key-security byte 3", bad[..], key[..]);
  bad = payload; bad[60] ^= 0x01;
  try_open("ciphertext byte 17 flipped", bad[..], key[..]);
  bad = payload; bad[90] ^= 0x80;
  try_open("tag byte 15 flipped", bad[..], key[..]);
  typo: [162] u8;
  for i: 0..<162 { typo[i] = VECTOR[i]; }
  typo[20] = 'q';
  print("one character changed: %\n", error_name(ncs_decode(cast(string) typo[..], payload[..])));
  print("an npub: %\n", error_name(ncs_decode("npub180cvv07tjdrrgpa0j7j7tmnyl2yr6yr7l8j4s3evf6u64th6gkwsyjh6w6", payload[..])));
}
```

Run it from the repository root:

```shell
coax/bin/coax run nostr/examples/ncryptsec.coax
```

The output has the hand-worked tag of block 1, the RFC's full tag, the NFKC
bytes from NIP-49, the field values from the table above, and the published
private key `3501...8683`:

```text
Poly1305 tag of the first 16 bytes: fd861c7184f98f45dc6d5b4dc6c081e4
Poly1305 tag of all 34 bytes: a8061dc1305136c6c22b8baf0c0127a9
NFKC: error 0, 7 bytes
NFKC bytes: c385cea9e1b9a9
decode: ok
version 2, log_n 16, key-security byte 0
salt: 52d7c3f8580e7b41953381e5bc49646b
nonce: c33f02a7dcaac8bdd8da23cd449783240b6ebc12edeea7bf
ciphertext: b8e8803440de7b3e9519c3e734cb2ac9a211ea2dc52312e5117a11a3022d813a
tag: b438719ca0b504a1193be510c3aee776
scrypt("nostr", salt, 2^4): 63587b915af5343363dbe8f9a811b1b7149d3866b40203a576e1ac7a50fd3e47
log_n 16 admitted with 64 MiB: true
log_n 16 with one byte less: false
log_n 18, the limit, with 256 MiB: true
log_n 22 with 4 GiB: false
open: ok
private key: 3501454135014541350145413501453fefb02227e449e57cf4d3a3ce05378683
password "Nostr": tag mismatch, output untouched true
version 1: unknown version, output untouched true
log_n 22: log_n outside 1..18, output untouched true
log_n 0: log_n outside 1..18, output untouched true
key-security byte 0 changed to 1: tag mismatch, output untouched true
key-security byte 3: bad key-security byte, output untouched true
ciphertext byte 17 flipped: tag mismatch, output untouched true
tag byte 15 flipped: tag mismatch, output untouched true
one character changed: bad bech32
an npub: not ncryptsec
```

`scrypt("nostr", salt, 2^4)` is the value `hashlib.scrypt` gives for the
same inputs.

The refusals:

- **Wrong password.** The key that scrypt gives for `Nostr` opens nothing:
  the tag fails, and `xcp_open` doesn't write the output buffer, which still
  holds its `0xa5` fill.
- **Version 1.** Only version `0x02` is defined; `ncs_parse` refuses before
  scrypt.
- **`log_n` 22 and 0.** Outside 1 to 18. 22 would need 4 GiB; the parser
  refuses it from one byte, and `nip49_scrypt_params` refuses it even with
  4 GiB offered.
- **Key-security byte changed from 0 to 1.** The byte is valid, but it is
  the associated data, so the tag fails. A value of 3 refuses in the parser.
- **A flipped ciphertext or tag bit.** The tag fails.
- **One changed character.** Bech32's checksum fails before any field is
  read.
- **An `npub`.** A valid Bech32 string with the wrong HRP.

The book's checks run this program in debug and release builds and compare
the output with [nostr/fixtures/ncryptsec.out](../fixtures/ncryptsec.out).
It runs in about 2 seconds, compile included.

## How the code is checked

### Checks in the example

The checks are in
[nostr/examples/checks/ncryptsec.coax](../examples/checks/ncryptsec.coax),
which loads the example. They compare the code with values from outside it:

- the payload bytes, decoded from the published string by a separate Python
  decoder, and the Bech32 re-encoding of the payload, which must be the
  published string;
- the published private key, and sealing it again with the vector's nonce,
  which must give the published ciphertext and tag;
- one flipped bit in each of bytes 18 to 90 (nonce, key-security byte,
  ciphertext, tag): each refuses and leaves the output alone. The salt and
  `log_n` (bytes 1 to 17) are protected differently: changing them changes
  the key that scrypt derives, which the fixed `KEY_NOSTR` doesn't model;
- every version, `log_n` and key-security value from 0 to 255;
- a changed character at each of the 152 data and checksum positions;
- all 32 values of the last word, of which only those with zero padding
  bits decode;
- key-security bytes 1 and 2 sealed as associated data;
- scrypt for `log_n` 1 to 6 against `hashlib.scrypt`;
- NIP-49's NFKC example;
- the RFC 8439 block-1 accumulator plus `s`, added byte by byte.

<!-- source: nostr/examples/checks/ncryptsec.coax check -->
```jai
// True when bytes equal the hex text.
hex_is :: (got: [] u8, want: string) -> bool {
  b: [128] u8;
  if got.count > 128 || want.count != 2 * got.count { return false; }
  unhex(want, b[0..<got.count]);
  for i: 0..<got.count { if got[i] != b[i] { return false; } }
  return true;
}
#test "hex_is refuses a different first or last byte and a wrong length" {
  b: [129] u8; text: [258] u8;
  b[0] = 0x35; b[1] = 0x01;
  #assert hex_is(b[0..<2], "3501") && !hex_is(b[0..<2], "3401") && !hex_is(b[0..<2], "3502");
  #assert !hex_is(b[0..<2], "35") && !hex_is(b[0..<1], "3501");
  for i: 0..<258 { text[i] = '0'; }
  b[0] = 0; b[1] = 0;
  #assert hex_is(b[0..<128], cast(string) text[0..<256]);
  #assert !hex_is(b[..], cast(string) text[..]);
}
// The vector's 91 payload bytes, decoded by hand in Python from the
// published bech32 text: 02, 10 (log_n 16), salt, nonce, 00, box.
PAYLOAD :: "021052d7c3f8580e7b41953381e5bc49646bc33f02a7dcaac8bdd8da23cd449783240b6ebc12edeea7bf00b8e8803440de7b3e9519c3e734cb2ac9a211ea2dc52312e5117a11a3022d813ab438719ca0b504a1193be510c3aee776";
PRIVATE :: "3501454135014541350145413501453fefb02227e449e57cf4d3a3ce05378683";
vector :: (out: [] u8) { #assert ncs_decode(VECTOR, out) == NCS_OK; }

#test "the vector decodes to the hand-decoded payload and re-encodes to itself" {
  payload: [NCS_BYTES] u8; text: [200] u8;
  vector(payload[..]);
  #assert hex_is(payload[..], PAYLOAD);
  r := bech32_encode_checked("ncryptsec", cast(string) payload[..], true, text[..], true, false);
  #assert r.error == NC_OK && cast(string) text[0..<r.written] == VECTOR;
}

#test "the vector opens to the published key, and sealing it gives the published box" {
  payload: [NCS_BYTES] u8; key: [32] u8; secret: [32] u8; box: [48] u8;
  vector(payload[..]);
  p := ncs_parse(payload[..]);
  #assert p.error == NCS_OK && p.value.log_n == 16 && p.value.security == 0;
  unhex(KEY_NOSTR, key[..]);
  #assert ncs_open(p.value, key[..], secret[..]) == NCS_OK;
  #assert hex_is(secret[..], PRIVATE);
  nonce := p.value.nonce; aad: [1] u8;
  #assert xcp_seal(key[..], nonce[..], aad[..], secret[..], box[..], false);
  #assert hex_is(box[..], "b8e8803440de7b3e9519c3e734cb2ac9a211ea2dc52312e5117a11a3022d813ab438719ca0b504a1193be510c3aee776");
}

// The nonce, the key-security byte, the ciphertext and the tag are all
// covered by the tag: one flipped bit in any of payload bytes 18..90
// refuses and leaves the output alone. (The salt and log_n are covered
// differently: changing them changes the key that scrypt derives.)
#forall "a flipped bit after the salt fails the tag or the parse" i: 18..<91 #steps 1000000000 {
  payload: [NCS_BYTES] u8; key: [32] u8; out: [32] u8;
  vector(payload[..]); unhex(KEY_NOSTR, key[..]);
  payload[i] ^= cast(u8) (1 << (i % 8));
  for j: 0..<32 { out[j] = 0xa5; }
  p := ncs_parse(payload[..]);
  #assert p.error != NCS_OK || ncs_open(p.value, key[..], out[..]) == NCS_TAG;
  for v: out { #assert v == 0xa5; }
}

#forall "every version but 2 refuses" v: 0..<256 {
  payload: [NCS_BYTES] u8; vector(payload[..]);
  payload[0] = cast(u8) v;
  #assert (ncs_parse(payload[..]).error == NCS_VERSION) == (v != 2);
}
#forall "log_n 1..18 parses and every other value refuses" n: 0..<256 {
  payload: [NCS_BYTES] u8; vector(payload[..]);
  payload[1] = cast(u8) n;
  #assert (ncs_parse(payload[..]).error == NCS_LOG_N) == (n < 1 || n > 18);
}
#forall "key-security bytes 0, 1 and 2 parse" s: 0..<256 {
  payload: [NCS_BYTES] u8; vector(payload[..]);
  payload[42] = cast(u8) s;
  #assert (ncs_parse(payload[..]).error == NCS_SECURITY) == (s > 2);
}

// Bech32 finds any one changed character.
#forall "one changed character fails the checksum" i: 10..<162 #steps 1000000000 {
  typo: [162] u8; payload: [NCS_BYTES] u8;
  for j: 0..<162 { typo[j] = VECTOR[j]; }
  typo[i] = ifx typo[i] == 'q' then 'p' else 'q';
  #assert ncs_decode(cast(string) typo[..], payload[..]) == NCS_BECH32;
}
#test "wrong lengths, padding bits and prefixes refuse" {
  payload: [NCS_BYTES] u8; words: [146] u8; text: [200] u8;
  vector(payload[..]);
  r := bech32_encode_checked("ncryptsec", cast(string) payload[0..<90], true, text[..], true, false);
  #assert ncs_decode(cast(string) text[0..<r.written], payload[..]) == NCS_LENGTH;
  r = bech32_encode_checked("nsec", cast(string) payload[..], true, text[..], true, false);
  #assert ncs_decode(cast(string) text[0..<r.written], payload[..]) == NCS_PREFIX;
  short: [90] u8;
  #assert ncs_decode(VECTOR, short[..]) == NCS_LENGTH;
  long: [92] u8; vector(long[0..<91]);
  #assert ncs_parse(long[0..<90]).error == NCS_LENGTH && ncs_parse(long[..]).error == NCS_LENGTH;
}
// 146 words carry 730 bits: 91 bytes and 2 padding bits, which must be 0.
#forall "the last word's two padding bits must be zero" w: 0..<32 #steps 1000000000 {
  payload: [NCS_BYTES] u8; words: [146] u8; text: [200] u8;
  words[145] = cast(u8) w;
  r := bech32_encode_checked("ncryptsec", cast(string) words[..], false, text[..], true, false);
  #assert (ncs_decode(cast(string) text[0..<r.written], payload[..]) == NCS_LENGTH) == ((w & 3) != 0);
}
// The key-security byte is the associated data: a payload sealed with
// byte 1 or 2 parses to that byte and opens only with it.
#forall "key-security bytes 1 and 2 are the associated data" s: 1..<3 #steps 1000000000 {
  payload: [NCS_BYTES] u8; key: [32] u8; secret: [32] u8; out: [32] u8; aad: [1] u8;
  vector(payload[..]); unhex(KEY_NOSTR, key[..]); unhex(PRIVATE, secret[..]);
  nonce := ncs_parse(payload[..]).value.nonce;
  aad[0] = cast(u8) s;
  #assert xcp_seal(key[..], nonce[..], aad[..], secret[..], payload[43..<91], false);
  payload[42] = cast(u8) s;
  p := ncs_parse(payload[..]);
  #assert p.error == NCS_OK && p.value.security == cast(u8) s;
  #assert ncs_open(p.value, key[..], out[..]) == NCS_OK && hex_is(out[..], PRIVATE);
  payload[42] = 0;
  #assert ncs_open(ncs_parse(payload[..]).value, key[..], out[..]) == NCS_TAG;
}

// scrypt(b"nostr", the vector's salt, 2^log_n, 8, 1, 32) from OpenSSL
// through Python's hashlib.scrypt, for log_n 1..6.
SCRYPT :: [6] string.[
  "1e69cb277f3127419fed938fad417bcc87d7e26cc04054f74aad32f027dcd239",
  "37d4e5e74740d024f0e836ad0d4b28bc7fc64daf9ba83a779accea03ef984d09",
  "aefdabe7f80b46f3b99bc22a46f2e08b2b549cec905654f0a22b8503313ef3f1",
  "63587b915af5343363dbe8f9a811b1b7149d3866b40203a576e1ac7a50fd3e47",
  "e03aab70bcd0bef66c7fbfd238112fd51a7322f430700ed358ff6520f0eae977",
  "7f61c6400dfffc3fa86c540d03dd2f2d3af1765282f008da7e3382e7d983590e"];
#forall "scrypt matches hashlib for log_n 1..6" n: 1..<7 #steps 1000000000 {
  payload: [NCS_BYTES] u8; vector(payload[..]);
  p := ncs_parse(payload[..]); salt := p.value.salt;
  memory: [65536] u8; out: [32] u8; word: [5] u8;
  #assert derive(bytes_of("nostr", word[..]), salt[..], n, memory[..], out[..]);
  #assert hex_is(out[..], SCRYPT[n - 1]);
}
#test "scrypt refuses log_n 19 and 22, and memory below 128 * 8 * 2^log_n" {
  #assert !nip49_scrypt_params(5, 16, 268435456, 19, false);
  #assert !nip49_scrypt_params(5, 16, 4294967296, 22, false);
  #assert nip49_scrypt_params(5, 16, 1024 * 16, 4, false);
  #assert !nip49_scrypt_params(5, 16, 1024 * 16 - 1, 4, false);
  memory: [16383] u8; out: [32] u8; salt: [16] u8; word: [5] u8;
  #assert !derive(bytes_of("nostr", word[..]), salt[..], 4, memory[..], out[..]);
}

#test "NIP-49's NFKC example" {
  typed: [11] u8; scratch: [64] u32; out: [32] u8;
  unhex("e284abe284a6e1ba9bcca3", typed[..]);
  n := un_nfkc(cast(string) typed[..], scratch[..], out[..], false);
  #assert n.error == UN_OK && hex_is(out[0..<n.written], "c385cea9e1b9a9");
}

// RFC 8439 section 2.5.2 prints the accumulator after block 1,
// 2c88c77849d64ae9147ddeb88e69c83fc, and s. Tagging the first 16 bytes
// alone stops there: the tag is (acc + s) mod 2^128, added here byte by
// byte in little-endian order.
#test "the first Poly1305 block matches RFC 8439's accumulator plus s" {
  acc_be: [17] u8; s_le: [16] u8; want: [16] u8; key: [32] u8; text: [34] u8; tag: [16] u8;
  unhex("02c88c77849d64ae9147ddeb88e69c83fc", acc_be[..]);
  unhex("85d6be7857556d337f4452fe42d506a80103808afb0db2fd4abff6af4149f51b", key[..]);
  for i: 0..<16 { s_le[i] = key[16 + i]; }
  carry := 0;
  for i: 0..<16 {
    sum := cast(int) acc_be[16 - i] + cast(int) s_le[i] + carry;
    want[i] = cast(u8) (sum & 255); carry = sum >> 8;
  }
  msg := bytes_of("Cryptographic Forum Research Group", text[..]);
  #assert poly1305_checked(key[..], msg[0..<16], tag[..], false);
  for i: 0..<16 { #assert tag[i] == want[i]; }
  #assert hex_is(tag[..], "fd861c7184f98f45dc6d5b4dc6c081e4");
}
```

To run them:

```shell
coax/bin/coax test nostr/examples/checks/ncryptsec.coax
coax/bin/coax mutate nostr/examples/checks/ncryptsec.coax --root-only
```

`coax test` runs 125 checks, including the loaded library files' own
checks, in about 3 seconds. On 2026-11-01, root-only mutation of a copy of
the example with these checks appended killed all 134 mutants in
`ncs_decode`, `ncs_parse`, `ncs_open`, `derive` and the check helpers.

### The full vector, scrypt included

```shell
coax/tests/nostr/ncryptsec_vector.sh
```

[ncryptsec_vector.c](../../coax/tests/nostr/ncryptsec_vector.c) is a small
C host for the library's generated C (checked and release builds, each
under ASan and UBSan). It `malloc`s the 64 MiB workspace, runs
`nip49_scrypt_start`, `nip49_scrypt_step` and `nip49_scrypt_finish` on
the vector's salt at `log_n = 16`, checks the key against `KEY_NOSTR`, then
opens the vector with `xcp_open` and checks NIP-49's published private key.
It also checks the refusals: a heap workspace one byte short (so ASan would
report any write past it) is refused before the state is touched, 64 MiB is
refused for `log_n = 17`, and a step given a shorter view is refused. On
2026-11-01 on Linux x86_64 both builds passed in about 31 seconds each
under the sanitizers. Earlier, a copy of the example run under `ulimit -s
unlimited` derived the same key on the stack (13 seconds debug, 3.8
release); that needs a stack larger than macOS allows, which is why the
workspace comes from the host.

### Library records

The library files have their own records, which were not rerun today:

- [2026-10-29-scrypt-nip49.md](../../docs/verification/2026-10-29-scrypt-nip49.md):
  the composition against `hashlib.scrypt` on 16 small inputs per native
  and Wasm profile, and one full `N = 65536` (64 MiB) native operation per
  profile, through C hosts that own the memory; all 1,434 mutants killed.
- [2026-10-29-xchacha20-poly1305.md](../../docs/verification/2026-10-29-xchacha20-poly1305.md)
  and [2026-10-29-poly1305.md](../../docs/verification/2026-10-29-poly1305.md):
  the AEAD against the published XChaCha20-Poly1305 draft vector, and the
  MAC against RFC 8439 section 2.5.2 and appendix A.3.
- [2026-10-29-unicode-nfkc.md](../../docs/verification/2026-10-29-unicode-nfkc.md):
  NFKC against the complete Unicode 18.0.0 `NormalizationTest.txt`.

The library's own comments say what is not claimed: secrets aren't proven
erased from the stack, and none of these files is approved for production
keys.

## Summary

- An `ncryptsec` is 91 bytes: version 2, `log_n`, salt, nonce, key-security
  byte, ciphertext and tag, in extended Bech32.
- The password is NFKC-normalized, and scrypt with `N = 2^log_n`, `r = 8`,
  `p = 1` turns it into the XChaCha20-Poly1305 key.
- The key-security byte is the associated data, so the tag covers it.
- `log_n` outside 1 to 18 refuses before any memory is used; a wrong
  password or any changed byte after the salt fails the tag, and nothing is
  written.
- The NIP-49 vector decrypts to its published key.

## Dependencies

- [18. ChaCha20](ch018-chacha20.md)
- [5. HMAC-SHA256](ch005-hmac-sha256.md)
- [15. Bech32](ch015-bech32.md)

## Sources

- [NIP-49](../nips/official/49.md)
- [RFC 7914](https://www.rfc-editor.org/rfc/rfc7914), scrypt.
- [RFC 8439](https://www.rfc-editor.org/rfc/rfc8439), ChaCha20 and
  Poly1305; section 2.5.2 for the worked tag.
- [draft-irtf-cfrg-xchacha](https://datatracker.ietf.org/doc/html/draft-irtf-cfrg-xchacha-03),
  XChaCha20-Poly1305.
- [UAX #15](https://www.unicode.org/reports/tr15/), Unicode normalization.
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
