# 11. ChaCha20

Bitcoin's v2 P2P transport (BIP-324, chapter 67) and Lightning's Noise
handshake and onions (chapters 105 and 111) encrypt with ChaCha20. NIP-44
encrypts with it too, and NIP-49's XChaCha20 starts from it. This
chapter builds the quarter round, the block function and the stream, and
the rules about nonces and counters that keep it safe. Every operation is a
32-bit addition, an exclusive or or a rotation, so the code has no tables
and no branches on the key or the data.

Part II, Hashes, MACs and ciphers. Result: RFC 8439 ChaCha20 with caller-supplied nonce and
counter, counter exhaustion refused, and HChaCha20 for NIP-49's XChaCha20.

## What this chapter teaches

- The 4x4 state: constants, 256-bit key, 32-bit counter, 96-bit nonce.
- The quarter round and 20 rounds; the block is the state plus the input.
- Counter overflow refuses; a key and nonce pair is never reused.
- ChaCha20 alone does not authenticate; NIP-44 adds HMAC.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [chacha20.coax](../../coax/lib/crypto/chacha20.coax) | `cc_quarter(a, b, c, d)` | one quarter round |
| | `cc_initial(key, nonce, counter)` | the 16-word starting state |
| | `cc_block_words(state)` | 20 rounds, then add the starting state |
| | `cc_admit(...)` | every size, counter and overlap check, before any write |
| | `chacha20_block_checked` | one 64-byte block of key stream |
| | `chacha20_xor_checked` | encrypt or decrypt any length up to 1 MiB |
| [chacha20-memory.coax](../../coax/lib/crypto/chacha20-memory.coax) | `chacha20_block`, `chacha20_xor` | the same, with overlap found from the buffers' addresses |
| [hchacha20.coax](../../coax/lib/crypto/hchacha20.coax) | `hchacha20_checked` | a 32-byte subkey from a key and a 16-byte nonce |

The file is `#authority`: it decides what a caller may encrypt, so its
checks must kill every mutant. Each `_checked` procedure takes an `overlap`
flag that its caller has worked out; the `-memory` wrappers compute it from
the buffers.

Every procedure returns `ChaChaResult { written, error }`. The errors are
`CC_KEY` (1), `CC_NONCE` (2), `CC_INPUT` (3), `CC_OUTPUT` (4),
`CC_COUNTER` (5) and `CC_ALIAS` (6); zero is success. On every refusal
`written` is zero and the output buffer is unchanged.

## The state

ChaCha20 works on 16 words of 32 bits, laid out as a 4x4 grid:

```jai
cccccccc  cccccccc  cccccccc  cccccccc     c: constants
kkkkkkkk  kkkkkkkk  kkkkkkkk  kkkkkkkk     k: key
kkkkkkkk  kkkkkkkk  kkkkkkkk  kkkkkkkk
bbbbbbbb  nnnnnnnn  nnnnnnnn  nnnnnnnn     b: block counter, n: nonce
```

- Words 0 to 3 are the ASCII text `expand 32-byte k`, read as four
  little-endian words: `61707865 3320646e 79622d32 6b206574`.
- Words 4 to 11 are the 32-byte key, read as eight little-endian words.
- Word 12 is the block counter.
- Words 13 to 15 are the 12-byte nonce, read as three little-endian words.

Every multi-byte read is little-endian: the first byte is the lowest. The
key bytes `00 01 02 03` become the word `03020100`, and the nonce bytes
`00 00 00 09` become `09000000`.

This is the IETF layout of RFC 8439. Bernstein's original ChaCha20 used a
64-bit counter and an 8-byte nonce. The IETF version takes 32 bits from the
counter for a longer nonce, so one key and nonce can encrypt at most
`2^32` blocks of 64 bytes, 256 GiB. The code refuses an 8-byte nonce.

<!-- source: coax/lib/crypto/chacha20.coax cc_initial -->
```jai
cc_initial :: (key:[]u8,nonce:[]u8,counter:u32)->CcWords {
  state:CcWords;
  state.words[0]=0x61707865;state.words[1]=0x3320646e;
  state.words[2]=0x79622d32;state.words[3]=0x6b206574;
  state.words[12]=counter;
  for i:0..<8 {state.words[4+i]=cb_load_le32(key,i*4).value;}
  for i:0..<3 {state.words[13+i]=cb_load_le32(nonce,i*4).value;}
  return state;
}
```

## The quarter round

A quarter round changes four words `a`, `b`, `c`, `d`. Each of its four
lines adds, exclusive-ors, then rotates left:

