# 19. NIP-04 and why it is retired

NIP-04 was Nostr's first encrypted direct message. The pinned text marks it
`unrecommended` in favor of NIP-17. This chapter describes it exactly,
states its weaknesses with the words of the pinned text, and builds what
this stack does with it: a reader that decrypts old messages and refuses to
make new ones.

Part I, Shared secrets, encodings and encryption. Result: NIP-04's AES-256-CBC with the shared x as key and
its `?iv=` suffix, its weaknesses (no authentication, metadata in the clear,
the key used as is), and a decrypt-only reader checked against FIPS-197,
NIST SP 800-38A and OpenSSL.

## What this chapter teaches

- The format: kind 4, content `base64(ciphertext)?iv=base64(iv)`,
  AES-256-CBC with the raw ECDH x as the key, PKCS#7 padding.
- AES-256 decryption, worked from one S-box value and the first steps of
  the FIPS-197 appendix C.3 trace, built without lookup tables.
- CBC decryption and why one flipped bit in the iv flips one plaintext bit.
- The weaknesses: no authentication, the sender, recipient and time in the
  clear, the key used without a key derivation function, and an iv the text
  doesn't require to be random.
- The stack's position: read only, never encrypt. A call to encrypt
  returns `NIP04_RETIRED`.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [aes.coax](../../coax/lib/crypto/aes.coax) | `aes_gf_mul`, `aes_gf_inverse` | multiply and invert in GF(2^8) with masks, no tables |
| | `aes_sbox`, `aes_inv_sbox` | the S-box and its inverse, computed |
| | `aes256_expand` | the AES-256 key schedule, 15 round keys |
| | `aes_add_round_key`, `aes_inv_shift_rows`, `aes_inv_sub_bytes`, `aes_inv_mix_columns` | the four inverse steps |
| | `aes256_decrypt_block` | the FIPS-197 inverse cipher, 14 rounds |
| | `aes256_cbc_decrypt_checked`, `aes256_cbc_decrypt_in_place_checked` | CBC decryption, no padding removed |
| [nip04.coax](../../coax/lib/nostr/nip04.coax) | `nip04_split` | split the content at `?iv=` |
| | `nip04_unpad` | check and remove PKCS#7 padding |
| | `nip04_decrypt_checked` | decrypt a message with the shared x |
| | `nip04_decrypt_from_keys_checked` | the same, from your secret and the other side's public key |
| | `nip04_encrypt` | refuse, always |

`aes.coax` has no forward cipher. The forward S-box is there only because
the key schedule uses it. Nothing in this stack can produce a NIP-04
message.

`nip04.coax` is `#authority`: it decides which legacy messages are admitted
as readable, so its checks must kill every mutant. Its results are
`Nip04Result { written, error }`. The errors are `NIP04_KEY` (1),
`NIP04_FORMAT` (2), `NIP04_BASE64` (3), `NIP04_IV` (4), `NIP04_LENGTH` (5),
`NIP04_OUTPUT` (6), `NIP04_ALIAS` (7), `NIP04_PADDING` (8), `NIP04_PEER`
(9), `NIP04_SECRET` (10), `NIP04_RETIRED` (11) and `NIP04_WORK` (12); zero
is success.

## The format

The pinned text, [04.md](../nips/official/04.md), defines a kind 4 event
whose content

> MUST be equal to the base64-encoded, aes-256-cbc encrypted string of
> anything a user wants to write, encrypted using a shared cipher generated
> by combining the recipient's public-key with the sender's private-key;
> this appended by the base64-encoded initialization vector as if it was a
> querystring parameter named "iv". The format is the following:
> `"content": "<encrypted_text>?iv=<initialization_vector>"`.

The key is the ECDH shared x of chapter 14, used as it is:

> In Nostr, only the X coordinate of the shared point is used as the secret
> and it is NOT hashed.

The cipher is AES with a 256-bit key in CBC mode. The JavaScript sample in
the text calls Node's `createCipheriv('aes-256-cbc', ...)`, which pads with
PKCS#7: it appends `p` bytes of value `p`, where `p` is 1 to 16, so the
padded length is the next multiple of 16 above the plaintext length. The
NIP text itself doesn't name the padding; every deployed client follows the
sample.

`nip04_split` takes the first `?` and requires it to start `?iv=`. Base64
never contains `?`, so anything else is a format error:

<!-- source: coax/lib/nostr/nip04.coax nip04_split -->
```jai
// "<cipher>?iv=<iv>": the first '?' must start "?iv=". An empty cipher
// text splits; it is refused later as a bad length.
nip04_split :: (content:string)->Nip04Split {
  q:=0;
  while q<content.count && content[q]!='?' {q+=1;}
  if content.count-q<4 {return Nip04Split.{};}
  if content[q+1]!='i' || content[q+2]!='v' || content[q+3]!='=' {return Nip04Split.{};}
  return Nip04Split.{cipher=content[0..<q],iv=content[q+4..<content.count],ok=true};
}
```

## AES-256, decryption only

AES works on a 16-byte state, written as a 4x4 grid of bytes filled column
by column: byte `r + 4c` is row `r` of column `c`. AES-256 runs 14 rounds
with 15 round keys of 16 bytes, expanded from the 32-byte key. Decryption,
the FIPS-197 *inverse cipher*, undoes the rounds in reverse order:

1. Add round key 14 (exclusive-or).
2. For rounds 13 down to 1: InvShiftRows, InvSubBytes, add the round key,
   InvMixColumns.
3. InvShiftRows, InvSubBytes, add round key 0.

<!-- source: coax/lib/crypto/aes.coax aes256_decrypt_block -->
```jai
// FIPS-197 5.3, the inverse cipher with Nr = 14 rounds.
aes256_decrypt_block :: (s:AesSchedule,input:AesBlock)->AesBlock {
  st:=aes_add_round_key(input,s,14);
  round:=13;
  while round>=1 {
    st=aes_add_round_key(aes_inv_sub_bytes(aes_inv_shift_rows(st)),s,round);
    st=aes_inv_mix_columns(st);
    round-=1;
  }
  return aes_add_round_key(aes_inv_sub_bytes(aes_inv_shift_rows(st)),s,0);
}
```

