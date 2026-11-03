# 31. Seed phrases

A wallet backup is usually 12 or 24 English words, such as
`abandon abandon … about`. BIP-39 defines how those words hold random
bytes with a checksum, and how they become a 64-byte seed. This chapter
turns 16 to 32 bytes of entropy into 12 to 24 words and back, checks the
checksum, and stretches the words and an optional passphrase into the seed
with PBKDF2-HMAC-SHA512 (chapters 7 and 8). The phrase, the passphrase and
the seed are secrets, so the code follows chapter 3's rules, and the
chapter says where a word lookup can run in constant time and where it
can't. It checks the result against the BIP-39 reference vectors, Python's
`hashlib` and OpenSSL.

## What this chapter teaches

- How 128 to 256 bits of entropy and a SHA-256 checksum of 4 to 8 bits
  become 12 to 24 words of 11 bits each, and how a reader checks them.
- The 2,048-word English list: where it comes from, its SHA-256, and why
  every word fits in 8 bytes.
- The seed: PBKDF2-HMAC-SHA512 over the words, with the salt `mnemonic`
  followed by the passphrase, 2,048 rounds and 64 bytes of output.
- Unicode NFKD: what BIP-39 asks for, and why this code supports ASCII
  only and refuses any other byte.
- A word lookup that reads the whole list, and a phrase split into words
  with masks, so no branch or address depends on a word.
- Why the passphrase is called a 25th word, and what the words protect
  against.
- The seven refusals and their codes.

## Where Bitcoin uses seed phrases

A wallet makes the entropy once (chapter 18 shows how to get random
bytes), shows the words to its owner, and keeps only the seed or the keys
derived from it. To restore, the owner types the words, and any wallet that
follows BIP-39 derives the same seed.

| Step | Input | Output | Chapter |
| --- | --- | --- | --- |
| Make a backup | 16 to 32 random bytes | 12 to 24 words | 18, 31 |
| Restore | the words and the passphrase | the 64-byte seed | 31 |
| Derive keys | the seed | the master key and the key tree | 32 |
| Choose accounts | the key tree | keys at standard paths, such as `m/84'/0'/0'/0/0` | 33 |

The Nostr book's chapter 46 uses the same code: NIP-06 takes a Nostr key
from a BIP-39 phrase at the path `m/44'/1237'/0'/0/0`.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [bip39.coax](../../coax/lib/crypto/bip39.coax) | `bip39_encode_ct(entropy, work, out)` | 16, 20, 24, 28 or 32 bytes to 12, 15, 18, 21 or 24 words |
| | `bip39_decode_ct(phrase, work, out)` | the words back to their entropy, checking each word and the checksum |
| | `bip39_seed_ct(phrase, passphrase, work, out)` | the 64-byte seed of a checked phrase |
| | `bip39_word(i)`, `bip39_index(w)` | a word and its index, in variable time, for public data |
| | `bip39_words`, `bip39_entropy_len`, `bip39_entropy`, `bip39_seed_with`, `bip39_seed` | the older `bool` and `-1` interface that NIP-06, the website log-in and the CLI use; they call the three above |
| [sha512.coax](../../coax/lib/crypto/sha512.coax) | `pbkdf2_hmac_sha512(password, salt, rounds, out)` | one 64-byte PBKDF2 block (chapter 7) |
| [sha256.coax](../../coax/lib/crypto/sha256.coax) | `sha256_hash_checked` | the checksum's hash (chapter 4) |

Each of the three `_ct` procedures takes a caller-owned work buffer of
`BIP39_WORK_BYTES` (1,520) bytes. The secret intermediates live there: the
words, their bits, the hash, the joined phrase and the salt. The procedure
fills it with zeros before it returns, on success and on every refusal
after it starts using it, so the caller can register it for erasure
(chapter 3) and gets it back clean. Each returns a count of bytes or
characters written, or one of seven negative refusals, and writes its
output only on success:

<!-- source: coax/lib/crypto/bip39.coax codes -->
```jai
BIP39_LIST_SHA256 :: "2f5eed53a4727b4bf8880d8f3f199efc90e58503646d9ff8eff3a2ed3b24dbda";
BIP39_MAX_PHRASE :: 1024;      // the longest phrase accepted, spaces included
BIP39_MAX_PASSPHRASE :: 1016;  // "mnemonic" + passphrase fits PBKDF2's 1,024-byte salt
BIP39_MAX_TEXT :: 215;         // 24 words of at most 8 letters and 23 spaces
BIP39_SEED_BYTES :: 64;
BIP39_ROUNDS :: 2048;

// Refusals.
BIP39_BAD_SIZE :: -1;          // entropy that isn't 16, 20, 24, 28 or 32 bytes
BIP39_BAD_COUNT :: -2;         // a word count that isn't 12, 15, 18, 21 or 24
BIP39_UNKNOWN_WORD :: -3;      // a word that isn't in the English list
BIP39_BAD_CHECKSUM :: -4;      // checksum bits that don't match the entropy's SHA-256
BIP39_NOT_ASCII :: -5;         // a byte above 0x7f, which would need NFKD
BIP39_CAPACITY :: -6;          // an output or work buffer that is too small
BIP39_TOO_LONG :: -7;          // a phrase or passphrase over its limit
```

## Words from entropy

A phrase holds the entropy followed by a checksum: the first `ENT / 32`
bits of the entropy's SHA-256, where `ENT` is the entropy's length in bits.
The bits are cut into groups of 11, read from the top, and each group is an
index into the list of 2,048 words. `abandon` is index 0 and `zoo` is
index 2047.

| Entropy | Checksum bits | Total bits | Words |
| --- | --- | --- | --- |
| 16 bytes (128 bits) | 4 | 132 | 12 |
| 20 bytes (160 bits) | 5 | 165 | 15 |
| 24 bytes (192 bits) | 6 | 198 | 18 |
| 28 bytes (224 bits) | 7 | 231 | 21 |
| 32 bytes (256 bits) | 8 | 264 | 24 |

With `n` bytes of entropy, the phrase has `3n / 4` words and `n / 4`
checksum bits, and `11 × 3n / 4 = 8n + n / 4`, so the words hold the
entropy and the checksum exactly. The checksum is never more than the first
byte of the hash.

### Worked example: 16 zero bytes

1. The entropy is 128 zero bits. Its SHA-256 starts with the byte `0x37`,
   which is `0011 0111`. The checksum is the top 4 bits, `0011`.
2. The 132 bits are 128 zeros and then `0011`.
3. Words 1 to 11 are 11 zero bits each: index 0, `abandon`.
4. Word 12 is the last 7 zeros of the entropy and the 4 checksum bits:
   `0000000 0011`, which is index 3, `about`.

```text
abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about
```

This is the first BIP-39 reference vector.

### Worked example: the last word of 24