<!-- source: coax/lib/crypto/chacha20.coax cc_quarter -->
```jai
cc_quarter :: (a_in:u32,b_in:u32,c_in:u32,d_in:u32)->CcQuarter {
  a:=a_in;b:=b_in;c:=c_in;d:=d_in;
  a=add_wrap(a,b);d=rotl(d^a,16);
  c=add_wrap(c,d);b=rotl(b^c,12);
  a=add_wrap(a,b);d=rotl(d^a,8);
  c=add_wrap(c,d);b=rotl(b^c,7);
  return CcQuarter.{a=a,b=b,c=c,d=d};
}
```

`add_wrap` is addition modulo `2^32`. Coax's plain `+` traps on overflow
in checked builds, so cipher arithmetic that is meant to wrap says so.
`rotl(x, n)` moves the top `n` bits to the bottom.

### Worked by hand

RFC 8439 section 2.1.1 starts from `a = 11111111`, `b = 01020304`,
`c = 9b8d6f43`, `d = 01234567` (all hex). Work the eight steps:

1. `a = a + b = 11111111 + 01020304 = 12131415`.
2. `d ^ a = 01234567 ^ 12131415 = 13305172`. Rotating left by 16 swaps
   the halves: `d = 51721330`.
3. `c = c + d = 9b8d6f43 + 51721330 = ecff8273`.
4. `b ^ c = 01020304 ^ ecff8273 = edfd8177`. Rotating left by 12 moves
   the top three hex digits to the end: `b = d8177edf`.
5. `a = a + b = 12131415 + d8177edf = ea2a92f4`.
6. `d ^ a = 51721330 ^ ea2a92f4 = bb5881c4`. Rotating left by 8 moves
   the top two hex digits to the end: `d = 5881c4bb`.
7. `c = c + d = ecff8273 + 5881c4bb = 1 4581472e`. The carry out of 32
   bits is dropped: `c = 4581472e`.
8. `b ^ c = d8177edf ^ 4581472e = 9d9639f1`. Rotating left by 7 is not a
   whole number of hex digits; in binary, the top seven bits `1001110` move
   to the bottom: `b = cb1cf8ce`.

The result is `ea2a92f4 cb1cf8ce 4581472e 5881c4bb`, the values RFC 8439
gives. The example prints them from `cc_quarter`.

## Twenty rounds and the block

A column round runs the quarter round on each column of the grid: words
(0, 4, 8, 12), (1, 5, 9, 13), (2, 6, 10, 14), (3, 7, 11, 15). A diagonal
round runs it on each diagonal: (0, 5, 10, 15), (1, 6, 11, 12),
(2, 7, 8, 13), (3, 4, 9, 14). ChaCha20 runs ten column rounds, each
followed by a diagonal round: 20 rounds and 80 quarter rounds.

The rounds alone can be run backwards, so the block function then adds the
starting state to the result, word by word. Without that addition, anyone
holding a block could undo the rounds and read the key out of words 4 to 11.

<!-- source: coax/lib/crypto/chacha20.coax cc_block_words -->
```jai
cc_block_words :: (initial:CcWords)->CcWords {
  x:=initial;
  for round:0..<10 {
    q:=cc_quarter(x.words[0],x.words[4],x.words[8],x.words[12]);
    x.words[0]=q.a;x.words[4]=q.b;x.words[8]=q.c;x.words[12]=q.d;
    q=cc_quarter(x.words[1],x.words[5],x.words[9],x.words[13]);
    x.words[1]=q.a;x.words[5]=q.b;x.words[9]=q.c;x.words[13]=q.d;
    q=cc_quarter(x.words[2],x.words[6],x.words[10],x.words[14]);
    x.words[2]=q.a;x.words[6]=q.b;x.words[10]=q.c;x.words[14]=q.d;
    q=cc_quarter(x.words[3],x.words[7],x.words[11],x.words[15]);
    x.words[3]=q.a;x.words[7]=q.b;x.words[11]=q.c;x.words[15]=q.d;
    q=cc_quarter(x.words[0],x.words[5],x.words[10],x.words[15]);
    x.words[0]=q.a;x.words[5]=q.b;x.words[10]=q.c;x.words[15]=q.d;
    q=cc_quarter(x.words[1],x.words[6],x.words[11],x.words[12]);
    x.words[1]=q.a;x.words[6]=q.b;x.words[11]=q.c;x.words[12]=q.d;
    q=cc_quarter(x.words[2],x.words[7],x.words[8],x.words[13]);
    x.words[2]=q.a;x.words[7]=q.b;x.words[8]=q.c;x.words[13]=q.d;
    q=cc_quarter(x.words[3],x.words[4],x.words[9],x.words[14]);
    x.words[3]=q.a;x.words[4]=q.b;x.words[9]=q.c;x.words[14]=q.d;
  }
  for i:0..<16 {x.words[i]=add_wrap(x.words[i],initial.words[i]);}
  return x;
}
```

### The section 2.3.2 block

