# 20. NIP-44 encrypted payloads

NIP-44 version 2 is the encryption that NIP-17 private messages, NIP-59 gift
wrap and many OpenAgents NIPs use. It joins four earlier chapters: the
conversation key from ECDH (chapter 14) and HKDF (chapter 6), the ChaCha20
stream (chapter 18) and an HMAC-SHA256 tag (chapter 5), all framed in base64
(chapter 1). This chapter builds the padding, the message keys, the payload
and the decryption order, and checks them against every entry of the
official vector file.

Part V, Encryption.

## What this chapter teaches

- Derive the conversation key with HKDF-extract over the raw ECDH x and the
  salt `nip44-v2`.
- Derive per-message keys with HKDF-expand, using the 32-byte nonce as
  `info`, and split the 76 bytes into a ChaCha20 key, a ChaCha20 nonce and
  an HMAC key.
- Pad the plaintext: a big-endian length prefix, then zeros up to a bucket
  that grows with the message (32 bytes at least).
- Lay out the payload as `version (0x02) || nonce || ciphertext || mac` and
  encode it in base64.
- Decrypt in a fixed order: check the version and the length, check the MAC
  in constant time, decrypt, and only then check the padding.
- Choose a profile: `NIP44_COMPAT` admits up to 65,535 bytes;
  `NIP44_EXTENDED` adds the 6-byte length prefix at 65,536 bytes and beyond.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [nip44.coax](../../coax/lib/nostr/nip44.coax) | `nip44_calc_padded_len(n)` | the padded length, with 64-bit arithmetic |
| | `nip44_write_prefix`, `nip44_unpad_span` | the 2-byte or 6-byte length prefix, written and read back |
| | `nip44_message_keys_checked(key, nonce, keys, overlap)` | HKDF-expand to 76 bytes |
| | `nip44_encrypt_checked(profile, key, nonce, plaintext, work, out, overlap)` | pad, encrypt, MAC, base64 |
| | `nip44_decrypt_checked(profile, key, payload, work, out, overlap)` | decode, check the MAC, decrypt, unpad |
| [nip44-key.coax](../../coax/lib/nostr/nip44-key.coax) | `nip44_conversation_key_from_keys_checked` | ECDH, then HKDF-extract (chapter 14) |

Both files are marked `#authority`: they decide whether a payload is
admitted, so every mutation of them must be killed by a check. The caller
supplies the nonce from a CSPRNG (`entropy.coax`, chapter 12). The code here
takes the nonce as input, so the same nonce always gives the same payload,
which is what lets the checks compare bytes with the published vectors.

## Keys

### The conversation key

Two parties share one conversation key. Alice computes it from her secret
key and Bob's public key; Bob computes the same key from his secret key and
Alice's public key:

```text
shared_x         = x(a·B) = x(b·A), 32 bytes, not hashed
conversation_key = HKDF-extract(salt = "nip44-v2", IKM = shared_x)
```

Chapter 14 builds this step. The key is fixed for the pair, so it is
computed once and kept.

### The message keys

Each message has a fresh 32-byte nonce. HKDF-expand turns the conversation
key and the nonce into 76 bytes, which are cut into three keys:

```text
keys         = HKDF-expand(PRK = conversation_key, info = nonce, L = 76)
chacha_key   = keys[0..32]
chacha_nonce = keys[32..44]
hmac_key     = keys[44..76]
```

HKDF-expand (chapter 6) produces 32-byte blocks, `T(1) = HMAC(PRK, nonce ||
01)`, `T(2) = HMAC(PRK, T(1) || nonce || 02)` and `T(3) = HMAC(PRK, T(2) ||
nonce || 03)`, and keeps the first 76 bytes of `T(1) || T(2) || T(3)`. So
the three keys do not line up with the blocks:

| Bytes | Block | Key |
| --- | --- | --- |
| 0 to 31 | all of `T(1)` | `chacha_key` |
| 32 to 43 | first 12 bytes of `T(2)` | `chacha_nonce` |
| 44 to 63 | last 20 bytes of `T(2)` | first 20 bytes of `hmac_key` |
| 64 to 75 | first 12 bytes of `T(3)` | last 12 bytes of `hmac_key` |

For row 0 of `get_message_keys` in the vector file, with conversation key
`a1a3d60f...c20b3b54` and nonce `e1e6f880...d2a02d72`:

```text
T(1) = f145f3bed47cb70dbeaac07f3a3fe683e822b3715edb7c4fe310829014ce7d76
T(2) = c4ad129bb01180c0933a160c 027c1db445f05e2eee864a0975b0ddef5b711058
T(3) = 3c8c192de3732571ca5838c4 526f41f27fb3b8e11b183b9909396f8401f1f6b3

chacha_key   = f145f3be...14ce7d76            (T(1))
chacha_nonce = c4ad129bb01180c0933a160c        (T(2), bytes 0-11)
hmac_key     = 027c1db4...5b711058 3c8c192de3732571ca5838c4
```

The rest of `T(3)` is not used. These are the published `chacha_key`,
`chacha_nonce` and `hmac_key` of that row; the example prints them.

Because every message has its own nonce, every message has its own ChaCha20
key and HMAC key. A repeated nonce would repeat the ChaCha20 keystream, and
the XOR of two ciphertexts would be the XOR of their plaintexts (chapter
18). It would not reveal the conversation key.

## Padding

The length of a ciphertext shows the length of the plaintext. NIP-44 rounds
the length up to a bucket, so that messages of similar length look the same.
The NIP's `calc_padded_len`:

```text
calc_padded_len(n):
  if n <= 32: return 32
  next_power = 1 << (floor(log2(n - 1)) + 1)
  chunk      = 32 if next_power <= 256 else next_power / 8
  return chunk * (floor((n - 1) / chunk) + 1)
```

`next_power` is the smallest power of two that is at least `n`. Up to 256
bytes the buckets are 32 bytes apart. Above that, each power-of-two range is
cut into 8 equal buckets, so the padding is less than `next_power / 8`, which
is less than a quarter of the message.

### Worked example: five lengths

1. `n = 1`: `n ≤ 32`, so 32.
2. `n = 32`: `n ≤ 32`, so 32.
3. `n = 33`: `n - 1 = 32`, `floor(log2 32) = 5`, `next_power = 64`, so
   `chunk = 32`. `floor(32 / 32) + 1 = 2`, so `32 · 2 = 64`.
4. `n = 37`: `n - 1 = 36`, `floor(log2 36) = 5`, `next_power = 64`,
   `chunk = 32`. `floor(36 / 32) + 1 = 2`, so 64.
5. `n = 300`: `n - 1 = 299`, `floor(log2 299) = 8` (256 ≤ 299 < 512),
   `next_power = 512`, so `chunk = 512 / 8 = 64`. `floor(299 / 64) + 1 =
   4 + 1 = 5`, so `64 · 5 = 320`.

The rows `(32, 32)`, `(33, 64)` and `(37, 64)` are in the vector file's
`calc_padded_len` list; 1 and 300 are checked against the NIP's pseudocode.

`nip44_calc_padded_len` computes `next_power` with a shift loop instead of a
logarithm, and uses 64-bit `int`: for `n = 2^32 - 1` the result is `2^32`,
which does not fit in 32 bits.

```jai
nip44_calc_padded_len :: (n:int)->int #export_c {
  if n<1 || n>NIP44_SPEC_MAX {return -1;}
  last:=n-1;rest:=last;power:=1;
  while rest>0 {rest=rest>>1;power=power<<1;}
  chunk:=32;
  if power>=512 {chunk=power/8;}
  return chunk*(last/chunk+1);
}
```

For `n ≤ 32` the loop gives `power ≤ 32`, `chunk = 32` and
`last / 32 + 1 = 1`, so the early return of the pseudocode is not needed.

### The length prefix

The padded plaintext is the length, the plaintext and zeros:

```text
n < 65,536:   [n: u16 big-endian]           plaintext  zeros
n ≥ 65,536:   [00 00] [n: u32 big-endian]   plaintext  zeros
```

The plaintext and zeros fill `calc_padded_len(n)` bytes; the prefix comes
before them. A plaintext is at least 1 byte, so a u16 of 0 never means a
length: it marks the 6-byte form. For `n = 1` and plaintext `a` the padded
plaintext is 34 bytes:

```text
00 01 61 00 00 ... 00      (2 + 32 = 34 bytes)
```

### Payload sizes

The decoded payload adds 65 bytes: the version byte, the 32-byte nonce and
the 32-byte MAC. Base64 writes 4 characters for every 3 bytes, rounded up.