The 24-word reference vector that starts `hamster diagram private` has the
entropy `68a79eac…74e6ce7c`. Its SHA-256 starts with `0x00`, so all 8
checksum bits are 0. The last word holds the last 3 bits of the entropy,
`100`, and then `00000000`: `10000000000` is index 1024, `length`. Of the
2,048 words that could end the first 23, exactly `2^11 / 2^8 = 8` give the
right checksum. At 12 words, with a 4-bit checksum, 128 of them do. A
phrase with one wrong word passes with a probability of 1 in 16 at 12
words and 1 in 256 at 24 words. The checksum catches most typing mistakes;
it is not a strong integrity check.

## The word list

The English list comes from BIP-39's repository. This chapter pins the copy
in trezor/python-mnemonic, the reference implementation that BIP-39 names,
as
[coax/tests/vectors/upstream/bip39/english.txt](../../coax/tests/vectors/upstream/bip39/english.txt).
Its SHA-256 is `2f5eed53…3b24dbda`. `bip39.coax` holds the same 13,116
bytes as the string `BIP39_ENGLISH`, and a check hashes the string and
compares it with that digest. The list has three properties that this
chapter relies on:

- Every word has 3 to 8 lowercase ASCII letters.
- The list is sorted.
- No two words share their first 4 letters, so a wallet may accept the
  first 4 letters of each word. A check compares neighbouring words, which
  is enough in a sorted list.

Because no word is longer than 8 letters, each word fits in one 64-bit
integer, one byte per letter and zeros after the last. `BIP39_TABLE` is the
list packed this way. A library can't raise the compile-time step budget, so
the table is written out in the source, and a check builds it again from
the list with `b39_make_table` and compares the two:

<!-- source: coax/lib/crypto/bip39.coax table -->
```jai
// The list packed for lookup: word k as a u64 whose
// byte c is its letter c, with zeros after the last letter. Every English
// word has 3 to 8 lowercase letters, so each fits and none packs to 0.
b39_make_table :: () -> [2048] u64 {
  table: [2048] u64;
  list := BIP39_ENGLISH;
  k := 0; c := 0;
  for i: 0..<list.count {
    if list[i] == '\n' { k += 1; c = 0; }
    else { table[k] |= cast(u64) list[i] << cast(u64) (8 * c); c += 1; }
  }
  return table;
}
```

No word packs to 0, so an empty slot never matches one.

## Secrets and constant time

Chapter 3's rules apply: a secret never decides a branch, never picks a
memory address and never reaches a variable-time routine. The phrase is a
secret: anyone who learns it can spend the coins. So are its words, their
indexes, the entropy, the passphrase and the seed.

A lookup that stops at the matching word leaks the word: a word near the
start of the list returns sooner than one near the end, and an observer who
times each lookup learns roughly where each word is in the list. The
constant-time lookups read all 2,048 entries for every word and keep the
match with a mask:

<!-- source: coax/lib/crypto/bip39.coax ct -->
```jai
B39_ALL :: CB_MAX64;                               // all ones (bytes.coax)
// All ones when a == b, else 0, without a branch: x | -x has its top bit
// set exactly when x is not 0.
b39_eq_mask :: (a: u64, b: u64) -> u64 {
  x := a ^ b;
  return value_barrier(sub_wrap((x | sub_wrap(cast(u64) 0, x)) >> 63, 1));
}
// All ones when a < b, for a and b below 2^63.
b39_less_mask :: (a: u64, b: u64) -> u64 {
  return value_barrier(sub_wrap(cast(u64) 0, sub_wrap(a, b) >> 63));
}
// The index of a packed word in the low 11 bits, with bit 11 set when the
// list doesn't hold it. It reads all 2,048 entries whatever the word is.
b39_index_ct :: (word: u64) -> u64 {
  index: u64 = 0; found: u64 = 0;
  for k: 0..<2048 {
    m := b39_eq_mask(word, BIP39_TABLE[k]);
    index |= m & cast(u64) k; found |= m;
  }
  return index | (((found & 1) ^ 1) << 11);
}
// The packed word at an index below 2048, reading all 2,048 entries.
b39_word_ct :: (index: u64) -> u64 {
  word: u64 = 0;
  for k: 0..<2048 { word |= b39_eq_mask(index, cast(u64) k) & BIP39_TABLE[k]; }
  return word;
}
// The number of letters in a packed word: its nonzero bytes.
b39_letters_ct :: (word: u64) -> u64 {
  n: u64 = 0;
  for c: 0..<8 { n = add_wrap(n, (add_wrap((word >> cast(u64) (8 * c)) & 255, 255)) >> 8); }
  return n;
}
// Slot s of work as a packed word, and back.
b39_slot :: (work: [] u8, s: int) -> u64 {
  word: u64 = 0;
  for c: 0..<8 { word |= cast(u64) work[B39_SLOTS + 8 * s + c] << cast(u64) (8 * c); }
  return word;
}
b39_put :: (work: [] u8, s: int, word: u64) {
  for c: 0..<8 { work[B39_SLOTS + 8 * s + c] = cast(u8) ((word >> cast(u64) (8 * c)) & 255); }
}
```

`b39_index_ct` sets bit 11 when no entry matched, so the caller learns
"unknown word" as a value and decides only once, at the end. Each lookup
reads the whole 16-kilobyte table in the same order, so the addresses it
touches don't depend on the word either.

### Splitting the phrase

The obvious split walks the phrase and branches at each space, and that
branch tells an observer where each word ends: the length of every word.
`b39_split` offers each character to every column of every slot and lets a
mask decide which slot keeps it. The running word number and column are
values computed with masks, never branches:

<!-- source: coax/lib/crypto/bip39.coax split -->
```jai
// Splits a phrase at its spaces into work's 24 slots, which must be zero.
// No branch or address depends on the text: every character is offered to
// every column of every slot, and a mask lets one take it. A slot's flag
// marks a word longer than 8 characters or with a character outside
// '!'..'~'; neither can be in the list. Returns the number of words, with
// bit 32 set when some byte is above 0x7f. Both are public from here on.
b39_split :: (phrase: string, work: [] u8) -> u64 {
  words: u64 = 0; high: u64 = 0;
  column: u64;                                    // set at each word's first character
  after_space: u64 = B39_ALL;                       // the text starts after a space
  at: [8] u64;
  for j: 0..<phrase.count {
    c := cast(u64) phrase[j];
    high |= c >> 7;
    letter := b39_eq_mask(c, 32) ^ B39_ALL;         // all ones unless a space
    first := letter & after_space;                  // this character starts a word
    words = add_wrap(words, first & 1);
    column = add_wrap(column, 1) & (first ^ B39_ALL);
    word := sub_wrap(words, 1);                     // the word this character is in
    odd := b39_less_mask(c, 33) | b39_less_mask(126, c) | b39_less_mask(7, column);
    for b: 0..<8 { at[b] = b39_eq_mask(column, cast(u64) b); }
    for s: 0..<24 {
      mine := letter & b39_eq_mask(word, cast(u64) s);
      work[B39_FLAGS + s] |= cast(u8) (odd & mine & 1);
      for b: 0..<8 { work[B39_SLOTS + 8 * s + b] |= cast(u8) (c & mine & at[b]); }
    }
    after_space = letter ^ B39_ALL;
  }
  return words | ((high & 1) << 32);
}
```

