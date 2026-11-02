# 12. Poly1305 and ChaCha20-Poly1305

ChaCha20 (chapter 11) hides a message but can't tell you if someone changed
it. Poly1305 is the matching one-time authenticator, and RFC 8439 joins the
two into one AEAD, ChaCha20-Poly1305: authenticated encryption with
associated data. Bitcoin and Lightning use the AEAD in three places:

- BIP-324's v2 P2P transport encrypts every packet with it (chapter 67).
- BOLT 8's Noise handshake and every Lightning message after it use it
  (chapter 105).
- BOLT 4's blinded paths encrypt the data for each hop with it (chapter 111).

NIP-49's XChaCha20-Poly1305 is the same AEAD under a derived key. This
chapter builds Poly1305 over state that you own, builds the AEAD with RFC
8439's 12-byte nonce, and rebuilds the XChaCha variant on top of it. It
works out the clamping of `r` and one block of the accumulator by hand, and
checks the code against every Poly1305 and AEAD example in RFC 8439, all 325
Wycheproof cases and OpenSSL.

## What this chapter teaches

- What Poly1305 computes: a polynomial in the message blocks, evaluated at
  a secret `r` modulo `2^130 - 5`, plus a secret `s`.
- Why `r` is clamped, and how to clamp it from the key bytes by hand.
- How five 26-bit limbs hold a 130-bit number, and why `2^130 = 5 (mod p)`
  makes the reduction cheap.
- How the AEAD derives a one-time Poly1305 key from ChaCha20 block 0, and
  what the tag covers.
- Why open must check the tag before it decrypts, and why a refused open
  must release no plaintext.
- How a MAC keeps its state in memory you own, refuses misuse without
  changing anything, and erases its secrets.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [poly1305.coax](../../coax/lib/crypto/poly1305.coax) | `poly1305_init`, `poly1305_update`, `poly1305_final` | Poly1305 over a `Poly1305State` that you own |
| | `poly1305_checked(key, msg, out, overlap)` | the one-shot MAC, for NIP-49 and the AEAD |
| [chacha20_poly1305.coax](../../coax/lib/crypto/chacha20_poly1305.coax) | `chacha20_poly1305_seal`, `chacha20_poly1305_open` | the RFC 8439 AEAD with a 12-byte nonce |
| | `ccp_admit(...)` | every size and overlap check, before any write |
| [xchacha20_poly1305.coax](../../coax/lib/crypto/xchacha20_poly1305.coax) | `xcp_seal`, `xcp_open` | XChaCha20-Poly1305 for NIP-49, now the RFC 8439 AEAD under an HChaCha20 subkey |

All three files are `#authority`: they decide what a caller may
authenticate, encrypt and release, so their checks must kill every mutant.

Before this chapter, `poly1305.coax` was a one-shot MAC written for NIP-49
(Nostr chapter 47), with its working values in stack locals that nothing
erased. There was no AEAD with RFC 8439's 12-byte nonce, only
XChaCha20-Poly1305, which copied the associated data and the ciphertext into
a 5,168-byte stack buffer to MAC them. The chapter:

- adds the Poly1305 stream over caller-owned state, with status codes, a
  check of the state before every use, and erasure at final;
- turns `poly1305_checked` into the same three steps over a local state, so
  the one-shot MAC and the stream share one implementation;
- adds `chacha20_poly1305.coax`, which MACs the pieces where they are,
  without a copy;
- rewrites `xcp_seal` and `xcp_open` as an HChaCha20 subkey plus the new
  AEAD, so the two AEADs share one implementation.

`xcp_seal` and `xcp_open` keep their signatures and their bounds (1,024
bytes of associated data, 4,096 bytes of plaintext). The NIP-49, peer-link
and AEAD-workspace checks give the same results as before.

## Poly1305

Poly1305 takes a 32-byte one-time key and a message, and returns a 16-byte
tag:

1. Split the key. The first 16 bytes, read as a little-endian number and
   clamped, are `r`. The last 16 bytes, little-endian, are `s`.
2. Cut the message into 16-byte blocks; the last one may be shorter. Append
   a `01` byte to each block and read the result as a little-endian number.
   A full block becomes a number of 129 bits.
3. Start an accumulator at 0. For each block: add the block to the
   accumulator, then multiply by `r`, modulo the prime `p = 2^130 - 5`.
4. The tag is `(accumulator + s) mod 2^128`, as 16 little-endian bytes.

The accumulator ends as `c1 r^n + c2 r^(n-1) + ... + cn r` modulo `p`,
where `c1` to `cn` are the blocks. Without `r`, an attacker can't predict
how a changed block moves that value, and `s` hides the value itself. The
appended `01` byte makes a short last block differ from the same bytes
followed by zeros.

The key is one-time. Two tags under one key give two equations in `r` and
`s`, and from them an attacker can forge tags. The AEAD below therefore
never takes a Poly1305 key from its caller: it derives a fresh one for
every key and nonce.

### Clamping r

Clamping clears 22 bits of `r`: the top four bits of bytes 3, 7, 11 and 15,
and the bottom two bits of bytes 4, 8 and 12. Each 32-bit word of `r` is
then below `2^28`, and words 1 to 3 are multiples of 4. That bounds the
products in the multiplication, so implementations can delay their carries;
Bernstein chose the shape for fast code on 2005 hardware. It costs 22 bits
of `r`'s 128, which the security proof accounts for.

The code clamps the two 64-bit halves of `r` with two masks:

<!-- source: coax/lib/crypto/poly1305.coax state -->
```jai
// The caller owns every byte of a MAC in progress: r and the accumulator h
// in 26-bit limbs, s, the partial block, and the product the multiplication
// builds. Final erases all of it.
Poly1305State :: struct {
  r:[5]u64; h:[5]u64; product:[5]u64;
  s:[16]u8; block:[16]u8;
  used:int;     // bytes in block, 0 to 15
  phase:u32;    // POLY_ACTIVE, or 0 before init and after final
}
// The bits of r that clamping keeps, as the low and high 64-bit halves.
// RFC 8439 clears the top four bits of bytes 3, 7, 11 and 15 and the bottom
// two bits of bytes 4, 8 and 12.
POLY_CLAMP_LO :: cast(u64)0x0ffffffc0fffffff;
POLY_CLAMP_HI :: cast(u64)0x0ffffffc0ffffffc;
```

### Worked by hand: clamping r for section 2.5.2

RFC 8439 section 2.5.2 uses the key
`85d6be78 57556d33 7f4452fe 42d506a8 0103808a fb0db2fd 4abff6af 4149f51b`.
Its first 16 bytes are:

```text
byte:   0  1  2  3  4  5  6  7  8  9 10 11 12 13 14 15
key:   85 d6 be 78 57 55 6d 33 7f 44 52 fe 42 d5 06 a8
```

Clear the bits byte by byte:

1. Byte 3: `78 & 0f = 08`.
2. Byte 4: `57 & fc = 54`.
3. Byte 7: `33 & 0f = 03`.
4. Byte 8: `7f & fc = 7c`.
5. Byte 11: `fe & 0f = 0e`.
6. Byte 12: `42 & fc = 40`.
7. Byte 15: `a8 & 0f = 08`.

The clamped bytes are `85 d6 be 08 54 55 6d 03 7c 44 52 0e 40 d5 06 08`.
Read little-endian, the last byte first:

```text
r = 0x0806d5400e52447c036d555408bed685
```

That is the RFC's "clamped r as a number", `806d5400e52447c036d555408bed685`.
The code reaches it in two steps: the low half `0x336d555778bed685` masked
with `0x0ffffffc0fffffff` is `0x036d555408bed685`, and the high half
`0xa806d542fe52447f` masked with `0x0ffffffc0ffffffc` is
`0x0806d5400e52447c`. The last 16 key bytes give
`s = 0x1bf54941aff6bf4afdb20dfb8a800301`.