| Plaintext | Padded | Prefix | Decoded payload | Base64 |
| ---: | ---: | ---: | ---: | ---: |
| 1 | 32 | 2 | 65 + 34 = 99 | 4 · 33 = 132 |
| 300 | 320 | 2 | 65 + 322 = 387 | 4 · 129 = 516 |
| 65,535 | 65,536 | 2 | 65,603 | 87,472 |
| 65,536 | 65,536 | 6 | 65,607 | 87,476 |
| 65,537 | 81,920 | 6 | 81,991 | 109,324 |

The first row is the smallest payload, so a decoder refuses anything under
132 characters or 99 decoded bytes before it does any cryptography. For
65,537, `n - 1 = 2^16`, `next_power = 2^17` and `chunk = 16,384`, so the
padded length jumps to `16,384 · 5 = 81,920`.

## Encrypt

`nip44_encrypt_checked` follows the NIP's steps:

1. Admit the request: a known profile, a 32-byte key and nonce, a plaintext
   of 1 byte up to the profile's maximum, and `work` and `out` large
   enough. Otherwise it returns a status and writes nothing.
2. Write the padded plaintext into `work`.
3. Derive the message keys.
4. Write the version byte `0x02` and the nonce, then ChaCha20 the padded
   plaintext with counter 0 into the ciphertext.
5. Compute `HMAC-SHA256(hmac_key, nonce || ciphertext)` into the last 32
   bytes. The nonce is the additional authenticated data: the MAC covers it
   as well as the ciphertext.
6. Base64-encode the 99 or more bytes into `out`.

### Worked example: row 0

Row 0 of `encrypt_decrypt` uses the secret keys 1 and 2, the nonce
`00...01` and the plaintext `a`. Its conversation key is `c41c7753...1ea1412d`.

1. The padded plaintext is `0001 61` and 31 zero bytes, 34 bytes.
2. The decoded payload is `02`, the 32-byte nonce, 34 bytes of ciphertext
   and 32 bytes of MAC: 99 bytes.
3. Base64 reads bits 6 at a time. The first three bytes `02 00 00` are
   `000000 100000 000000 000000`, the characters `A g A A`. That is why every
   version 2 payload starts with `A`, and this one starts `AgAAAA`. The
   nonce ends in `01`, so the long run of `A` ends in `B`.

The example prints the pieces and compares the payload with the published
one.

## Decrypt

`nip44_decrypt_checked` checks the cheap facts first and decrypts last:

```jai
  if payload.count==0 {return nip44_fail(NIP44_PAYLOAD);}
  if payload[0]=='#' {return nip44_fail(NIP44_VERSION);}
  if payload.count<NIP44_MIN_PAYLOAD {return nip44_fail(NIP44_PAYLOAD);}
  if payload.count>max_chars {return nip44_fail(NIP44_CAPACITY);}
```

1. **Empty** payload: `NIP44_PAYLOAD`.
2. **`#`** as the first character: `NIP44_VERSION`. The NIP reserves `#` for a
   future non-base64 encoding, so a reader says "unknown version", not "bad
   base64".
3. **Length.** Fewer than 132 characters is `NIP44_PAYLOAD`. More than the
   profile's largest payload is `NIP44_CAPACITY`, before any base64 is
   decoded, so a huge payload costs nothing.
4. **Base64.** Characters outside the alphabet or a non-canonical ending:
   `NIP44_BASE64`. Fewer than 99 decoded bytes: `NIP44_PAYLOAD`.
5. **Version byte.** Anything but `0x02`: `NIP44_VERSION`.
6. **MAC.** Derive the message keys, compute the MAC over nonce and
   ciphertext, and compare it with the last 32 bytes using `cb_ct_equal`
   (chapter 3), which reads every byte whatever the first difference.
   A mismatch is `NIP44_MAC`, and nothing has been decrypted.
7. **Decrypt** with ChaCha20 into `work`.
8. **Padding.** `nip44_unpad_span` reads the prefix and checks that the
   padded length is exactly `prefix + calc_padded_len(n)`. Otherwise
   `NIP44_PADDING`.
9. Copy the `n` plaintext bytes into `out`.

`out` is written only in step 9. Every refusal leaves it as it was.

Checking the MAC before decrypting means that a forged or altered payload
never reaches ChaCha20 or the padding check. An attacker who changes bits
learns only that the MAC failed; there is no padding error to probe, and no
plaintext is produced. The padding check runs after the MAC, so only the
holder of the conversation key could have made a payload that fails it.

Following the NIP's pseudocode, unpadding checks the prefix and the total
length, not that the padding bytes are zero. Encrypt always writes zeros.

## Profiles

The NIP text and the vector file disagree about long messages. The 2023
vector file lists 65,536 and 100,000 bytes under
`invalid.encrypt_msg_lengths`. The pinned NIP text, which is newer, makes
them valid with the 6-byte prefix and gives payload hashes for 65,535,
65,536 and 65,537 bytes. Each source has a named profile:

| Profile | Plaintext | Largest payload | Longer input |
| --- | --- | --- | --- |
| `NIP44_COMPAT` | 1 to 65,535 bytes | 87,472 characters | `NIP44_CAPACITY` |
| `NIP44_EXTENDED` | 1 to 917,504 bytes | 1,223,436 characters | `NIP44_CAPACITY` |

A compat reader refuses an extended payload by its length. It never reads
the `00 00` prefix as a 0-byte message. The extended maximum is a local
limit: the next bucket after 917,504, 1,048,576 bytes, would put the HMAC
input over the 1 MiB ceiling that this library's SHA-256, HMAC and ChaCha20
share. A check in `nip44.coax` asserts that the HMAC, ChaCha20 and base64
admission accept the maximum and refuse the next bucket.

## The example

The example is [nostr/examples/nip44.coax](../examples/nip44.coax). It
prints the padding table, splits a message key, derives row 0's
conversation key from both sides, encrypts row 0 and row 3 byte for byte,
decrypts row 0 back, and then shows the refusals: three edits of the row 0
payload, all 12 `invalid.decrypt` rows and the length limits. All keys are
published test values from the vector file; never use them for real
messages.