A slot whose word is longer than 8 characters, or holds a byte outside
`!`…`~` such as a tab, gets its flag set. Neither can be a list word, and
the flag makes it unknown even when its first 8 characters spell one.

### Joining the words

The encoder and the seed both need the words joined by single spaces. A
word's position in the text depends on the lengths of the words before
it, so each letter is offered to every position, and the one position
where the mask is all ones takes it:

<!-- source: coax/lib/crypto/bip39.coax join -->
```jai
// Writes the first count slots into work's text with single spaces between
// them and returns its length. Where a word goes depends on the lengths of
// the words before it, so each letter is offered to every position of the
// text and a mask lets one take it.
b39_join :: (work: [] u8, count: int) -> int {
  limit := 9 * count - 1;                           // 8 letters a word and the spaces
  for t: 0..<limit { work[B39_TEXT + t] = 0; }    // mutant: equivalent (the first word's letter always lands at 0)
  at: u64 = 0;
  for s: 0..<count {
    word := b39_slot(work, s);
    if s > 0 { b39_place(work, limit, at, 32); at = add_wrap(at, 1); }
    for c: 0..<8 { b39_place(work, limit, add_wrap(at, cast(u64) c), (word >> cast(u64) (8 * c)) & 255); }
    at = add_wrap(at, b39_letters_ct(word));
  }
  return cast(int) at;
}
b39_place :: (work: [] u8, limit: int, at: u64, value: u64) {
  for t: 0..<limit { work[B39_TEXT + t] |= cast(u8) (value & b39_eq_mask(at, cast(u64) t)); }
}
```

For 24 words that is 24 × 9 × 215, about 46,000 masked writes.

### What stays public, and where a lookup can't be constant time

These procedures make some facts public on purpose:

- The phrase's length in bytes and its word count. Both decide loop bounds
  and refusals, and the count is the entropy's size.
- The result code. A refusal says what was wrong but never which word.
- The length of the encoded phrase, which the caller needs to read it.

Some lookups can't be constant time at all:

- A screen that suggests words while the owner types, or marks a word as
  wrong, reacts to each word as it is typed. The screen shows the word
  anyway, so the lookup has nothing left to hide. Such a screen can use
  `bip39_index` on the word being typed, and pass the finished phrase to
  `bip39_decode_ct`.
- Accepting the first 4 letters of each word means the lookup depends on
  what was typed; this code doesn't accept prefixes.
- Showing the new phrase to the owner, and storing it, are outside this
  code's control.

`bip39_word` and `bip39_index` are the variable-time pair. They stop at the
match, and are for public data only: printing an index, a test, or the
word under a cursor.

The lookups, the split and the join have no branch or address that depends
on a word in the Coax source. As in chapter 3, that is a property of the
source and of the masks with `value_barrier`, not a proof about a
particular CPU or compiler.

## Entropy to words

`bip39_encode_ct` copies the entropy into the work buffer, hashes it,
appends the hash's first byte (only its top `n / 4` bits are read), reads
11 bits at a time and looks up each word:

<!-- source: coax/lib/crypto/bip39.coax encode -->
```jai
// 16, 20, 24, 28 or 32 bytes of entropy into 12, 15, 18, 21 or 24 words
// with single spaces. Returns the number of characters written, or
// BIP39_BAD_SIZE or BIP39_CAPACITY. work holds the secrets on the way and
// comes back as zeros.
bip39_encode_ct :: (entropy: [] u8, work: [] u8, out: [] u8) -> int #export_c {
  n := entropy.count;
  if n % 4 != 0 || n / 4 < 4 || n / 4 > 8 { return BIP39_BAD_SIZE; }
  if work.count < BIP39_WORK_BYTES { return BIP39_CAPACITY; }
  count := n * 3 / 4;                               // 11 * count = 8 * n + n / 4
  bits := work[B39_BITS..<B39_HASH];               // mutant: equivalent (a longer slice holds the same 33 bytes)
  for i: 0..<n { bits[i] = entropy[i]; }
  #assert sha256_hash_checked(entropy, work[B39_HASH..<B39_TEXT], false) == SHA_OK;  // mutant: equivalent (the hash fills the first 32 bytes of any longer slice)
  bits[n] = work[B39_HASH];                         // its first n / 4 bits are the checksum
  for i: 0..<count {
    v: u64 = 0;                                     // bits 11i to 11i + 10, from the top
    for b: 0..<11 {
      at := 11 * i + b;
      v = (v << 1) | ((cast(u64) bits[at / 8] >> cast(u64) (7 - at % 8)) & 1);
    }
    b39_put(work, i, b39_word_ct(v));
  }
  length := b39_join(work, count);
  result := length;
  if out.count < length { result = BIP39_CAPACITY; }
  else { for i: 0..<length { out[i] = work[B39_TEXT + i]; } }
  for i: 0..<BIP39_WORK_BYTES { work[i] = 0; }
  return result;
}
```

The bit positions are public: they depend only on the entropy's size.
The procedure refuses a size BIP-39 doesn't define with `BIP39_BAD_SIZE`,
and an output smaller than the phrase with `BIP39_CAPACITY`. The longest
phrase is 24 words of 8 letters and 23 spaces, 215 characters
(`BIP39_MAX_TEXT`).

## Words to entropy

`b39_read` splits the phrase, looks up every word, packs the 11-bit indexes
back into bits and compares the checksum. `bip39_decode_ct` calls it and
copies the entropy out:

<!-- source: coax/lib/crypto/bip39.coax decode -->
```jai
// Reads a phrase into work: its words in the slots, and their 11-bit
// indexes packed into the bits, entropy first and checksum after. Returns
// the entropy's byte count or a refusal, and leaves work to the caller.
b39_read :: (phrase: string, work: [] u8, room: int) -> int {
  for i: 0..<BIP39_WORK_BYTES { work[i] = 0; }
  if phrase.count > BIP39_MAX_PHRASE { return BIP39_TOO_LONG; }
  split := b39_split(phrase, work);
  if split >> 32 != 0 { return BIP39_NOT_ASCII; }
  count := cast(int) split;
  if count % 3 != 0 || count / 3 < 4 || count / 3 > 8 { return BIP39_BAD_COUNT; }
  n := count * 4 / 3;
  if room < n { return BIP39_CAPACITY; }
  bits := work[B39_BITS..<B39_HASH];               // mutant: equivalent (a longer slice holds the same 33 bytes)
  unknown: u64 = 0;
  for s: 0..<count {
    found := b39_index_ct(b39_slot(work, s));
    unknown |= (found >> 11) | cast(u64) work[B39_FLAGS + s];
    for b: 0..<11 {
      at := 11 * s + b;
      bits[at / 8] |= cast(u8) (((found >> cast(u64) (10 - b)) & 1) << cast(u64) (7 - at % 8));
    }
  }
  #assert sha256_hash_checked(bits[0..<n], work[B39_HASH..<B39_TEXT], false) == SHA_OK;  // mutant: equivalent (the hash fills the first 32 bytes of any longer slice)
  // keep the first count / 3 bits
  keep := cast(u64) ((0xff00 >> (count / 3)) & 0xff);  // mutant: equivalent (0xff01 differs only in bits the shift drops)
  wrong := cast(u64) (work[B39_HASH] ^ bits[n]) & keep;
  if unknown != 0 { return BIP39_UNKNOWN_WORD; }
  if wrong != 0 { return BIP39_BAD_CHECKSUM; }
  return n;
}
// 12, 15, 18, 21 or 24 words, separated by one or more spaces, back into
// their entropy. Returns its byte count (16 to 32) or a refusal, and
// writes out only on success. work comes back as zeros.
bip39_decode_ct :: (phrase: string, work: [] u8, out: [] u8) -> int #export_c {
  if work.count < BIP39_WORK_BYTES { return BIP39_CAPACITY; }
  result := b39_read(phrase, work, out.count);
  if result > 0 { for i: 0..<result { out[i] = work[B39_BITS + i]; } }  // mutant: equivalent (a result is 16 to 32 or negative, never 0 or 1)
  for i: 0..<BIP39_WORK_BYTES { work[i] = 0; }
  return result;
}
```