## Numbers of 130 bits in five limbs

The accumulator needs 130 bits and Coax's widest integer has 64. The code
keeps every 130-bit number in five limbs of 26 bits in `u64`, the layout of
chapter 2: limb `i` holds bits `26i` to `26i + 25`. `poly_split` cuts a
128-bit number, given as its two 64-bit halves, into limbs. Bits 52 to 77
straddle the halves, so limb 2 takes the top 12 bits of the low half and the
bottom 14 of the high half:

<!-- source: coax/lib/crypto/poly1305.coax start -->
```jai
// Add a 128-bit number, given as its low and high 64 bits, to five 26-bit
// limbs: bits 0-25, 26-51, 52-77, 78-103 and 104-127.
poly_split :: (lo:u64,hi:u64,limbs:[]u64) {
  limbs[0]+=lo&POLY_BASE_MASK;
  limbs[1]+=(lo>>26)&POLY_BASE_MASK;
  limbs[2]+=((lo>>52)|(hi<<12))&POLY_BASE_MASK;
  limbs[3]+=(hi>>14)&POLY_BASE_MASK;
  limbs[4]+=hi>>40;
}
// r is the first 16 key bytes, little-endian and clamped; s is the last 16.
poly_start :: (state:[]Poly1305State,key:[]u8) {
  state[0]=Poly1305State.{phase=POLY_ACTIVE};
  lo:=cb_load_le64(key,0).value&POLY_CLAMP_LO;
  hi:=cb_load_le64(key,8).value&POLY_CLAMP_HI;
  poly_split(lo,hi,state[0].r[..]);
  for i:0..<16 {state[0].s[i]=key[16+i];}
}
```

For section 2.5.2, the limbs of `r` are `0bed685 3555502 047c036 1003949
00806d5`. Limb 0 is the low 26 bits of `0x08bed685`, which drops the top
two bits of the `0x08`.

### Multiplying modulo 2^130 - 5

Limb `i` of `h` times limb `j` of `r` has weight `2^(26(i + j))`. When
`i + j` is 5 or more, the weight is at least `2^130`, and since
`2^130 = p + 5`, `2^130 = 5 (mod p)`. So the product folds back into limb
`i + j - 5`, times 5. Each of the five product limbs is a sum of five such
terms. A limb of `h` is below `2^27` after a block is added, and a limb of
`r` is below `2^26`, so each sum is below `25 * 2^27 * 2^26 < 2^58`: it fits
in `u64` with room to spare.

<!-- source: coax/lib/crypto/poly1305.coax block -->
```jai
// One block: h = (h + block + 2^(8 * length)) * r mod 2^130 - 5. The block's
// first `length` bytes are the message; the rest are zero. Adding 2^(8 *
// length) appends the 0x01 byte. Limb i times limb j lands in limb i + j;
// past limb 4 it wraps to limb i + j - 5 times 5, because 2^130 = 5 mod p.
poly_block :: (state:[]Poly1305State,length:int) {
  poly_split(cb_load_le64(state[0].block[..],0).value,cb_load_le64(state[0].block[..],8).value,state[0].h[..]);
  top:=length*8;state[0].h[top/26]+=cast(u64)1<<(top%26);
  for i:0..<5 {for j:0..<5 {
    index:=i+j;factor:=cast(u64)1;if index>=5 {index-=5;factor=5;}
    state[0].product[index]+=state[0].h[i]*state[0].r[j]*factor;
  }}
  poly_normalize(state[0].product[..]);
  for i:0..<5 {state[0].h[i]=state[0].product[i];state[0].product[i]=0;}
  for i:0..<16 {state[0].block[i]=0;}
}
```

The appended `01` byte is the bit `8 * length`: bit 128 for a full block,
which lands in limb 4 at bit 24.

The products are then carried back into 26-bit limbs. A carry out of
limb 4 is worth `2^130`, so it comes back into limb 0 times 5. Three passes
bring every limb below `2^26`; `coax/tests/nostr/poly1305_bounds.py`
proves that with Z3 for every input below `2^58`, and that two passes are
not enough:

<!-- source: coax/lib/crypto/poly1305.coax carry -->
```jai
// One carry pass: each limb keeps its low 26 bits and passes the rest up.
// What passes out of limb 4 is a multiple of 2^130, and 2^130 = 5 mod p, so
// it comes back into limb 0 times 5. Three passes bring every limb below
// 2^26 (coax/tests/nostr/poly1305_bounds.py checks this with Z3).
poly_carry :: (x:[]u64) {
  for i:0..<5 {
    carry:=x[i]>>26;x[i]&=POLY_BASE_MASK;
    if i==4 {x[0]+=carry*5;} else {x[i+1]+=carry;}
  }
}
poly_normalize :: (x:[]u64) {poly_carry(x);poly_carry(x);poly_carry(x);}
```

The `if` branches on the loop counter `i`, which is public, not on a limb.

### Worked by hand: block 1 of section 2.5.2

The message is `Cryptographic Forum Research Group`, 34 bytes: two full
blocks and 2 bytes.

1. Block 1 is `Cryptographic Fo`, bytes `43 72 79 70 74 6f 67 72 61 70 68
   69 63 20 46 6f`. Append `01` and read little-endian:
   `0x016f4620636968706172676f7470797243`, 129 bits.
2. The accumulator starts at 0, so the accumulator plus the block is the
   block.
3. Times `r`:

   ```text
   0x016f4620636968706172676f7470797243 * 0x0806d5400e52447c036d555408bed685
     = 0xb83fe991ca66800489155dcd69e8426ba2779453994ac90ed284034da565ecf
   ```

   That is 252 bits, the value the RFC prints for `(Acc+Block) * r`.
4. Reduce modulo `p` by splitting at bit 130. The bits above are
   `high = 0x2e0ffa647299a001224557735a7a109`, and the low 130 bits are
   `low = 0x2ba2779453994ac90ed284034da565ecf`. Because `2^130 = 5 (mod p)`,
   the product is `low + 5 * high`:

   ```text
     0x2ba2779453994ac90ed284034da565ecf
   + 0x0e64fe3f63d002005ab5ab540c46252d    (5 * high)
   = 0x2c88c77849d64ae9147ddeb88e69c83fc
   ```

5. The sum is below `p`, so it is the new accumulator,
   `2c88c77849d64ae9147ddeb88e69c83fc`, the RFC's `Acc` after block 1.

The code gets the same value with limbs: `poly_block` folds the high
products into the low limbs times 5 as it multiplies, and the carries do the
rest. The example prints the accumulator after one 16-byte update.

Blocks 2 and 3 (`rum Research Gro` and `up`, which gets its `01` after
2 bytes) bring the accumulator to `0x28d31b7caff946c77c8844335369d03a7`.
Adding `s` gives `0x2a927010caf8b2bc2c6365130c11d06a8`; its low 128 bits,
as little-endian bytes, are the tag `a8061dc1305136c6c22b8baf0c0127a9`.

### The last reduction and the tag

After the last block, the accumulator is below `2^130` in canonical limbs,
but may still be `p` or more. `poly_reduce` subtracts `p` at most once,
without a branch: `h + 5` reaches `2^130` exactly when `h >= p`, and the bit
above 130 bits becomes an all-ones or all-zeros mask that picks `h + 5 -
2^130` or `h`:

<!-- source: coax/lib/crypto/poly1305.coax reduce -->
```jai
// h is below 2^130 in canonical limbs, so h mod p is h or h - p. h + 5
// reaches 2^130 exactly when h >= p, and then its low 130 bits are h - p.
// The bit above them becomes a mask that selects without a branch.
poly_reduce :: (h:[]u64,scratch:[]u64) {
  for i:0..<5 {scratch[i]=h[i];}scratch[0]+=5;
  for i:0..<4 {carry:=scratch[i]>>26;scratch[i]&=POLY_BASE_MASK;scratch[i+1]+=carry;}
  overflow:=scratch[4]>>26;scratch[4]&=POLY_BASE_MASK;
  mask:=sub_wrap(cast(u64)0,overflow);
  for i:0..<5 {h[i]=(scratch[i]&mask)|(h[i]^(h[i]&mask));scratch[i]=0;}
}
```

Appendix A.3 test vector 8 ends with an accumulator of exactly `2^130 - 5`,
which must become 0.

## The stream over your state

`Poly1305State` holds every value of a MAC in progress: `r` and the
accumulator `h` in limbs, `s`, a partial block and the scratch product. You
own it, so you can erase it. `poly_absorb` takes full blocks as soon as they
fill and keeps a partial block in the state. A full block always gets its
`01` at bit 128, so it can be processed before the caller says whether more
bytes follow:

<!-- source: coax/lib/crypto/poly1305.coax absorb -->
```jai
// Full blocks are taken as soon as they fill; a partial one waits in the
// state for more bytes or for final.
poly_absorb :: (state:[]Poly1305State,msg:[]u8) {
  used:=state[0].used;
  for byte:msg {
    state[0].block[used]=byte;used+=1;
    if used==16 {poly_block(state,16);used=0;}
  }
  state[0].used=used;
}
```

`poly_finish` processes the partial block, reduces, adds `s` in 32-bit
pieces so that each carry is just the bits above 32, writes the tag and
then erases the whole state:

<!-- source: coax/lib/crypto/poly1305.coax finish -->
```jai
// The tag is (h mod p + s) mod 2^128, little-endian. Every field of the
// state is erased afterwards.
poly_finish :: (state:[]Poly1305State,out:[]u8) {
  if state[0].used>0 {poly_block(state,state[0].used);}
  poly_reduce(state[0].h[..],state[0].product[..]);
  // The low 128 bits of h as two 64-bit halves; bits 128 and 129 drop out.
  h:=state[0].h[..];
  lo:=h[0]|(h[1]<<26)|(h[2]<<52);
  hi:=(h[2]>>12)|(h[3]<<14)|(h[4]<<40);
  // Add s in 32-bit pieces, so each carry is the bits above 32.
  slo:=cb_load_le64(state[0].s[..],0).value;shi:=cb_load_le64(state[0].s[..],8).value;
  w0:=(lo&POLY_LOW32)+(slo&POLY_LOW32);
  w1:=(lo>>32)+(slo>>32)+(w0>>32);
  w2:=(hi&POLY_LOW32)+(shi&POLY_LOW32)+(w1>>32);
  w3:=(hi>>32)+(shi>>32)+(w2>>32);
  a:=cb_store_le32(out,0,cast(u32)(w0&POLY_LOW32));b:=cb_store_le32(out,4,cast(u32)(w1&POLY_LOW32));
  c:=cb_store_le32(out,8,cast(u32)(w2&POLY_LOW32));d:=cb_store_le32(out,12,cast(u32)(w3&POLY_LOW32));
  state[0]=Poly1305State.{};
}
```

The public calls return a status, and do all their checks before they
write anything:

| Status | Name | Meaning |
| --- | --- | --- |
| 0 | `POLY_OK` | Success |
| 1 | `POLY_BAD_STATE` | The state isn't one state, or isn't a valid state |
| 2 | `POLY_BAD_KEY` | The key isn't 32 bytes |
| 3 | `POLY_BAD_INPUT` | The input is negative or larger than 1 MiB |
| 4 | `POLY_BAD_OUTPUT` | The output has fewer than 16 bytes |
| 5 | `POLY_PHASE` | An update or final before init or after final |
| 6 | `POLY_ALIAS` | The key, input or output overlaps the state |

<!-- source: coax/lib/crypto/poly1305.coax check -->
```jai
// A state is usable only if it is active, its partial block is in range and
// zero past `used`, every limb is below 2^26 and the product is zero (each
// block clears it). Anything else was damaged or forged: refuse it rather
// than compute with it. The secret fields are folded into one value that is
// tested once, so a valid state takes the same path whatever its key and
// message.
poly_check :: (state:[]Poly1305State)->u32 {
  if state.count!=1 {return POLY_BAD_STATE;}
  if state[0].phase==0 {return POLY_PHASE;}
  used:=state[0].used;
  if state[0].phase!=POLY_ACTIVE || used<0 || used>15 {return POLY_BAD_STATE;}
  excess:=cast(u64)0;
  for i:used..<16 {excess|=cast(u64)state[0].block[i];}
  for i:0..<5 {excess|=((state[0].h[i]|state[0].r[i])>>26)|state[0].product[i];}
  if excess!=0 {return POLY_BAD_STATE;}
  return POLY_OK;
}
// The size and overlap checks of one update's input and of final's output.
poly_input :: (count:int,overlap:bool)->u32 {
  if count<0 || count>CB_MAX_BYTES {return POLY_BAD_INPUT;}
  if count>0 && overlap {return POLY_ALIAS;}
  return POLY_OK;
}
poly_output :: (count:int,overlap:bool)->u32 {
  if count<16 || count>CB_MAX_BYTES {return POLY_BAD_OUTPUT;}
  if overlap {return POLY_ALIAS;}
  return POLY_OK;
}
poly1305_init :: (state:[]Poly1305State,key:[]u8,overlap:bool)->u32 #export_c {
  if state.count!=1 {return POLY_BAD_STATE;}
  if key.count!=32 {return POLY_BAD_KEY;}
  if overlap {return POLY_ALIAS;}
  poly_start(state,key);return POLY_OK;
}
poly1305_update :: (state:[]Poly1305State,msg:[]u8,overlap:bool)->u32 #export_c {
  status:=poly_check(state);if status!=POLY_OK {return status;}
  status=poly_input(msg.count,overlap);if status!=POLY_OK {return status;}
  poly_absorb(state,msg);return POLY_OK;
}
poly1305_final :: (state:[]Poly1305State,out:[]u8,overlap:bool)->u32 #export_c {
  status:=poly_check(state);if status!=POLY_OK {return status;}
  status=poly_output(out.count,overlap);if status!=POLY_OK {return status;}
  poly_finish(state,out);return POLY_OK;
}
```

`poly_check` refuses a state that no sequence of calls could produce: a
partial block with bytes past `used`, a limb at `2^26` or more, or a product
left over. Such a state was damaged or forged, and computing with it could
overflow a limb. The checks on secret fields are folded into one value and
tested once, so a valid state takes the same path for every key and
message (chapter 3).

Final erases the state, so a finished state reads as "before init": an
update or a second final returns `POLY_PHASE`. Erasure in Coax reaches the
state's fields; it doesn't reach scalar copies the compiler makes in
registers. To be sure, wipe the whole state afterwards with
`coax_secure_zero` (chapter 3).

The one-shot `poly1305_checked`, which NIP-49 calls, runs the same three
steps over a local state:

<!-- source: coax/lib/crypto/poly1305.coax oneshot -->
```jai
// One-shot MAC for NIP-49 and the AEADs: the same steps over a local state,
// which poly_finish erases.
poly1305_checked :: (key:[]u8,msg:[]u8,out:[]u8,overlap:bool)->bool #export_c {
  if !poly_admit(key.count,msg.count,out.count,overlap) {return false;}
  state:[1]Poly1305State;
  poly_start(state[..],key);poly_absorb(state[..],msg);poly_finish(state[..],out);
  return true;
}
```