<!-- source: nostr/examples/nip44.coax example -->
```jai
#load "../../coax/lib/nostr/nip44-key.coax";
#load "../../coax/lib/nostr/hex.coax";
#import "Basic";

// Print a label and bytes as lowercase hex.
show :: (label: string, bytes: [] u8) {
  text: [160] u8;
  r := hex_encode_checked(cast(string) bytes, text[..], false);
  #assert r.error == 0;
  print("%: %\n", label, cast(string) text[0..<r.written]);
}
// Decode 64 hex digits into 32 bytes.
bytes_of :: (text: string) -> [32] u8 {
  b: [32] u8;
  #assert hex_decode_checked(text, b[..], false).error == 0;
  return b;
}
// The x-only public key of a secret: x(k·G) on the ladder of chapter 10.
public_x :: (secret: [] u8) -> [32] u8 {
  k := sc_reduce_bytes(secret).value;
  a := sg_to_affine(smc_multiply(k, sg_generator()).value).value;
  b: [32] u8;
  #assert sf_store(a.x, b[..]);
  return b;
}
same_bytes :: (a: [] u8, b: [] u8) -> bool {
  return a.count == b.count && cb_ct_equal(a, b) == CB_MAX64;
}
// The name of a NIP-44 status.
nip44_name :: (code: u32) -> string {
  if code == NIP44_OK { return "NIP44_OK"; }
  if code == NIP44_VERSION { return "NIP44_VERSION"; }
  if code == NIP44_PAYLOAD { return "NIP44_PAYLOAD"; }
  if code == NIP44_BASE64 { return "NIP44_BASE64"; }
  if code == NIP44_MAC { return "NIP44_MAC"; }
  if code == NIP44_PADDING { return "NIP44_PADDING"; }
  if code == NIP44_LENGTH { return "NIP44_LENGTH"; }
  if code == NIP44_CAPACITY { return "NIP44_CAPACITY"; }
  return "other";
}
// Decrypt with work and out filled with 0xa5 first. Report the status,
// whether out kept every 0xa5, and whether the area where the padded
// plaintext would be decrypted kept every 0xa5.
refuse :: (label: string, key: [] u8, payload: string) {
  work: [512] u8; for i: 0..<512 { work[i] = 165; }
  out: [256] u8; for i: 0..<256 { out[i] = 165; }
  r := nip44_decrypt_checked(NIP44_COMPAT, key, payload, work[..], out[..], false);
  out_kept := true; for b: out { if b != 165 { out_kept = false; } }
  dlen := base64_decode_size(payload);
  decrypted := false;
  if dlen >= NIP44_MIN_DECODED {
    for i: dlen..<2 * dlen - NIP44_OVERHEAD { if work[i] != 165 { decrypted = true; } }
  }
  print("%: %, out untouched %, ciphertext decrypted %\n", label, nip44_name(r.error), out_kept, decrypted);
}

main :: () {
  // Padding: the padded length, the prefix and the payload sizes.
  lengths := [8] int.[1, 32, 33, 37, 300, 65535, 65536, 65537];
  for n: lengths {
    print("% bytes: padded %, prefix %, payload % bytes, % characters\n", n,
          nip44_calc_padded_len(n), nip44_prefix_len(n),
          nip44_raw_size(NIP44_EXTENDED, n), nip44_payload_size(NIP44_EXTENDED, n));
  }

  // Message keys: get_message_keys row 0. HKDF-expand(conversation key,
  // info = nonce, L = 76), split 32 / 12 / 32.
  mk := bytes_of("a1a3d60f3470a8612633924e91febf96dc5366ce130f658b1f0fc652c20b3b54");
  mn := bytes_of("e1e6f880560d6d149ed83dcc7e5861ee62a5ee051f7fde9975fe5d25d2a02d72");
  keys: [76] u8;
  #assert nip44_message_keys_checked(mk[..], mn[..], keys[..], false) == NIP44_OK;
  show("chacha key", keys[0..<32]);
  show("chacha nonce", keys[32..<44]);
  show("hmac key", keys[44..<76]);
  want_key := bytes_of("f145f3bed47cb70dbeaac07f3a3fe683e822b3715edb7c4fe310829014ce7d76");
  want_mac := bytes_of("027c1db445f05e2eee864a0975b0ddef5b7110583c8c192de3732571ca5838c4");
  print("match the vector: %\n", same_bytes(keys[0..<32], want_key[..]) && same_bytes(keys[44..<76], want_mac[..]));

  // encrypt_decrypt row 0: sec1 = 1, sec2 = 2. Published test values only.
  sec1 := bytes_of("0000000000000000000000000000000000000000000000000000000000000001");
  sec2 := bytes_of("0000000000000000000000000000000000000000000000000000000000000002");
  pub1 := public_x(sec1[..]); pub2 := public_x(sec2[..]);
  scratch: [1] EcdhScratch; ck1: [32] u8; ck2: [32] u8;
  #assert nip44_conversation_key_from_keys_checked(sec1[..], pub2[..], scratch[..], ck1[..], false) == NIP44_OK;
  #assert nip44_conversation_key_from_keys_checked(sec2[..], pub1[..], scratch[..], ck2[..], false) == NIP44_OK;
  show("conversation key", ck1[..]);
  print("same from both sides: %\n", same_bytes(ck1[..], ck2[..]));

  nonce := bytes_of("0000000000000000000000000000000000000000000000000000000000000001");
  plain := [1] u8.['a'];
  work: [512] u8; payload: [256] u8;
  r := nip44_encrypt_checked(NIP44_COMPAT, ck1[..], nonce[..], plain[..], work[..], payload[..], false);
  text := cast(string) payload[0..<r.written];
  print("encrypt: %, % characters\n", nip44_name(r.error), r.written);
  // The encrypt work holds the 99 decoded payload bytes, then the padded plaintext.
  show("padded plaintext", work[99..<133]);
  show("version", work[0..<1]);
  show("nonce", work[1..<33]);
  show("ciphertext", work[33..<67]);
  show("mac", work[67..<99]);
  print("payload: %\n", text);
  published := "AgAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABee0G5VSK0/9YypIObAtDKfYEAjD35uVkHyB0F4DwrcNaCXlCWZKaArsGrY6M9wnuTMxWfp1RTN9Xga8no+kF5Vsb";
  print("equals the published payload: %\n", text == published);
  dwork: [512] u8; back: [256] u8;
  d := nip44_decrypt_checked(NIP44_COMPAT, ck2[..], text, dwork[..], back[..], false);
  print("decrypt with the other side's key: %, \"%\"\n", nip44_name(d.error), cast(string) back[0..<d.written]);

  // encrypt_decrypt row 3: 19 bytes of UTF-8, one 32-byte bucket.
  ck3 := bytes_of("d5a2f879123145a4b291d767428870f5a8d9e5007193321795b40183d4ab8c2b");
  nonce3 := bytes_of("b20989adc3ddc41cd2c435952c0d59a91315d8c5218d5040573fc3749543acaf");
  text3 := "ability🤝的 ȺȾ";
  plain3: [19] u8; for i: 0..<19 { plain3[i] = text3[i]; }
  r3 := nip44_encrypt_checked(NIP44_COMPAT, ck3[..], nonce3[..], plain3[..], work[..], payload[..], false);
  published3 := "ArIJia3D3cQc0sQ1lSwNWakTFdjFIY1QQFc/w3SVQ6yvbG2S0x4Yu86QGwPTy7mP3961I1XqB6SFFTzqDZZavhxoWMj7mEVGMQIsh2RLWI5EYQaQDIePSnXPlzf7CIt+voTD";
  print("row 3: % plaintext bytes, payload equals the published one: %\n", text3.count, cast(string) payload[0..<r3.written] == published3);

  // Refusals. First, three edits of the row 0 payload.
  edited: [132] u8;
  for i: 0..<132 { edited[i] = text[i]; }
  edited[131] = 'c';
  refuse("last MAC character b -> c", ck2[..], cast(string) edited[..]);
  edited[131] = text[131]; edited[60] = 'A';
  refuse("ciphertext character 60 -> A", ck2[..], cast(string) edited[..]);
  wrong := ck2; wrong[0] = wrong[0] ^ 1;
  refuse("conversation key with one bit flipped", wrong[..], text);

  // The invalid.decrypt rows of nip44.vectors.json.
  k0 := bytes_of("ca2527a037347b91bea0c8a30fc8d9600ffd81ec00038671e3a0f0cb0fc9f642");
  refuse("row 0, leading #", k0[..], "#Atqupco0WyaOW2IGDKcshwxI9xO8HgD/P8Ddt46CbxDbrhdG8VmJdU0MIDf06CUvEvdnr1cp1fiMtlM/GrE92xAc1K5odTpCzUB+mjXgbaqtntBUbTToSUoT0ovrlPwzGjyp");
  k1 := bytes_of("36f04e558af246352dcf73b692fbd3646a2207bd8abd4b1cd26b234db84d9481");
  refuse("row 1, version byte 0", k1[..], "AK1AjUvoYW3IS7C/BGRUoqEC7ayTfDUgnEPNeWTF/reBZFaha6EAIRueE9D1B1RuoiuFScC0Q94yjIuxZD3JStQtE8JMNacWFs9rlYP+ZydtHhRucp+lxfdvFlaGV/sQlqZz");
  refuse("row 2, a Cyrillic letter in the base64", k0[..], "Atфupco0WyaOW2IGDKcshwxI9xO8HgD/P8Ddt46CbxDbrhdG8VmJZE0UICD06CUvEvdnr1cp1fiMtlM/GrE92xAc1EwsVCQEgWEu2gsHUVf4JAa3TpgkmFc3TWsax0v6n/Wq");
  k3 := bytes_of("cff7bd6a3e29a450fd27f6c125d5edeb0987c475fd1e8d97591e0d4d8a89763c");
  refuse("row 3, MAC of zeros", k3[..], "Agn/l3ULCEAS4V7LhGFM6IGA17jsDUaFCKhrbXDANholyySBfeh+EN8wNB9gaLlg4j6wdBYh+3oK+mnxWu3NKRbSvQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA");
  k4 := bytes_of("cfcc9cf682dfb00b11357f65bdc45e29156b69db424d20b3596919074f5bf957");
  refuse("row 4, wrong MAC", k4[..], "AmWxSwuUmqp9UsQX63U7OQ6K1thLI69L7G2b+j4DoIr0oRWQ8avl4OLqWZiTJ10vIgKrNqjoaX+fNhE9RqmR5g0f6BtUg1ijFMz71MO1D4lQLQfW7+UHva8PGYgQ1QpHlKgR");
  k5 := bytes_of("5254827d29177622d40a7b67cad014fe7137700c3c523903ebbe3e1b74d40214");
  refuse("row 5, bad padding", k5[..], "Anq2XbuLvCuONcr7V0UxTh8FAyWoZNEdBHXvdbNmDZHB573MI7R7rrTYftpqmvUpahmBC2sngmI14/L0HjOZ7lWGJlzdh6luiOnGPc46cGxf08MRC4CIuxx3i2Lm0KqgJ7vA");
  k6 := bytes_of("fea39aca9aa8340c3a78ae1f0902aa7e726946e4efcd7783379df8096029c496");
  refuse("row 6, bad padding", k6[..], "An1Cg+O1TIhdav7ogfSOYvCj9dep4ctxzKtZSniCw5MwRrrPJFyAQYZh5VpjC2QYzny5LIQ9v9lhqmZR4WBYRNJ0ognHVNMwiFV1SHpvUFT8HHZN/m/QarflbvDHAtO6pY16");
  k7 := bytes_of("0c4cffb7a6f7e706ec94b2e879f1fc54ff8de38d8db87e11787694d5392d5b3f");
  refuse("row 7, bad padding", k7[..], "Am+f1yZnwnOs0jymZTcRpwhDRHTdnrFcPtsBzpqVdD6b2NZDaNm/TPkZGr75kbB6tCSoq7YRcbPiNfJXNch3Tf+o9+zZTMxwjgX/nm3yDKR2kHQMBhVleCB9uPuljl40AJ8kXRD0gjw+aYRJFUMK9gCETZAjjmrsCM+nGRZ1FfNsHr6Z");
  refuse("row 8, empty payload", k0[..], "");
  refuse("row 9, 4 characters", k0[..], "Ag==");
  refuse("row 10, 48 characters", k0[..], "AqxgToSh3H7iLYRJjoWAM+vSv/Y1mgNlm6OWWjOYUClrFF8=");
  refuse("row 11, 92 characters", k0[..], "Ap/2SEZCVFIhYk6qx7nqJxM6TMI1ZoKmAzrO7vBDVJhhuZXWiM20i/tIsbjT0KxkJs2MZjh1oXNYMO9ggfk7i47WQA==");

  // Lengths at the profile edges. The request check runs before any work.
  print("encrypt 0 bytes: %\n", nip44_name(nip44_encrypt_request(NIP44_COMPAT, 32, 32, 0, 1 << 22, 1 << 22, false)));
  for n: [3] int.[65535, 65536, 65537] {
    compat := nip44_encrypt_request(NIP44_COMPAT, 32, 32, n, 1 << 22, 1 << 22, false);
    extended := nip44_encrypt_request(NIP44_EXTENDED, 32, 32, n, 1 << 22, 1 << 22, false);
    print("encrypt % bytes: compat %, extended %\n", n, nip44_name(compat), nip44_name(extended));
  }
  // A compat reader refuses an 87,476-character payload (a 65,536-byte
  // plaintext) by its length, before decoding any base64.
  wide: [87476] u8; for i: 0..<87476 { wide[i] = 'A'; }
  out: [1] u8;
  big := nip44_decrypt_checked(NIP44_COMPAT, ck1[..], cast(string) wide[..], work[..], out[..], false);
  print("decrypt 87476 characters as compat: %\n", nip44_name(big.error));
}
```