RFC 8439 section 2.3.2 uses the key `00 01 02 ... 1f`, the nonce
`00 00 00 09 00 00 00 4a 00 00 00 00` and block counter 1. By the layout
above, the starting state is:

```jai
61707865  3320646e  79622d32  6b206574
03020100  07060504  0b0a0908  0f0e0d0c
13121110  17161514  1b1a1918  1f1e1d1c
00000001  09000000  4a000000  00000000
```

The RFC gives the first row after the 20 rounds as
`837778ab e238d763 a67ae21e 5950bb2f`. Adding the first starting row:
`837778ab + 61707865 = e4e7f110`, `e238d763 + 3320646e = 1 15593bd1`, which
drops its carry to `15593bd1`, and so on. Serialized little-endian, the
word `e4e7f110` is the bytes `10 f1 e7 e4`, the first four bytes of the
RFC's serialized block.

## The stream

To encrypt, generate blocks at counter, counter + 1, counter + 2 and so on,
lay them end to end as a key stream, and exclusive-or it with the plaintext.
The last block is cut to the length of the message. Decryption is the same
operation, because `(p ^ s) ^ s = p`.

<!-- source: coax/lib/crypto/chacha20.coax chacha20_xor_checked -->
```jai
chacha20_xor_checked :: (key:[]u8,nonce:[]u8,counter:u32,input:[]u8,out:[]u8,overlap:bool)->ChaChaResult #export_c {
  error:=cc_admit(key.count,nonce.count,counter,input.count,out.count,overlap);
  if error!=CC_OK {return ChaChaResult.{error=error};}
  initial:=cc_initial(key,nonce,counter);
  for block_index:0..<cc_blocks(input.count) {
    at:=block_index*64;
    initial.words[12]=add_wrap(counter,cast(u32)block_index);
    block:=cc_block_words(initial);
    for j:0..<64 {
      if at+j>=input.count {break;}
      stream:=cast(u8)((block.words[j/4]>>((j%4)*8))&255);
      out[at+j]=input[at+j]^stream;
    }
  }
  return ChaChaResult.{written=input.count};
}
```

RFC 8439 section 2.4.2 encrypts the 114-byte sunscreen sentence with the
same key, the nonce `00 00 00 00 00 00 00 4a 00 00 00 00` and counter 1.
114 bytes is one full block and 50 bytes of a second, so counters 1 and 2
are used. The RFC's key stream starts `22 4f`; the plaintext starts `L a`,
bytes `4c 61`. `22 ^ 4c = 6e` and `4f ^ 61 = 2e`, the RFC's first two
ciphertext bytes.

### Nonces and counters

The key stream depends only on the key, the nonce and the counter. Two
messages encrypted with the same three values share a key stream, and the
exclusive-or of the two ciphertexts is the exclusive-or of the two
plaintexts: no key needed. So:

- Never use one key and nonce twice, unless the counter ranges don't
  overlap. NIP-44 derives a fresh key and nonce for every message from a
  random 32-byte value (Nostr chapter 20).
- The counter must not wrap. At `ffffffff` the next counter would be 0, the
  block that started the stream. RFC 8439 says the counter must not carry
  into the nonce either.

`cc_admit` checks the whole request before anything is written. A request
of `n` bytes needs `ceil(n / 64)` blocks, and from counter `c` only
`2^32 - c` blocks are left:

<!-- source: coax/lib/crypto/chacha20.coax cc_admit -->
```jai
cc_admit :: (key_count:int,nonce_count:int,counter:u32,n:int,capacity:int,overlap:bool)->u32 #export_c {
  if key_count!=32 {return CC_KEY;}
  if nonce_count!=12 {return CC_NONCE;}
  if n<0 || n>CB_MAX_BYTES {return CC_INPUT;}
  if capacity<n || capacity>CB_MAX_BYTES {return CC_OUTPUT;}
  blocks:=cc_blocks(n);
  if cast(u64)blocks > 4294967296-cast(u64)counter {return CC_COUNTER;}
  if n>0 && overlap {return CC_ALIAS;}
  return CC_OK;
}
```

Worked: at counter `ffffffff`, `2^32 - ffffffff = 1` block is left. A 64-byte
request needs one block and passes. A 65-byte request needs two and refuses
with `CC_COUNTER`. From `fffffffe`, two blocks are left, and 65 bytes pass.
The sums are done in 64 bits, so `2^32 - c` itself can't wrap.

The 1 MiB limit (`CB_MAX_BYTES`) is the shared limit of the byte helpers in
chapter 1; NIP-44 messages are at most 64 KiB. The overlap refusal means the
output may not share memory with the key, the nonce or the input, so
in-place encryption is refused.

### No authentication