## ChaCha20-Poly1305

RFC 8439 section 2.8 builds the AEAD from the two primitives. To seal a
plaintext under a 32-byte key and a 12-byte nonce, with associated data
(AAD) that is authenticated but not encrypted:

1. Run ChaCha20 block 0 under the key and nonce. Its first 32 bytes are the
   one-time Poly1305 key; the other 32 are discarded.
2. Encrypt the plaintext with ChaCha20 from counter 1.
3. MAC this with Poly1305:
   - the AAD, then zero bytes up to a multiple of 16;
   - the ciphertext, then zero bytes up to a multiple of 16;
   - the AAD's length and the ciphertext's length, each as an 8-byte
     little-endian number.
4. Output the ciphertext followed by the 16-byte tag.

Every key and nonce gives a different ChaCha20 block 0, so every message
gets a fresh Poly1305 key, which is what Poly1305 requires. The lengths at
the end stop bytes from moving between the AAD and the ciphertext: without
them, `AAD = 01, ciphertext = 02 03` and `AAD = 01 02, ciphertext = 03` would
MAC the same padded bytes only if the padding happened to line up, and the
lengths rule it out for every pair.

<!-- source: coax/lib/crypto/chacha20_poly1305.coax tag -->
```jai
// The one-time Poly1305 key is the first 32 bytes of the ChaCha20 block at
// counter 0. The rest of that block is never used.
ccp_poly_key :: (key:[]u8,nonce:[]u8,out:[]u8) {
  words:=cc_block_words(cc_initial(key,nonce,0));
  for i:0..<8 {stored:=cb_store_le32(out,i*4,words.words[i]);}
  words=CcWords.{}; // mutant: equivalent (erases a dead local; no result reads it)
}
// Zero bytes that bring n up to a multiple of 16.
ccp_pad :: (n:int)->int {return (16-n%16)%16;}
// The tag is Poly1305 over the AAD, zeros to a multiple of 16, the
// ciphertext, zeros to a multiple of 16, then the two lengths as 64-bit
// little-endian numbers. The stream absorbs the pieces where they are.
ccp_tag :: (key:[]u8,nonce:[]u8,aad:[]u8,cipher:[]u8,tag:[]u8) {
  polykey:[32]u8;state:[1]Poly1305State;zeros:[16]u8;lengths:[16]u8;
  ccp_poly_key(key,nonce,polykey[..]);
  poly_start(state[..],polykey[..]);
  for i:0..<32 {polykey[i]=0;} // mutant: equivalent (erases a dead local; no result reads it)
  poly_absorb(state[..],aad);poly_absorb(state[..],zeros[0..<ccp_pad(aad.count)]);
  poly_absorb(state[..],cipher);poly_absorb(state[..],zeros[0..<ccp_pad(cipher.count)]);
  a:=cb_store_le64(lengths[..],0,cast(u64)aad.count);
  b:=cb_store_le64(lengths[..],8,cast(u64)cipher.count);
  poly_absorb(state[..],lengths[..]);
  poly_finish(state[..],tag);
}
```

`ccp_tag` streams the pieces into a local `Poly1305State` where they lie,
so neither the AAD nor the ciphertext is copied.

For the section 2.8.2 example, the AAD is 12 bytes and the ciphertext 114,
so the MAC input is 160 bytes: the AAD and 4 zero bytes, the ciphertext and
14 zero bytes, then `0c 00 00 00 00 00 00 00 72 00 00 00 00 00 00 00`
(12 and 114).

### Seal and open

<!-- source: coax/lib/crypto/chacha20_poly1305.coax seal -->
```jai
// Encrypt from counter 1 into the output, then tag the ciphertext there.
// Admission has already made every ChaCha20 check pass: the key and nonce
// sizes, and at most 2^14 blocks from counter 1.
chacha20_poly1305_seal :: (key:[]u8,nonce:[]u8,aad:[]u8,plain:[]u8,out:[]u8,overlap:bool)->u32 #export_c {
  status:=ccp_admit(key.count,nonce.count,aad.count,plain.count,out.count,overlap,true);
  if status!=CCP_OK {return status;}
  n:=plain.count;
  stream:=chacha20_xor_checked(key,nonce,1,plain,out[0..<n],false);
  ccp_tag(key,nonce,aad,out[0..<n],out[n..<n+16]);
  return CCP_OK;
}
```

Open recomputes the tag over the ciphertext it received and compares the
two tags with `cb_ct_equal`, which reads all 16 bytes whatever they hold
(chapter 3). Only then does it decrypt:

<!-- source: coax/lib/crypto/chacha20_poly1305.coax open -->
```jai
// Recompute the tag over the received ciphertext and compare all 16 bytes
// without a branch. Only a match decrypts; a mismatch writes nothing.
chacha20_poly1305_open :: (key:[]u8,nonce:[]u8,aad:[]u8,box:[]u8,out:[]u8,overlap:bool)->u32 #export_c {
  status:=ccp_admit(key.count,nonce.count,aad.count,box.count,out.count,overlap,false);
  if status!=CCP_OK {return status;}
  n:=box.count-16;tag:[16]u8;
  ccp_tag(key,nonce,aad,box[0..<n],tag[..]);
  agree:=cb_ct_equal(tag[..],box[n..<box.count]);
  for i:0..<16 {tag[i]=0;} // mutant: equivalent (erases a dead local; no result reads it)
  if agree!=CB_MAX64 {return CCP_TAG;}
  stream:=chacha20_xor_checked(key,nonce,1,box[0..<n],out[0..<n],false);
  return CCP_OK;
}
```

The order matters. An open that decrypts first and checks afterwards hands
out plaintext of a forged message if its caller ignores the status, or if
the plaintext was already written to memory the caller reads. Here a
mismatch returns `CCP_TAG` before the output is touched. The branch on the
comparison reveals only the verdict, which the status reveals anyway.

The statuses:

| Status | Name | Meaning |
| --- | --- | --- |
| 0 | `CCP_OK` | Success |
| 1 | `CCP_KEY` | The key isn't 32 bytes |
| 2 | `CCP_NONCE` | The nonce isn't 12 bytes |
| 3 | `CCP_AAD` | The AAD is negative or larger than 1 MiB |
| 4 | `CCP_INPUT` | Seal: the plaintext is over 1 MiB − 16. Open: the box is under 16 bytes or over 1 MiB |
| 5 | `CCP_OUTPUT` | The output is too short, or larger than 1 MiB |
| 6 | `CCP_ALIAS` | The output overlaps an input |
| 7 | `CCP_TAG` | Open: the tag doesn't match |

<!-- source: coax/lib/crypto/chacha20_poly1305.coax admit -->
```jai
// Every size and overlap check, before anything is written. To seal, text
// is the plaintext and the output needs 16 more bytes for the tag. To open,
// text is the ciphertext and tag, so it has at least 16 bytes.
ccp_admit :: (key:int,nonce:int,aad:int,text:int,capacity:int,overlap:bool,seal:bool)->u32 #export_c {
  if key!=32 {return CCP_KEY;}
  if nonce!=12 {return CCP_NONCE;}
  if aad<0 || aad>CB_MAX_BYTES {return CCP_AAD;}
  if seal {
    if text<0 || text>CB_MAX_BYTES-16 {return CCP_INPUT;}
    if capacity<text+16 || capacity>CB_MAX_BYTES {return CCP_OUTPUT;}
  } else {
    if text<16 || text>CB_MAX_BYTES {return CCP_INPUT;}
    if capacity<text-16 || capacity>CB_MAX_BYTES {return CCP_OUTPUT;}
  }
  if overlap {return CCP_ALIAS;}
  return CCP_OK;
}
```

