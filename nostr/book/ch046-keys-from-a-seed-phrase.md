# 46. Keys from a seed phrase

NIP-06 derives a Nostr key from BIP-39 words, so a person can restore it from
a phrase. The pinned text marks it `unrecommended: prefer a single nsec`; the
website's words account uses it
([docs/web-log-in.md](../../docs/web-log-in.md)). This chapter turns 12 words
into entropy and back with a checksum, stretches the words into a 64-byte
seed, and walks the seed down the BIP-32 path `m/44'/1237'/0'/0/0` to the
private key, its public key and its `npub`.

## What this chapter teaches

- BIP-39: how 128 bits of entropy and a 4-bit SHA-256 checksum become 12
  words of 11 bits each, and how a reader checks them.
- The seed: PBKDF2-HMAC-SHA512 over the words, 2,048 iterations, salt
  `mnemonic` followed by an optional passphrase.
- BIP-32: the master key, and hardened and normal child keys with
  HMAC-SHA512.
- The NIP-06 path `m/44'/1237'/<account>'/0/0` and the two test vectors in
  the pinned text.
- Which step refuses what: the checksum belongs to the caller, not to the
  key derivation.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [bip39.coax](../../coax/lib/crypto/bip39.coax) | `bip39_words(entropy, out)` | 16, 20, 24, 28 or 32 bytes to 12, 15, 18, 21 or 24 words |
| | `bip39_entropy_len(phrase, out)`, `bip39_entropy(phrase, out)` | the words back to their entropy (its byte count, or `-1`; or `true`/`false`); refuses an unknown word, a count BIP-39 doesn't define, a wrong checksum or too small an `out` |
| | `bip39_seed(phrase, out)`, `bip39_seed_with(phrase, passphrase, out)` | the 64-byte seed |
| | `bip39_word(i)`, `bip39_index(w)` | the English list, checked against its published SHA-256 |
| [sha512.coax](../../coax/lib/crypto/sha512.coax) | `sha512`, `hmac_sha512`, `pbkdf2_hmac_sha512` | the hash, the MAC and the one-block PBKDF2 that BIP-39 uses |
| [bip32.coax](../../coax/lib/crypto/bip32.coax) | `bip32_master`, `bip32_child`, `bip32_path`, `bip32_public` | private derivation on Bitcoin chapter 16's constant-time multiplication |
| [nip06.coax](../../coax/lib/nostr/nip06.coax) | `nip06_key(phrase, key)`, `nip06_npub(key, out)` | the NIP-06 key of checked words, and its `npub` (chapter 16) |

[pbkdf2.coax](../../coax/lib/crypto/pbkdf2.coax) is a different PBKDF2: a
resumable PBKDF2-HMAC-SHA256 for the website's phrase gate. NIP-06 doesn't
use it; it is listed because the gate and this chapter share the words.

Every procedure takes caller-owned byte views, returns `false` (or `-1` for
a length) on a bad size, and zeroes the secret locals it used before it
returns. The `bip39` procedures here call the constant-time
`bip39_encode_ct`, `bip39_decode_ct` and `bip39_seed_ct`, which
[Bitcoin chapter 31](../../bitcoin/book/ch031-seed-phrases.md) teaches.

## BIP-39 words

A 12-word phrase holds 132 bits: 128 bits of entropy followed by the first
4 bits of the SHA-256 of that entropy. Each word is 11 of those bits, read
from the top, as an index into a list of 2,048 English words. `abandon` is
index 0 and `zoo` is index 2047.

BIP-39 also defines 15, 18, 21 and 24 words, with 1 checksum bit per 32
bits of entropy:

| Words | Entropy | Checksum bits |
| --- | --- | --- |
| 12 | 16 bytes | 4 |
| 15 | 20 bytes | 5 |
| 18 | 24 bytes | 6 |
| 21 | 28 bytes | 7 |
| 24 | 32 bytes | 8 |

`n` words hold `11n` bits: `32n/3` bits of entropy and `n/3` checksum bits.
The words account makes 16 bytes of entropy, so it uses 12 words.

To read a phrase, `bip39_entropy_len`:

1. splits it on spaces (any number of them) and looks up each word;
2. refuses an unknown word or a 25th word;
3. refuses a count that isn't 12, 15, 18, 21 or 24, and an `out` smaller
   than the entropy;
4. packs the bits, hashes the entropy and compares the first `n/3` bits of
   the hash with the last `n/3` bits of the phrase;
5. returns the entropy's byte count. `bip39_entropy` returns `true` when
   that count is positive.

### Worked example: the all-zero phrase

Take 16 zero bytes. Their SHA-256 begins `37 47...`, so the checksum is the
top 4 bits of `0x37`, `0011`. The 132 bits are 128 zeros and `0011`. Words
1 to 11 are 11 zero bits each, index 0, `abandon`. Word 12 is 7 zero bits
followed by `0011`, which is index 3, `about`:

```text
abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about
```

This is the first BIP-39 reference vector.

### Worked example: NIP-06 vector 1

The first NIP-06 phrase is
`leader monkey parrot ring guide accident before fence cannon height naive bean`.
Look up each word and write its index in 11 bits:

| Word | Index | Bits |
| --- | ---: | --- |
| leader | 1012 | `01111110100` |
| monkey | 1145 | `10001111001` |
| parrot | 1283 | `10100000011` |
| ring | 1488 | `10111010000` |
| guide | 828 | `01100111100` |
| accident | 11 | `00000001011` |
| before | 161 | `00010100001` |
| fence | 680 | `01010101000` |
| cannon | 267 | `00100001011` |
| height | 853 | `01101010101` |
| naive | 1173 | `10010010101` |
| bean | 156 | `00010011100` |

1. Join the bits. The first 8 are `01111110`, `0x7e`; the next 8 are
   `10010001`, `0x91`. All 128 give the entropy
   `7e91e681dd067802c50aa8216d564a89`.
2. The last 4 bits of `bean`, `1100`, are the checksum in the phrase.
3. SHA-256 of the entropy begins `c0`. Its top 4 bits are `1100`. They
   match, so the phrase is valid.

Change `bean` (156) to the next word, `beauty` (157). The last 4 bits become
`1101`, and the entropy is unchanged, so the hash still says `1100` and the
phrase is refused. One word in 16 at the last position passes for any given
first 11 words.

## The seed

`bip39_seed_with` joins the words with single spaces and computes

```jai
seed = PBKDF2-HMAC-SHA512(password = the words, salt = "mnemonic" ‖ passphrase, 2048 iterations, 64 bytes)
```

The seed is one 64-byte PBKDF2 block, so `pbkdf2_hmac_sha512` computes
`U1 = HMAC(words, salt ‖ 00 00 00 01)`, then `U(i) = HMAC(words, U(i-1))`
2,047 more times, and XORs all 2,048 values. NIP-06 uses no passphrase:
`bip39_seed` passes the empty string. A passphrase gives a different seed
and so a different key; nothing records whether one was used.

The seed hashes the words, not the entropy. BIP-39 lets a reader derive a
seed from words with a wrong checksum after a warning; `bip39_seed` refuses
them instead (Bitcoin chapter 31), and `nip06_key` also checks the words
with `bip39_entropy` before it derives.

## BIP-32 derivation

BIP-32 turns the seed into a tree of keys. Each node is a private key `k`
and a 32-byte chain code `c`.

- **Master:** `I = HMAC-SHA512(key = "Bitcoin seed", seed)`. The left 32
  bytes are `k`, the right 32 bytes are `c`.