### Arithmetic in GF(2^8)

AES treats a byte as a polynomial over GF(2) of degree below 8: bit `i` is
the coefficient of `x^i`. Addition is exclusive-or. Multiplication is
polynomial multiplication reduced modulo `x^8 + x^4 + x^3 + x + 1`. To
multiply by `x` (`aes_xtime`), shift left; if bit 7 was set, the `x^8` term
is replaced by `x^4 + x^3 + x + 1`, which is the byte `1b`. FIPS-197 section
4.2 gives `{57} x {83} = {c1}`; the checks confirm it.

<!-- source: coax/lib/crypto/aes.coax aes_gf_mul -->
```jai
// Multiply by x in GF(2^8) modulo x^8 + x^4 + x^3 + x + 1: shift left, and
// if bit 7 fell off, exclusive-or with 1b. The "if" is a multiply by the bit.
aes_xtime :: (a:u32)->u32 {return ((a<<1)&255)^(((a>>7)&1)*0x1b);}
aes_gf_mul :: (a_in:u32,b:u32)->u32 {
  a:=a_in&255;r:=cast(u32)0;
  for i:0..<8 {
    r^=a&(((b>>i)&1)*255);
    a=aes_xtime(a);
  }
  return r;
}
```

`aes_gf_mul` never branches on its inputs: "add `a` if bit `i` of `b` is
set" is `a & (bit * 255)`, a mask of all ones or all zeros.

### The S-box, computed

SubBytes replaces each byte `x` by `S(x)`: its inverse in GF(2^8) (0 stays
0), then a fixed affine map. Most AES code stores `S` as a 256-byte table,
and fast AES code stores four 1 KiB "T-tables" that merge SubBytes and
MixColumns. Both index memory with bytes that depend on the key. A process
sharing the CPU cache can measure which cache lines were touched and
recover the key; this is the attack Bernstein and Osvik, Shamir and Tromer
published in 2005. Here the key is your secret ECDH x, and the ciphertext is
public on relays, so an attacker can make you decrypt as often as they like.

This file computes every S-box value instead. The inverse is `x^254`, since
`x^255 = 1` for every nonzero `x`: six square-and-multiply steps make
`x^127`, and one more squaring makes `x^254`. The cost is 13
multiplications per byte, slow but with no table and no branch on the
data.

<!-- source: coax/lib/crypto/aes.coax aes_sbox -->
```jai
// x^254 = x^-1 for x != 0, and 0 for 0: x^127 = x^(2^7 - 1) by six
// square-and-multiply steps, then one more squaring.
aes_gf_inverse :: (x:u32)->u32 {
  y:=x&255;
  for i:0..<6 {y=aes_gf_mul(aes_gf_mul(y,y),x);}
  return aes_gf_mul(y,y);
}
aes_rotl8 :: (b:u32,n:u32)->u32 {return ((b<<n)|(b>>(8-n)))&255;}
// FIPS-197 5.1.1: the inverse, then the affine map b ^ rotl(b,1..4) ^ 63.
aes_sbox :: (x:u32)->u32 {
  b:=aes_gf_inverse(x);
  return b^aes_rotl8(b,1)^aes_rotl8(b,2)^aes_rotl8(b,3)^aes_rotl8(b,4)^0x63;
}
// FIPS-197 5.3.2: undo the affine map, then invert.
aes_inv_sbox :: (y_in:u32)->u32 {
  y:=y_in&255;
  return aes_gf_inverse(aes_rotl8(y,1)^aes_rotl8(y,3)^aes_rotl8(y,6)^0x05);
}
```