ChaCha20 hides the plaintext but doesn't protect it. Flip a bit of the
ciphertext and decryption still succeeds, with the same bit flipped in the
plaintext; the receiver can't tell. An attacker who knows or guesses part of
a message can change it to anything of the same length. NIP-44 therefore
adds an HMAC-SHA256 over the nonce and ciphertext (chapter 8), and checks it
before decrypting. NIP-49 uses XChaCha20-Poly1305, whose Poly1305 tag does
the same job.

## HChaCha20

XChaCha20 takes a 24-byte nonce, long enough to pick at random without
worrying about a repeat. Its first step, HChaCha20, puts the key and the
first 16 nonce bytes in the state (the first nonce word where the counter
goes), runs the 20 rounds, and outputs words 0 to 3 and 12 to 15 as a
32-byte subkey. It has no final addition: `hchacha20_checked` subtracts the
starting state back out of `cc_block_words`' result. XChaCha20 then runs
ordinary ChaCha20 with the subkey and the last 8 nonce bytes. Nostr chapter 47
uses it for NIP-49 key backups.

<!-- source: coax/lib/crypto/hchacha20.coax hchacha20_checked -->
```jai
hchacha20_checked :: (key:[]u8,nonce:[]u8,out:[]u8,overlap:bool)->ChaChaResult #export_c {
  error:=hc_admit(key.count,nonce.count,out.count,overlap);
  if error!=CC_OK {return ChaChaResult.{error=error};}
  initial:=cc_initial(key,nonce[4..<16],cb_load_le32(nonce,0).value);
  block:=cc_block_words(initial);
  for i:0..<8 {
    source:=ifx i<4 then i else i+8;
    word:=sub_wrap(block.words[source],initial.words[source]);
    stored:=cb_store_le32(out,i*4,word);
  }
  return ChaChaResult.{written=32};
}
```

## The example

The example is [bitcoin/examples/chacha20.coax](../examples/chacha20.coax).
It runs the section 2.1.1 quarter round, sets up and runs the section 2.3.2
block, encrypts and decrypts the section 2.4.2 sentence, shows that a
flipped ciphertext bit goes undetected, derives the HChaCha20 test subkey,
and shows the counter and size refusals.