Admission settles every ChaCha20 check too: a 1 MiB plaintext from counter 1
needs 16,384 blocks, far from the counter limit of chapter 11. RFC 8439
allows up to `2^38 - 64` bytes; BIP-324 packets and Lightning messages are
far smaller than this library's 1 MiB.

### Nonces

The AEAD inherits ChaCha20's rule: never seal two messages under one key and
nonce. A repeat leaks the exclusive-or of the plaintexts and, through the
one-time Poly1305 key, lets an attacker forge tags. The 12-byte nonce is too
short to pick at random for many messages, so the protocols count:

- BIP-324 puts a packet counter in the nonce and rekeys every 224 packets
  (chapter 67).
- BOLT 8's nonce is 4 zero bytes and a 64-bit little-endian message
  counter, and each key rotates after 1,000 messages (chapter 105).
- NIP-49 picks a random 24-byte nonce, long enough for random choice, and
  uses XChaCha20-Poly1305.

### XChaCha20-Poly1305

XChaCha20-Poly1305 is this AEAD under an HChaCha20 subkey (chapter 11) of
the key and the first 16 nonce bytes, with the 12-byte nonce `00 00 00 00`
followed by the last 8 nonce bytes. `xcp_seal` and `xcp_open` now do exactly
that, then call `chacha20_poly1305_seal` and `chacha20_poly1305_open`, and
erase the subkey. The example seals the sunscreen sentence with the
XChaCha draft's section A.3.1 key and nonce, and its tag is the draft's.

## The example

The example is
[bitcoin/examples/chacha20-poly1305.coax](../examples/chacha20-poly1305.coax).
It clamps the section 2.5.2 key and prints `r` and its limbs, runs block 1
and prints the accumulator, finishes the tag and shows the erased state,
derives the section 2.6.2 Poly1305 key, seals and opens the section 2.8.2
message, flips one bit of the tag, the ciphertext and the AAD, shows four
refusals, and seals the XChaCha draft's message.

<!-- source: bitcoin/examples/chacha20-poly1305.coax example -->
```jai
#load "../../coax/lib/crypto/chacha20_poly1305.coax";
#load "../../coax/lib/crypto/xchacha20_poly1305.coax";
#load "../../coax/lib/nostr/hex.coax";
#import "Basic";

// Print a label and bytes as lowercase hex.
show :: (label: string, bytes: [] u8) {
  text: [300] u8;
  r := hex_encode_checked(cast(string) bytes, text[..], false);
  print("%: %\n", label, cast(string) text[0..<r.written]);
}
// Print five 26-bit limbs as one number: 17 big-endian bytes, 130 bits.
show_limbs :: (label: string, h: [] u64) {
  lo := h[0] | (h[1] << 26) | (h[2] << 52);
  hi := (h[2] >> 12) | (h[3] << 14) | (h[4] << 40);
  b: [17] u8;
  b[0] = cast(u8) (h[4] >> 24);
  for i: 0..<8 { b[1 + i] = cast,trunc(u8) (hi >> ((7 - i) * 8)); b[9 + i] = cast,trunc(u8) (lo >> ((7 - i) * 8)); }
  show(label, b[..]);
}
// Print five 26-bit limbs one by one, as 7 hex digits each, limb 0 first.
show_each :: (label: string, h: [] u64) {
  b: [20] u8;
  ok := true;
  for i: 0..<5 { ok = ok && cb_store_be32(b[..], i * 4, cast(u32) h[i]); }
  text: [40] u8;
  r := hex_encode_checked(cast(string) b[..], text[..], false);
  t := cast(string) text[0..<r.written];
  if ok { print("%: % % % % %\n", label, t[1..<8], t[9..<16], t[17..<24], t[25..<32], t[33..<40]); }
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
// True when a and b hold the same bytes.
same :: (a: [] u8, b: [] u8) -> bool {
  if a.count != b.count { return false; }
  for i: 0..<a.count { if a[i] != b[i] { return false; } }
  return true;
}
// True when every byte still holds the fill value 0xa5.
untouched :: (bytes: [] u8) -> bool {
  for b: bytes { if b != 165 { return false; } }
  return true;
}

STATUS :: [8] string.["ok", "bad key", "bad nonce", "bad aad", "bad input",
  "bad output", "overlap", "tag mismatch"];
POLY_STATUS :: [7] string.["ok", "bad state", "bad key", "bad input",
  "bad output", "already final", "overlap"];

SUNSCREEN :: "Ladies and Gentlemen of the class of '99: If I could offer you only one tip for the future, sunscreen would be it.";
SUNSCREEN_SEALED :: "d31a8d34648e60db7b86afbc53ef7ec2a4aded51296e08fea9e2b5a736ee62d63dbea45e8ca9671282fafb69da92728b1a71de0a9e060b2905d6a5b67ecd3b3692ddbd7f2d778b8c9803aee328091b58fab324e4fad675945585808b4831d7bc3ff4def08e4b7a9de576d26586cec64b61161ae10b594f09e26a7e902ecbd0600691";

main :: () {
  // RFC 8439 section 2.5.2: clamping r and one block of the accumulator.
  kb: [32] u8;
  key := from_hex("85d6be7857556d337f4452fe42d506a80103808afb0db2fd4abff6af4149f51b", kb[..]);
  mb: [34] u8;
  msg := bytes_of("Cryptographic Forum Research Group", mb[..]);
  state: [1] Poly1305State;
  print("init: %\n", POLY_STATUS[poly1305_init(state[..], key, false)]);
  show_limbs("r", state[0].r[..]);
  show_each("r limbs", state[0].r[..]);
  print("update 16 bytes: %\n", POLY_STATUS[poly1305_update(state[..], msg[0..<16], false)]);
  show_limbs("accumulator after block 1", state[0].h[..]);
  print("update 18 bytes: %, % bytes wait for final\n", POLY_STATUS[poly1305_update(state[..], msg[16..<34], false)], state[0].used);
  tag: [16] u8;
  print("final: %\n", POLY_STATUS[poly1305_final(state[..], tag[..], false)]);
  show("tag", tag[..]);
  erased := state[0].phase == 0 && state[0].used == 0;
  for i: 0..<5 { if state[0].r[i] != 0 || state[0].h[i] != 0 { erased = false; } }
  for i: 0..<16 { if state[0].s[i] != 0 { erased = false; } }
  print("state erased after final: %\n", erased);
  print("update after final: %\n", POLY_STATUS[poly1305_update(state[..], msg, false)]);
  one: [16] u8;
  print("one-shot: %\n", poly1305_checked(key, msg, one[..], false));
  show("one-shot tag", one[..]);

  // RFC 8439 section 2.6.2: the one-time key is ChaCha20 block 0.
  key2: [32] u8;
  for i: 0..<32 { key2[i] = cast(u8) (128 + i); }
  nb: [12] u8;
  polykey: [32] u8;
  ccp_poly_key(key2[..], from_hex("000000000001020304050607", nb[..]), polykey[..]);
  show("Poly1305 key from block 0", polykey[..]);

  // RFC 8439 section 2.8.2: seal the sunscreen sentence with 12 bytes of AAD.
  nonce := from_hex("070000004041424344454647", nb[..]);
  ab: [12] u8;
  aad := from_hex("50515253c0c1c2c3c4c5c6c7", ab[..]);
  pb: [114] u8; box: [130] u8; wb: [130] u8;
  plain := bytes_of(SUNSCREEN, pb[..]);
  print("seal: %\n", STATUS[chacha20_poly1305_seal(key2[..], nonce, aad, plain, box[..], false)]);
  show("ciphertext 0-15", box[0..<16]);
  show("tag", box[114..<130]);
  print("sealed box equals RFC 8439 2.8.2: %\n", same(box[..], from_hex(SUNSCREEN_SEALED, wb[..])));
  back: [114] u8;
  print("open: %, \"%\"\n", STATUS[chacha20_poly1305_open(key2[..], nonce, aad, box[..], back[..], false)], cast(string) back[0..<23]);

  // The broken case: a tag that differs in one bit. Open refuses and
  // releases no plaintext.
  for i: 0..<114 { back[i] = 165; }
  box[129] ^= 1;
  print("open with tag bit 0 of byte 15 flipped: %, output untouched %\n",
    STATUS[chacha20_poly1305_open(key2[..], nonce, aad, box[..], back[..], false)], untouched(back[..]));
  box[129] ^= 1;
  box[0] ^= 1;
  print("open with ciphertext bit 0 of byte 0 flipped: %, output untouched %\n",
    STATUS[chacha20_poly1305_open(key2[..], nonce, aad, box[..], back[..], false)], untouched(back[..]));
  box[0] ^= 1;
  aad[0] ^= 1;
  print("open with AAD bit 0 of byte 0 flipped: %, output untouched %\n",
    STATUS[chacha20_poly1305_open(key2[..], nonce, aad, box[..], back[..], false)], untouched(back[..]));
  aad[0] ^= 1;

  // Refusals before any write.
  out: [131] u8;
  for i: 0..<131 { out[i] = 165; }
  print("8-byte nonce: %\n", STATUS[chacha20_poly1305_seal(key2[..], nonce[0..<8], aad, plain, out[..], false)]);
  print("129-byte output for 114 bytes: %\n", STATUS[chacha20_poly1305_seal(key2[..], nonce, aad, plain, out[0..<129], false)]);
  print("open a 15-byte box: %\n", STATUS[chacha20_poly1305_open(key2[..], nonce, aad, box[0..<15], out[..], false)]);
  print("output untouched by the refusals: %\n", untouched(out[..]));

  // XChaCha20-Poly1305 (NIP-49) is this AEAD under an HChaCha20 subkey.
  // The XChaCha draft's section A.3.1 seals the same sentence.
  xkey: [32] u8; xnonce: [24] u8; xbox: [130] u8;
  for i: 0..<32 { xkey[i] = cast(u8) (128 + i); }
  for i: 0..<24 { xnonce[i] = cast(u8) (64 + i); }
  print("XChaCha20-Poly1305 seal: %\n", xcp_seal(xkey[..], xnonce[..], aad, plain, xbox[..], false));
  show("XChaCha20-Poly1305 tag", xbox[114..<130]);
}
```