The intent is constant time; it isn't proven. The C compiler could, in
principle, turn the masks back into branches, and no timing measurement
(Bitcoin chapter 3's harness) has been run on this file. The caveat is recorded in
the file's header.

### Worked by hand: S(53)

FIPS-197 section 5.1.1 uses the byte `53`. Its inverse in GF(2^8) is `ca`;
the checks confirm that `aes_gf_mul(53, ca) = 1`. The affine map
exclusive-ors `b` with its rotations left by 1, 2, 3 and 4 bits, then with
`63`:

1. `b = ca = 11001010`.
2. Rotations: by 1, `10010101 = 95`; by 2, `00101011 = 2b`; by 3,
   `01010110 = 56`; by 4, `10101100 = ac`.
3. `ca ^ 95 = 5f`, `5f ^ 2b = 74`, `74 ^ 56 = 22`, `22 ^ ac = 8e`.
4. `8e ^ 63 = ed`.

So `S(53) = ed`, the value in FIPS-197. `aes_inv_sbox` undoes the affine map
(rotations by 1, 3 and 6, then `05`) and inverts, so `InvS(ed) = 53`.

### The inverse round steps

<!-- source: coax/lib/crypto/aes.coax aes_inverse_steps -->
```jai
// The state is column-major: byte r + 4c is row r of column c.
aes_add_round_key :: (st:AesBlock,s:AesSchedule,round:int)->AesBlock {
  out:AesBlock;
  for i:0..<16 {out.b[i]=st.b[i]^s.b[round*16+i];}
  return out;
}
// Row r moves r places to the right.
aes_inv_shift_rows :: (st:AesBlock)->AesBlock {
  out:AesBlock;
  for c:0..<4 {for r:0..<4 {out.b[r+4*((c+r)%4)]=st.b[r+4*c];}}
  return out;
}
aes_inv_sub_bytes :: (st:AesBlock)->AesBlock {
  out:AesBlock;
  for i:0..<16 {out.b[i]=cast(u8)aes_inv_sbox(cast(u32)st.b[i]);}
  return out;
}
// Each column times the matrix rows (0e 0b 0d 09), rotated.
aes_inv_mix_columns :: (st:AesBlock)->AesBlock {
  out:AesBlock;
  m:=[4]u32.[0x0e,0x0b,0x0d,0x09];
  for c:0..<4 {
    for r:0..<4 {
      v:=cast(u32)0;
      for k:0..<4 {v^=aes_gf_mul(cast(u32)st.b[4*c+k],m[(k-r+4)%4]);}
      out.b[4*c+r]=cast(u8)v;
    }
  }
  return out;
}
```

InvMixColumns multiplies each column by the matrix whose rows are
`0e 0b 0d 09` rotated, the inverse of MixColumns' `02 03 01 01`.

### Worked by hand: the first steps of FIPS-197 appendix C.3

Appendix C.3 decrypts with the key `00 01 02 ... 1f`. Its input is
`8ea2b7ca516745bfeafc49904b496089`, and the appendix lists every
intermediate state. Follow byte 0 and one byte that moves:

1. Round key 14 (`round[ 0].ik_sch`) is
   `24fc79ccbf0979e9371ac23c6d68de36`. Byte 0: `8e ^ 24 = aa`, the first
   byte of `round[ 1].istart`, `aa5ece06ee6e3c56dde68bac2621bebf`.
2. InvShiftRows moves row 1 one place right, so row 1 of column 0 comes
   from row 1 of column 3. Column 3 of `istart` is bytes 12 to 15,
   `26 21 be bf`; its row 1 is `21`. `round[ 1].is_row` is
   `aa218b56...`: byte 1 is `21`. Row 0 doesn't move, so byte 0 stays
   `aa`.
3. InvSubBytes: `InvS(aa) = 62`, the first byte of `round[ 1].is_box`,
   `627bceb9...`.
4. Round key 13 starts `4e`. `62 ^ 4e = 2c`, the first byte of
   `round[ 1].ik_add`, `2c21a820...`.

The example prints each of these states from the code. After all 14 rounds
the output is `00112233445566778899aabbccddeeff`, the appendix's plaintext.

## CBC decryption

CBC encrypts block `i` as `C_i = E(P_i ^ C_(i-1))`, with `C_(-1)` the iv.
Decryption is `P_i = D(C_i) ^ C_(i-1)`. Each plaintext block needs only two
ciphertext blocks, so decryption can run in place from the last block back
to the first, while `C_(i-1)` is still intact:

<!-- source: coax/lib/crypto/aes.coax aes256_cbc_decrypt_in_place_checked -->
```jai
// SP 800-38A 6.2: P_i = D(C_i) ^ C_(i-1), with C_(-1) the iv. Working from
// the last block back to the first keeps C_(i-1) intact until it is used.
// The expanded key lives in the caller's work buffer and is cleared before
// return.
aes256_cbc_decrypt_in_place_checked :: (key:[]u8,iv:[]u8,data:[]u8,work:[]AesSchedule)->AesResult {
  error:=aes_cbc_admit(key.count,iv.count,work.count,data.count,data.count,false);
  if error!=AES_OK {return AesResult.{error=error};}
  work[0]=aes256_expand(key);
  i:=data.count/16-1;
  while i>=0 {
    c:AesBlock;
    for j:0..<16 {c.b[j]=data[i*16+j];}
    p:=aes256_decrypt_block(work[0],c);
    for j:0..<16 {
      previous:=ifx i==0 then iv[j] else data[(i-1)*16+j];
      data[i*16+j]=p.b[j]^previous;
    }
    i-=1;
  }
  work[0]=AesSchedule.{};
  return AesResult.{written=data.count};
}
```

`aes_cbc_admit` checks the 32-byte key, the 16-byte iv, one work buffer, a
whole number of blocks up to 1 MiB, the output capacity and overlap before
any byte is written. The expanded key lives in the caller's work buffer and
is cleared before return.

## The reader

`nip04_decrypt_checked` checks the whole request, decodes the iv and the
ciphertext, decrypts in place, then removes the padding:

<!-- source: coax/lib/nostr/nip04.coax nip04_decrypt_checked -->
```jai
nip04_decrypt_checked :: (shared_x:[]u8,content:string,out:[]u8,work:[]Nip04Work,overlap:bool)->Nip04Result {
  if shared_x.count!=32 {return Nip04Result.{error=NIP04_KEY};}
  if work.count!=1 {return Nip04Result.{error=NIP04_WORK};}
  parts:=nip04_split(content);
  if !parts.ok {return Nip04Result.{error=NIP04_FORMAT};}
  n:=base64_decode_size(parts.cipher);iv_count:=base64_decode_size(parts.iv);
  if n<0 || iv_count<0 {return Nip04Result.{error=NIP04_BASE64};}
  if iv_count!=16 {return Nip04Result.{error=NIP04_IV};}
  if n==0 || n%16!=0 {return Nip04Result.{error=NIP04_LENGTH};}
  if out.count<n {return Nip04Result.{error=NIP04_OUTPUT};}
  if overlap {return Nip04Result.{error=NIP04_ALIAS};}
  iv:[16]u8;
  decoded:=base64_decode_checked(parts.iv,iv[..],false);
  decoded=base64_decode_checked(parts.cipher,out[0..<n],false);
  done:=aes256_cbc_decrypt_in_place_checked(shared_x,iv[..],out[0..<n],work[0].aes[..]);
  size:=nip04_unpad(out[0..<n]);
  if size<0 {
    for i:0..<n {out[i]=0;}
    return Nip04Result.{error=NIP04_PADDING};
  }
  return Nip04Result.{written=size};
}
```

Every refusal but one happens before a byte is written. A bad padding is
found only after decryption, so then the decrypted bytes are set to zero
before the refusal returns. PKCS#7 is checked like this:

<!-- source: coax/lib/nostr/nip04.coax nip04_unpad -->
```jai
// PKCS#7: the last byte p is 1 to 16 and the last p bytes all equal p.
// Returns the plaintext length, or -1.
nip04_unpad :: (data:[]u8)->int {
  n:=data.count;p:=cast(int)data[n-1];
  if p<1 || p>16 {return -1;}
  for i:n-p..<n-1 {if cast(int)data[i]!=p {return -1;}}
  return n-p;
}
```

`nip04_decrypt_from_keys_checked` derives the shared x with chapter 14's
`ecdh_stage`, decrypts, and clears the ECDH work. Either side can read the
message: `x(a·B) = x(b·A)`.

<!-- source: coax/lib/nostr/nip04.coax nip04_decrypt_from_keys_checked -->
```jai
// The shared x from your secret and the other side's x-only key (chapter
// 14), used unhashed, as NIP-04 requires. It is cleared before return.
nip04_decrypt_from_keys_checked :: (secret:[]u8,peer_x:[]u8,content:string,out:[]u8,work:[]Nip04Work,overlap:bool)->Nip04Result {
  if work.count!=1 {return Nip04Result.{error=NIP04_WORK};}
  code:=ecdh_stage(secret,peer_x,work[0].ecdh[..]);
  result:=Nip04Result.{error=NIP04_KEY};
  if code==EC_PEER {result.error=NIP04_PEER;}
  if code==EC_SCALAR {result.error=NIP04_SECRET;}
  if code==EC_OK {result=nip04_decrypt_checked(work[0].ecdh[0].staged[..],content,out,work,overlap);}
  work[0].ecdh[0]=EcdhScratch.{};
  return result;
}
```

### Worked by hand: the book's message

The example's message was made for this book with OpenSSL, from the shared x
of chapter 14's test keys `sec1` and `pub2`:

```jai
shared x  94da47d851b9c1ed33b3b72f35434f56aa608d60e573e9c295f568011f4f50a4
iv        a1b2c3d4e5f60718293a4b5c6d7e8f90
plaintext "NIP-04 is retired; use NIP-17."  (30 bytes)
content   zooMmtdlrlTCiIbfLqTqYrpCbTWrcVkP7RdcqCuM8TQ=?iv=obLD1OX2BxgpOktcbX6PkA==
```

1. Split at `?iv=`. The cipher text is 44 base64 characters with one `=`:
   `44 / 4 * 3 - 1 = 32` bytes, two blocks. The iv text is 24 characters
   with `==`: `24 / 4 * 3 - 2 = 16` bytes.
2. The iv's first character `o` is 40 and `b` is 27 in the base64 alphabet:
   `101000 011011`, whose first eight bits `10100001` are `a1`.
3. 30 bytes of plaintext pad to 32 with two bytes `02 02`. After
   decryption, `nip04_unpad` reads the last byte, 2, checks that the byte
   before it is also 2, and returns 30.

## The weaknesses

Each weakness below cites the pinned text.

### No authentication

The content is defined as "the base64-encoded, aes-256-cbc encrypted
string" and nothing more. There is no MAC, so a changed ciphertext still
decrypts. In CBC, `P_0 = D(C_0) ^ iv`: flip a bit of the iv and the same bit
of the first plaintext block flips, with no other effect. The example flips
bit 0 of the iv, `a1` to `a0`, and the message decrypts as
`OIP-04 is retired...`: `N` is `4e`, `O` is `4f`. An attacker who guesses
the first 16 bytes of a message can rewrite them to anything. Flipping a bit
of `C_i` garbles plaintext block `i` and flips the same bit of block
`i + 1`.

The padding check is the only integrity check, and it is weak. A message
decrypted with the wrong key ends in a valid pad about once in 256 tries
(a last byte of `01`), and the reader then returns garbage as `ok`. When a
client reacts differently to bad padding and to a bad message, an attacker
who can submit ciphertexts can decrypt them byte by byte: Vaudenay's
padding-oracle attack (2002).

The text's own warning is:

> This standard does not go anywhere near what is considered the
> state-of-the-art in encrypted communication between peers

NIP-44 (chapter 20) checks an HMAC over the nonce and ciphertext before it
decrypts.

### Metadata in the clear

> **`tags`** MUST contain an entry identifying the receiver of the message
> (such that relays may naturally forward this event to them), in the form
> `["p", "<pubkey, as a hex string>"]`.

The event's `pubkey` is the sender and its `created_at` is the time, both
signed and public. Anyone who can read the relay sees who wrote to whom,
when, and how long the message was, to within 16 bytes. An `e` tag, which
the text allows, links the message to the one it answers. The warning says
so:

> it leaks metadata in the events, therefore it must not be used for
> anything you really need to keep secret, and only with relays that use
> `AUTH` to restrict who can fetch your `kind:4` events.

The client warning adds that turning `@npub` mentions in the content into
`p` tags sends the message to the mentioned user and leaks it. NIP-17 wraps
the message in NIP-59 gift wraps, so a relay sees only a random key and the
recipient.

### The key used as it is

> In Nostr, only the X coordinate of the shared point is used as the secret
> and it is NOT hashed.

The 32-byte x is not uniformly random: it is a field element below `p`, and
it is the x of a curve point. The AES key should come from a key derivation
function. The same key also serves every message between the two keys, in
both directions, forever: there is no per-message key and no forward
secrecy. Anyone who learns one shared x reads the whole conversation, past
and future. NIP-44 runs the x through HKDF-extract to a conversation key and
derives fresh keys for each message from a random nonce.

### The iv

The text says only that the iv is appended "as if it was a querystring
parameter named "iv"". It doesn't say how to choose it. The sample draws 16
random bytes with `crypto.randomFillSync`, but nothing requires it. Since the
key never changes, the iv is the only thing that varies between messages. A
repeated iv shows when two messages start with the same blocks, and an iv an
attacker can predict lets them test guesses about a plaintext they can get
encrypted. A reader can't check how an iv was chosen.

## What this stack does

The Coax Nostr plan says no AES is needed for core Nostr, and its NIP-46
section says ([coax/nostr.md](../../coax/nostr.md), section 12):

> Unsupported legacy NIP-04 methods return an explicit error; this profile
> does not add NIP-04.

This chapter keeps both rules. The reader exists so that old kind 4 messages
can be read, for example by a client importing a history. New messages use
NIP-17. There is no AES encryption anywhere in the library, and the one
procedure named for encryption refuses:

<!-- source: coax/lib/nostr/nip04.coax nip04_encrypt -->
```jai
// NIP-04 is retired. This takes the arguments an encrypter would and
// refuses; new direct messages use NIP-17 (NIP-44 inside NIP-59).
nip04_encrypt :: (shared_x:[]u8,plaintext:string,out:[]u8)->Nip04Result {
  return Nip04Result.{error=NIP04_RETIRED};
}
```

Not built: NIP-04's NIP-46 methods (`nip04_encrypt`, `nip04_decrypt`) and
re-encrypting old NIP-04 private list items with NIP-44 (chapter 54). Plaintext
is returned as bytes; checking that it is UTF-8 is the caller's job.

## The example

The example is [nostr/examples/nip04.coax](../examples/nip04.coax). It
computes the S-box value worked above, prints the first steps of the
FIPS-197 appendix C.3 trace and the whole block, decrypts the SP 800-38A
CBC vectors, splits and decrypts the book's message with the shared x and
with the other side's keys, flips an iv bit, and shows the refusals.

<!-- source: nostr/examples/nip04.coax example -->
```jai
#load "../../coax/lib/nostr/nip04.coax";
#import "Basic";

// Print a label and bytes as lowercase hex.
show :: (label: string, bytes: [] u8) {
  text: [160] u8;
  r := hex_encode_checked(cast(string) bytes, text[..], false);
  print("%: %\n", label, cast(string) text[0..<r.written]);
}
// Decode hex that the program itself spells; it must be valid.
from_hex :: (text: string, out: [] u8) -> [] u8 {
  r := hex_decode_checked(text, out, false);
  #assert r.error == 0;
  return out[0..<r.written];
}
// Print a label and one byte value in hex.
show_byte :: (label: string, v: u32) {
  b: [1] u8;
  b[0] = cast(u8) v;
  show(label, b[..]);
}

ERROR :: [13] string.["ok", "bad key", "bad format", "bad base64", "bad iv",
  "bad length", "bad output", "overlap", "bad padding", "bad peer",
  "bad secret", "retired: use NIP-17", "bad work"];

// Test keys: the NIP-44 encrypt_decrypt vector 3 keys from chapter 14.
SEC1 :: "5c0c523f52a5b6fad39ed2403092df8cebc36318b39383bca6c00808626fab3a";
SEC2 :: "4b22aa260e4acb7021e32f38a6cdf4b673c6a277755bfce287e370c924dc936d";
PUB1 :: "87d3561f19b74adbe8bf840682992466068830a9d8c36b4a0c99d36f826cb6cb";
PUB2 :: "fa3b4f81a620c66514bda0302847df167ed02a483141b5939e57bdd0cf76ad3b";
// Made with OpenSSL from the shared x of SEC1 and PUB2.
CONTENT :: "zooMmtdlrlTCiIbfLqTqYrpCbTWrcVkP7RdcqCuM8TQ=?iv=obLD1OX2BxgpOktcbX6PkA==";

decrypt :: (label: string, key: [] u8, content: string) {
  out: [64] u8; work: [1] Nip04Work;
  r := nip04_decrypt_checked(key, content, out[..], work[..], false);
  if r.error == NIP04_OK { print("%: ok, \"%\"\n", label, cast(string) out[0..<r.written]); }
  else { print("%: %\n", label, ERROR[r.error]); }
}

main :: () {
  // One S-box value, FIPS-197 section 5.1.1.
  show_byte("inverse of 53 in GF(2^8)", aes_gf_inverse(0x53));
  show_byte("S(53)", aes_sbox(0x53));
  show_byte("InvS(ed)", aes_inv_sbox(0xed));

  // FIPS-197 appendix C.3: the inverse cipher, round 0 and the first steps
  // of round 1, then the whole block.
  key: [32] u8;
  for i: 0..<32 { key[i] = cast(u8) i; }
  s := aes256_expand(key[..]);
  st: AesBlock; ib: [16] u8;
  input := from_hex("8ea2b7ca516745bfeafc49904b496089", ib[..]);
  for i: 0..<16 { st.b[i] = input[i]; }
  show("round[ 0].iinput", st.b[..]);
  show("round[ 0].ik_sch", s.b[224..<240]);
  st = aes_add_round_key(st, s, 14);
  show("round[ 1].istart", st.b[..]);
  st = aes_inv_shift_rows(st);
  show("round[ 1].is_row", st.b[..]);
  st = aes_inv_sub_bytes(st);
  show("round[ 1].is_box", st.b[..]);
  show("round[ 1].ik_sch", s.b[208..<224]);
  st = aes_add_round_key(st, s, 13);
  show("round[ 1].ik_add", st.b[..]);
  for i: 0..<16 { st.b[i] = input[i]; }
  st = aes256_decrypt_block(s, st);
  show("round[14].ioutput", st.b[..]);

  // NIST SP 800-38A F.2.6, CBC-AES256.Decrypt.
  kb: [32] u8; vb: [16] u8; cb: [64] u8; plain: [64] u8; work: [1] AesSchedule;
  r := aes256_cbc_decrypt_checked(
    from_hex("603deb1015ca71be2b73aef0857d77811f352c073b6108d72d9810a30914dff4", kb[..]),
    from_hex("000102030405060708090a0b0c0d0e0f", vb[..]),
    from_hex("f58c4c04d6e5f1ba779eabfb5f7bfbd69cfc4e967edb808d679f777bc6702c7d39f23369a9d9bacfa530e26304231461b2eb05e2c39be9fcda6c19078c6a9d1b", cb[..]),
    plain[..], work[..], false);
  print("SP 800-38A F.2.6: %, % bytes\n", ifx r.error == AES_OK then "ok" else "refused", r.written);
  show("plaintext block 1", plain[0..<16]);
  show("plaintext block 4", plain[48..<64]);

  // A NIP-04 message. Split the content, then decrypt with the raw shared x.
  parts := nip04_split(CONTENT);
  print("cipher text: %\n", parts.cipher);
  print("iv text: %\n", parts.iv);
  bytes: [32] u8;
  n := base64_decode_checked(parts.cipher, bytes[..], false).written;
  show("ciphertext", bytes[0..<n]);
  n = base64_decode_checked(parts.iv, bytes[..], false).written;
  show("iv", bytes[0..<n]);
  s1: [32] u8; s2: [32] u8; p1: [32] u8; p2: [32] u8;
  sec1 := from_hex(SEC1, s1[..]); sec2 := from_hex(SEC2, s2[..]);
  pub1 := from_hex(PUB1, p1[..]); pub2 := from_hex(PUB2, p2[..]);
  ew: [1] EcdhScratch;
  #assert ecdh_stage(sec1, pub2, ew[..]) == EC_OK;
  x := ew[0].staged;
  ew[0] = EcdhScratch.{};
  show("shared x (the AES key, unhashed)", x[..]);
  decrypt("decrypt with x", x[..], CONTENT);
  out: [64] u8; nw: [1] Nip04Work;
  d := nip04_decrypt_from_keys_checked(sec2, pub1, CONTENT, out[..], nw[..], false);
  print("decrypt with sec2 and pub1: %, \"%\"\n", ERROR[d.error], cast(string) out[0..<d.written]);

  // No authentication: flip bit 0 of the iv (a1 becomes a0). Decryption
  // succeeds and plaintext byte 0 changes from N (4e) to O (4f).
  decrypt("iv byte 0 flipped", x[..], "zooMmtdlrlTCiIbfLqTqYrpCbTWrcVkP7RdcqCuM8TQ=?iv=oLLD1OX2BxgpOktcbX6PkA==");

  // Refusals.
  decrypt("last block dropped", x[..], "zooMmtdlrlTCiIbfLqTqYg==?iv=obLD1OX2BxgpOktcbX6PkA==");
  decrypt("no ?iv= part", x[..], "zooMmtdlrlTCiIbfLqTqYrpCbTWrcVkP7RdcqCuM8TQ=");
  decrypt("12-byte iv", x[..], "zooMmtdlrlTCiIbfLqTqYrpCbTWrcVkP7RdcqCuM8TQ=?iv=obLD1OX2BxgpOktc");
  decrypt("15-byte ciphertext", x[..], "zooMmtdlrlTCiIbfLqTq?iv=obLD1OX2BxgpOktcbX6PkA==");
  decrypt("base64 without its padding", x[..], "zooMmtdlrlTCiIbfLqTqYg?iv=obLD1OX2BxgpOktcbX6PkA==");
  sec3: [32] u8;
  for i: 0..<32 { sec3[i] = sec1[i]; }
  sec3[31] ^= 1;
  d = nip04_decrypt_from_keys_checked(sec3[..], pub2, CONTENT, out[..], nw[..], false);
  print("decrypt with another secret: %\n", ERROR[d.error]);
  d = nip04_encrypt(x[..], "a new message", out[..]);
  print("encrypt: %, % bytes\n", ERROR[d.error], d.written);
}
```

Run it from the repository root:

```shell
coax/bin/coax run nostr/examples/nip04.coax
```

The output matches the values worked out by hand: `ca` and `ed`, the
`istart` byte `aa`, the `is_row` byte `21`, the `is_box` byte `62`, the
`ik_add` byte `2c`, and the iv `a1b2...`:

```text
inverse of 53 in GF(2^8): ca
S(53): ed
InvS(ed): 53
round[ 0].iinput: 8ea2b7ca516745bfeafc49904b496089
round[ 0].ik_sch: 24fc79ccbf0979e9371ac23c6d68de36
round[ 1].istart: aa5ece06ee6e3c56dde68bac2621bebf
round[ 1].is_row: aa218b56ee5ebeacdd6ecebf26e63c06
round[ 1].is_box: 627bceb9999d5aaac945ecf423f56da5
round[ 1].ik_sch: 4e5a6699a9f24fe07e572baacdf8cdea
round[ 1].ik_add: 2c21a820306f154ab712c75eee0da04f
round[14].ioutput: 00112233445566778899aabbccddeeff
SP 800-38A F.2.6: ok, 64 bytes
plaintext block 1: 6bc1bee22e409f96e93d7e117393172a
plaintext block 4: f69f2445df4f9b17ad2b417be66c3710
cipher text: zooMmtdlrlTCiIbfLqTqYrpCbTWrcVkP7RdcqCuM8TQ=
iv text: obLD1OX2BxgpOktcbX6PkA==
ciphertext: ce8a0c9ad765ae54c28886df2ea4ea62ba426d35ab71590fed175ca82b8cf134
iv: a1b2c3d4e5f60718293a4b5c6d7e8f90
shared x (the AES key, unhashed): 94da47d851b9c1ed33b3b72f35434f56aa608d60e573e9c295f568011f4f50a4
decrypt with x: ok, "NIP-04 is retired; use NIP-17."
decrypt with sec2 and pub1: ok, "NIP-04 is retired; use NIP-17."
iv byte 0 flipped: ok, "OIP-04 is retired; use NIP-17."
last block dropped: bad padding
no ?iv= part: bad format
12-byte iv: bad iv
15-byte ciphertext: bad length
base64 without its padding: bad base64
decrypt with another secret: bad padding
encrypt: retired: use NIP-17, 0 bytes
```

The FIPS-197 output and the SP 800-38A plaintext blocks are the published
values. The message decrypts with `sec1`'s shared x and with `sec2` and
`pub1`, the other side's keys. With bit 0 of the iv flipped, decryption
still reports `ok`, with `O` for `N`.

The refusals:

- With the last block dropped, the remaining block decrypts to
  `NIP-04 is retire`, whose last byte `e` (`65`) is not a pad length:
  `bad padding` (`NIP04_PADDING`), and the decrypted bytes are cleared.
- Content with no `?iv=` refuses with `bad format` (`NIP04_FORMAT`).
- A 12-byte iv refuses with `bad iv` (`NIP04_IV`).
- A 15-byte ciphertext, not a whole block, refuses with `bad length`
  (`NIP04_LENGTH`).
- Base64 without its `==` refuses with `bad base64` (`NIP04_BASE64`); Bitcoin chapter
  1's decoder takes only canonical, padded base64.
- A secret with one bit changed gives another x, and the padding check
  refuses. This key fails; about one wrong key in 256 would not.
- `nip04_encrypt` refuses with `retired: use NIP-17` (`NIP04_RETIRED`) and
  writes nothing.

The book's checks run this program in debug and release builds and compare
the output with [nostr/fixtures/nip04.out](../fixtures/nip04.out).

## How the code is checked

### Checks in the library

`aes.coax` carries its own `#test` and `#forall` checks: the FIPS-197
section 4.2 products and section 5.1.1 S-box value, the S-box and inverse
S-box inverting each other on all 256 bytes, the appendix A.3 AES-256 key
expansion words, the appendix C.3 inverse-cipher steps and output, known
InvMixColumns columns, the SP 800-38A F.2.6 vector both copied and in place,
the bit-flip behavior of CBC, and every admission refusal with the output
untouched and the work buffer cleared.

`nip04.coax` checks the book's message from the shared x and from both key
pairs, an empty message (one block of `10` bytes), the iv bit flip, every
refusal with the output untouched, the padding refusal with the output
cleared, PKCS#7 for every pad value 0 to 19 and every changed pad byte, the
split, and that `nip04_encrypt` refuses.

`coax mutate --root-only` was run on both files on 2026-11-01:
`aes.coax` 279 mutants, 279 killed; `nip04.coax` 112 mutants, 112 killed.
No timing measurement was run on `aes.coax`.

### Checks in the example

The checks in
[nostr/examples/checks/nip04.coax](../examples/checks/nip04.coax) load the
example and compare it with independent values:

- the FIPS-197 appendix C.3 plaintext;
- each SP 800-38A F.2.6 block decrypted alone, with the previous ciphertext
  block as its iv;
- five messages made with OpenSSL 3 (`openssl enc -aes-256-cbc -K <x> -iv
  a1b2c3d4e5f60718293a4b5c6d7e8f90 -a`), an AES implementation independent
  of this one, of 0, 15, 16, 17 and 30 bytes, so the pads are 16, 1, 16, 15
  and 2 bytes;
- the example's message from both key pairs, and each of its refusals;
- `nip04_encrypt` refusing for every length 0 to 39.

<!-- source: nostr/examples/checks/nip04.coax check -->
```jai
// True when bytes equal the hex text.
same_hex :: (bytes: [] u8, text: string) -> bool {
  want: [64] u8;
  w := from_hex(text, want[..]);
  if w.count != bytes.count { return false; }
  for i: 0..<w.count { if w[i] != bytes[i] { return false; } }
  return true;
}
#test "same_hex tells a differing byte and another length apart" {
  a := [2] u8.[0xab, 0xcd];
  #assert same_hex(a[..], "abcd") && !same_hex(a[..], "abce") && !same_hex(a[..], "ab");
}
// FIPS-197 appendix C.3, AES-256: the published plaintext and output.
#test "FIPS-197 C.3: the inverse cipher gives the plaintext" {
  key: [32] u8; for i: 0..<32 { key[i] = cast(u8) i; }
  c: AesBlock; cb: [16] u8;
  input := from_hex("8ea2b7ca516745bfeafc49904b496089", cb[..]);
  for i: 0..<16 { c.b[i] = input[i]; }
  p := aes256_decrypt_block(aes256_expand(key[..]), c);
  #assert same_hex(p.b[..], "00112233445566778899aabbccddeeff");
}
// NIST SP 800-38A F.2.6 CBC-AES256.Decrypt: key, iv, four ciphertext
// blocks and their plaintext.
SP_KEY :: "603deb1015ca71be2b73aef0857d77811f352c073b6108d72d9810a30914dff4";
SP_IV :: [4] string.["000102030405060708090a0b0c0d0e0f", "f58c4c04d6e5f1ba779eabfb5f7bfbd6",
  "9cfc4e967edb808d679f777bc6702c7d", "39f23369a9d9bacfa530e26304231461"];
SP_CIPHER :: [4] string.["f58c4c04d6e5f1ba779eabfb5f7bfbd6", "9cfc4e967edb808d679f777bc6702c7d",
  "39f23369a9d9bacfa530e26304231461", "b2eb05e2c39be9fcda6c19078c6a9d1b"];
SP_PLAIN :: [4] string.["6bc1bee22e409f96e93d7e117393172a", "ae2d8a571e03ac9c9eb76fac45af8e51",
  "30c81c46a35ce411e5fbc1191a0a52ef", "f69f2445df4f9b17ad2b417be66c3710"];
// Each block alone, with the previous ciphertext block as its iv, gives
// the published plaintext block: CBC chains through ciphertext only.
#forall "SP 800-38A F.2.6, block by block" i: 0..<4 {
  kb: [32] u8; vb: [16] u8; cb: [16] u8; out: [16] u8; work: [1] AesSchedule;
  r := aes256_cbc_decrypt_checked(from_hex(SP_KEY, kb[..]), from_hex(SP_IV[i], vb[..]),
    from_hex(SP_CIPHER[i], cb[..]), out[..], work[..], false);
  #assert r.error == AES_OK && r.written == 16;
  #assert same_hex(out[..], SP_PLAIN[i]);
}
// Messages made with OpenSSL 3 (`openssl enc -aes-256-cbc -K <x> -iv
// a1b2c3d4e5f60718293a4b5c6d7e8f90 -a`), an AES implementation independent
// of this one, under the shared x of chapter 14's test keys. Their lengths
// 0, 15, 16, 17 and 30 give PKCS#7 pads of 16, 1, 16, 15 and 2 bytes.
X :: "94da47d851b9c1ed33b3b72f35434f56aa608d60e573e9c295f568011f4f50a4";
OPENSSL_CIPHER :: [5] string.["iUdipJZvO3VdgsMSaTL2kQ==", "2i5vnNPruQiiEq12swcSzA==",
  "YB9MjuMA/rzcSEIDlSqXvWmsrcaT7c/FI92sITYe8dM=", "wWmkCyYMuMCxVGdlqbNfG0lwlOzho6l7gHfIkfZp/pw=",
  "zooMmtdlrlTCiIbfLqTqYrpCbTWrcVkP7RdcqCuM8TQ="];
OPENSSL_PLAIN :: [5] string.["", "fifteen bytes!!", "sixteen bytes!!!", "seventeen bytes!!",
  "NIP-04 is retired; use NIP-17."];
#forall "OpenSSL-made NIP-04 messages decrypt to their plaintext" i: 0..<5 #steps 100000000 {
  xb: [32] u8; out: [48] u8; work: [1] Nip04Work; content: [128] u8;
  text := OPENSSL_CIPHER[i];
  n := 0;
  for j: 0..<text.count { content[n] = text[j]; n += 1; }
  tail := "?iv=obLD1OX2BxgpOktcbX6PkA==";
  for j: 0..<tail.count { content[n] = tail[j]; n += 1; }
  r := nip04_decrypt_checked(from_hex(X, xb[..]), cast(string) content[0..<n], out[..], work[..], false);
  #assert r.error == NIP04_OK && r.written == OPENSSL_PLAIN[i].count;
  #assert cast(string) out[0..<r.written] == OPENSSL_PLAIN[i];
}
// The example's message from either side's keys, and the refusals it shows.
#test "both key pairs read the message; the broken inputs refuse" #steps 400000000 {
  s1: [32] u8; s2: [32] u8; p1: [32] u8; p2: [32] u8; xb: [32] u8;
  out: [64] u8; work: [1] Nip04Work;
  r := nip04_decrypt_from_keys_checked(from_hex(SEC1, s1[..]), from_hex(PUB2, p2[..]), CONTENT, out[..], work[..], false);
  #assert r.error == NIP04_OK && cast(string) out[0..<r.written] == "NIP-04 is retired; use NIP-17.";
  r = nip04_decrypt_from_keys_checked(from_hex(SEC2, s2[..]), from_hex(PUB1, p1[..]), CONTENT, out[..], work[..], false);
  #assert r.error == NIP04_OK && r.written == 30;
  x := from_hex(X, xb[..]);
  #assert nip04_decrypt_checked(x, "zooMmtdlrlTCiIbfLqTqYg==?iv=obLD1OX2BxgpOktcbX6PkA==", out[..], work[..], false).error == NIP04_PADDING;
  #assert nip04_decrypt_checked(x, "zooMmtdlrlTCiIbfLqTqYrpCbTWrcVkP7RdcqCuM8TQ=", out[..], work[..], false).error == NIP04_FORMAT;
  #assert nip04_decrypt_checked(x, "zooMmtdlrlTCiIbfLqTqYrpCbTWrcVkP7RdcqCuM8TQ=?iv=obLD1OX2BxgpOktc", out[..], work[..], false).error == NIP04_IV;
  #assert nip04_decrypt_checked(x, "zooMmtdlrlTCiIbfLqTq?iv=obLD1OX2BxgpOktcbX6PkA==", out[..], work[..], false).error == NIP04_LENGTH;
  #assert nip04_decrypt_checked(x, "zooMmtdlrlTCiIbfLqTqYg?iv=obLD1OX2BxgpOktcbX6PkA==", out[..], work[..], false).error == NIP04_BASE64;
}
// Encryption is refused for every input length and leaves the output alone.
#forall "encrypt refuses for every length" n: 0..<40 {
  xb: [32] u8; out: [64] u8; text: [40] u8;
  for i: 0..<64 { out[i] = 77; }
  r := nip04_encrypt(from_hex(X, xb[..]), cast(string) text[0..<n], out[..]);
  #assert r.error == NIP04_RETIRED && r.written == 0;
  for b: out { #assert b == 77; }
}
```

## Summary

- NIP-04 content is `base64(AES-256-CBC(x, iv, PKCS#7(text)))?iv=base64(iv)`,
  with `x` the unhashed ECDH x.
- AES decryption here computes the S-box from the GF(2^8) inverse, with no
  table indexed by secret bytes; constant time is intended, not measured.
- The reader passes FIPS-197, SP 800-38A and OpenSSL-made messages, and
  refuses bad format, base64, iv, length and padding.
- NIP-04 has no authentication, leaks sender, recipient, time and length,
  uses one unhashed key for every message, and doesn't require a random iv.
- This stack reads NIP-04 and never writes it: `nip04_encrypt` returns
  `NIP04_RETIRED`.

## Dependencies

- [14. Shared secrets with ECDH](ch014-shared-secrets-with-ecdh.md)
- [1. Bytes, hex and base64](../../bitcoin/book/ch001-bytes-hex-and-base64.md)

## Sources

- [NIP-04](../nips/official/04.md), pinned.
- [FIPS-197](https://csrc.nist.gov/pubs/fips/197/final), *Advanced
  Encryption Standard*, sections 4.2, 5.1.1, 5.2 and 5.3, and appendices A.3
  and C.3.
- [NIST SP 800-38A](https://csrc.nist.gov/pubs/sp/800/38/a/final),
  *Recommendation for Block Cipher Modes of Operation*, section 6.2 and
  appendix F.2.6.
- [RFC 5652](https://www.rfc-editor.org/rfc/rfc5652), section 6.3, for PKCS#7
  padding.
- D. J. Bernstein, *Cache-timing attacks on AES* (2005); D. A. Osvik,
  A. Shamir and E. Tromer, *Cache Attacks and Countermeasures: the Case of
  AES* (2005); S. Vaudenay, *Security Flaws Induced by CBC Padding* (2002).
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