<!-- source: bitcoin/examples/chacha20.coax example -->
```jai
#load "../../coax/lib/crypto/chacha20.coax";
#load "../../coax/lib/crypto/hchacha20.coax";
#load "../../coax/lib/nostr/hex.coax";
#import "Basic";

// Print a label and bytes as lowercase hex.
show :: (label: string, bytes: [] u8) {
  text: [160] u8;
  r := hex_encode_checked(cast(string) bytes, text[..], false);
  print("%: %\n", label, cast(string) text[0..<r.written]);
}
// Print a label and four 32-bit words as 8 hex digits each.
show_words :: (label: string, a: u32, b: u32, c: u32, d: u32) {
  w: [16] u8;
  ok := cb_store_be32(w[..], 0, a) && cb_store_be32(w[..], 4, b) &&
        cb_store_be32(w[..], 8, c) && cb_store_be32(w[..], 12, d);
  text: [32] u8;
  r := hex_encode_checked(cast(string) w[..], text[..], false);
  t := cast(string) text[0..<r.written];
  if ok { print("%: % % % %\n", label, t[0..<8], t[8..<16], t[16..<24], t[24..<32]); }
}
// Copy a string's bytes into a buffer and return the filled part.
bytes_of :: (text: string, out: [] u8) -> [] u8 {
  for i: 0..<text.count { out[i] = text[i]; }
  return out[0..<text.count];
}
// Decode hex that the program itself spells; it must be valid.
from_hex :: (text: string, out: [] u8) -> [] u8 {
  r := hex_decode_checked(text, out, false);
  #assert r.error == 0;
  return out[0..<r.written];
}
// True when the first bytes of a and b agree.
same :: (a: [] u8, b: [] u8) -> bool {
  if a.count != b.count { return false; }
  for i: 0..<a.count { if a[i] != b[i] { return false; } }
  return true;
}

ERROR :: [7] string.["ok", "bad key", "bad nonce", "bad input",
  "bad output", "counter exhausted", "overlap"];

ROW :: [4] string.["state row 0", "state row 1", "state row 2", "state row 3"];

SUNSCREEN :: "Ladies and Gentlemen of the class of '99: If I could offer you only one tip for the future, sunscreen would be it.";
SUNSCREEN_CIPHER :: "6e2e359a2568f98041ba0728dd0d6981e97e7aec1d4360c20a27afccfd9fae0bf91b65c5524733ab8f593dabcd62b3571639d624e65152ab8f530c359f0861d807ca0dbf500d6a6156a38e088a22b65e52bc514d16ccf806818ce91ab77937365af90bbf74a35be6b40b8eedf2785e42874d";

main :: () {
  // RFC 8439 section 2.1.1: one quarter round.
  q := cc_quarter(0x11111111, 0x01020304, 0x9b8d6f43, 0x01234567);
  show_words("quarter round a b c d", q.a, q.b, q.c, q.d);

  // RFC 8439 section 2.3.2: the key 00..1f, nonce 000000090000004a00000000,
  // block counter 1.
  key: [32] u8;
  for i: 0..<32 { key[i] = cast(u8) i; }
  nb: [12] u8;
  nonce := from_hex("000000090000004a00000000", nb[..]);
  s := cc_initial(key[..], nonce, 1);
  for row: 0..<4 {
    show_words(ROW[row], s.words[4 * row], s.words[4 * row + 1], s.words[4 * row + 2], s.words[4 * row + 3]);
  }
  after := cc_block_words(s);
  show_words("block words 0-3", after.words[0], after.words[1], after.words[2], after.words[3]);
  block: [64] u8;
  r := chacha20_block_checked(key[..], nonce, 1, block[..], false);
  print("block: %, % bytes\n", ERROR[r.error], r.written);
  show("serialized block 0-15", block[0..<16]);
  show("serialized block 48-63", block[48..<64]);

  // RFC 8439 section 2.4.2: encrypt 114 bytes from counter 1, so counters
  // 1 and 2 make the two blocks of key stream.
  nonce = from_hex("000000000000004a00000000", nb[..]);
  pb: [114] u8; cipher: [114] u8; back: [114] u8; wb: [114] u8;
  plain := bytes_of(SUNSCREEN, pb[..]);
  r = chacha20_xor_checked(key[..], nonce, 1, plain, cipher[..], false);
  print("encrypt: %, % bytes\n", ERROR[r.error], r.written);
  show("ciphertext 0-15", cipher[0..<16]);
  show("ciphertext 112-113", cipher[112..<114]);
  print("ciphertext equals RFC 8439 2.4.2: %\n", same(cipher[..], from_hex(SUNSCREEN_CIPHER, wb[..])));
  r = chacha20_xor_checked(key[..], nonce, 1, cipher[..], back[..], false);
  print("decrypt: %, \"%\"\n", ERROR[r.error], cast(string) back[0..<23]);

  // No authentication: flip one ciphertext bit and decryption still
  // succeeds, with the same bit flipped in the plaintext.
  cipher[0] ^= 1;
  r = chacha20_xor_checked(key[..], nonce, 1, cipher[..], back[..], false);
  print("decrypt with byte 0 flipped: %, \"%\"\n", ERROR[r.error], cast(string) back[0..<23]);

  // HChaCha20, from the XChaCha draft section 2.2.1: a 16-byte nonce gives
  // a 32-byte subkey.
  hb: [16] u8; sub: [32] u8;
  r = hchacha20_checked(key[..], from_hex("000000090000004a0000000031415927", hb[..]), sub[..], false);
  print("hchacha20: %, % bytes\n", ERROR[r.error], r.written);
  show("subkey", sub[..]);

  // The counter is 32 bits. At 0xffffffff one block is left: 64 bytes pass,
  // 65 bytes need a second block and refuse before writing.
  out: [65] u8; zero: [65] u8;
  for i: 0..<65 { out[i] = 165; }
  r = chacha20_xor_checked(key[..], nonce, 0xffffffff, zero[0..<64], out[0..<64], false);
  print("counter 0xffffffff, 64 bytes: %, % bytes\n", ERROR[r.error], r.written);
  for i: 0..<65 { out[i] = 165; }
  r = chacha20_xor_checked(key[..], nonce, 0xffffffff, zero[..], out[..], false);
  untouched := true;
  for b: out { if b != 165 { untouched = false; } }
  print("counter 0xffffffff, 65 bytes: %, % bytes, output untouched %\n", ERROR[r.error], r.written, untouched);
  print("counter 0xfffffffe, 65 bytes: %\n", ERROR[cc_admit(32, 12, 0xfffffffe, 65, 65, false)]);

  // Wrong sizes: a 31-byte key, the 8-byte nonce of the original ChaCha20,
  // and an output buffer one byte short.
  r = chacha20_xor_checked(key[0..<31], nonce, 1, plain, cipher[..], false);
  print("31-byte key: %, % bytes\n", ERROR[r.error], r.written);
  r = chacha20_xor_checked(key[..], nonce[0..<8], 1, plain, cipher[..], false);
  print("8-byte nonce: %, % bytes\n", ERROR[r.error], r.written);
  r = chacha20_xor_checked(key[..], nonce, 1, plain, cipher[0..<113], false);
  print("113-byte output for 114 bytes: %, % bytes\n", ERROR[r.error], r.written);
}
```

Run it from the repository root:

```shell
coax/bin/coax run bitcoin/examples/chacha20.coax
```