Run it from the repository root:

```shell
coax/bin/coax run nostr/examples/nip44.coax
```

It prints:

```text
1 bytes: padded 32, prefix 2, payload 99 bytes, 132 characters
32 bytes: padded 32, prefix 2, payload 99 bytes, 132 characters
33 bytes: padded 64, prefix 2, payload 131 bytes, 176 characters
37 bytes: padded 64, prefix 2, payload 131 bytes, 176 characters
300 bytes: padded 320, prefix 2, payload 387 bytes, 516 characters
65535 bytes: padded 65536, prefix 2, payload 65603 bytes, 87472 characters
65536 bytes: padded 65536, prefix 6, payload 65607 bytes, 87476 characters
65537 bytes: padded 81920, prefix 6, payload 81991 bytes, 109324 characters
chacha key: f145f3bed47cb70dbeaac07f3a3fe683e822b3715edb7c4fe310829014ce7d76
chacha nonce: c4ad129bb01180c0933a160c
hmac key: 027c1db445f05e2eee864a0975b0ddef5b7110583c8c192de3732571ca5838c4
match the vector: true
conversation key: c41c775356fd92eadc63ff5a0dc1da211b268cbea22316767095b2871ea1412d
same from both sides: true
encrypt: NIP44_OK, 132 characters
padded plaintext: 00016100000000000000000000000000000000000000000000000000000000000000
version: 02
nonce: 0000000000000000000000000000000000000000000000000000000000000001
ciphertext: 79ed06e5548ad3ff58ca920e6c0b4329f6040230f7e6e5641f20741780f0adc35a09
mac: 794259929a02bb06ad8e8cf709ee4ccc567e9d514cdf5781af27a3e905e55b1b
payload: AgAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABee0G5VSK0/9YypIObAtDKfYEAjD35uVkHyB0F4DwrcNaCXlCWZKaArsGrY6M9wnuTMxWfp1RTN9Xga8no+kF5Vsb
equals the published payload: true
decrypt with the other side's key: NIP44_OK, "a"
row 3: 19 plaintext bytes, payload equals the published one: true
last MAC character b -> c: NIP44_MAC, out untouched true, ciphertext decrypted false
ciphertext character 60 -> A: NIP44_MAC, out untouched true, ciphertext decrypted false
conversation key with one bit flipped: NIP44_MAC, out untouched true, ciphertext decrypted false
row 0, leading #: NIP44_VERSION, out untouched true, ciphertext decrypted false
row 1, version byte 0: NIP44_VERSION, out untouched true, ciphertext decrypted false
row 2, a Cyrillic letter in the base64: NIP44_BASE64, out untouched true, ciphertext decrypted false
row 3, MAC of zeros: NIP44_MAC, out untouched true, ciphertext decrypted false
row 4, wrong MAC: NIP44_MAC, out untouched true, ciphertext decrypted false
row 5, bad padding: NIP44_PADDING, out untouched true, ciphertext decrypted true
row 6, bad padding: NIP44_PADDING, out untouched true, ciphertext decrypted true
row 7, bad padding: NIP44_PADDING, out untouched true, ciphertext decrypted true
row 8, empty payload: NIP44_PAYLOAD, out untouched true, ciphertext decrypted false
row 9, 4 characters: NIP44_PAYLOAD, out untouched true, ciphertext decrypted false
row 10, 48 characters: NIP44_PAYLOAD, out untouched true, ciphertext decrypted false
row 11, 92 characters: NIP44_PAYLOAD, out untouched true, ciphertext decrypted false
encrypt 0 bytes: NIP44_LENGTH
encrypt 65535 bytes: compat NIP44_OK, extended NIP44_OK
encrypt 65536 bytes: compat NIP44_CAPACITY, extended NIP44_OK
encrypt 65537 bytes: compat NIP44_CAPACITY, extended NIP44_OK
decrypt 87476 characters as compat: NIP44_CAPACITY
```

The first eight lines are the padding table above. The message keys match
row 0 of `get_message_keys`. The row 0 payload is the published one byte for
byte, and the other side's key decrypts it to `a`. Row 3 is 19 bytes of
UTF-8 (`🤝` is 4 bytes, `的` is 3 and each of `Ⱥ` and `Ⱦ` is 2), so it fits
one 32-byte bucket.

The refusals, with the 0xa5 bytes that `refuse` puts in `work` and `out`
first:

- **A changed MAC character, a changed ciphertext character and a wrong
  key** each give `NIP44_MAC`. The MAC covers the nonce and the ciphertext,
  so a change in either is caught, and a wrong key gives a wrong HMAC key.
  `ciphertext decrypted false` means the area where ChaCha20 would write
  still holds 0xa5: nothing was decrypted.
- **Rows 0 and 1** give `NIP44_VERSION`: a leading `#`, then a decoded
  version byte of 0.
- **Row 2** has the Cyrillic `ф` (2 bytes of UTF-8) in place of a base64
  character: `NIP44_BASE64`.
- **Rows 3 and 4** have a MAC of zeros and a wrong MAC: `NIP44_MAC`, with
  nothing decrypted.
- **Rows 5 to 7** have a valid MAC, so they are decrypted, and the padding
  is refused. Their decrypted prefixes, computed separately in Python, are
  `0039` (57 bytes, which needs 2 + 64 = 66 bytes, not 34), `0000` (the
  extended form, with the next 4 bytes `nobl` read as the length
  1,852,793,452) and `002e` (46 bytes, which needs 66 bytes, not 67). `out`
  is still untouched.
- **Rows 8 to 11** are 0, 4, 48 and 92 characters, all under 132:
  `NIP44_PAYLOAD`.
- An empty plaintext is `NIP44_LENGTH`. 65,535 bytes is admitted by both
  profiles; 65,536 and 65,537 only by `NIP44_EXTENDED`. A compat reader
  refuses an 87,476-character payload as `NIP44_CAPACITY` before decoding
  it.