Run it from the repository root:

```shell
coax/bin/coax run bitcoin/examples/chacha20-poly1305.coax
```

The output matches the values worked out by hand: the clamped `r`, its
limbs, the accumulator after block 1 (with a leading zero digit, because it
is printed as 17 bytes) and the RFC's tag:

```text
init: ok
r: 000806d5400e52447c036d555408bed685
r limbs: 0bed685 3555502 047c036 1003949 00806d5
update 16 bytes: ok
accumulator after block 1: 02c88c77849d64ae9147ddeb88e69c83fc
update 18 bytes: ok, 2 bytes wait for final
final: ok
tag: a8061dc1305136c6c22b8baf0c0127a9
state erased after final: true
update after final: already final
one-shot: true
one-shot tag: a8061dc1305136c6c22b8baf0c0127a9
Poly1305 key from block 0: 8ad5a08b905f81cc815040274ab29471a833b637e3fd0da508dbb8e2fdd1a646
seal: ok
ciphertext 0-15: d31a8d34648e60db7b86afbc53ef7ec2
tag: 1ae10b594f09e26a7e902ecbd0600691
sealed box equals RFC 8439 2.8.2: true
open: ok, "Ladies and Gentlemen of"
open with tag bit 0 of byte 15 flipped: tag mismatch, output untouched true
open with ciphertext bit 0 of byte 0 flipped: tag mismatch, output untouched true
open with AAD bit 0 of byte 0 flipped: tag mismatch, output untouched true
8-byte nonce: bad nonce
129-byte output for 114 bytes: bad output
open a 15-byte box: bad input
output untouched by the refusals: true
XChaCha20-Poly1305 seal: true
XChaCha20-Poly1305 tag: c0875924c1c7987947deafd8780acf49
```

The Poly1305 key is the RFC's section 2.6.2 output, and the sealed box is
section 2.8.2's ciphertext and tag byte for byte. The XChaCha20-Poly1305
tag is the last 16 bytes of the draft's section A.3.1 ciphertext.

The book's checks run this program in debug and release builds and compare
the output with
[bitcoin/fixtures/chacha20-poly1305.out](../fixtures/chacha20-poly1305.out).

## A broken case and its refusal

The broken case is a tag that differs in one bit. The example flips bit 0
of the last tag byte, `91` to `90`, and opens: the result is `tag mismatch`
(`CCP_TAG`), and the 114-byte output still holds its fill byte `a5` in
every position. No plaintext is released. Flipping one bit of the
ciphertext or of the AAD instead gives the same refusal. Compare chapter 11,
where a flipped ciphertext bit decrypted to `Madies` with no complaint.

The other refusals happen before any write:

- An 8-byte nonce, the size of the original ChaCha20's, returns
  `bad nonce`.
- A 129-byte output for 114 bytes of plaintext returns `bad output`: the
  box needs 130.
- A 15-byte box returns `bad input`: it can't even hold a tag.
- An update after final returns `already final` (`POLY_PHASE`).

## How the code is checked

### Published vectors, Wycheproof and OpenSSL

[coax/tests/bitcoin/chacha20_poly1305.py](../../coax/tests/bitcoin/chacha20_poly1305.py)
reads its vectors from pinned files after checking their SHA-256:

- From [rfc8439.txt](../../coax/tests/vectors/upstream/rfc/rfc8439.txt):
  section 2.5.2 and the eleven appendix A.3 Poly1305 tags, the section 2.6.2
  and three appendix A.4 one-time keys, the section 2.8.2 seal and the
  appendix A.5 decryption. A one-time key is checked through the AEAD: with
  no AAD and no message, the tag is Poly1305 of 16 zero bytes under it.
- All 325 cases of Wycheproof's
  [chacha20_poly1305_test.json](../../coax/tests/vectors/upstream/wycheproof/chacha20_poly1305_test.json),
  pinned for this chapter: 256 valid, 60 with a 12-byte nonce that must
  fail the tag, and 9 with another nonce size that must refuse it.
- 100 random Poly1305 tags from a model in Python's arbitrary-precision
  integers, written straight from section 2.5.1.
- 120 random AEAD boxes sealed by OpenSSL's `chacha20-poly1305` through
  Node, with lengths around the 16- and 64-byte boundaries.

It compiles `chacha20_poly1305.coax` to C in checked and release builds.
Each Poly1305 case runs through the one-shot call and the stream in 1-, 7-,
16- and 17-byte updates, which must also leave the state all zeros. Each
valid AEAD case must seal to its ciphertext and tag, open to its plaintext,
and refuse with `CCP_TAG` when one tag bit, chosen at random, is flipped,
leaving the output unwritten. Each invalid case must refuse with its status
and leave the output unwritten. Then it compiles the same C to wasm32 and
runs every case again in Node:

```shell
python3 coax/tests/bitcoin/chacha20_poly1305.py
```

On 2026-11-02 it passed 112 Poly1305 tags and 451 AEAD cases natively in
both builds and in Wasm, against Node v24.13.1 with OpenSSL 3.5.5.

### Checks in Coax