The checks run in this order, and the first that fails is the result:

1. A phrase over 1,024 bytes: `BIP39_TOO_LONG`.
2. A byte above `0x7f`: `BIP39_NOT_ASCII`.
3. A word count other than 12, 15, 18, 21 or 24: `BIP39_BAD_COUNT`.
4. An output smaller than the entropy: `BIP39_CAPACITY`.
5. A word that isn't in the list: `BIP39_UNKNOWN_WORD`.
6. Checksum bits that differ from the hash's: `BIP39_BAD_CHECKSUM`.

The phrase may have any number of spaces before, between and after its
words; only the space character `0x20` separates them. BIP-39's reference
implementation splits on single spaces and refuses the empty words that
extra spaces make. This code accepts them, because a person who types two
spaces still means the same words. Capitals aren't folded: `About` is not
`about`, and refuses as an unknown word.

## NFKD and ASCII

BIP-39 says that both the phrase and the passphrase are normalized with
Unicode NFKD before they are hashed. NFKD splits a character such as `é`
into `e` and a combining accent, and replaces compatibility characters such
as the full-width forms with their plain ones, so text that looks the same
hashes the same. NFKD changes no ASCII character.

This code has no Unicode tables, so it supports ASCII only. Rather than hash
text that might not be normalized, it refuses any byte above `0x7f` with
`BIP39_NOT_ASCII`, in the phrase and in the passphrase. The English list is
pure ASCII, so every English phrase works. A passphrase with an accent,
such as `café`, refuses.

The reference vectors show why the refusal matters. The file stores its
Japanese phrase in composed form, and BIP-39 hashes the NFKD form. The
example below runs PBKDF2 directly on each other language's bytes: for ten
languages it gives the published seed, because their phrases are already
in NFKD form, and for Japanese it doesn't. Code that skipped NFKD would
give a Japanese user a different seed, and so different keys, from every
other wallet.

## The seed

The seed is one 64-byte block of PBKDF2-HMAC-SHA512 (RFC 8018):

```text
seed = PBKDF2(HMAC-SHA512, password = the words joined by single spaces,
              salt = "mnemonic" ‖ passphrase, rounds = 2048, length = 64)
```

PBKDF2 computes `U1 = HMAC(password, salt ‖ 00 00 00 01)`, then
`U(i) = HMAC(password, U(i-1))` 2,047 more times, and XORs all 2,048
values. Chapter 7 builds `pbkdf2_hmac_sha512`, and chapter 8 explains
HMAC. The 2,048 rounds make each guess of a phrase or passphrase cost an
attacker 2,048 HMAC calls, while a wallet that runs it once barely notices
the cost.

<!-- source: coax/lib/crypto/bip39.coax seed -->
```jai
// The 64-byte seed of a checked phrase and a passphrase:
// PBKDF2-HMAC-SHA512 of the words joined by single spaces, with the salt
// "mnemonic" + passphrase and 2,048 rounds. Returns 64 or a refusal, and
// writes out only on success. work comes back as zeros.
bip39_seed_ct :: (phrase: string, passphrase: string, work: [] u8, out: [] u8) -> int #export_c {
  return b39_seed(phrase, passphrase, BIP39_ROUNDS, work, out);
}
// The same with any number of rounds, so the checks can run one.
b39_seed :: (phrase: string, passphrase: string, rounds: int, work: [] u8, out: [] u8) -> int {
  if out.count < BIP39_SEED_BYTES || work.count < BIP39_WORK_BYTES { return BIP39_CAPACITY; }
  if passphrase.count > BIP39_MAX_PASSPHRASE { return BIP39_TOO_LONG; }
  high: u64 = 0;
  for i: 0..<passphrase.count { high |= cast(u64) passphrase[i] >> 7; }
  if high != 0 { return BIP39_NOT_ASCII; }
  result := b39_read(phrase, work, 32);            // mutant: equivalent (any room of 32 or more fits the entropy)
  if result > 0 {                                   // mutant: equivalent (a result is 16 to 32 or negative, never 0 or 1)
    length := b39_join(work, result * 3 / 4);
    salt := work[B39_SALT..<B39_SALT + 8 + passphrase.count];
    prefix := "mnemonic";
    for i: 0..<8 { salt[i] = prefix[i]; }
    for i: 0..<passphrase.count { salt[8 + i] = passphrase[i]; }
    #assert pbkdf2_hmac_sha512(work[B39_TEXT..<B39_TEXT + length], salt, rounds, out);
    result = BIP39_SEED_BYTES;
  }
  for i: 0..<BIP39_WORK_BYTES { work[i] = 0; }
  return result;
}
```