- **Child `i`:** `I = HMAC-SHA512(key = c, data ‖ ser32(i))`, where
  `ser32` is 4 bytes big-endian. The child key is `(I_L + k) mod n` and its
  chain code is `I_R`.
  - For a **hardened** index, `i ≥ 2^31`, the data is `0x00 ‖ k`: the
    parent's private key.
  - For a **normal** index, `i < 2^31`, the data is the 33-byte compressed
    public key `k·G` (Bitcoin chapter 16's constant-time multiplication).

`bip32_child` refuses an index below 0 or above `2^32 - 1`, an `I_L` that is
`n` or more, and a child key of zero. `bip32_master` refuses a seed shorter
than 16 or longer than 64 bytes. Each refusal is `false`.

An index written `44'` means `44 + 2^31`. A hardened child can't be derived
from the parent's public key and chain code alone, so a leaked child key and
the parent's public data don't reveal siblings.

### Worked example: the path m/44'/1237'/0'/0/0

| Level | Meaning | Index | Kind |
| --- | --- | ---: | --- |
| `44'` | BIP-44 purpose | 2147483692 | hardened |
| `1237'` | Nostr's coin type in SLIP-44 | 2147484885 | hardened |
| `0'` | account | 2147483648 | hardened |
| `0` | external chain | 0 | normal |
| `0` | address index | 0 | normal |

`NIP06_PATH` in [nip06.coax](../../coax/lib/nostr/nip06.coax) holds these
five numbers. `44 + 2^31 = 44 + 2147483648 = 2147483692`, and
`1237 + 2147483648 = 2147484885`. The first three levels hash the private
key, the last two hash the public key. NIP-06 lets a client raise the
account, `m/44'/1237'/1'/0/0`, for a second key from the same words.

The path is data: `bip32_child` doesn't know which levels NIP-06 hardens. A
path written without the apostrophes, `m/44/1237/0/0/0`, is accepted and
gives a different key. A client must use `NIP06_PATH` as written.

## The NIP-06 key

`nip06_key(phrase, key)` checks the words with `bip39_entropy`, then runs
`bip39_seed` and `bip32_path` with `NIP06_PATH` and writes the 32-byte
private key. It refuses what `bip39_entropy` refuses and zeroes `key`, so a
mistyped word never yields a key. The x coordinate of `k·G` is the Nostr public key, and
`nip06_npub` encodes it as an `npub`.

## The example

The example is [nostr/examples/seed-phrase.coax](../examples/seed-phrase.coax).
It works the all-zero checksum, prints vector 1's word indexes, entropy,
hash byte and seed, walks the seed down the path one level at a time,
derives both NIP-06 vectors, shows two other paths, reads vector 2's 24
words back to 32 bytes, and prints eight refusals.

<!-- source: nostr/examples/seed-phrase.coax example -->
```jai
#load "../../coax/lib/nostr/nip06.coax";
#load "../../coax/lib/nostr/hex.coax";
#import "Basic";

// Print a label and bytes as lowercase hex.
show :: (label: string, bytes: [] u8) {
  text: [160] u8;
  r := hex_encode_checked(cast(string) bytes, text[..], false);
  print("%: %\n", label, cast(string) text[0..<r.written]);
}
// Print the 11-bit index of each word, with its bits.
show_indexes :: (phrase: string) {
  at := 0;
  while at < phrase.count {
    e := at;
    while e < phrase.count && phrase[e] != ' ' { e += 1; }
    v := bip39_index(phrase[at..<e]);
    bits: [11] u8;
    for b: 0..<11 { bits[b] = cast(u8) (ifx (v >> (10 - b)) & 1 == 1 then '1' else '0'); }
    print("  % %: %\n", phrase[at..<e], v, cast(string) bits[..]);
    at = e + 1;
  }
}
// The key at a path from a seed, printed one level at a time.
show_path :: (seed: [] u8, path: [] int) {
  key: [32] u8; chain: [32] u8; k: [32] u8; c: [32] u8;
  #assert bip32_master(seed, key[..], chain[..]);
  show("  m key", key[..]);
  show("  m chain", chain[..]);
  for step: path {
    #assert bip32_child(key[..], chain[..], step, k[..], c[..]);
    hardened := step >= BIP32_HARDENED;
    index := ifx hardened then step - BIP32_HARDENED else step;
    mark := ifx hardened then "'" else "";
    print("  child %", index); print("% (index %): ", mark, step);
    show("key", k[..]);
    for j: 0..<32 { key[j] = k[j]; chain[j] = c[j]; }
  }
}
// The NIP-06 key of a phrase, its x-only public key and its npub.
show_nip06 :: (label: string, phrase: string) {
  key: [32] u8; pub: [33] u8; npub: [80] u8;
  ok := nip06_key(phrase, key[..]);
  print("% key: ok %\n", label, ok);
  show("  private key", key[..]);
  #assert bip32_public(key[..], pub[..]);
  show("  public key", pub[1..<33]);
  n := nip06_npub(key[..], npub[..]);
  print("  npub: %\n", cast(string) npub[0..<n]);
}

VECTOR_1 :: "leader monkey parrot ring guide accident before fence cannon height naive bean";
VECTOR_2 :: "what bleak badge arrange retreat wolf trade produce cricket blur garlic valid proud rude strong choose busy staff weather area salt hollow arm fade";
ZEROS    :: "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

main :: () {
  entropy: [16] u8; h: [32] u8; words: [200] u8; seed: [64] u8;

  // The checksum by hand: 128 zero bits; SHA-256 starts 0x37, so the
  // checksum is 0011 and the last word is index 3, "about".
  #assert bip39_entropy(ZEROS, entropy[..]);
  show("all-zero entropy", entropy[..]);
  #assert sha256_hash_checked(entropy[..], h[..], false) == SHA_OK;
  show("its SHA-256, first 2 bytes", h[0..<2]);
  print("word 3 is %\n", bip39_word(3));

  // NIP-06 vector 1: words to indexes, entropy and checksum.
  print("vector 1 word indexes:\n");
  show_indexes(VECTOR_1);
  print("vector 1 checksum: ok %\n", bip39_entropy(VECTOR_1, entropy[..]));
  show("  entropy", entropy[..]);
  #assert sha256_hash_checked(entropy[..], h[..], false) == SHA_OK;
  show("  SHA-256 of entropy, first byte", h[0..<1]);
  n := bip39_words(entropy[..], words[..]);
  print("  entropy back to words: %\n", cast(string) words[0..<n]);

  // Words to seed: PBKDF2-HMAC-SHA512, 2048 rounds, salt "mnemonic".
  #assert bip39_seed(VECTOR_1, seed[..]);
  show("vector 1 seed", seed[..]);

  // The seed down m/44'/1237'/0'/0/0.
  path := NIP06_PATH;
  print("path m/44'/1237'/0'/0/0:\n");
  show_path(seed[..], path[..]);

  show_nip06("vector 1", VECTOR_1);
  show_nip06("vector 2", VECTOR_2);

  // Other paths from the same seed are other keys.
  key: [32] u8; chain: [32] u8;
  account := [5] int.[44 + BIP32_HARDENED, 1237 + BIP32_HARDENED, 1 + BIP32_HARDENED, 0, 0];
  #assert bip32_path(seed[..], account[..], key[..], chain[..]);
  show("account 1, m/44'/1237'/1'/0/0", key[..]);
  plain := [5] int.[44, 1237, 0, 0, 0];
  #assert bip32_path(seed[..], plain[..], key[..], chain[..]);
  show("no hardening, m/44/1237/0/0/0", key[..]);

  // Refusals.
  print("checksum, last word beauty: ok %\n",
        bip39_entropy("leader monkey parrot ring guide accident before fence cannon height naive beauty", entropy[..]));
  print("unknown word xyzzy: ok %\n",
        bip39_entropy("leader monkey parrot ring guide accident before fence cannon height naive xyzzy", entropy[..]));
  print("11 words: ok %\n",
        bip39_entropy("leader monkey parrot ring guide accident before fence cannon height naive", entropy[..]));
  print("24 words (vector 2) into 16 bytes: ok %\n", bip39_entropy(VECTOR_2, entropy[..]));
  wide: [32] u8;
  print("24 words (vector 2) into 32 bytes: % bytes\n", bip39_entropy_len(VECTOR_2, wide[..]));
  print("nip06_key, last word beauty: ok %\n",
        nip06_key("leader monkey parrot ring guide accident before fence cannon height naive beauty", key[..]));
  show("  key", key[..]);
  k: [32] u8; c: [32] u8;
  #assert bip32_master(seed[..], key[..], chain[..]);
  print("child index 2^32: ok %\n", bip32_child(key[..], chain[..], 4294967296, k[..], c[..]));
  print("child index -1: ok %\n", bip32_child(key[..], chain[..], -1, k[..], c[..]));
  print("master of a 15-byte seed: ok %\n", bip32_master(seed[0..<15], key[..], chain[..]));
  print("31-byte key buffer: ok %\n", nip06_key(VECTOR_1, key[0..<31]));
}
```

Run it from the repository root:

```shell
coax/bin/coax run nostr/examples/seed-phrase.coax
```

The output matches the values worked out by hand: the SHA-256 of zero
entropy starts `37`, word 3 is `about`, vector 1's indexes and entropy
match the table, and its hash byte is `c0`. Both private keys, public keys
and `npub`s are the ones published in NIP-06:

```text
all-zero entropy: 00000000000000000000000000000000
its SHA-256, first 2 bytes: 3747
word 3 is about
vector 1 word indexes:
  leader 1012: 01111110100
  monkey 1145: 10001111001
  parrot 1283: 10100000011
  ring 1488: 10111010000
  guide 828: 01100111100
  accident 11: 00000001011
  before 161: 00010100001
  fence 680: 01010101000
  cannon 267: 00100001011
  height 853: 01101010101
  naive 1173: 10010010101
  bean 156: 00010011100
vector 1 checksum: ok true
  entropy: 7e91e681dd067802c50aa8216d564a89
  SHA-256 of entropy, first byte: c0
  entropy back to words: leader monkey parrot ring guide accident before fence cannon height naive bean
vector 1 seed: 173b9c5f0d165502d08a4d122b2c9bf1e33e27806eac119713600a263c1241101dc55fb7cffb8f48a59b19a5ba65b037904f907bb8d08eb5bff8a17e85c2ee93
path m/44'/1237'/0'/0/0:
  m key: dbbcc0e112894d1430d5bc348d1bd72e8ac339952702be1fe572de80fe1b7fcb
  m chain: 9ebdd3f7c9808336542f29a7b53f7a31290619cefc9dc54d5c8bd2d83d930bdc
  child 44' (index 2147483692): key: 2ae2db9aa7a3f566e41ceb7d0b6ea3aec082ba395822063f32d2d8a789ba8921
  child 1237' (index 2147484885): key: 6437d89d482a14aad16a4dde69ee28be50b49e83a31e9aa048564606fd75f1b4
  child 0' (index 2147483648): key: 1b794bd84d20a272bbe312deb2966b4aa9ac989784bb0c27aa0b995189ad95cd
  child 0 (index 0): key: 2ba8f6f59d4a6e323b3b1af12633bcf7fd295ac5ee903a2492b3d0da4f1fb818
  child 0 (index 0): key: 7f7ff03d123792d6ac594bfa67bf6d0c0ab55b6b1fdb6249303fe861f1ccba9a
vector 1 key: ok true
  private key: 7f7ff03d123792d6ac594bfa67bf6d0c0ab55b6b1fdb6249303fe861f1ccba9a
  public key: 17162c921dc4d2518f9a101db33695df1afb56ab82f5ff3e5da6eec3ca5cd917
  npub: npub1zutzeysacnf9rru6zqwmxd54mud0k44tst6l70ja5mhv8jjumytsd2x7nu
vector 2 key: ok true
  private key: c15d739894c81a2fcfd3a2df85a0d2c0dbc47a280d092799f144d73d7ae78add
  public key: d41b22899549e1f3d335a31002cfd382174006e166d3e658e3a5eecdb6463573
  npub: npub16sdj9zv4f8sl85e45vgq9n7nsgt5qphpvmf7vk8r5hhvmdjxx4es8rq74h
account 1, m/44'/1237'/1'/0/0: 3790c23940f62b23754115ef70f16e63cca8e9015a532b8a891171ccdadcf910
no hardening, m/44/1237/0/0/0: 88a7533bccda9ab25127c961764d309dd4181763730814ca1ea94bcb311aa754
checksum, last word beauty: ok false
unknown word xyzzy: ok false
11 words: ok false
24 words (vector 2) into 16 bytes: ok false
24 words (vector 2) into 32 bytes: 32 bytes
nip06_key, last word beauty: ok false
  key: 0000000000000000000000000000000000000000000000000000000000000000
child index 2^32: ok false
child index -1: ok false
master of a 15-byte seed: ok false
31-byte key buffer: ok false
```

The refusals:

- `beauty` as the last word leaves the entropy unchanged and the checksum
  wrong, so `bip39_entropy` returns `false`.
- `xyzzy` is not in the list, so `bip39_entropy` returns `false`.
- 11 words is not a BIP-39 count, so `bip39_entropy` returns `false`.
- Vector 2's 24 words hold 32 bytes of entropy, which don't fit a 16-byte
  `out`, so `bip39_entropy` returns `false`; with 32 bytes of room
  `bip39_entropy_len` returns 32.
- `nip06_key` refuses the `beauty` phrase and leaves `key` zero.
- `bip32_child` refuses index `2^32` and index `-1`.
- `bip32_master` refuses a 15-byte seed.
- `nip06_key` refuses a 31-byte key buffer.

The book's checks run this program in debug and release builds and compare
the output with [nostr/fixtures/seed-phrase.out](../fixtures/seed-phrase.out).

## How the code is checked

### Checks in the example

The checks are in
[nostr/examples/checks/seed-phrase.coax](../examples/checks/seed-phrase.coax),
which loads the example. They compare the code with published and
independent values:

- both NIP-06 vectors: the private key, the public key and the `npub`
  published in [NIP-06](../nips/official/06.md);
- the Trezor BIP-39 reference seed of the all-zero phrase with passphrase
  `TREZOR`;
- for each of the 128 values of the last 7 entropy bits after 15 zero bytes,
  the checksum nibble from Python's `hashlib.sha256`: the last word must be
  index `16·p + nibble` and must read back;
- vector 2's seed and its key at each of the 6 depths of the path, account
  1, the unhardened path and the `beauty` phrase's key, computed with
  Python's `hashlib.pbkdf2_hmac`, `hmac` and affine secp256k1 arithmetic on
  Python integers, which share no code with the Coax files.

<!-- source: nostr/examples/checks/seed-phrase.coax check -->
```jai
// Text to bytes for the hex comparisons.
hex_is :: (got: [] u8, want: string) -> bool { return sha512_hexeq(got, want); }

// The checksum nibble of 15 zero bytes followed by p, for p = 0..127, from
// Python's hashlib.sha256: the last word of "abandon" x 11 and the word of
// index p * 16 + nibble is the only valid last word with those 7 bits.
CHECKSUMS :: "37686c969c4e984503c9b55edb55ecc6a103f08b6afc56827f166c659c88ce6e09bd46075cefb4301e6c3c0c7ee01469ab948bd58bc763da4b35b71bd1c037d0";
#forall "the last word carries the SHA-256 checksum nibble" p: 0..<128 #steps 1000000000 {
  e: [16] u8; out: [200] u8; back: [16] u8;
  e[15] = cast(u8) p;
  n := bip39_words(e[..], out[..]);
  #assert n > 0;
  at := n;
  while out[at - 1] != ' ' { at -= 1; }
  c := CHECKSUMS[p];
  nibble := ifx c <= '9' then cast(int) (c - '0') else cast(int) (c - 'a') + 10;
  #assert bip39_index(cast(string) out[at..<n]) == p * 16 + nibble;
  #assert bip39_entropy(cast(string) out[0..<n], back[..]) && back[15] == cast(u8) p;
}

// The NIP-06 vectors, as published in nostr/nips/official/06.md.
#test "NIP-06 vector 1: the published private key, public key and npub" #steps 1000000000 {
  key: [32] u8; pub: [33] u8; npub: [80] u8; e: [16] u8;
  #assert bip39_entropy(VECTOR_1, e[..]);
  #assert nip06_key(VECTOR_1, key[..]) && hex_is(key[..], "7f7ff03d123792d6ac594bfa67bf6d0c0ab55b6b1fdb6249303fe861f1ccba9a");
  #assert bip32_public(key[..], pub[..]) && hex_is(pub[1..<33], "17162c921dc4d2518f9a101db33695df1afb56ab82f5ff3e5da6eec3ca5cd917");
  n := nip06_npub(key[..], npub[..]);
  #assert n > 0 && cast(string) npub[0..<n] == "npub1zutzeysacnf9rru6zqwmxd54mud0k44tst6l70ja5mhv8jjumytsd2x7nu";
}
#test "NIP-06 vector 2: the published private key, public key and npub" #steps 1000000000 {
  key: [32] u8; pub: [33] u8; npub: [80] u8;
  #assert nip06_key(VECTOR_2, key[..]) && hex_is(key[..], "c15d739894c81a2fcfd3a2df85a0d2c0dbc47a280d092799f144d73d7ae78add");
  #assert bip32_public(key[..], pub[..]) && hex_is(pub[1..<33], "d41b22899549e1f3d335a31002cfd382174006e166d3e658e3a5eecdb6463573");
  n := nip06_npub(key[..], npub[..]);
  #assert n > 0 && cast(string) npub[0..<n] == "npub16sdj9zv4f8sl85e45vgq9n7nsgt5qphpvmf7vk8r5hhvmdjxx4es8rq74h";
}

// The BIP-39 reference vectors (Trezor's vectors.json): the all-zero phrase
// with passphrase "TREZOR".
#test "BIP-39: the Trezor seed of the all-zero phrase" #steps 1000000000 {
  seed: [64] u8;
  #assert bip39_seed_with(ZEROS, "TREZOR", seed[..]) && hex_is(seed[..], "c55257c360c07c72029aebc1b53c05ed0362ada38ead3e3e9efa3708e53495531f09a6987599d18264c1e1c92f2cf141630c7a3c4ab7c81b2f001698e7463b04");
}

// Vector 2's seed and the key at each level of m/44'/1237'/0'/0/0, from
// Python's hashlib.pbkdf2_hmac and hmac with affine secp256k1 arithmetic on
// Python integers. They share no code with bip39.coax or bip32.coax.
VECTOR_2_SEED :: "5e2bd11b4d371f25098ed95ded029e2b9268cf188e6b764023bafbbd8fe843244fb72ca8f66c9378085d69fcb4d4224e709ffe071acafa7b7d5eb54b2905d553";
VECTOR_2_LEVELS :: [6] string.[
    "d58d40d5724435552fa442350b75e0ff95a19d990e908e3a516bcc88f780108f",
    "ccb98b8a39bb4576372fcec6ddcc4e69092ffeda24890008f8da497047ba1ddc",
    "a6a8f65beb0c2a884544f4f47495755052fd113feff4c6fb77307c25d9fab649",
    "3144192d9242c098869bf21842d7c18c0419c57a656b789a68f5f17e1f2e28fa",
    "60fe6da5e9988ab13b317e1db6b0955ee0c8da863b8c2457afab6833d1c96098",
    "c15d739894c81a2fcfd3a2df85a0d2c0dbc47a280d092799f144d73d7ae78add"];
#test "vector 2's seed is Python's PBKDF2-HMAC-SHA512" #steps 1000000000 {
  seed: [64] u8;
  #assert bip39_seed(VECTOR_2, seed[..]) && hex_is(seed[..], VECTOR_2_SEED);
}
#forall "every level of the NIP-06 path matches Python" depth: 0..<6 #steps 1000000000 {
  sb: [64] u8; key: [32] u8; chain: [32] u8;
  seed := bip32_hex(VECTOR_2_SEED, sb[..]);
  path := NIP06_PATH;
  #assert bip32_path(seed, path[0..<depth], key[..], chain[..]);
  #assert hex_is(key[..], VECTOR_2_LEVELS[depth]);
}

// Vector 1's seed (Python) down other paths: account 1, the same indexes
// without hardening, and the key of a phrase whose checksum is wrong.
VECTOR_1_SEED :: "173b9c5f0d165502d08a4d122b2c9bf1e33e27806eac119713600a263c1241101dc55fb7cffb8f48a59b19a5ba65b037904f907bb8d08eb5bff8a17e85c2ee93";
#test "other paths are other keys, as Python computes them" #steps 1000000000 {
  sb: [64] u8; key: [32] u8; chain: [32] u8;
  seed := bip32_hex(VECTOR_1_SEED, sb[..]);
  account := [5] int.[44 + BIP32_HARDENED, 1237 + BIP32_HARDENED, 1 + BIP32_HARDENED, 0, 0];
  #assert bip32_path(seed, account[..], key[..], chain[..]) && hex_is(key[..], "3790c23940f62b23754115ef70f16e63cca8e9015a532b8a891171ccdadcf910");
  plain := [5] int.[44, 1237, 0, 0, 0];
  #assert bip32_path(seed, plain[..], key[..], chain[..]) && hex_is(key[..], "88a7533bccda9ab25127c961764d309dd4181763730814ca1ea94bcb311aa754");
}
#test "a wrong checksum is refused by bip39_entropy and by nip06_key" #steps 1000000000 {
  bad := "leader monkey parrot ring guide accident before fence cannon height naive beauty";
  e: [32] u8; key: [32] u8;
  #assert !bip39_entropy(bad, e[..]);
  #assert !nip06_key(bad, key[..]) && hex_is(key[..], "0000000000000000000000000000000000000000000000000000000000000000");
  // 24 words read back to 32 bytes of entropy
  #assert bip39_entropy_len(VECTOR_2, e[..]) == 32;
}
```

To run them:

```shell
coax/bin/coax test nostr/examples/checks/seed-phrase.coax
coax/bin/coax mutate nostr/examples/checks/seed-phrase.coax --root-only
```

`coax test` runs 146 checks, including the loaded files' own checks, in
about 90 seconds. The checks run in the compile-time interpreter, where one
2,048-round PBKDF2 takes about 10 seconds.

`coax mutate --root-only` on the checks file finds no reachable mutation
sites: its only procedure is a comparison helper. The library files are
not `#authority` files, and their full mutation run was not part of this
chapter.

### Library checks

[bip39.coax](../../coax/lib/crypto/bip39.coax) checks the English list's
SHA-256 (`2f5eed53...dbda`, the pinned `english.txt`), reference vectors
from entropy to words and back, a 15- and a 21-word phrase built with
Python's `hashlib`, a wrong checksum bit at each length, and each refusal;
[Bitcoin chapter 31](../../bitcoin/book/ch031-seed-phrases.md) describes
them. [sha512.coax](../../coax/lib/crypto/sha512.coax) checks the FIPS
180-4 SHA-512 examples, RFC 4231 HMAC-SHA512 cases 1, 2, 6 and 7, and
PBKDF2-HMAC-SHA512 at 1 and 2 rounds. The slower vectors, two more Trezor
seeds, BIP-32 test vectors 1 and 2 and the chain to a signed NIP-98 login,
are in [weblogin_vectors.coax](../../coax/tests/nostr/weblogin_vectors.coax):

```shell
coax/bin/coax test --library coax/tests/nostr/weblogin_vectors.coax --steps 8000000000 --run-steps 250000000
```

That file was not rerun for this chapter; its BIP-32 vectors are published
and unchanged.

## Summary

- 12 BIP-39 words are 128 bits of entropy and a 4-bit SHA-256 checksum, 11
  bits per word.
- The seed is PBKDF2-HMAC-SHA512 of the words with salt `mnemonic`, 2,048
  rounds; it doesn't depend on the checksum.
- BIP-32 hardened children hash the parent private key; normal children
  hash its public key. NIP-06 uses `m/44'/1237'/0'/0/0`.
- `bip39_entropy` refuses a wrong checksum, an unknown word and a count
  other than 12, 15, 18, 21 or 24; `nip06_key` refuses the same phrases.
- Both NIP-06 vectors give their published keys and `npub`s.

## Dependencies

- [Bitcoin chapter 24. Sign with Schnorr](../../bitcoin/book/ch024-sign-with-schnorr.md)
- [Bitcoin chapter 8. HMAC-SHA256](../../bitcoin/book/ch008-hmac-sha256.md)

## Sources

- [NIP-06](../nips/official/06.md)
- [BIP-39](https://github.com/bitcoin/bips/blob/master/bip-0039.mediawiki)
  and the Trezor reference vectors in
  [python-mnemonic](https://github.com/trezor/python-mnemonic/blob/master/vectors.json).
- [BIP-32](https://github.com/bitcoin/bips/blob/master/bip-0032.mediawiki)
  and [BIP-44](https://github.com/bitcoin/bips/blob/master/bip-0044.mediawiki).
- [SLIP-44](https://github.com/satoshilabs/slips/blob/master/slip-0044.md),
  coin type 1237 for Nostr.
- The words account: [docs/web-log-in.md](../../docs/web-log-in.md).
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