The book's checks run this program in debug and release builds and compare
the output with [nostr/fixtures/nip44.out](../fixtures/nip44.out).

## How the code is checked

### Checks in the example

The checks are in
[nostr/examples/checks/nip44.coax](../examples/checks/nip44.coax), which
loads the example. They compare the code with the vector file, the NIP's
pseudocode and the NIP's extended-prefix table:

- all 24 `calc_padded_len` rows, the pseudocode's `next_power` for nine
  lengths worked by hand, and agreement with the pseudocode for
  every `n` from 1 to 2,999, around every power of two up to `2^31` and at
  `2^32 - 1`;
- four `get_message_keys` rows, split 32 / 12 / 32;
- all 10 `encrypt_decrypt` rows the way the NIP describes: compute each
  public key, derive the conversation key from both sides through chapter
  14's ECDH, encrypt and compare the payload, decrypt and compare the
  plaintext, under both profiles;
- all 12 `invalid.decrypt` rows, each with its status under both profiles,
  `out` never written, and no decryption before a MAC match;
- one flipped bit in each of the 98 decoded bytes after the version byte of
  row 0, each refused as `NIP44_MAC`;
- all 4 `invalid.encrypt_msg_lengths` under both profiles;
- the NIP's 65,535, 65,536 and 65,537-byte payloads, whose SHA-256 must
  match the NIP's table, with `NIP44_COMPAT` refusing the two longer ones
  both to encrypt and to decrypt.