- `poly1305.coax` checks the section 2.5.2 tag, appendix A.3 vector 8, the
  clamped limbs of an all-ones key, the tag of a single block for every
  length 1 to 16 with `r = 1` and `s = 0` (the padded block itself), every
  split of a 49-byte message into two updates, the reduction at `p - 1`,
  `p` and `p + 1`, that final erases the state, that every refusal leaves
  the state and the output unchanged, and that each kind of forged state
  refuses.
- `chacha20_poly1305.coax` checks the section 2.6.2 key, the section 2.8.2
  seal and open, the empty-message tag, a flipped bit in every byte of the
  AAD, ciphertext and tag, round trips for every message length 0 to 33
  with AAD of 0, 12 and 17 bytes, and every admission boundary.
- `xchacha20_poly1305.coax` keeps its draft A.3.1 seal and open and its
  refusals, and adds the subkey and short nonce and the 4,096-byte seal
  limit.

The checks in
[bitcoin/examples/checks/chacha20-poly1305.coax](../examples/checks/chacha20-poly1305.coax)
load the example and compare it with bytes copied from the pinned RFC text:

<!-- source: bitcoin/examples/checks/chacha20-poly1305.coax check -->
```jai
// Byte strings copied from the pinned RFC 8439 text
// (coax/tests/vectors/upstream/rfc/rfc8439.txt) by its hex dumps:
// appendix A.3's eleven Poly1305 tags, appendix A.4's three one-time keys
// and appendix A.5's AEAD decryption.
A3_KEY :: [11] string.[
  "0000000000000000000000000000000000000000000000000000000000000000",
  "0000000000000000000000000000000036e5f6b5c5e06070f0efca96227a863e",
  "36e5f6b5c5e06070f0efca96227a863e00000000000000000000000000000000",
  "1c9240a5eb55d38af333888604f6b5f0473917c1402b80099dca5cbc207075c0",
  "0200000000000000000000000000000000000000000000000000000000000000",
  "02000000000000000000000000000000ffffffffffffffffffffffffffffffff",
  "0100000000000000000000000000000000000000000000000000000000000000",
  "0100000000000000000000000000000000000000000000000000000000000000",
  "0200000000000000000000000000000000000000000000000000000000000000",
  "0100000000000000040000000000000000000000000000000000000000000000",
  "0100000000000000040000000000000000000000000000000000000000000000"];
A3_TEXT :: [11] string.[
  "00000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000",
  "416e79207375626d697373696f6e20746f20746865204945544620696e74656e6465642062792074686520436f6e7472696275746f7220666f72207075626c69636174696f6e20617320616c6c206f722070617274206f6620616e204945544620496e7465726e65742d4472616674206f722052464320616e6420616e792073746174656d656e74206d6164652077697468696e2074686520636f6e74657874206f6620616e204945544620616374697669747920697320636f6e7369646572656420616e20224945544620436f6e747269627574696f6e222e20537563682073746174656d656e747320696e636c756465206f72616c2073746174656d656e747320696e20494554462073657373696f6e732c2061732077656c6c206173207772697474656e20616e6420656c656374726f6e696320636f6d6d756e69636174696f6e73206d61646520617420616e792074696d65206f7220706c6163652c207768696368206172652061646472657373656420746f",
  "416e79207375626d697373696f6e20746f20746865204945544620696e74656e6465642062792074686520436f6e7472696275746f7220666f72207075626c69636174696f6e20617320616c6c206f722070617274206f6620616e204945544620496e7465726e65742d4472616674206f722052464320616e6420616e792073746174656d656e74206d6164652077697468696e2074686520636f6e74657874206f6620616e204945544620616374697669747920697320636f6e7369646572656420616e20224945544620436f6e747269627574696f6e222e20537563682073746174656d656e747320696e636c756465206f72616c2073746174656d656e747320696e20494554462073657373696f6e732c2061732077656c6c206173207772697474656e20616e6420656c656374726f6e696320636f6d6d756e69636174696f6e73206d61646520617420616e792074696d65206f7220706c6163652c207768696368206172652061646472657373656420746f",
  "2754776173206272696c6c69672c20616e642074686520736c6974687920746f7665730a446964206779726520616e642067696d626c6520696e2074686520776162653a0a416c6c206d696d737920776572652074686520626f726f676f7665732c0a416e6420746865206d6f6d65207261746873206f757467726162652e",
  "ffffffffffffffffffffffffffffffff",
  "02000000000000000000000000000000",
  "fffffffffffffffffffffffffffffffff0ffffffffffffffffffffffffffffff11000000000000000000000000000000",
  "fffffffffffffffffffffffffffffffffbfefefefefefefefefefefefefefefe01010101010101010101010101010101",
  "fdffffffffffffffffffffffffffffff",
  "e33594d7505e43b900000000000000003394d7505e4379cd01000000000000000000000000000000000000000000000001000000000000000000000000000000",
  "e33594d7505e43b900000000000000003394d7505e4379cd010000000000000000000000000000000000000000000000"];
A3_TAG :: [11] string.[
  "00000000000000000000000000000000",
  "36e5f6b5c5e06070f0efca96227a863e",
  "f3477e7cd95417af89a6b8794c310cf0",
  "4541669a7eaaee61e708dc7cbcc5eb62",
  "03000000000000000000000000000000",
  "03000000000000000000000000000000",
  "05000000000000000000000000000000",
  "00000000000000000000000000000000",
  "faffffffffffffffffffffffffffffff",
  "14000000000000005500000000000000",
  "13000000000000000000000000000000"];
A4_KEY :: [3] string.[
  "0000000000000000000000000000000000000000000000000000000000000000",
  "0000000000000000000000000000000000000000000000000000000000000001",
  "1c9240a5eb55d38af333888604f6b5f0473917c1402b80099dca5cbc207075c0"];
A4_NONCE :: [3] string.[
  "000000000000000000000000",
  "000000000000000000000002",
  "000000000000000000000002"];
A4_POLYKEY :: [3] string.[
  "76b8e0ada0f13d90405d6ae55386bd28bdd219b8a08ded1aa836efcc8b770dc7",
  "ecfa254f845f647473d3cb140da9e87606cb33066c447b87bc2666dde3fbb739",
  "965e3bc6f9ec7ed9560808f4d229f94b137ff275ca9b3fcbdd59deaad23310ae"];
A5_KEY :: "1c9240a5eb55d38af333888604f6b5f0473917c1402b80099dca5cbc207075c0";
A5_NONCE :: "000000000102030405060708";
A5_AAD :: "f33388860000000000004e91";
A5_CT :: "64a0861575861af460f062c79be643bd5e805cfd345cf389f108670ac76c8cb24c6cfc18755d43eea09ee94e382d26b0bdb7b73c321b0100d4f03b7f355894cf332f830e710b97ce98c8a84abd0b948114ad176e008d33bd60f982b1ff37c8559797a06ef4f0ef61c186324e2b3506383606907b6a7c02b0f9f6157b53c867e4b9166c767b804d46a59b5216cde7a4e99040c5a40433225ee282a1b0a06c523eaf4534d7f83fa1155b0047718cbc546a0d072b04b3564eea1b422273f548271a0bb2316053fa76991955ebd63159434ecebb4e466dae5a1073a6727627097a1049e617d91d361094fa68f0ff77987130305beaba2eda04df997b714d6c6f2c29a6ad5cb4022b02709b";
A5_TAG :: "eead9d67890cbb22392336fea1851f38";
A5_MSG :: "496e7465726e65742d4472616674732061726520647261667420646f63756d656e74732076616c696420666f722061206d6178696d756d206f6620736978206d6f6e74687320616e64206d617920626520757064617465642c207265706c616365642c206f72206f62736f6c65746564206279206f7468657220646f63756d656e747320617420616e792074696d652e20497420697320696e617070726f70726961746520746f2075736520496e7465726e65742d447261667473206173207265666572656e6365206d6174657269616c206f7220746f2063697465207468656d206f74686572207468616e206173202fe2809c776f726b20696e2070726f67726573732e2fe2809d";

#forall "RFC 8439 appendix A.3 Poly1305 tags, one-shot and streamed" i: 0..<11 #steps 200000000 {
  kb: [32] u8; tb: [400] u8; wb: [16] u8; out: [16] u8;
  key := from_hex(A3_KEY[i], kb[..]); text := from_hex(A3_TEXT[i], tb[..]); want := from_hex(A3_TAG[i], wb[..]);
  #assert poly1305_checked(key, text, out[..], false);
  #assert same(out[..], want);
  // The same text in 7-byte updates.
  state: [1] Poly1305State;
  #assert poly1305_init(state[..], key, false) == POLY_OK;
  at := 0;
  while at < text.count {
    end := ifx at + 7 < text.count then at + 7 else text.count;
    #assert poly1305_update(state[..], text[at..<end], false) == POLY_OK;
    at = end;
  }
  #assert poly1305_final(state[..], out[..], false) == POLY_OK;
  #assert same(out[..], want);
}
#forall "RFC 8439 appendix A.4 one-time keys from ChaCha20 block 0" i: 0..<3 {
  kb: [32] u8; nb: [12] u8; wb: [32] u8; out: [32] u8;
  ccp_poly_key(from_hex(A4_KEY[i], kb[..]), from_hex(A4_NONCE[i], nb[..]), out[..]);
  #assert same(out[..], from_hex(A4_POLYKEY[i], wb[..]));
}
// Appendix A.5 receives a ciphertext and tag, checks the tag, then decrypts.
#test "RFC 8439 appendix A.5 opens to the Internet-Drafts text" {
  kb: [32] u8; nb: [12] u8; ab: [12] u8; box: [281] u8; out: [265] u8; wb: [265] u8;
  key := from_hex(A5_KEY, kb[..]); nonce := from_hex(A5_NONCE, nb[..]); aad := from_hex(A5_AAD, ab[..]);
  ct := from_hex(A5_CT, box[0..<265]); tag := from_hex(A5_TAG, box[265..<281]);
  #assert chacha20_poly1305_open(key, nonce, aad, box[..], out[..], false) == CCP_OK;
  #assert same(out[..], from_hex(A5_MSG, wb[..]));
  // Sealing the plaintext gives the received ciphertext and tag back.
  sealed: [281] u8;
  #assert chacha20_poly1305_seal(key, nonce, aad, wb[..], sealed[..], false) == CCP_OK;
  #assert same(sealed[..], box[..]);
}
// Each of the 128 tag bits, flipped alone: open refuses and writes nothing.
#forall "a tag one bit wrong releases no plaintext" bit: 0..<128 #steps 400000000 {
  kb: [32] u8; nb: [12] u8; ab: [12] u8; box: [281] u8; out: [265] u8;
  key := from_hex(A5_KEY, kb[..]); nonce := from_hex(A5_NONCE, nb[..]); aad := from_hex(A5_AAD, ab[..]);
  ct := from_hex(A5_CT, box[0..<265]); tag := from_hex(A5_TAG, box[265..<281]);
  box[265 + bit / 8] ^= cast(u8) (1 << (bit % 8));
  for i: 0..<265 { out[i] = 165; }
  #assert chacha20_poly1305_open(key, nonce, aad, box[..], out[..], false) == CCP_TAG;
  #assert untouched(out[..]);
}
// The comparisons the checks rely on tell different bytes apart.
#test "same and untouched refuse a difference" {
  a := [3] u8.[1, 2, 3]; b := [3] u8.[0, 2, 3]; c := [3] u8.[1, 2, 4]; d := [2] u8.[165, 164];
  #assert same(a[..], a[..]) && !same(a[..], a[0..<2]);
  #assert !same(a[..], b[..]) && !same(a[..], c[..]);
  #assert untouched(d[0..<1]) && !untouched(d[..]);
}
```