`bip39_seed_ct` checks the phrase first, with the same refusals as
`bip39_decode_ct`, and hashes the checked words joined by single spaces.
BIP-39 itself allows a seed from a phrase with a wrong checksum after a
warning; this code refuses instead, so a mistyped phrase never gives a
wallet with the wrong keys. Before it reads the phrase, it refuses an
output smaller than 64 bytes or a work buffer that is too small
(`BIP39_CAPACITY`), a passphrase over 1,016 bytes (`BIP39_TOO_LONG`, so
that `mnemonic` and the passphrase fit PBKDF2's 1,024-byte salt) and a
passphrase byte above `0x7f` (`BIP39_NOT_ASCII`). `b39_seed` takes the
number of rounds, so the checks can run one or two rounds against Python;
the public procedure always runs 2,048.

The seed hashes the words, not the entropy. A phrase typed with extra
spaces gives the same seed here as the same words with single spaces,
because the words are joined again before they are hashed.

### Worked example: one round and 2,048

The example prints the first 16 bytes of PBKDF2 over the all-zero phrase
with the salt `mnemonicTREZOR`: after 1 round they are
`93b97bc5354ec701eae542a773d5de60`, and after 2,048 rounds
`c55257c360c07c72029aebc1b53c05ed`, the start of the published seed.

## The passphrase, the 25th word

Any passphrase, including the empty one, gives a valid seed. Nothing in
the words records whether a passphrase was used, and a wrong passphrase
isn't an error: it opens a different, empty wallet. The example shows four
passphrases on the same 12 words, including `TREZOR` and `TREZOR` with a
trailing space, and four unrelated seeds. People call the passphrase a 25th
word because the owner must keep it with the words, and losing it loses the
coins just as surely.

That property gives the passphrase its uses and its danger:

- Someone who finds the written words can't spend from the passphrase's
  wallet without the passphrase, and can't tell that it exists.
- An owner under duress can give up the words and a decoy passphrase.
- A typo in the passphrase gives no warning. A wallet should ask for it
  twice and check it against a known address before it receives coins.

## What the words protect against

- **Losing a device.** The words restore every key on any BIP-39 wallet.
- **Mistakes in writing the words down.** The checksum catches most single
  wrong words, and the 4-letter prefixes make words easy to tell apart.
- **Guessing.** 12 words hold 128 bits of entropy, and 24 hold 256. Each
  guess also pays for 2,048 rounds of PBKDF2.

They don't protect against:

- **Someone who reads the words.** The words are the keys. Only the
  passphrase stands between a found backup and the coins.
- **Weak entropy.** The checksum and PBKDF2 can't add randomness. Words
  that a person chose, or that came from a bad random generator, can be
  guessed.
- **A changed word that keeps the checksum.** One in 16 wrong 12-word
  phrases, and 1 in 256 wrong 24-word phrases, pass.

## The audit

`bip39.coax` was written for Nostr chapter 46 and the website's words
account. This chapter audited it and rebuilt its procedures on the code
above. The old procedures keep their names and answers for their callers.

- `bip39_index` walked the list and stopped at the match, and the reader
  called it on the typed words, so the time leaked each word's place in
  the list. The checked path now uses `b39_index_ct`.
- `bip39_words` walked the list to each word's index, which leaked the
  index, and `bip39_entropy_len` set each bit with a branch on its value.
- The split branched on each space, which leaked every word's length.
- `bip39_seed_with` copied the phrase into 200 bytes and refused anything
  longer. A 24-word phrase can be 215 characters, so a valid phrase of
  long words had no seed. The first Czech reference vector is 213
  characters long. The limit is now 1,024 bytes of input and 215 of joined
  words.
- A passphrase above `0x7f` was hashed as given, without NFKD. It now
  refuses.
- Every refusal was `-1` or `false`. The new procedures return seven
  distinct codes; the old ones still return `-1` or `false`.
- `bip39_seed` derived a seed from any words. It now refuses what
  `bip39_decode_ct` refuses. `nip06_key` already checked the words first,
  so NIP-06 keys don't change.
- The list's source was the iOS app's copy. It is now the pinned upstream
  `english.txt`, with the same SHA-256.

Nostr chapter 46's listings show its example and checks, which call the
old procedures; they are unchanged, and its fixture still matches.

## The example

The example is
[bitcoin/examples/seed-phrases.coax](../examples/seed-phrases.coax). It
works the all-zero checksum by hand, shows each size, reads the hamster
vector's first words as indexes, runs all 24 English reference vectors both
ways with their seeds, shows the passphrase and PBKDF2's rounds, counts the
last words that pass a checksum, tries the first vector of every other
language in the file, and prints each refusal.

<!-- source: bitcoin/examples/seed-phrases.coax example -->
```jai
#load "../../coax/lib/crypto/bip39.coax";
#load "../../coax/lib/nostr/hex.coax";
#load "../../coax/tests/bitcoin/bip39_vectors.coax";
#import "Basic";

// Print a label and bytes as lowercase hex.
show :: (label: string, bytes: [] u8) {
  text: [200] u8;
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
all_zero :: (work: [] u8) -> bool {
  for b: work { if b != 0 { return false; } }
  return true;
}
// The name of a refusal.
REASONS :: [7] string.["bad size", "bad count", "unknown word", "bad checksum", "not ASCII", "capacity", "too long"];
reason :: (status: int) -> string { return ifx status < 0 && status >= -7 then REASONS[-status - 1] else "accepted"; }
// The 11 bits of an index as text.
bits11 :: (v: int, out: [] u8) -> string {
  for b: 0..<11 { out[b] = cast(u8) (ifx (v >> (10 - b)) & 1 == 1 then '1' else '0'); }
  return cast(string) out[0..<11];
}

ZEROS :: "abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about";

main :: () {
  work: [BIP39_WORK_BYTES] u8; entropy: [32] u8; back: [32] u8; text: [BIP39_MAX_TEXT] u8;
  seed: [64] u8; want: [64] u8; h: [32] u8; buf: [11] u8;

  // The checksum by hand: 16 zero bytes. Their SHA-256 starts 0x37, so the
  // 4 checksum bits are 0011, and the 12th word is 7 zero bits then 0011.
  print("the checksum by hand\n");
  #assert sha256_hash_checked(entropy[0..<16], h[..], false) == SHA_OK;
  show("SHA-256 of 16 zero bytes, first byte", h[0..<1]);
  print("word 12: 0000000 + 0011 = % = %, \"%\"\n", bits11(3, buf[..]), 3, bip39_word(3));
  n := bip39_encode_ct(entropy[0..<16], work[..], text[..]);
  print("%\n", cast(string) text[0..<n]);

  // Each size: words, checksum bits, and the last word of all-zero entropy.
  print("sizes\n");
  for bytes: 4..<9 {
    m := bip39_encode_ct(entropy[0..<4 * bytes], work[..], text[..]);
    at := m; while text[at - 1] != ' ' { at -= 1; }
    print("% bytes: % words, % checksum bits, last word %\n", 4 * bytes, 3 * bytes, bytes, cast(string) text[at..<m]);
  }

  // A phrase back to its indexes and entropy: the hamster vector.
  print("words to indexes\n");
  v := BIP39_VECTORS[14];
  at := 0; k := 0;
  while at < v.phrase.count && k < 3 {
    e := at; while e < v.phrase.count && v.phrase[e] != ' ' { e += 1; }
    i := bip39_index(v.phrase[at..<e]);
    print("  %: % = %\n", v.phrase[at..<e], i, bits11(i, buf[..]));
    at = e + 1; k += 1;
  }
  m := bip39_decode_ct(v.phrase, work[..], back[..]);
  show("entropy", back[0..<m]);
  #assert sha256_hash_checked(back[0..<m], h[..], false) == SHA_OK;
  show("its SHA-256, first byte, all 8 bits checksum", h[0..<1]);

  // Every English vector both ways and its seed with the passphrase TREZOR.
  print("trezor/python-mnemonic vectors.json, English\n");
  to_words := 0; to_entropy := 0; seeds := 0; wiped := 0;
  for x: BIP39_VECTORS {
    want_e := decode(x.entropy, entropy[..]);
    n = bip39_encode_ct(want_e, work[..], text[..]);
    if n == x.phrase.count && cast(string) text[0..<n] == x.phrase { to_words += 1; }
    if all_zero(work[..]) { wiped += 1; }
    m = bip39_decode_ct(x.phrase, work[..], back[..]);
    if m == want_e.count && equal_bytes(back[0..<m], want_e) { to_entropy += 1; }
    if all_zero(work[..]) { wiped += 1; }
    r := bip39_seed_ct(x.phrase, "TREZOR", work[..], seed[..]);
    if r == 64 && equal_bytes(seed[..], decode(x.seed, want[..])) { seeds += 1; }
    if all_zero(work[..]) { wiped += 1; }
  }
  print("% of 24 entropy to words, % of 24 words to entropy, % of 24 seeds\n", to_words, to_entropy, seeds);
  print("work buffer zero after % of 72 calls\n", wiped);

  // The seed and its passphrase, the "25th word".
  print("the passphrase\n");
  #assert bip39_seed_ct(ZEROS, "", work[..], seed[..]) == 64;
  show("no passphrase", seed[0..<16]);
  #assert bip39_seed_ct(ZEROS, "TREZOR", work[..], seed[..]) == 64;
  show("TREZOR", seed[0..<16]);
  #assert bip39_seed_ct(ZEROS, "TREZOR ", work[..], seed[..]) == 64;
  show("TREZOR and a space", seed[0..<16]);
  #assert bip39_seed_ct(ZEROS, "trezor", work[..], seed[..]) == 64;
  show("trezor", seed[0..<16]);
  // PBKDF2 by hand: salt "mnemonicTREZOR", one round and then all 2048.
  salt := "mnemonicTREZOR"; pw: [1024] u8; sb: [14] u8;
  #assert pbkdf2_hmac_sha512(sha512_bytes(ZEROS, pw[..]), sha512_bytes(salt, sb[..]), 1, seed[..]);
  show("PBKDF2, 1 round", seed[0..<16]);
  #assert pbkdf2_hmac_sha512(sha512_bytes(ZEROS, pw[..]), sha512_bytes(salt, sb[..]), 2048, seed[..]);
  show("PBKDF2, 2048 rounds", seed[0..<16]);

  // How often a wrong last word passes the checksum.
  print("every last word\n");
  for which: 0..<2 {
    x := BIP39_VECTORS[ifx which == 0 then 0 else 14];
    e := x.phrase.count; while x.phrase[e - 1] != ' ' { e -= 1; }
    words := 0; pass := 0;
    for i: 0..<2048 {
      w := bip39_word(i); phrase: [BIP39_MAX_TEXT] u8;
      for c: 0..<e { phrase[c] = x.phrase[c]; }
      for c: 0..<w.count { phrase[e + c] = w[c]; }
      if bip39_decode_ct(cast(string) phrase[0..<e + w.count], work[..], back[..]) > 0 { pass += 1; }
      words += 1;
    }
    print("% words, last word replaced by each of %: % pass\n", ifx which == 0 then 12 else 24, words, pass);
  }

  // The other languages in vectors.json: this code reads English only, and
  // ASCII only. The seed hashes the phrase's bytes, not the list.
  print("other languages\n");
  for x: BIP39_OTHERS {
    r := bip39_decode_ct(x.phrase, work[..], back[..]);
    #assert pbkdf2_hmac_sha512(sha512_bytes(x.phrase, pw[..]), sha512_bytes(salt, sb[..]), 2048, seed[..]);
    print("%: %, PBKDF2 of its bytes is the published seed: %\n", x.language, reason(r), equal_bytes(seed[..], decode(x.seed, want[..])));
  }

  // Refusals.
  print("refusals\n");
  print("15 bytes of entropy: %\n", reason(bip39_encode_ct(entropy[0..<15], work[..], text[..])));
  print("11 words: %\n", reason(bip39_decode_ct("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about", work[..], back[..])));
  print("abut for about: %\n", reason(bip39_decode_ct("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abut", work[..], back[..])));
  print("About with a capital: %\n", reason(bip39_decode_ct("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon About", work[..], back[..])));
  print("abandon for about: %\n", reason(bip39_decode_ct("abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon", work[..], back[..])));
  print("words swapped: %\n", reason(bip39_decode_ct("legal winner thank year wave sausage worth useful legal winner yellow thank", work[..], back[..])));
  print("a passphrase with an accent: %\n", reason(bip39_seed_ct(ZEROS, "café", work[..], seed[..])));
  print("12 words into 15 bytes: %\n", reason(bip39_decode_ct(ZEROS, work[..], back[0..<15])));
  print("a seed into 63 bytes: %\n", reason(bip39_seed_ct(ZEROS, "", work[..], seed[0..<63])));
  long: [1025] u8; for i: 0..<1025 { long[i] = ' '; }
  print("1,025 bytes of phrase: %\n", reason(bip39_decode_ct(cast(string) long[..], work[..], back[..])));
  print("extra spaces: %\n", reason(bip39_decode_ct("  abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon   about ", work[..], back[..])));
}
```

Run it from the repository root:

```shell
coax/bin/coax run bitcoin/examples/seed-phrases.coax
```

The output:

```text
the checksum by hand
SHA-256 of 16 zero bytes, first byte: 37
word 12: 0000000 + 0011 = 00000000011 = 3, "about"
abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon abandon about
sizes
16 bytes: 12 words, 4 checksum bits, last word about
20 bytes: 15 words, 5 checksum bits, last word address
24 bytes: 18 words, 6 checksum bits, last word agent
28 bytes: 21 words, 7 checksum bits, last word admit
32 bytes: 24 words, 8 checksum bits, last word art
words to indexes
  hamster: 837 = 01101000101
  diagram: 487 = 00111100111
  private: 1369 = 10101011001
entropy: 68a79eaca2324873eacc50cb9c6eca8cc68ea5d936f98787c60c7ebc74e6ce7c
its SHA-256, first byte, all 8 bits checksum: 00
trezor/python-mnemonic vectors.json, English
24 of 24 entropy to words, 24 of 24 words to entropy, 24 of 24 seeds
work buffer zero after 72 of 72 calls
the passphrase
no passphrase: 5eb00bbddcf069084889a8ab91555681
TREZOR: c55257c360c07c72029aebc1b53c05ed
TREZOR and a space: 9d970e6454c54edc6e22391edcf27a07
trezor: f5057546039282b59dad6d6470999857
PBKDF2, 1 round: 93b97bc5354ec701eae542a773d5de60
PBKDF2, 2048 rounds: c55257c360c07c72029aebc1b53c05ed
every last word
12 words, last word replaced by each of 2048: 128 pass
24 words, last word replaced by each of 2048: 8 pass
other languages
chinese_simplified: not ASCII, PBKDF2 of its bytes is the published seed: true
chinese_traditional: not ASCII, PBKDF2 of its bytes is the published seed: true
czech: unknown word, PBKDF2 of its bytes is the published seed: true
french: unknown word, PBKDF2 of its bytes is the published seed: true
italian: unknown word, PBKDF2 of its bytes is the published seed: true
japanese: not ASCII, PBKDF2 of its bytes is the published seed: false
korean: not ASCII, PBKDF2 of its bytes is the published seed: true
portuguese: unknown word, PBKDF2 of its bytes is the published seed: true
spanish: not ASCII, PBKDF2 of its bytes is the published seed: true
russian: not ASCII, PBKDF2 of its bytes is the published seed: true
turkish: unknown word, PBKDF2 of its bytes is the published seed: true
refusals
15 bytes of entropy: bad size
11 words: bad count
abut for about: unknown word
About with a capital: unknown word
abandon for about: bad checksum
words swapped: bad checksum
a passphrase with an accent: not ASCII
12 words into 15 bytes: capacity
a seed into 63 bytes: capacity
1,025 bytes of phrase: too long
extra spaces: accepted
```

What the output shows:

- The checksum by hand: `0x37`, `0011`, index 3, `about`.
- Each size's word count and checksum bits. The last word of all-zero
  entropy changes with the size, because the checksum does.
- The hamster vector's first three words as indexes and bits, its entropy,
  and its hash's first byte, `00`, which is the whole 8-bit checksum.
- All 24 English vectors of trezor/python-mnemonic's `vectors.json`: entropy
  to words, words to entropy, and the seed with the passphrase `TREZOR`,
  with the work buffer zero after all 72 calls.
- Four passphrases give four seeds. A trailing space or a lowercase letter
  makes a different passphrase.
- PBKDF2 after 1 round and after 2,048, by direct calls.
- 128 of the 2,048 last words pass a 12-word checksum, and 8 pass a 24-word
  one.
- The other languages. The ASCII ones refuse as unknown words, the others as
  not ASCII. PBKDF2 of each phrase's bytes gives the published seed for
  every language but Japanese, whose phrase is stored in composed form.
- The refusals, one for each code, and a phrase with extra spaces that is
  accepted.

The book's checks run this program in debug and release builds and compare
the output with [bitcoin/fixtures/seed-phrases.out](../fixtures/seed-phrases.out).

## How the code is checked

### Checks in the example

The checks are in
[bitcoin/examples/checks/seed-phrases.coax](../examples/checks/seed-phrases.coax),
which loads the example:

- all 24 English vectors both ways, with the work buffer zero after each
  call and nothing written past the result;
- the last vector's seed, because 2,048 rounds are slow in the checker
  (all 24 seeds are in the example's output and in `bip39.py`);
- the first vector of every other language refuses as an unknown word or
  as not ASCII, in decoding and in the seed;
- the worked example's hash byte, from Python's `hashlib`;
- the hamster vector's last word: for each of the 8 values of its 3 entropy
  bits, the word that carries the hash's first byte passes and the word one
  checksum bit away fails;
- a passphrase with a trailing space, against `hashlib`.

<!-- source: bitcoin/examples/checks/seed-phrases.coax check -->
```jai
// Every English vector in trezor/python-mnemonic's vectors.json, both ways,
// with the work buffer zero after each call and nothing written past the
// result.
#forall "the reference vectors, both ways" i: 0..<24 #steps 400000000 {
  v := BIP39_VECTORS[i]; e: [32] u8; text: [BIP39_MAX_TEXT + 1] u8; back: [33] u8; work: [BIP39_WORK_BYTES] u8;
  want := decode(v.entropy, e[..]);
  text[v.phrase.count] = 7; back[want.count] = 7;
  #assert bip39_encode_ct(want, work[..], text[..]) == v.phrase.count && cast(string) text[0..<v.phrase.count] == v.phrase;
  #assert all_zero(work[..]) && text[v.phrase.count] == 7;
  #assert bip39_decode_ct(v.phrase, work[..], back[..]) == want.count && equal_bytes(back[0..<want.count], want);
  #assert all_zero(work[..]) && back[want.count] == 7;
}
// The last vector's seed with the passphrase TREZOR. All 24 seeds are in the
// example's output and in bip39.py; PBKDF2's 2,048 rounds are slow in the
// checker, so the checks run one.
#test "a reference seed" #steps 2000000000 {
  v := BIP39_VECTORS[23]; seed: [65] u8; want: [64] u8; work: [BIP39_WORK_BYTES] u8;
  seed[64] = 7;
  #assert bip39_seed_ct(v.phrase, "TREZOR", work[..], seed[..]) == 64;
  #assert equal_bytes(seed[0..<64], decode(v.seed, want[..])) && seed[64] == 7 && all_zero(work[..]);
}
// The other languages refuse: ASCII ones as unknown words, the rest before
// any lookup, as not ASCII. The seed procedure refuses the same way.
#forall "the other languages refuse" i: 0..<11 #steps 400000000 {
  x := BIP39_OTHERS[i]; back: [32] u8; seed: [64] u8; work: [BIP39_WORK_BYTES] u8;
  ascii := true;
  for c: 0..<x.phrase.count { if x.phrase[c] > 0x7f { ascii = false; } }
  want := ifx ascii then BIP39_UNKNOWN_WORD else BIP39_NOT_ASCII;
  #assert bip39_decode_ct(x.phrase, work[..], back[..]) == want;
  #assert bip39_seed_ct(x.phrase, "TREZOR", work[..], seed[..]) == want;
}
// The worked example: 16 zero bytes hash to 0x37..., from Python's hashlib.
#test "the checksum by hand" {
  e: [16] u8; h: [32] u8; buf: [11] u8;
  #assert sha256_hash_checked(e[..], h[..], false) == SHA_OK && h[0] == 0x37;
  #assert bits11(3, buf[..]) == "00000000011" && bits11(2047, buf[..]) == "11111111111" && bits11(1024, buf[..]) == "10000000000";
  #assert bip39_index("about") == 3;
}
// The hamster vector's last word holds 3 entropy bits and the 8-bit
// checksum. For each of the 8 values of those bits, the word whose low 8 bits
// are the first hash byte passes, and the word one checksum bit away fails.
// So exactly 8 of the 2,048 words can end the phrase.
#test "a 24-word phrase's last word: 3 entropy bits, 8 checksum bits" #steps 400000000 {
  x := BIP39_VECTORS[14]; work: [BIP39_WORK_BYTES] u8; back: [32] u8; phrase: [BIP39_MAX_TEXT] u8;
  e: [32] u8; h: [32] u8;
  #assert decode(x.entropy, e[..]).count == 32;
  cut := x.phrase.count; while x.phrase[cut - 1] != ' ' { cut -= 1; }
  for c: 0..<cut { phrase[c] = x.phrase[c]; }
  for k: 0..<8 {
    e[31] = cast(u8) ((cast(int) e[31] & 0xf8) | k);
    #assert sha256_hash_checked(e[..], h[..], false) == SHA_OK;
    for wrong: 0..<2 {
      w := bip39_word((k << 8) | (cast(int) h[0] ^ wrong));
      for c: 0..<w.count { phrase[cut + c] = w[c]; }
      n := bip39_decode_ct(cast(string) phrase[0..<cut + w.count], work[..], back[..]);
      if wrong == 0 { #assert n == 32 && equal_bytes(back[..], e[..]); }
      else { #assert n == BIP39_BAD_CHECKSUM; }
    }
  }
}
// A changed passphrase gives a different seed; the first bytes are from
// Python's hashlib (the empty passphrase's seed is in the example's output).
#test "a passphrase changes the seed" #steps 2000000000 {
  work: [BIP39_WORK_BYTES] u8; seed: [64] u8;
  #assert bip39_seed_ct(ZEROS, "TREZOR ", work[..], seed[..]) == 64 && seed[0] == 0x9d && seed[1] == 0x97;
}
#test "the helpers" {
  a := [3] u8.[1, 2, 3]; b := a; z: [3] u8;
  #assert equal_bytes(a[..], b[..]) && !equal_bytes(a[..], b[0..<2]);
  b[2] = 9; #assert !equal_bytes(a[..], b[..]);
  #assert all_zero(z[..]) && !all_zero(a[..]);
  #assert reason(-1) == "bad size" && reason(-7) == "too long" && reason(0) == "accepted" && reason(64) == "accepted" && reason(-8) == "accepted";
}
```

To run them:

```shell
coax/bin/coax test bitcoin/examples/checks/seed-phrases.coax
coax/bin/coax mutate bitcoin/examples/checks/seed-phrases.coax --root-only
```

`coax test` runs 85 checks, including the loaded files' own checks, in about 100 seconds on 2026-11-02 on a loaded machine. The checks file has no procedures of its own, so root-only mutation finds no site in it; the example's own procedures were not mutated for this chapter.

### The library's own checks

`bip39.coax` is an `#authority` file. Its own checks cover:

- the list's SHA-256, the table against `b39_make_table`, every word's
  length, and the distinct 4-letter prefixes;
- the masks, at small values and at the top bit;
- a spread of 64 indexes that sets every bit, through both constant-time
  lookups and both variable-time ones;
- the split with extra spaces, a 9-letter word, a tab, `DEL`, a control
  byte and a 25th word, and the join, which writes nothing past the text;
- reference vectors at 12, 18 and 24 words, and 15- and 21-word phrases
  from `hashlib`, both ways, with the work buffer zero and one byte too
  little room refused;
- each refusal code, with the output unwritten and the work buffer zero,
  including a wrong checksum bit at each length;
- the seed's password and salt at 1 and 2 rounds against `hashlib`,
  including the longest phrase (215 characters) and the longest passphrase
  (1,016 bytes), and one full 2,048-round seed;
- the older interface's answers.

```shell
coax/bin/coax test coax/lib/crypto/bip39.coax
coax/bin/coax mutate coax/lib/crypto/bip39.coax --root-only
```

`coax test` runs 75 checks, including those of the SHA-256 and SHA-512 files it loads, in about 30 seconds on 2026-11-02. Root-only mutation on 2026-11-02 makes 481 mutants: it kills 460, and 21 are marked equivalent in the source, each with its reason, such as a slice one byte longer than the bytes it holds. None survives.

### Python and OpenSSL, natively and in Wasm

[bip39.py](../../coax/tests/bitcoin/bip39.py) compiles `bip39.coax` to C in
checked and release builds and runs the three `_ct` procedures against a
separate Python implementation that reads the pinned `english.txt` and uses
`hashlib.sha256` and `hashlib.pbkdf2_hmac`. It shares no code with Coax.
Its cases are:

- every English vector and the first vector of each other language;
- 187 twelve-word phrases that between them use each of the 2,048 words,
  and 300 seeded random entropies of every size, with seeds under random
  ASCII passphrases for one phrase in six;
- one seeded edit of each phrase: a word replaced, swapped, removed or
  added, a letter changed, a capital, a tab, an accent, extra spaces, a
  letter added or removed;
- the limits: 1,024 and 1,025 phrase bytes, 1,016 and 1,017 passphrase
  bytes, every byte below `0x80` as a passphrase, and outputs one byte
  short.

Every result, every output and the work buffer must match. The OpenSSL
command line (`openssl kdf … PBKDF2`) computes the published vectors' seeds
and a sample of the others again, as a second PBKDF2. Then the harness
builds the same C for wasm32 and runs every case in Node.

```shell
python3 coax/tests/bitcoin/bip39.py
```

On 2026-11-02, 1,830 cases agree with Python in native checked and release builds and in Wasm checked and release builds, and OpenSSL agrees on 44 seeds. The run takes about 70 seconds.

## What this chapter leaves out

- Other languages' lists, and Unicode NFKD. Non-ASCII text refuses.
- SLIP-39 (Shamir shares of a seed) and Electrum's seed format, which uses
  a different checksum and salt. Neither is BIP-39, and neither is built.
- Accepting a 4-letter prefix for each word, and suggesting words while the
  owner types.
- Making the entropy: chapter 18.
- The `xprv` column of the reference vectors, the master key of each seed.
  That is BIP-32, chapter 32.

## Summary

- A phrase is the entropy and the first `ENT / 32` bits of its SHA-256, 11
  bits per word: 12 words for 16 bytes, up to 24 words for 32 bytes.
- The English list has 2,048 sorted words of 3 to 8 letters with distinct
  4-letter prefixes, pinned by its SHA-256.
- The seed is PBKDF2-HMAC-SHA512 of the words with the salt `mnemonic` and
  the passphrase, 2,048 rounds, 64 bytes. Any passphrase gives a valid seed.
- Lookups read the whole list, the phrase is split and joined with masks,
  and the secrets live in a work buffer that comes back as zeros. The
  length, the count and the result are public.
- The code supports ASCII only and refuses other bytes instead of skipping
  NFKD.
- Seven refusals have distinct codes.

## Dependencies

- [3. Constant-time rules and secrets](ch003-constant-time-rules.md)
- [4. SHA-256](ch004-sha-256.md)
- [7. SHA-512](ch007-sha-512.md)
- [8. HMAC-SHA256](ch008-hmac-sha256.md)

## Sources

- [BIP-39](https://github.com/bitcoin/bips/blob/master/bip-0039.mediawiki),
  pinned in
  [coax/tests/vectors/upstream/bip39/](../../coax/tests/vectors/upstream/bip39/bip-0039.mediawiki).
- trezor/python-mnemonic's `vectors.json` and `english.txt` at revision
  `b57a5ad7`, pinned in
  [coax/tests/vectors/upstream/bip39/](../../coax/tests/vectors/upstream/bip39/vectors.json)
  and listed in [SOURCES.md](../../coax/tests/vectors/SOURCES.md).
- [RFC 8018](https://www.rfc-editor.org/rfc/rfc8018) (PBKDF2).
- [Unicode Standard Annex #15](https://unicode.org/reports/tr15/)
  (normalization forms, NFKD).