The output matches the values worked out by hand: the quarter round
`ea2a92f4 cb1cf8ce 4581472e 5881c4bb`, the starting state, the block's first
word `e4e7f110` and its bytes `10 f1 e7 e4`, and the ciphertext's first
bytes `6e 2e`:

```text
quarter round a b c d: ea2a92f4 cb1cf8ce 4581472e 5881c4bb
state row 0: 61707865 3320646e 79622d32 6b206574
state row 1: 03020100 07060504 0b0a0908 0f0e0d0c
state row 2: 13121110 17161514 1b1a1918 1f1e1d1c
state row 3: 00000001 09000000 4a000000 00000000
block words 0-3: e4e7f110 15593bd1 1fdd0f50 c47120a3
block: ok, 64 bytes
serialized block 0-15: 10f1e7e4d13b5915500fdd1fa32071c4
serialized block 48-63: b5129cd1de164eb9cbd083e8a2503c4e
encrypt: ok, 114 bytes
ciphertext 0-15: 6e2e359a2568f98041ba0728dd0d6981
ciphertext 112-113: 874d
ciphertext equals RFC 8439 2.4.2: true
decrypt: ok, "Ladies and Gentlemen of"
decrypt with byte 0 flipped: ok, "Madies and Gentlemen of"
hchacha20: ok, 32 bytes
subkey: 82413b4227b27bfed30e42508a877d73a0f9e4d58a74a853c12ec41326d3ecdc
counter 0xffffffff, 64 bytes: ok, 64 bytes
counter 0xffffffff, 65 bytes: counter exhausted, 0 bytes, output untouched true
counter 0xfffffffe, 65 bytes: ok
31-byte key: bad key, 0 bytes
8-byte nonce: bad nonce, 0 bytes
113-byte output for 114 bytes: bad output, 0 bytes
```

The ciphertext is the RFC's byte for byte, and decrypting it gives the
plaintext back. With ciphertext byte 0 changed by one bit, decryption still
reports `ok` and gives `Madies` for `Ladies`: `L` is `4c` and `M` is `4d`.
The subkey is the one in section 2.2.1 of the XChaCha draft.

The refusals:

- At counter `ffffffff`, 64 bytes pass and 65 bytes refuse with
  `counter exhausted` (`CC_COUNTER`). Nothing is written: the output still
  holds its fill byte `a5`. From `fffffffe`, 65 bytes pass.
- A 31-byte key refuses with `bad key` (`CC_KEY`).
- An 8-byte nonce, the original ChaCha20's size, refuses with `bad nonce`
  (`CC_NONCE`).
- A 113-byte output buffer for 114 bytes refuses with `bad output`
  (`CC_OUTPUT`).

The book's checks run this program in debug and release builds and compare
the output with [bitcoin/fixtures/chacha20.out](../fixtures/chacha20.out).

## How the code is checked

### Checks in the library

`chacha20.coax` and `hchacha20.coax` carry their own `#test` and `#forall`
checks: the section 2.1.1 quarter round, the section 2.3.2 starting state
and block words, the appendix A.1 zero block, encrypt and decrypt for every
length from 0 to 65, every short capacity and overlap, the counter endpoints
(`ffffffff` with 64 and 65 bytes, a full 1 MiB from `ffffc000` and
`ffffc001`), and the HChaCha20 draft subkey and refusals.

### Checks in the example

The checks in
[bitcoin/examples/checks/chacha20.coax](../examples/checks/chacha20.coax)
load the example and compare it with bytes copied from the pinned RFC 8439
text, [rfc8439.txt](../../coax/tests/vectors/upstream/rfc/rfc8439.txt):

- the section 2.3.2 serialized block and the section 2.4.2 ciphertext, with
  the decryption back to the plaintext;
- the five appendix A.1 key streams, which vary the key, the nonce and the
  counter;
- appendix A.2 test vector 3, 127 bytes from counter 42, whose last block is
  partial;
- the stream from counter `c` against the blocks at `c` and `c + 1`;
- admission for every length 0 to 259 with one, two or three blocks left
  before `2^32`, against `ceil(n / 64) <= left`;
- every key length 0 to 39 and nonce length 0 to 19.