<!-- source: nostr/examples/checks/nip44.coax check -->
```jai
// Decode an even number of hex digits into buf; return the byte count.
hex_into :: (text: string, buf: [] u8) -> int {
  r := hex_decode_checked(text, buf[0..<text.count / 2], false);
  #assert r.error == 0;
  return text.count / 2;
}

// The NIP's next_power, 1 << (floor(log2(n - 1)) + 1), with floor(log2(n - 1))
// as the index of the top set bit of n - 1. For n = 1 it gives 2.
spec_next_power :: (n: int) -> int {
  top := 0; v := n - 1;
  while v > 1 { v = v >> 1; top += 1; }
  return 1 << (top + 1);
}
#test "next_power by hand" {
  ns := [9] int.[2, 3, 4, 5, 33, 37, 256, 257, 300];
  powers := [9] int.[2, 4, 4, 8, 64, 64, 256, 512, 512];
  for i: 0..<9 { #assert spec_next_power(ns[i]) == powers[i]; }
}
// calc_padded_len from the NIP's pseudocode. Its chunk is 32 while
// next_power <= 256 and next_power / 8 above: the loop doubles 32 until
// 8 * chunk reaches next_power. Its n <= 32 case needs no branch: there
// next_power <= 32, chunk is 32 and (n - 1) / 32 is 0.
spec_padded_len :: (n: int) -> int {
  next_power := spec_next_power(n);
  chunk := 32;
  while (chunk << 3) < next_power { chunk = chunk * 2; }
  return chunk * ((n - 1) / chunk + 1);
}
// The 24 valid.calc_padded_len rows: (unpadded, padded).
PADDED :: [48] int.[16, 32, 32, 32, 33, 64, 37, 64, 45, 64, 49, 64, 64, 64, 65, 96, 100, 128, 111, 128, 200, 224, 250, 256, 320, 320, 383, 384, 384, 384, 400, 448, 500, 512, 512, 512, 515, 640, 700, 768, 800, 896, 900, 1024, 1020, 1024, 65536, 65536];
#test "calc_padded_len: the 24 published rows and the hand values" {
  for i: 0..<24 { #assert nip44_calc_padded_len(PADDED[2 * i]) == PADDED[2 * i + 1]; }
  #assert nip44_calc_padded_len(1) == 32 && nip44_calc_padded_len(37) == 64 && nip44_calc_padded_len(300) == 320;
}
#forall "calc_padded_len agrees with the NIP's pseudocode" n: 1..<3000 {
  #assert nip44_calc_padded_len(n) == spec_padded_len(n);
}
#test "calc_padded_len agrees with the pseudocode at every power of two" {
  for k: 1..<32 {
    p := 1 << k;
    #assert nip44_calc_padded_len(p - 1) == spec_padded_len(p - 1);
    #assert nip44_calc_padded_len(p) == spec_padded_len(p);
    #assert nip44_calc_padded_len(p + 1) == spec_padded_len(p + 1);
  }
  #assert nip44_calc_padded_len(4294967295) == spec_padded_len(4294967295);
}

// Four valid.get_message_keys rows: (nonce, chacha_key, chacha_nonce, hmac_key).
MK_KEY :: "a1a3d60f3470a8612633924e91febf96dc5366ce130f658b1f0fc652c20b3b54";
MK_ROWS :: [16] string.[
    "e1e6f880560d6d149ed83dcc7e5861ee62a5ee051f7fde9975fe5d25d2a02d72", "f145f3bed47cb70dbeaac07f3a3fe683e822b3715edb7c4fe310829014ce7d76", "c4ad129bb01180c0933a160c", "027c1db445f05e2eee864a0975b0ddef5b7110583c8c192de3732571ca5838c4", // row 0
    "e1d6d28c46de60168b43d79dacc519698512ec35e8ccb12640fc8e9f26121101", "e35b88f8d4a8f1606c5082f7a64b100e5d85fcdb2e62aeafbec03fb9e860ad92", "22925e920cee4a50a478be90", "46a7c55d4283cb0df1d5e29540be67abfe709e3b2e14b7bf9976e6df994ded30", // row 1
    "e9d1eba47dd7e6c1532dc782ff63125db83042bb32841db7eeafd528f3ea7af9", "54241f68dc2e50e1db79e892c7c7a471856beeb8d51b7f4d16f16ab0645d2f1a", "a963ed7dc29b7b1046820a1d", "aba215c8634530dc21c70ddb3b3ee4291e0fa5fa79be0f85863747bde281c8b2", // row 15
    "2dcf39b9d4c52f1cb9db2d516c43a7c6c3b8c401f6a4ac8f131a9e1059957036", "17f8057e6156ba7cc5310d01eda8c40f9aa388f9fd1712deb9511f13ecc37d27", "a8188daff807a1182200b39d", "47b89da97f68d389867b5d8a2d7ba55715a30e3d88a3cc11f3646bc2af5580ef"]; // row 31
#forall "message keys split 32 / 12 / 32 as published" i: 0..<4 {
  ck := bytes_of(MK_KEY); nonce := bytes_of(MK_ROWS[4 * i]);
  keys: [77] u8; keys[76] = 165;
  #assert nip44_message_keys_checked(ck[..], nonce[..], keys[..], false) == NIP44_OK;
  want: [32] u8;
  #assert hex_into(MK_ROWS[4 * i + 1], want[..]) == 32 && same_bytes(keys[0..<32], want[..]);
  #assert hex_into(MK_ROWS[4 * i + 2], want[..]) == 12 && same_bytes(keys[32..<44], want[0..<12]);
  #assert hex_into(MK_ROWS[4 * i + 3], want[..]) == 32 && same_bytes(keys[44..<76], want[..]);
  #assert keys[76] == 165;
}

// All 10 valid.encrypt_decrypt rows: (sec1, sec2, conversation_key, nonce,
// plaintext as UTF-8 hex, payload). Published test keys only.
ED_ROWS :: [60] string.[
    "0000000000000000000000000000000000000000000000000000000000000001", "0000000000000000000000000000000000000000000000000000000000000002", "c41c775356fd92eadc63ff5a0dc1da211b268cbea22316767095b2871ea1412d", "0000000000000000000000000000000000000000000000000000000000000001", "61", "AgAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAABee0G5VSK0/9YypIObAtDKfYEAjD35uVkHyB0F4DwrcNaCXlCWZKaArsGrY6M9wnuTMxWfp1RTN9Xga8no+kF5Vsb",
    "0000000000000000000000000000000000000000000000000000000000000002", "0000000000000000000000000000000000000000000000000000000000000001", "c41c775356fd92eadc63ff5a0dc1da211b268cbea22316767095b2871ea1412d", "f00000000000000000000000000000f00000000000000000000000000000000f", "f09f8d95f09fab83", "AvAAAAAAAAAAAAAAAAAAAPAAAAAAAAAAAAAAAAAAAAAPSKSK6is9ngkX2+cSq85Th16oRTISAOfhStnixqZziKMDvB0QQzgFZdjLTPicCJaV8nDITO+QfaQ61+KbWQIOO2Yj",
    "5c0c523f52a5b6fad39ed2403092df8cebc36318b39383bca6c00808626fab3a", "4b22aa260e4acb7021e32f38a6cdf4b673c6a277755bfce287e370c924dc936d", "3e2b52a63be47d34fe0a80e34e73d436d6963bc8f39827f327057a9986c20a45", "b635236c42db20f021bb8d1cdff5ca75dd1a0cc72ea742ad750f33010b24f73b", "e8a1a8e3839de3818241e9b797c592c3a9efbca2e9808dc39cc39fc2aac485c3b1e4b882e39080f0a08080", "ArY1I2xC2yDwIbuNHN/1ynXdGgzHLqdCrXUPMwELJPc7s7JqlCMJBAIIjfkpHReBPXeoMCyuClwgbT419jUWU1PwaNl4FEQYKCDKVJz+97Mp3K+Q2YGa77B6gpxB/lr1QgoqpDf7wDVrDmOqGoiPjWDqy8KzLueKDcm9BVP8xeTJIxs=",
    "8f40e50a84a7462e2b8d24c28898ef1f23359fff50d8c509e6fb7ce06e142f9c", "b9b0a1e9cc20100c5faa3bbe2777303d25950616c4c6a3fa2e3e046f936ec2ba", "d5a2f879123145a4b291d767428870f5a8d9e5007193321795b40183d4ab8c2b", "b20989adc3ddc41cd2c435952c0d59a91315d8c5218d5040573fc3749543acaf", "6162696c697479f09fa49de79a8420c8bac8be", "ArIJia3D3cQc0sQ1lSwNWakTFdjFIY1QQFc/w3SVQ6yvbG2S0x4Yu86QGwPTy7mP3961I1XqB6SFFTzqDZZavhxoWMj7mEVGMQIsh2RLWI5EYQaQDIePSnXPlzf7CIt+voTD",
    "875adb475056aec0b4809bd2db9aa00cff53a649e7b59d8edcbf4e6330b0995c", "9c05781112d5b0a2a7148a222e50e0bd891d6b60c5483f03456e982185944aae", "3b15c977e20bfe4b8482991274635edd94f366595b1a3d2993515705ca3cedb8", "8d4442713eb9d4791175cb040d98d6fc5be8864d6ec2f89cf0895a2b2b72d1b1", "706570706572f09f9180d197d0b6d0b0d0ba", "Ao1EQnE+udR5EXXLBA2Y1vxb6IZNbsL4nPCJWisrctGxY3AduCS+jTUgAAnfvKafkmpy15+i9YMwCdccisRa8SvzW671T2JO4LFSPX31K4kYUKelSAdSPwe9NwO6LhOsnoJ+",
    "eba1687cab6a3101bfc68fd70f214aa4cc059e9ec1b79fdb9ad0a0a4e259829f", "dff20d262bef9dfd94666548f556393085e6ea421c8af86e9d333fa8747e94b3", "4f1538411098cf11c8af216836444787c462d47f97287f46cf7edb2c4915b8a5", "2180b52ae645fcf9f5080d81b1f0b5d6f2cd77ff3c986882bb549158462f3407", "2820cda1c2b020cd9cca9620cda1c2b029", "AiGAtSrmRfz59QgNgbHwtdbyzXf/PJhogrtUkVhGLzQHv4qhKQwnFQ54OjVMgqCea/Vj0YqBSdhqNR777TJ4zIUk7R0fnizp6l1zwgzWv7+ee6u+0/89KIjY5q1wu6inyuiv",
    "d5633530f5bcfebceb5584cfbbf718a30df0751b729dd9a789b9f30c0587d74e", "b74e6a341fb134127272b795a08b59250e5fa45a82a2eb4095e4ce9ed5f5e214", "75fe686d21a035f0c7cd70da64ba307936e5ca0b20710496a6b6b5f573377bdd", "e4cd5f7ce4eea024bc71b17ad456a986a74ac426c2c62b0a15eb5c5c8f888b68", "d985d98fd986d98ed8a7d982d98ed8b4d98ed8a9d98f20d8b3d98fd8a8d98fd984d99020d8a7d990d8b3d992d8aad990d8aed992d8afd98ed8a7d985d99020d8a7d984d984d98fd991d8bad98ed8a9d99020d981d990d98a20d8a7d984d986d98fd991d8b8d98fd985d99020d8a7d984d992d982d98ed8a7d8a6d990d985d98ed8a9d99020d988d98ed981d990d98ad98520d98ad98ed8aed98fd8b5d98ed99120d8a7d984d8aad98ed991d8b7d992d8a8d990d98ad982d98ed8a7d8aad98f20d8a7d984d992d8add8a7d8b3d98fd988d8a8d990d98ad98ed991d8a9d98fd88c", "AuTNX3zk7qAkvHGxetRWqYanSsQmwsYrChXrXFyPiItoIBsWu1CB+sStla2M4VeANASHxM78i1CfHQQH1YbBy24Tng7emYW44ol6QkFD6D8Zq7QPl+8L1c47lx8RoODEQMvNCbOk5ffUV3/AhONHBXnffrI+0025c+uRGzfqpYki4lBqm9iYU+k3Tvjczq9wU0mkVDEaM34WiQi30MfkJdRbeeYaq6kNvGPunLb3xdjjs5DL720d61Flc5ZfoZm+CBhADy9D9XiVZYLKAlkijALJur9dATYKci6OBOoc2SJS2Clai5hOVzR0yVeyHRgRfH9aLSlWW5dXcUxTo7qqRjNf8W5+J4jF4gNQp5f5d0YA4vPAzjBwSP/5bGzNDslKfcAH",
    "d5633530f5bcfebceb5584cfbbf718a30df0751b729dd9a789b9f30c0587d74e", "b74e6a341fb134127272b795a08b59250e5fa45a82a2eb4095e4ce9ed5f5e214", "75fe686d21a035f0c7cd70da64ba307936e5ca0b20710496a6b6b5f573377bdd", "38d1ca0abef9e5f564e89761a86cee04574b6825d3ef2063b10ad75899e4b023", "d8a7d984d983d98420d981d98a20d8a7d984d985d8acd985d98820d8b9d8a920283529", "AjjRygq++eX1ZOiXYahs7gRXS2gl0+8gY7EK11iZ5LAjbOTrlfrxak5Lki42v2jMPpLSicy8eHjsWkkMtF0i925vOaKG/ZkMHh9ccQBdfTvgEGKzztedqDCAWb5TP1YwU1PsWaiiqG3+WgVvJiO4lUdMHXL7+zKKx8bgDtowzz4QAwI=",
    "d5633530f5bcfebceb5584cfbbf718a30df0751b729dd9a789b9f30c0587d74e", "b74e6a341fb134127272b795a08b59250e5fa45a82a2eb4095e4ce9ed5f5e214", "75fe686d21a035f0c7cd70da64ba307936e5ca0b20710496a6b6b5f573377bdd", "4f1a31909f3483a9e69c8549a55bbc9af25fa5bbecf7bd32d9896f83ef2e12e0", "f09d9691f09d9686f09d969ff09d969e20e7a4bee69c83e7a791e5adb8e999a2e8aa9ee5adb8e7a094e7a9b6e68980", "Ak8aMZCfNIOp5pyFSaVbvJryX6W77Pe9MtmJb4PvLhLgh/TsxPLFSANcT67EC1t/qxjru5ZoADjKVEt2ejdx+xGvH49mcdfbc+l+L7gJtkH7GLKpE9pQNQWNHMAmj043PAXJZ++fiJObMRR2mye5VHEANzZWkZXMrXF7YjuG10S1pOU=",
    "d5633530f5bcfebceb5584cfbbf718a30df0751b729dd9a789b9f30c0587d74e", "b74e6a341fb134127272b795a08b59250e5fa45a82a2eb4095e4ce9ed5f5e214", "75fe686d21a035f0c7cd70da64ba307936e5ca0b20710496a6b6b5f573377bdd", "a3e219242d85465e70adcd640b564b3feff57d2ef8745d5e7a0663b2dccceb54", "f09f998820f09f998920f09f998a2030efb88fe283a32031efb88fe283a32032efb88fe283a32033efb88fe283a32034efb88fe283a32035efb88fe283a32036efb88fe283a32037efb88fe283a32038efb88fe283a32039efb88fe283a320f09f949f20506f776572d984d98fd984d98fd8b5d991d8a8d98fd984d98fd984d8b5d991d8a8d98fd8b1d8b1d98b20e0a5a320e0a5a36820e0a5a320e0a5a3e58697", "AqPiGSQthUZecK3NZAtWSz/v9X0u+HRdXnoGY7LczOtUf05aMF89q1FLwJvaFJYICZoMYgRJHFLwPiOHce7fuAc40kX0wXJvipyBJ9HzCOj7CgtnC1/cmPCHR3s5AIORmroBWglm1LiFMohv1FSPEbaBD51VXxJa4JyWpYhreSOEjn1wd0lMKC9b+osV2N2tpbs+rbpQem2tRen3sWflmCqjkG5VOVwRErCuXuPb5+hYwd8BoZbfCrsiAVLd7YT44dRtKNBx6rkabWfddKSLtreHLDysOhQUVOp/XkE7OzSkWl6sky0Hva6qJJ/V726hMlomvcLHjE41iKmW2CpcZfOedg=="];
// The NIP's procedure for these rows: pub2 from sec2, the conversation key
// from (sec1, pub2), encrypt, compare the payload; then pub1 from sec1, the
// key from (sec2, pub1), decrypt, compare the plaintext. Both profiles.
#forall "encrypt_decrypt rows run end to end from the two secret keys" i: 0..<10 #steps 2000000000 {
  sec1 := bytes_of(ED_ROWS[6 * i]); sec2 := bytes_of(ED_ROWS[6 * i + 1]);
  want := bytes_of(ED_ROWS[6 * i + 2]); nonce := bytes_of(ED_ROWS[6 * i + 3]);
  plain: [256] u8; n := hex_into(ED_ROWS[6 * i + 4], plain[..]);
  published := ED_ROWS[6 * i + 5];
  pub1 := public_x(sec1[..]); pub2 := public_x(sec2[..]);
  scratch: [1] EcdhScratch; ck1: [32] u8; ck2: [32] u8;
  #assert nip44_conversation_key_from_keys_checked(sec1[..], pub2[..], scratch[..], ck1[..], false) == NIP44_OK;
  #assert nip44_conversation_key_from_keys_checked(sec2[..], pub1[..], scratch[..], ck2[..], false) == NIP44_OK;
  #assert same_bytes(ck1[..], want[..]) && same_bytes(ck2[..], want[..]);
  for profile: [2] u32.[NIP44_COMPAT, NIP44_EXTENDED] {
    work: [1024] u8; for j: 0..<1024 { work[j] = 165; }
    out: [512] u8;
    r := nip44_encrypt_checked(profile, ck1[..], nonce[..], plain[0..<n], work[..], out[..], false);
    #assert r.error == NIP44_OK && cast(string) out[0..<r.written] == published;
    back: [256] u8;
    d := nip44_decrypt_checked(profile, ck2[..], published, work[..], back[..], false);
    #assert d.error == NIP44_OK && d.written == n && same_bytes(back[0..<n], plain[0..<n]);
  }
}

// All 12 invalid.decrypt rows: (conversation_key, payload), and the status
// each must give.
BAD_ROWS :: [24] string.[
    "ca2527a037347b91bea0c8a30fc8d9600ffd81ec00038671e3a0f0cb0fc9f642", "#Atqupco0WyaOW2IGDKcshwxI9xO8HgD/P8Ddt46CbxDbrhdG8VmJdU0MIDf06CUvEvdnr1cp1fiMtlM/GrE92xAc1K5odTpCzUB+mjXgbaqtntBUbTToSUoT0ovrlPwzGjyp", // unknown encryption version
    "36f04e558af246352dcf73b692fbd3646a2207bd8abd4b1cd26b234db84d9481", "AK1AjUvoYW3IS7C/BGRUoqEC7ayTfDUgnEPNeWTF/reBZFaha6EAIRueE9D1B1RuoiuFScC0Q94yjIuxZD3JStQtE8JMNacWFs9rlYP+ZydtHhRucp+lxfdvFlaGV/sQlqZz", // unknown encryption version 0
    "ca2527a037347b91bea0c8a30fc8d9600ffd81ec00038671e3a0f0cb0fc9f642", "Atфupco0WyaOW2IGDKcshwxI9xO8HgD/P8Ddt46CbxDbrhdG8VmJZE0UICD06CUvEvdnr1cp1fiMtlM/GrE92xAc1EwsVCQEgWEu2gsHUVf4JAa3TpgkmFc3TWsax0v6n/Wq", // invalid base64
    "cff7bd6a3e29a450fd27f6c125d5edeb0987c475fd1e8d97591e0d4d8a89763c", "Agn/l3ULCEAS4V7LhGFM6IGA17jsDUaFCKhrbXDANholyySBfeh+EN8wNB9gaLlg4j6wdBYh+3oK+mnxWu3NKRbSvQAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA", // invalid MAC
    "cfcc9cf682dfb00b11357f65bdc45e29156b69db424d20b3596919074f5bf957", "AmWxSwuUmqp9UsQX63U7OQ6K1thLI69L7G2b+j4DoIr0oRWQ8avl4OLqWZiTJ10vIgKrNqjoaX+fNhE9RqmR5g0f6BtUg1ijFMz71MO1D4lQLQfW7+UHva8PGYgQ1QpHlKgR", // invalid MAC
    "5254827d29177622d40a7b67cad014fe7137700c3c523903ebbe3e1b74d40214", "Anq2XbuLvCuONcr7V0UxTh8FAyWoZNEdBHXvdbNmDZHB573MI7R7rrTYftpqmvUpahmBC2sngmI14/L0HjOZ7lWGJlzdh6luiOnGPc46cGxf08MRC4CIuxx3i2Lm0KqgJ7vA", // invalid padding
    "fea39aca9aa8340c3a78ae1f0902aa7e726946e4efcd7783379df8096029c496", "An1Cg+O1TIhdav7ogfSOYvCj9dep4ctxzKtZSniCw5MwRrrPJFyAQYZh5VpjC2QYzny5LIQ9v9lhqmZR4WBYRNJ0ognHVNMwiFV1SHpvUFT8HHZN/m/QarflbvDHAtO6pY16", // invalid padding
    "0c4cffb7a6f7e706ec94b2e879f1fc54ff8de38d8db87e11787694d5392d5b3f", "Am+f1yZnwnOs0jymZTcRpwhDRHTdnrFcPtsBzpqVdD6b2NZDaNm/TPkZGr75kbB6tCSoq7YRcbPiNfJXNch3Tf+o9+zZTMxwjgX/nm3yDKR2kHQMBhVleCB9uPuljl40AJ8kXRD0gjw+aYRJFUMK9gCETZAjjmrsCM+nGRZ1FfNsHr6Z", // invalid padding
    "5cd2d13b9e355aeb2452afbd3786870dbeecb9d355b12cb0a3b6e9da5744cd35", "", // invalid payload length: 0
    "d61d3f09c7dfe1c0be91af7109b60a7d9d498920c90cbba1e137320fdd938853", "Ag==", // invalid payload length: 4
    "873bb0fc665eb950a8e7d5971965539f6ebd645c83c08cd6a85aafbad0f0bc47", "AqxgToSh3H7iLYRJjoWAM+vSv/Y1mgNlm6OWWjOYUClrFF8=", // invalid payload length: 48
    "9f2fef8f5401ac33f74641b568a7a30bb19409c76ffdc5eae2db6b39d2617fbe", "Ap/2SEZCVFIhYk6qx7nqJxM6TMI1ZoKmAzrO7vBDVJhhuZXWiM20i/tIsbjT0KxkJs2MZjh1oXNYMO9ggfk7i47WQA=="]; // invalid payload length: 92
BAD_CODES :: [12] u32.[NIP44_VERSION, NIP44_VERSION, NIP44_BASE64, NIP44_MAC, NIP44_MAC, NIP44_PADDING, NIP44_PADDING, NIP44_PADDING, NIP44_PAYLOAD, NIP44_PAYLOAD, NIP44_PAYLOAD, NIP44_PAYLOAD];
// No row writes out. A row refused before its MAC matches never decrypts:
// the area after the decoded bytes, where the padded plaintext would go,
// keeps its 0xa5 bytes.
#forall "invalid.decrypt rows refuse with their status and never write out" i: 0..<12 {
  ck := bytes_of(BAD_ROWS[2 * i]); payload := BAD_ROWS[2 * i + 1];
  for profile: [2] u32.[NIP44_COMPAT, NIP44_EXTENDED] {
    work: [512] u8; for j: 0..<512 { work[j] = 165; }
    out: [256] u8; for j: 0..<256 { out[j] = 165; }
    r := nip44_decrypt_checked(profile, ck[..], payload, work[..], out[..], false);
    #assert r.error == BAD_CODES[i] && r.written == 0;
    for b: out { #assert b == 165; }
    dlen := base64_decode_size(payload);
    if r.error != NIP44_PADDING && dlen >= NIP44_MIN_DECODED {
      for j: dlen..<2 * dlen - NIP44_OVERHEAD { #assert work[j] == 165; }
    }
  }
}
// One flipped bit in each decoded byte of the row 0 payload after the
// version byte (bit i mod 8 of byte i) is refused as NIP44_MAC, and none
// writes out.
#forall "a flipped bit in nonce, ciphertext or MAC never yields plaintext" i: 1..<99 #steps 200000000 {
  bit := i * 8 + i % 8;
  raw: [99] u8;
  #assert base64_decode_checked(ED_ROWS[5], raw[..], false).error == NC_OK;
  raw[bit / 8] = raw[bit / 8] ^ cast(u8) (1 << (bit % 8));
  text: [132] u8;
  #assert base64_encode_checked(cast(string) raw[..], text[..], false).written == 132;
  ck := bytes_of(ED_ROWS[2]);
  work: [133] u8; out := [1] u8.[165];
  r := nip44_decrypt_checked(NIP44_COMPAT, ck[..], cast(string) text[..], work[..], out[..], false);
  #assert r.error == NIP44_MAC && out[0] == 165;
}

// invalid.encrypt_msg_lengths: 0, 65536, 100000 and 10000000 bytes.
#test "invalid.encrypt_msg_lengths under both profiles" {
  lengths := [4] int.[0, 65536, 100000, 10000000];
  for n: lengths {
    compat := nip44_encrypt_request(NIP44_COMPAT, 32, 32, n, 1 << 24, 1 << 24, false);
    extended := nip44_encrypt_request(NIP44_EXTENDED, 32, 32, n, 1 << 24, 1 << 24, false);
    if n == 0 { #assert compat == NIP44_LENGTH && extended == NIP44_LENGTH; }
    else if n <= NIP44_EXTENDED_MAX { #assert compat == NIP44_CAPACITY && extended == NIP44_OK; }
    else { #assert compat == NIP44_CAPACITY && extended == NIP44_CAPACITY; }
  }
}

// The NIP's extended-prefix table: 'a' repeated n times under the row 0 key
// and nonce, and the SHA-256 of the payload.
BOUNDARY :: [3] string.[
    "6d8c2810d1e870fbaa1f0a0937126cca837a15f9260e27060c331d70a3c0bc84", // 65535, u16 prefix
    "b7b4edb36ba92e267d322d56d9aebc22e7fa96ff52e3c12adc07f07a43cbc616", // 65536, 6-byte prefix
    "eeb7c7c5373894ea2c1547cfd3ccb15d5a0b2d619da852e5c79df792dcc9e435"]; // 65537, 6-byte prefix
#forall "65535, 65536 and 65537 bytes: the NIP's payload hash, or compat refuses" i: 0..<3 #steps 1000000000 {
  n := 65535 + i;
  ck := bytes_of(ED_ROWS[2]); nonce := bytes_of(ED_ROWS[3]);
  plain: [65537] u8; for j: 0..<n { plain[j] = 'a'; }
  work: [163917] u8; out: [109324] u8;
  r := nip44_encrypt_checked(NIP44_EXTENDED, ck[..], nonce[..], plain[0..<n], work[..], out[..], false);
  #assert r.error == NIP44_OK && r.written == nip44_payload_size(NIP44_EXTENDED, n);
  digest: [32] u8; want := bytes_of(BOUNDARY[i]);
  #assert sha256_hash_checked(out[0..<r.written], digest[..], false) == SHA_OK && same_bytes(digest[..], want[..]);
  if n > 65535 {
    // A compat reader refuses the extended payload by its length.
    back := [1] u8.[165];
    #assert nip44_decrypt_checked(NIP44_COMPAT, ck[..], cast(string) out[0..<r.written], work[..], back[..], false).error == NIP44_CAPACITY;
    #assert back[0] == 165;
  }
  c := nip44_encrypt_checked(NIP44_COMPAT, ck[..], nonce[..], plain[0..<n], work[..], out[..], false);
  #assert c.error == (ifx n <= 65535 then NIP44_OK else NIP44_CAPACITY);
}
```