To run them:

```shell
coax/bin/coax test bitcoin/examples/checks/chacha20-poly1305.coax
coax/bin/coax mutate bitcoin/examples/checks/chacha20-poly1305.coax
coax/bin/coax mutate coax/lib/crypto/poly1305.coax --root-only
coax/bin/coax mutate coax/lib/crypto/chacha20_poly1305.coax --root-only
coax/bin/coax mutate coax/lib/crypto/xchacha20_poly1305.coax --root-only
```

`coax test` runs 52 checks, including the loaded files' own. On 2026-11-02,
`coax mutate` on the checks file, which mutates the example and every loaded
file, killed 1,005 of 1,018 mutants; the other 13 are marked equivalent.
With `--root-only`, `poly1305.coax` killed all 324 of its mutants,
`chacha20_poly1305.coax` 88 of 95 and `xchacha20_poly1305.coax` 77 of 83,
the rest marked equivalent. Each equivalent mark is on a line that erases a
dead local, such as the one-time key or the computed tag. Erasure is a rule
from chapter 3, not a result, so no check can see it.

### The users' checks

The existing users pass unchanged: `coax test` on
`coax/lib/crypto/aead_workspace.coax`, `poly1305_workspace.coax`,
`nostr/examples/checks/ncryptsec.coax`, `coax/tests/nostr/ncryptsec_vector.coax`
and `psionic/lib/peer/link.coax`, and the native, sanitizer and Wasm runs of
`coax/tests/nostr/poly1305.py`, `xchacha20_poly1305.py`,
`poly1305_workspace.py`, `aead_workspace.py`, `aead_host.py` and
`ncryptsec_vector.sh`, which compare with OpenSSL and NIP-49's published
vector.

## Summary

- Poly1305 evaluates the message blocks, each with a `01` byte appended, as
  a polynomial at a clamped secret `r` modulo `2^130 - 5`, and adds a
  secret `s`.
- Five 26-bit limbs hold the 130-bit numbers; `2^130 = 5 (mod p)` folds the
  high products back, and one masked subtraction finishes the reduction.
- The AEAD takes its one-time Poly1305 key from ChaCha20 block 0, encrypts
  from counter 1, and MACs the padded AAD, the padded ciphertext and both
  lengths.
- Open compares tags in constant time and decrypts only on a match; a tag
  one bit wrong releases nothing.
- The MAC state is yours: every call checks it first, refusals change
  nothing, and final erases it. XChaCha20-Poly1305 is the same AEAD under an
  HChaCha20 subkey.

## Dependencies

- [3. Constant-time rules and secrets](ch003-constant-time-rules.md)
- [11. ChaCha20](ch011-chacha20.md)

## Sources

- [RFC 8439](https://www.rfc-editor.org/rfc/rfc8439), *ChaCha20 and Poly1305
  for IETF Protocols*, sections 2.5 to 2.8 and appendices A.3 to A.5,
  pinned as
  [rfc8439.txt](../../coax/tests/vectors/upstream/rfc/rfc8439.txt).
- D. J. Bernstein, *The Poly1305-AES message-authentication code* (2005),
  for the clamping and the security bound.
- [draft-irtf-cfrg-xchacha-03](https://datatracker.ietf.org/doc/html/draft-irtf-cfrg-xchacha-03),
  section A.3.1, for the XChaCha20-Poly1305 example.
- Wycheproof `chacha20_poly1305_test.json`, pinned at revision `d17cc89f`
  in [coax/tests/vectors/SOURCES.md](../../coax/tests/vectors/SOURCES.md).
- BIP-324, BOLT 8 and BOLT 4, for the uses in chapters 67, 105 and 111.