<!-- source: bitcoin/examples/checks/chacha20.coax check -->
```jai
// Byte strings copied from the pinned RFC 8439 text
// (coax/tests/vectors/upstream/rfc/rfc8439.txt) by its hex dumps.
RFC_232_BLOCK :: "10f1e7e4d13b5915500fdd1fa32071c4c7d1f4c733c068030422aa9ac3d46c4ed2826446079faa0914c2d705d98b02a2b5129cd1de164eb9cbd083e8a2503c4e";
#test "RFC 8439 section 2.3.2: the serialized block" {
  key: [32] u8; for i: 0..<32 { key[i] = cast(u8) i; }
  nb: [12] u8; out: [64] u8; want: [64] u8;
  r := chacha20_block_checked(key[..], from_hex("000000090000004a00000000", nb[..]), 1, out[..], false);
  #assert r.error == CC_OK && r.written == 64;
  #assert same(out[..], from_hex(RFC_232_BLOCK, want[..]));
}
#test "RFC 8439 section 2.4.2: the sunscreen ciphertext, both ways" {
  key: [32] u8; for i: 0..<32 { key[i] = cast(u8) i; }
  nb: [12] u8; pb: [114] u8; out: [114] u8; back: [114] u8; want: [114] u8;
  nonce := from_hex("000000000000004a00000000", nb[..]);
  r := chacha20_xor_checked(key[..], nonce, 1, bytes_of(SUNSCREEN, pb[..]), out[..], false);
  #assert r.error == CC_OK && r.written == 114;
  #assert same(out[..], from_hex(SUNSCREEN_CIPHER, want[..]));
  r = chacha20_xor_checked(key[..], nonce, 1, out[..], back[..], false);
  #assert r.error == CC_OK && same(back[..], pb[..]);
}
// The comparison the checks rely on tells different bytes apart.
#test "same refuses another length and a differing byte" {
  a := [3] u8.[1, 2, 3]; b := [3] u8.[0, 2, 3]; c := [3] u8.[1, 2, 4];
  #assert same(a[..], a[..]) && !same(a[..], a[0..<2]);
  #assert !same(a[..], b[..]) && !same(a[..], c[..]);
}
// RFC 8439 appendix A.1: five blocks. Each row is the key byte that is
// set (index, value), the nonce byte that is set, the counter, and the key
// stream.
A1_KEYSTREAM :: [5] string.[
  "76b8e0ada0f13d90405d6ae55386bd28bdd219b8a08ded1aa836efcc8b770dc7da41597c5157488d7724e03fb8d84a376a43b8f41518a11cc387b669b2ee6586",
  "9f07e7be5551387a98ba977c732d080dcb0f29a048e3656912c6533e32ee7aed29b721769ce64e43d57133b074d839d531ed1f28510afb45ace10a1f4b794d6f",
  "3aeb5224ecf849929b9d828db1ced4dd832025e8018b8160b82284f3c949aa5a8eca00bbb4a73bdad192b5c42f73f2fd4e273644c8b36125a64addeb006c13a0",
  "72d54dfbf12ec44b362692df94137f328fea8da73990265ec1bbbea1ae9af0ca13b25aa26cb4a648cb9b9d1be65b2c0924a66c54d545ec1b7374f4872e99f096",
  "c2c64d378cd536374ae204b9ef933fcd1a8b2288b3dfa49672ab765b54ee27c78a970e0e955c14f3a88e741b97c286f75f8fc299e8148362fa198a39531bed6d"];
A1_KEY_AT :: [5] int.[0, 0, 31, 1, 0];
A1_KEY_BYTE :: [5] u8.[0, 0, 1, 255, 0];
A1_NONCE_BYTE :: [5] u8.[0, 0, 0, 0, 2];
A1_COUNTER :: [5] u32.[0, 1, 1, 2, 0];
#forall "RFC 8439 appendix A.1 block key streams" i: 0..<5 {
  key: [32] u8; nonce: [12] u8; out: [64] u8; want: [64] u8;
  key[A1_KEY_AT[i]] = A1_KEY_BYTE[i];
  nonce[11] = A1_NONCE_BYTE[i];
  #assert chacha20_block_checked(key[..], nonce[..], A1_COUNTER[i], out[..], false).error == CC_OK;
  #assert same(out[..], from_hex(A1_KEYSTREAM[i], want[..]));
}
// RFC 8439 appendix A.2 test vector 3: 127 bytes from counter 42, so the
// last block is partial.
A23_KEY :: "1c9240a5eb55d38af333888604f6b5f0473917c1402b80099dca5cbc207075c0";
A23_PLAIN :: "2754776173206272696c6c69672c20616e642074686520736c6974687920746f7665730a446964206779726520616e642067696d626c6520696e2074686520776162653a0a416c6c206d696d737920776572652074686520626f726f676f7665732c0a416e6420746865206d6f6d65207261746873206f757467726162652e";
A23_CIPHER :: "62e6347f95ed87a45ffae7426f27a1df5fb69110044c0d73118effa95b01e5cf166d3df2d721caf9b21e5fb14c616871fd84c54f9d65b283196c7fe4f60553ebf39c6402c42234e32a356b3e764312a61a5532055716ead6962568f87d3f3f7704c6a8d1bcd1bf4d50d6154b6da731b187b58dfd728afa36757a797ac188d1";
#test "RFC 8439 appendix A.2 vector 3: a partial last block" {
  kb: [32] u8; nonce: [12] u8; pb: [127] u8; out: [127] u8; want: [127] u8;
  nonce[11] = 2;
  r := chacha20_xor_checked(from_hex(A23_KEY, kb[..]), nonce[..], 42, from_hex(A23_PLAIN, pb[..]), out[..], false);
  #assert r.error == CC_OK && r.written == 127;
  #assert same(out[..], from_hex(A23_CIPHER, want[..]));
}
// The stream is the blocks at counter, counter + 1, ... laid end to end.
#forall "the stream is consecutive blocks" c: 0..<4 {
  key: [32] u8; for i: 0..<32 { key[i] = cast(u8) (7 * i); }
  nonce: [12] u8; nonce[4] = 74;
  zero: [128] u8; stream: [128] u8; block: [64] u8;
  #assert chacha20_xor_checked(key[..], nonce[..], cast(u32) c, zero[..], stream[..], false).error == CC_OK;
  #assert chacha20_block_checked(key[..], nonce[..], cast(u32) c, block[..], false).error == CC_OK;
  #assert same(stream[0..<64], block[..]);
  #assert chacha20_block_checked(key[..], nonce[..], cast(u32) c + 1, block[..], false).error == CC_OK;
  #assert same(stream[64..<128], block[..]);
}
// Near the top of the counter: n bytes from counter 2^32 - left need
// ceil(n / 64) blocks and pass only when that is at most left.
#forall "a request past counter 2^32 - 1 refuses" left: 1..<4, n: 0..<260 {
  counter := cast(u32) (4294967296 - left);
  want := ifx (n + 63) / 64 <= left then CC_OK else CC_COUNTER;
  #assert cc_admit(32, 12, counter, n, n, false) == want;
}
#forall "only 32-byte keys and 12-byte nonces pass" k: 0..<40, m: 0..<20 {
  want := CC_OK;
  if k != 32 { want = CC_KEY; } else if m != 12 { want = CC_NONCE; }
  #assert cc_admit(k, m, 0, 64, 64, false) == want;
}
```