To run them:

```shell
coax/bin/coax test nostr/examples/checks/nip44.coax
coax/bin/coax mutate nostr/examples/checks/nip44.coax --root-only
```

On 2026-11-01, `coax test` ran 225 checks, including the loaded files' own
checks, and all passed in about 82 seconds; the ten end-to-end rows took 39
seconds and the three long payloads 12. The root-only mutation run killed 38 of 38 mutants in the checks file, including the reference `calc_padded_len`. The compiler was a cached release build (`~/.cache/coax-release/8925679b...`), because another agent held the build lock.

### Checks in the library

`nip44.coax` loads the generated
[nip44_vectors.coax](../../coax/tests/nostr/nip44_vectors.coax), so the
run above also covered every `calc_padded_len`, `get_message_keys`,
`encrypt_decrypt` and `invalid.decrypt` row from fixed inputs, the
`get_conversation_key` rows from a recorded shared x, and 11 payloads from
the independent nostr-effect implementation. The library's own checks cover
the prefix switch at 65,536, unpadding at every edge, buffer and alias
refusals, and the profile sizes.

Three more files run outside that graph:

- [nip44_long.coax](../../coax/tests/nostr/nip44_long.coax) holds the three
  `encrypt_decrypt_long_msg` rows (65,535 and 65,532 bytes) and the
  boundary table. Its three long-message rows were run on 2026-11-01 and
  passed, about 15 seconds each.
- [nip44_key_vectors.coax](../../coax/tests/nostr/nip44_key_vectors.coax)
  runs all 35 valid and 8 invalid `get_conversation_key` rows through ECDH;
  chapter 14 records its 238 of 238 pass on 2026-11-01.
- [nip44_native.c](../../coax/tests/nostr/nip44_native.c) runs encrypt and
  decrypt through the C exports under ASan and UBSan.

The library files were not mutated again today. The
[NIP-44 v2 record](../../docs/verification/2026-10-29-nip44-v2.md) gives
1,700 of 1,700 mutants of `nip44.coax` killed, 120 of 120 long checks, and
the native ASan/UBSan run whose 65,535, 65,536 and 65,537-byte payload
hashes match the NIP's table. The
[conversation-key record](../../docs/verification/2026-10-30-nip44-conversation-key.md)
covers `nip44-key.coax`, with 3,434 of 3,434 mutants of its whole reachable
graph killed on 2026-10-30. Neither was rerun today; since those records
only comments have changed in either file.

With these, every `valid` and `invalid` entry in `nip44.vectors.json` gives
its expected result: `calc_padded_len`, `get_message_keys`, `encrypt_decrypt`
and `invalid.decrypt` in the checks above, `get_conversation_key` (valid and
invalid) in chapter 14's checks and `nip44_key_vectors.coax`, and
`encrypt_decrypt_long_msg` in `nip44_long.coax`.

## Summary

- The conversation key is HKDF-extract of the raw ECDH x with salt
  `nip44-v2`; each message's keys are HKDF-expand of it with the nonce,
  split 32 / 12 / 32.
- Padding rounds the plaintext up to 32-byte buckets below 256 bytes and to
  eighths of a power of two above, after a 2-byte prefix, or a 6-byte
  prefix from 65,536 bytes.
- The payload is `02 || nonce || ciphertext || mac` in base64, at least 132
  characters.
- Decryption checks lengths and the version, then the MAC in constant
  time, and decrypts only after the MAC matches; `out` is written only
  after the padding is valid.
- `NIP44_COMPAT` stops at 65,535 bytes and refuses longer payloads by their
  length; `NIP44_EXTENDED` follows the NIP's 6-byte prefix up to 917,504
  bytes.

## Dependencies

- [6. HKDF](ch006-hkdf.md)
- [14. Shared secrets with ECDH](ch014-shared-secrets-with-ecdh.md)
- [18. ChaCha20](ch018-chacha20.md)
- [5. HMAC-SHA256](ch005-hmac-sha256.md)
- [1. Bytes, hex and base64](ch001-bytes-hex-and-base64.md)

## Sources

- [NIP-44](../nips/official/44.md), and its pinned
  [nip44.vectors.json](../../coax/tests/vectors/upstream/nip44/nip44.vectors.json)
  (sha256 `269ed0f6...`), listed in
  [coax/tests/vectors/SOURCES.md](../../coax/tests/vectors/SOURCES.md).
- [RFC 5869](https://datatracker.ietf.org/doc/html/rfc5869), HKDF;
  [RFC 8439](https://datatracker.ietf.org/doc/html/rfc8439), ChaCha20;
  [RFC 2104](https://datatracker.ietf.org/doc/html/rfc2104), HMAC;
  [RFC 4648](https://datatracker.ietf.org/doc/html/rfc4648), base64.
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