To run them:

```shell
coax/bin/coax test bitcoin/examples/checks/chacha20.coax
coax/bin/coax mutate bitcoin/examples/checks/chacha20.coax
```

`coax test` runs 31 checks, including the loaded files' own checks, in under a second. `coax mutate` without `--root-only` mutates the example and every loaded file, ChaCha20 and HChaCha20 included: on 2026-11-01 it killed 493 of 493 mutants. The checks file has no procedures of its own, so `--root-only` finds no mutation sites.

### OpenSSL and the native and wasm builds

[chacha20.py](../../coax/tests/nostr/chacha20.py) reads all 10 block and
cipher outputs of RFC 8439 (sections 2.3.2 and 2.4.2, appendices A.1 and
A.2) from the pinned RFC text after checking its SHA-256, and compares the
compiled C and wasm builds with them and with OpenSSL's ChaCha20, through
Node, on 269 records, under ASan and UBSan. Its run on 2026-10-28 passed,
and mutation of `chacha20.coax` killed 351 of 351 mutants; the record is
[coax-chacha20-2026-10-28.md](../../docs/verification/coax-chacha20-2026-10-28.md).
[hchacha20.py](../../coax/tests/nostr/hchacha20.py) compared HChaCha20 with
an independent model on 1,002 cases per profile on 2026-10-29, with all 378
mutants killed:
[2026-10-29-hchacha20.md](../../docs/verification/2026-10-29-hchacha20.md).
These runs were not repeated for this chapter; the library code is
unchanged since them.

## Summary

- The state is four constant words, eight key words, one counter word and
  three nonce words, all little-endian.
- A quarter round is four add, exclusive-or and rotate steps; 20 rounds run
  it on columns and diagonals, and the block adds the starting state back.
- The stream is consecutive blocks exclusive-ored with the message;
  decryption is the same operation.
- A request that would pass counter `2^32 - 1` refuses before writing, as do
  wrong key and nonce sizes.
- ChaCha20 doesn't detect changes; NIP-44 adds an HMAC.

## Dependencies

- [1. Bytes, hex and base64](ch001-bytes-hex-and-base64.md)

## Sources

- [RFC 8439](https://www.rfc-editor.org/rfc/rfc8439), *ChaCha20 and Poly1305
  for IETF Protocols*, sections 2.1 to 2.4 and appendices A.1 and A.2,
  pinned as
  [rfc8439.txt](../../coax/tests/vectors/upstream/rfc/rfc8439.txt).
- [draft-irtf-cfrg-xchacha-03](https://datatracker.ietf.org/doc/html/draft-irtf-cfrg-xchacha-03),
  section 2.2, for HChaCha20.
- The primary sources are pinned in [coax/tests/vectors/SOURCES.md](../../coax/tests/vectors/SOURCES.md).
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
