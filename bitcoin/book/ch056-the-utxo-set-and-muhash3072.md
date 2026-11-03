# 56. The UTXO set and MuHash3072

A full node keeps one table that every block changes: the set of
unspent transaction outputs, the UTXO set. Each entry is a coin: an
outpoint (a txid and an output index), the amount, the scriptPubKey, the
height of the block that created it and whether that block's coinbase
created it. Validating a transaction is mostly looking its inputs up in
this set. Connecting a block removes the coins it spends and adds the
coins it creates.

Two nodes that agree on the chain must agree on the set, and a node that
loads a snapshot of the set (assumeutxo) must know the snapshot is the
right one. Bitcoin Core answers both with a hash of the whole set, which
`gettxoutsetinfo` prints. It offers two: `hash_serialized_3`, a SHA-256
over every coin in key order, and `muhash`, a hash of the set as a set,
which a node can update one coin at a time. This chapter builds both, and
Core's compact on-disk form of a coin, and checks them against the values
Core's own functional tests publish.

## What this chapter teaches

- What a coin is, and the record Bitcoin Core stores for it in the
  chainstate: Core's VARINT, `CompressAmount` and the six special script
  forms of `ScriptCompression`.
- The chainstate key of a coin: `'C'`, the txid and a VARINT of the
  output index.
- `TxOutSer`, the 4-field record both set hashes read.
- `hash_serialized_3`: one SHA-256d stream over the set in key order.
- MuHash3072: each element hashed to a number below a 3072-bit prime, the
  set as the product of its elements, removal as division, and a modular
  inverse only when the hash is read.
- Seven result codes, with the error Core throws for each that has one.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [muhash.coax](../../coax/lib/bitcoin/muhash.coax) | `mh_mul_into`, `mh_multiply` | multiplication mod `2^3072 - 1103717` |
| | `mh_is_overflow`, `mh_full_reduce` | Core's `IsOverflow` and `FullReduce` |
| | `mh_inverse` | the inverse, as `a^(p-2)` |
| | `mh_element_vartime` | Core's `ToNum3072`: SHA-256, then ChaCha20 |
| | `muhash_new`, `muhash_insert_vartime`, `muhash_remove_vartime` | the empty set, insertion and removal |
| | `muhash_combine`, `muhash_divide` | Core's `operator*=` and `operator/=` |
| | `muhash_finalize_vartime` | the 32-byte hash |
| | `muhash_serialize`, `muhash_deserialize` | the 768-byte state Core writes |
| [coins.coax](../../coax/lib/bitcoin/coins.coax) | `coin_varint_write_vartime`, `coin_varint_read_vartime` | serialize.h's VARINT |
| | `coin_compress_amount`, `coin_decompress_amount` | Core's amount compression |
| | `coin_compress_script_vartime`, `coin_decompress_script_vartime` | Core's six special scripts |
| | `coin_write_vartime`, `coin_read_vartime` | the chainstate's coin record |
| | `coin_key_write_vartime`, `coin_key_read_vartime`, `utxo_order_vartime` | the chainstate key and its order |
| | `coin_txout_ser_vartime` | `TxOutSer` from `kernel/coinstats.cpp` |
| | `utxo_hash_init`, `utxo_hash_add_vartime`, `utxo_hash_final` | `hash_serialized_3` |
| | `utxo_muhash_apply_vartime` | one coin into or out of a MuHash |
| [chacha20.coax](../../coax/lib/crypto/chacha20.coax) | `chacha20_block` | the keystream (chapter 11) |
| [sha256.coax](../../coax/lib/crypto/sha256.coax) | `sha256_hash_checked` | SHA-256 (chapter 4) |
| [secp_encode.coax](../../coax/lib/crypto/secp_encode.coax) | point decoding | the `y` of an uncompressed key (chapter 17) |

A coin is public data, so every procedure runs in variable time and its
name ends in `_vartime` where that matters (chapter 3). The arithmetic of
MuHash works on views of 48 limbs of 64 bits, one 3072-bit number, so a
multiplication allocates nothing and writes its result in place.

The two files return these codes:

<!-- source: coax/lib/bitcoin/coins.coax codes -->
```jai
// Refusal codes. Core throws std::ios_base::failure with the message in
// quotes.
COIN_OK :: 0;
COIN_TRUNCATED :: 1;       // the bytes end inside a field: "end of data"
COIN_VARINT_LARGE :: 2;    // a VARINT above its type: "ReadVarInt(): size too large"
COIN_CAPACITY :: 3;        // the caller's output is too small
COIN_BAD_AMOUNT :: 4;      // an amount above MAX_MONEY to compress (Core's precondition)
COIN_BAD_HEIGHT :: 5;      // a height that does not fit in 31 bits
COIN_BAD_KEY :: 6;         // a chainstate key that is not 'C', 32 bytes and a VARINT

COIN_MAX_MONEY :: cast(u64) 2100000000000000;   // 21,000,000 BTC in satoshis
COIN_MAX_SCRIPT :: 10000;                        // script.h MAX_SCRIPT_SIZE
COIN_SPECIAL :: 6;                               // ScriptCompression::nSpecialScripts
COIN_KEY_PREFIX :: 'C';                          // txdb.cpp DB_COIN
```

<!-- source: coax/lib/bitcoin/muhash.coax codes -->
```jai
MUHASH_OK :: 0;
MUHASH_CAPACITY :: 1;   // a view is not the size the call needs
MUHASH_BAD_STATE :: 2;  // a serialized state is not 768 bytes

MH_LIMBS :: 48;                      // 48 limbs of 64 bits: 3072 bits
MH_BYTES :: 384;                     // one number, little-endian
MH_STATE_BYTES :: 768;               // numerator, then denominator
MH_DIFF :: cast(u64) 1103717;        // the modulus is 2^3072 - MH_DIFF
```

The types are small. A coin carries its script beside it as a byte view:

<!-- source: coax/lib/bitcoin/coins.coax types -->
```jai
// One unspent output. The scriptPubKey travels beside it as a byte view.
Coin :: struct { value: u64; height: int; coinbase: bool; }
// A VARINT read: its value, where reading goes on, and a refusal.
CoinNum :: struct { value: u64; next: int; code: int; }
// A coin record read: the coin, its script's length in the caller's
// output, and where the record ended. bad_key is Core's silent case: a
// compressed uncompressed key (kinds 4 and 5) whose x is on no point
// decompresses to an empty script.
CoinRead :: struct { code: int; next: int; coin: Coin; script_n: int; bad_key: bool; }
// A chainstate key read: the output index and the txid's position.
CoinKey :: struct { code: int; n: int; txid: int; }
```

## A coin on disk

Bitcoin Core stores the UTXO set in a LevelDB database called the
chainstate. Each coin is one key and one value. The value is short:
most coins are pay-to-pubkey-hash or witness outputs with round amounts,
and Core writes them in about 20 to 40 bytes. Three tools make that so.

### VARINT

The chainstate does not use the CompactSize of transactions (chapter
34). It uses Core's VARINT from `serialize.h`: groups of 7 bits, most
significant first, every byte but the last with its top bit set. Each
continuation also subtracts one, so every number has exactly one
encoding. With plain base-128, `80 00` and `00` would both mean zero; with
the subtraction, `80 00` is 128.

<!-- source: coax/lib/bitcoin/coins.coax varint -->
```jai
// serialize.h's VARINT: big-endian groups of 7 bits, every byte but the
// last with its top bit set. Each continuation also subtracts one, so every
// number has exactly one encoding: 127 is 7f, 128 is 80 00 (not 81 00).
coin_varint_size :: (v: u64) -> int {
  n := 1; x := v;
  while x > 0x7f { x = (x >> 7) - 1; n += 1; }
  return n;
}
// Writes VARINT(v) into out at at; where writing goes on, or -1 when it
// does not fit.
coin_varint_write_vartime :: (out: [] u8, at: int, v: u64) -> int {
  size := coin_varint_size(v);
  if at < 0 || size > out.count - at { return -1; }
  x := v;
  for k: 0..<size {
    low := cast(u8) (x & 0x7f);
    out[at + size - 1 - k] = ifx k == 0 then low else low | 0x80;
    x = (x >> 7) - (ifx k < size - 1 then cast(u64) 1 else 0);
  }
  return at + size;
}
// ReadVarInt for a type whose largest value is max (0xffffffff for
// uint32_t, all ones for uint64_t). Core refuses before a shift would lose
// a bit, and before the plus one would pass max.
coin_varint_read_vartime :: (b: [] u8, at: int, max: u64) -> CoinNum {
  n := cast(u64) 0; i := at;
  while true {
    if i >= b.count { return CoinNum.{next = at, code = COIN_TRUNCATED}; }
    byte := b[i]; i += 1;
    if n > (max >> 7) { return CoinNum.{next = at, code = COIN_VARINT_LARGE}; }
    n = (n << 7) | cast(u64) (byte & 0x7f);
    if (byte & 0x80) == 0 { return CoinNum.{value = n, next = i}; }
    if n == max { return CoinNum.{next = at, code = COIN_VARINT_LARGE}; }
    n += 1;
  }
  return CoinNum.{};
}
```

Reading a VARINT takes the largest value of the type it fills. Core
throws "ReadVarInt(): size too large" before a shift would push a bit out
of the type, and before the plus one would pass its largest value. A
height code of `2^32` (`8e fe fe ff 00`) is refused when read into a
`uint32_t`.

### Amounts

`CompressAmount` strips trailing decimal zeros. Most amounts are round in
some unit, so a large number of satoshis becomes a small number and a
short VARINT. Fifty bitcoin, 5,000,000,000 satoshis, compresses to 50, and
its VARINT is one byte, `32`.

<!-- source: coax/lib/bitcoin/coins.coax amount -->
```jai
// CompressAmount: strip up to nine trailing decimal zeros into e. Below
// nine, the last digit d (1-9) is kept apart: 1 + 10 * (9 * n + d - 1) + e.
// With nine zeros: 1 + 10 * (n - 1) + 9. Zero is 0. Round amounts, the
// usual case, become small numbers: 50 BTC is 0x32.
coin_compress_amount :: (amount: u64) -> u64 {
  if amount == 0 { return 0; }
  n := amount; e := cast(u64) 0;
  while n % 10 == 0 && e < 9 { n /= 10; e += 1; }
  if e < 9 {
    d := n % 10;
    n /= 10;
    return 1 + (n * 9 + d - 1) * 10 + e;
  }
  return 1 + (n - 1) * 10 + 9;
}
// DecompressAmount, the inverse. It reads any uint64_t, as Core does, and
// a number no compressed amount gives can wrap past 2^64 the way Core's
// uint64_t does.
coin_decompress_amount :: (compressed: u64) -> u64 {
  if compressed == 0 { return 0; }
  x := compressed - 1;
  e := x % 10;
  x /= 10;
  n := cast(u64) 0;
  if e < 9 {
    d := x % 9 + 1;
    x /= 9;
    n = add_wrap(mul_wrap(x, 10), d);
  } else {
    n = x + 1;
  }
  while e > 0 { n = mul_wrap(n, 10); e -= 1; }
  return n;
}
```

Compression is defined up to `MAX_MONEY`, which is a precondition in Core;
the coin writer refuses a larger value with `COIN_BAD_AMOUNT`.
Decompression reads any `uint64_t`, as Core's does. A number that no
amount compresses to can decompress past `2^64`, and it wraps the way
Core's `uint64_t` arithmetic does: `DecompressAmount(2^64 - 1)` is
`0x1c71c71c71c72520`.

### Scripts

`ScriptCompression` writes six common scripts as a kind byte and the
bytes that vary. Every other script is written whole, after a VARINT of
its length plus six, so that lengths and kinds never collide.

| Kind | Script | Bytes kept |
| --- | --- | --- |
| 0 | P2PKH: `OP_DUP OP_HASH160 <20> OP_EQUALVERIFY OP_CHECKSIG` | the 20-byte key hash |
| 1 | P2SH: `OP_HASH160 <20> OP_EQUAL` | the 20-byte script hash |
| 2, 3 | P2PK with a compressed key | `x`; the kind is the key's prefix |
| 4, 5 | P2PK with an uncompressed key on the curve | `x`; the kind is 4 plus `y`'s low bit |

<!-- source: coax/lib/bitcoin/coins.coax script -->
```jai
// CompressScript: the three script shapes worth a short form, each written
// as a kind byte and the bytes that vary. Kind 0 is P2PKH (the 20-byte key
// hash), 1 is P2SH (the script hash), 2 and 3 are P2PK with a compressed key
// (the prefix is the kind, then x), and 4 and 5 are P2PK with an
// uncompressed key on the curve (4 plus y's low bit, then x). Writes 21 or
// 33 bytes into out and returns that, 0 for any other script, or -1 when
// out is under 33 bytes.
coin_compress_script_vartime :: (s: [] u8, out: [] u8) -> int {
  if out.count < 33 { return -1; }
  if s.count == 25 && s[0] == 0x76 && s[1] == 0xa9 && s[2] == 20 && s[23] == 0x88 && s[24] == 0xac {
    out[0] = 0; for i: 0..<20 { out[1+i] = s[3+i]; }
    return 21;
  }
  if s.count == 23 && s[0] == 0xa9 && s[1] == 20 && s[22] == 0x87 {
    out[0] = 1; for i: 0..<20 { out[1+i] = s[2+i]; }
    return 21;
  }
  if s.count == 35 && s[0] == 33 && s[34] == 0xac && (s[1] == 2 || s[1] == 3) {
    for i: 0..<33 { out[i] = s[1+i]; }
    return 33;
  }
  // An uncompressed key must be a point: off the curve it has no short form.
  if s.count == 67 && s[0] == 65 && s[66] == 0xac && s[1] == 4 && se_decode(s[1..<66]).ok {
    out[0] = 4 | (s[65] & 1);
    for i: 0..<32 { out[1+i] = s[2+i]; }
    return 33;
  }
  return 0;
}
// DecompressScript: kind and its 20 or 32 bytes back to the script in out.
// Returns the script's length; 0 for kinds 4 and 5 when x is on no point
// (Core leaves the script empty); -1 for a kind above 5, data of the wrong
// size or an out under 67 bytes.
coin_decompress_script_vartime :: (kind: int, data: [] u8, out: [] u8) -> int {
  if kind < 0 || kind >= COIN_SPECIAL || data.count != coin_special_size(kind) || out.count < 67 { return -1; }
  if kind == 0 {
    out[0] = 0x76; out[1] = 0xa9; out[2] = 20;
    for i: 0..<20 { out[3+i] = data[i]; }
    out[23] = 0x88; out[24] = 0xac;
    return 25;
  }
  if kind == 1 {
    out[0] = 0xa9; out[1] = 20;
    for i: 0..<20 { out[2+i] = data[i]; }
    out[22] = 0x87;
    return 23;
  }
  if kind == 2 || kind == 3 {
    out[0] = 33; out[1] = cast(u8) kind;
    for i: 0..<32 { out[2+i] = data[i]; }
    out[34] = 0xac;
    return 35;
  }
  // Kinds 4 and 5: the compressed key with prefix kind - 2, decompressed.
  key: [33] u8; full: [65] u8;
  key[0] = cast(u8) (kind - 2);
  for i: 0..<32 { key[1+i] = data[i]; }
  if se_recode(key[..], SE_UNCOMPRESSED, full[..]) != SE_UNCOMPRESSED { return 0; }
  out[0] = 65;
  for i: 0..<65 { out[1+i] = full[i]; }
  out[66] = 0xac;
  return 67;
}
// GetSpecialScriptSize: 20 bytes for kinds 0 and 1, 32 for 2 to 5.
coin_special_size :: (kind: int) -> int { return ifx kind < 2 then 20 else 32; }
```

Kinds 4 and 5 store an uncompressed key as if it were compressed, which
halves it. That needs the key to be a point on the curve: only then is
`y` one of the two roots of `x^3 + 7`, and its low bit picks which.
Compression checks this with chapter 17's decoder. An uncompressed key
off the curve stays a raw script.

The reverse direction has a corner. A database can hold a kind 4 or 5 with
an `x` that has no `y`; Core's `DecompressScript` then leaves the script
empty and says nothing. `coin_read_vartime` does the same, so that it reads
what Core reads, and sets `bad_key` so that the caller can tell.

Witness outputs have no special kind. A P2WPKH script is 22 bytes and a
P2TR script 34, so both are written raw, with one byte of length.

A script longer than `MAX_SCRIPT_SIZE` (10,000 bytes) can never be spent.
Core does not store it: it reads it as a single `OP_RETURN` and skips its
bytes, and the reader here does the same.

### The record

The record is three fields: a VARINT of `height * 2 + coinbase`, a VARINT
of the compressed amount, then the compressed script.

<!-- source: coax/lib/bitcoin/coins.coax record -->
```jai
// Coin::Serialize into out: VARINT(height * 2 + coinbase), VARINT of the
// compressed amount, then the script: its short form, or VARINT(length + 6)
// and the bytes. Returns the length, or minus a refusal code; a refusal
// may leave out partly written.
coin_write_vartime :: (c: Coin, script: [] u8, out: [] u8) -> int {
  if c.value > COIN_MAX_MONEY { return -COIN_BAD_AMOUNT; }
  if c.height < 0 || c.height > 0x7fffffff { return -COIN_BAD_HEIGHT; }
  code := cast(u64) c.height * 2 + (ifx c.coinbase then cast(u64) 1 else 0);
  at := coin_varint_write_vartime(out, 0, code);
  if at < 0 { return -COIN_CAPACITY; }
  at = coin_varint_write_vartime(out, at, coin_compress_amount(c.value));
  if at < 0 { return -COIN_CAPACITY; }
  short: [33] u8;
  k := coin_compress_script_vartime(script, short[..]);
  if k > 0 {
    if k > out.count - at { return -COIN_CAPACITY; }
    for i: 0..<k { out[at+i] = short[i]; }
    return at + k;
  }
  at = coin_varint_write_vartime(out, at, cast(u64) (script.count + COIN_SPECIAL));
  if at < 0 || script.count > out.count - at { return -COIN_CAPACITY; }
  for i: 0..<script.count { out[at+i] = script[i]; }
  return at + script.count;
}
// Coin::Unserialize from b at at, the script into script_out (10,000 bytes
// hold any). A script longer than MAX_SCRIPT_SIZE reads as one OP_RETURN
// with its bytes skipped, as Core's ScriptCompression::Unser does.
coin_read_vartime :: (b: [] u8, at: int, script_out: [] u8) -> CoinRead {
  r: CoinRead;
  code := coin_varint_read_vartime(b, at, 0xffffffff);
  if code.code != COIN_OK { r.code = code.code; return r; }
  amount := coin_varint_read_vartime(b, code.next, 0xffffffffffffffff);
  if amount.code != COIN_OK { r.code = amount.code; return r; }
  size := coin_varint_read_vartime(b, amount.next, 0xffffffff);
  if size.code != COIN_OK { r.code = size.code; return r; }
  r.coin.height = cast(int) (code.value >> 1);
  r.coin.coinbase = (code.value & 1) == 1;
  r.coin.value = coin_decompress_amount(amount.value);
  at2 := size.next;
  if size.value < cast(u64) COIN_SPECIAL {
    kind := cast(int) size.value;
    need := coin_special_size(kind);
    if need > b.count - at2 { r.code = COIN_TRUNCATED; return r; }
    if script_out.count < 67 { r.code = COIN_CAPACITY; return r; }
    k := coin_decompress_script_vartime(kind, b[at2..<at2+need], script_out);
    r.script_n = k; r.bad_key = k == 0; r.next = at2 + need;
    return r;
  }
  n := cast(int) (size.value - cast(u64) COIN_SPECIAL);
  if n > b.count - at2 { r.code = COIN_TRUNCATED; return r; }
  if n > COIN_MAX_SCRIPT {
    if script_out.count < 1 { r.code = COIN_CAPACITY; return r; }
    script_out[0] = 0x6a; r.script_n = 1; r.next = at2 + n;
    return r;
  }
  if n > script_out.count { r.code = COIN_CAPACITY; return r; }
  for i: 0..<n { script_out[i] = b[at2+i]; }
  r.script_n = n; r.next = at2 + n;
  return r;
}
```

The coinbase of regtest block 100 in Core's `rpc_dumptxoutset.py` pays
50 bitcoin to a P2PKH script. Its record is 24 bytes:

```text
80 49                         VARINT(100 * 2 + 1) = VARINT(201)
32                            VARINT(CompressAmount(5,000,000,000)) = VARINT(50)
00                            kind 0: P2PKH
2b4569203694fc997e13f2c0a1383b9e16c77a0d   the key hash
```

The same output in a transaction is 8 bytes of amount, a byte of length
and 25 bytes of script: 34 bytes, and the height and coinbase flag are
not in it at all.

### The key

The key of a coin is the byte `'C'`, the txid in hash order (the bytes
SHA-256d produced, not the reversed form explorers show) and a VARINT of
the output index. LevelDB sorts keys as bytes, so the chainstate is in
txid order, and the outputs of one transaction are together and in index
order: a VARINT keeps numeric order.

<!-- source: coax/lib/bitcoin/coins.coax key -->
```jai
// The chainstate key of an outpoint: 'C', the txid in hash order, VARINT(n).
// Returns the length, or -1 when out is too small or txid is not 32 bytes.
coin_key_write_vartime :: (txid: [] u8, n: int, out: [] u8) -> int {
  if txid.count != 32 || n < 0 || n > 0xffffffff || out.count < 33 { return -1; }
  out[0] = COIN_KEY_PREFIX;
  for i: 0..<32 { out[1+i] = txid[i]; }
  return coin_varint_write_vartime(out, 33, cast(u64) n);
}
// A whole key back: the prefix, 32 bytes and a VARINT that ends the key.
coin_key_read_vartime :: (b: [] u8) -> CoinKey {
  if b.count < 34 || b[0] != COIN_KEY_PREFIX { return CoinKey.{code = COIN_BAD_KEY}; }
  v := coin_varint_read_vartime(b, 33, 0xffffffff);
  if v.code != COIN_OK || v.next != b.count { return CoinKey.{code = COIN_BAD_KEY}; }
  return CoinKey.{n = cast(int) v.value, txid = 1};
}
// The order of the chainstate, and of both hashes: txid bytes as stored
// (hash order), then the output index as a number. Negative, zero or
// positive as a sorts before, with or after b.
utxo_order_vartime :: (a_txid: [] u8, a_n: int, b_txid: [] u8, b_n: int) -> int {
  for i: 0..<32 {
    if a_txid[i] != b_txid[i] { return ifx a_txid[i] < b_txid[i] then -1 else 1; }
  }
  return ifx a_n < b_n then -1 else ifx a_n > b_n then 1 else 0;
}
```

## Hashing the whole set

`gettxoutsetinfo` reads every coin and hashes it. Both hashes read the
same record per coin, `TxOutSer`, which is not the chainstate's compact
record: the amount is 8 bytes and the script is uncompressed, so the hash
does not depend on how a node stores its coins.

<!-- source: coax/lib/bitcoin/coins.coax txoutser -->
```jai
// TxOutSer (kernel/coinstats.cpp): the record both set hashes read. The
// outpoint (txid, 4-byte n), a 4-byte height * 2 + coinbase, the 8-byte
// value, then the script with its CompactSize length. All little-endian.
// Returns the length, or -1 when out is too small.
coin_txout_ser_vartime :: (txid: [] u8, n: int, c: Coin, script: [] u8, out: [] u8) -> int {
  size := 32 + 4 + 4 + 8 + tx_compact_size_vartime(script.count) + script.count;
  if txid.count != 32 || out.count < size { return -1; }
  for i: 0..<32 { out[i] = txid[i]; }
  code := c.height * 2 + (ifx c.coinbase then 1 else 0);
  for i: 0..<4 { out[32+i] = cast(u8) ((n >> (8*i)) & 255); out[36+i] = cast(u8) ((code >> (8*i)) & 255); }
  #assert cb_store_le64(out, 40, c.value);
  at := tx_compact_write_vartime(out, 48, script.count);
  for i: 0..<script.count { out[at+i] = script[i]; }
  return size;
}
```

`hash_serialized_3` feeds every `TxOutSer` into one SHA-256 stream in
chainstate order and hashes the result again. It is cheap and simple, and
it is what assumeutxo snapshots are checked against. It has a cost: the
hash depends on every coin and on their order, so a node can only compute
it by reading the whole set again.

`muhash` hashes each `TxOutSer` as an element of a set. A node can keep it
current as blocks connect: each new coin is inserted, each spent coin is
removed, and order does not matter.

<!-- source: coax/lib/bitcoin/coins.coax hashes -->
```jai
// hash_serialized_3: one SHA-256 stream over every coin's TxOutSer in
// chainstate order, then SHA-256 of that (HashWriter's GetHash). state is a
// SHA-256 state of SHA_STATE_BYTES.
utxo_hash_init :: (state: [] u8) -> bool {
  return sha256_init(state) == SHA_OK;
}
utxo_hash_add_vartime :: (state: [] u8, txid: [] u8, n: int, c: Coin, script: [] u8) -> bool {
  rec: [10100] u8;
  k := coin_txout_ser_vartime(txid, n, c, script, rec[..]);
  return k > 0 && sha256_update_checked(state, rec[0..<k], false) == SHA_OK;
}
utxo_hash_final :: (state: [] u8, out: [] u8) -> bool {
  once: [32] u8;
  if out.count != 32 || sha256_final_checked(state, once[..], false) != SHA_OK { return false; }
  return sha256_hash_checked(once[..], out, false) == SHA_OK;
}
// muhash: each coin's TxOutSer is one element. Adding a block's new coins
// and removing its spent ones keeps the hash current without the set.
utxo_muhash_apply_vartime :: (m: MuHash3072, txid: [] u8, n: int, c: Coin, script: [] u8, spend: bool) -> MuHash3072 {
  rec: [10100] u8;
  k := coin_txout_ser_vartime(txid, n, c, script, rec[..]);
  #assert k > 0;
  return ifx spend then muhash_remove_vartime(m, rec[0..<k]) else muhash_insert_vartime(m, rec[0..<k]);
}
```

## MuHash3072

MuHash, from Bellare and Micciancio's incremental hashing, hashes a set by
multiplying its elements' hashes in a group where finding a second set
with the same product is hard. Core's MuHash3072 uses the integers modulo
the prime `p = 2^3072 - 1103717`.

<!-- source: coax/lib/bitcoin/muhash.coax types -->
```jai
// A number below 2^3072, limb 0 least significant. Most are below the
// modulus; IsOverflow says when one is not.
Num3072 :: struct { limb: [48] u64; }
// A set: the product of what was inserted over the product of what was
// removed.
MuHash3072 :: struct { numerator: Num3072; denominator: Num3072; }
```

A set is a fraction: the product of what was inserted over the product of
what was removed. Insertion and removal each cost one multiplication. The
division happens once, when the hash is read.

### An element's number

An element is any byte string; here it is a `TxOutSer`. Core's
`ToNum3072` hashes it with SHA-256, uses the digest as a ChaCha20 key, and
reads 384 bytes of keystream (six blocks from counter 0, with a zero
nonce) as one little-endian 3072-bit number. The result is close to
uniform below `2^3072`, which is close to uniform modulo `p`.

<!-- source: coax/lib/bitcoin/muhash.coax element -->
```jai
// One element's number (Core's ToNum3072): SHA-256 of the bytes is a
// ChaCha20 key; its keystream from counter 0 with a zero nonce, six 64-byte
// blocks, read little-endian, is the number.
mh_element_vartime :: (data: [] u8) -> Num3072 {
  key: [32] u8; nonce: [12] u8; stream: [384] u8;
  #assert sha256_hash_checked(data, key[..], false) == SHA_OK;
  for k: 0..<6 {
    #assert chacha20_block_checked(key[..], nonce[..], cast(u32) k, stream[64*k..<64*k+64], false).error == CC_OK;
  }
  return mh_from_bytes(stream[..]);
}
```

The number is very rarely at least `p`: only 1,103,717 of the `2^3072`
values are. Core does not reduce it here; the multiplication handles
inputs up to `2^3072 - 1`.

### Multiplication

Multiplying two 48-limb numbers gives a 96-limb product. The modulus is
just under a power of two, so the top half folds back cheaply:
`2^3072 = 1103717 (mod p)`. The product `hi * 2^3072 + lo` is congruent to
`hi * 1103717 + lo`, which is a 48-limb number plus a small carry, and a
second fold with the carry finishes. The column helpers follow Core's
`muhash.cpp` exactly, with 192-bit accumulators held as three limbs.

<!-- source: coax/lib/bitcoin/muhash.coax columns -->
```jai
// A running column sum is three limbs c[0], c[1], c[2], c[0] least
// significant, held in a view so each step updates it in place.
// c += a * b. mul_wide gives the 128-bit product as two limbs; each carry
// is "did the addition wrap", a sum smaller than what was added.
mh_muladd3 :: (c: [] u64, a: u64, b: u64) {
  t := mul_wide(a, b);
  c[0] = add_wrap(c[0], t.lo);
  th := t.hi + (ifx c[0] < t.lo then cast(u64) 1 else 0);   // t.hi <= 2^64 - 2: no overflow
  c[1] = add_wrap(c[1], th);
  c[2] += ifx c[1] < th then cast(u64) 1 else 0;
}
// c += n * d, with c[2] zero on entry (Core's mulnadd3).
mh_mulnadd3 :: (c: [] u64, d: [] u64, n: u64) {
  t := mul_wide(d[0], n);
  c[0] = add_wrap(t.lo, c[0]);
  hi := t.hi + (ifx c[0] < t.lo then cast(u64) 1 else 0);
  u := mul_wide(d[1], n);
  lo := add_wrap(u.lo, hi);
  top := u.hi + (ifx lo < hi then cast(u64) 1 else 0);
  c[1] = add_wrap(lo, c[1]);
  top += ifx c[1] < lo then cast(u64) 1 else 0;
  c[2] = top + d[2] * n;
}
// [c0, c1] *= n, keeping two limbs (Core's muln2).
mh_muln2 :: (c: [] u64, n: u64) {
  t := mul_wide(c[0], n);
  c[0] = t.lo;
  c[1] = add_wrap(mul_wrap(c[1], n), t.hi);
}
// Shift the sum down one limb and return the limb that falls out (Core's
// extract3).
mh_extract3 :: (c: [] u64) -> u64 {
  n := c[0];
  c[0] = c[1]; c[1] = c[2]; c[2] = 0;
  return n;
}
// [c0, c1] += a, then the low limb falls out and the rest shifts down one
// limb (Core's addnextract2).
mh_addnextract2 :: (c: [] u64, a: u64) -> u64 {
  c0 := add_wrap(c[0], a); c1 := c[1]; c2 := cast(u64) 0;
  if c0 < a {
    c1 = add_wrap(c1, 1);
    if c1 == 0 { c2 = 1; }
  }
  c[0] = c1; c[1] = c2;
  return c0;
}
```

<!-- source: coax/lib/bitcoin/muhash.coax multiply -->
```jai
// out = a * b mod p, Core's Num3072::Multiply. out may be a or b. The full
// product has 96 limbs; limb 48 + j counts 2^3072 times as much as limb j,
// and 2^3072 is 1103717 mod p. So column j adds 1103717 times column 48 + j
// as it goes (the first reduction). What carries past limb 47 is reduced
// once more, and a result still at least p is reduced a last time.
mh_mul_into :: (a: [] u64, b: [] u64, out: [] u64) {
  tmp: [48] u64; c: [3] u64; d: [3] u64;
  // Columns 0..46, each with its high partner folded in.
  for j: 0..<MH_LIMBS-1 {
    d[0] = 0; d[1] = 0; d[2] = 0;
    for i: j+1..<MH_LIMBS { mh_muladd3(d[..], a[i], b[MH_LIMBS + j - i]); }
    mh_mulnadd3(c[..], d[..], MH_DIFF);
    for i: 0..<j+1 { mh_muladd3(c[..], a[i], b[j - i]); }
    tmp[j] = mh_extract3(c[..]);
  }
  // Column 47 has no high partner.
  #assert c[2] == 0;
  for i: 0..<MH_LIMBS { mh_muladd3(c[..], a[i], b[MH_LIMBS - 1 - i]); }
  tmp[MH_LIMBS-1] = mh_extract3(c[..]);
  // The second reduction: what carried past limb 47, times 1103717, back in.
  mh_muln2(c[..], MH_DIFF);
  for j: 0..<MH_LIMBS { out[j] = mh_addnextract2(c[..], tmp[j]); }
  #assert c[1] == 0 && (c[0] == 0 || c[0] == 1);
  if mh_overflows(out) { mh_reduce(out); }
  if c[0] != 0 { mh_reduce(out); }
}
// The same on values.
mh_multiply :: (a: Num3072, b: Num3072) -> Num3072 {
  x := a; y := b; r: Num3072;
  mh_mul_into(x.limb[..], y.limb[..], r.limb[..]);
  return r;
}
mh_is_overflow :: (a: Num3072) -> bool { x := a; return mh_overflows(x.limb[..]); }
mh_full_reduce :: (a: Num3072) -> Num3072 { r := a; mh_reduce(r.limb[..]); return r; }
```

The result is below `2^3072` but can still be at least `p`. Core leaves it
so and reduces only when it must: before the inverse and before the final
hash. A number is at least `p` only when every limb is all ones and limb 0
is at least `2^64 - 1103717`; subtracting `p` is adding 1103717 and
dropping the carry out of the top.

<!-- source: coax/lib/bitcoin/muhash.coax reduce -->
```jai
// Is a at least the modulus? Only 1103717 numbers below 2^3072 are: every
// limb all ones except limb 0, which is at least 2^64 - 1103717.
mh_overflows :: (a: [] u64) -> bool {
  if a[0] <= 0xffffffffffffffff - MH_DIFF { return false; }
  for i: 1..<MH_LIMBS { if a[i] != 0xffffffffffffffff { return false; } }
  return true;
}
// a - p in place, for an a at least p: add 1103717 and drop the carry out
// of the top limb, which is 2^3072.
mh_reduce :: (a: [] u64) {
  c := [3] u64.[MH_DIFF, 0, 0];
  for i: 0..<MH_LIMBS { a[i] = mh_addnextract2(c[..], a[i]); }
}
```

### The inverse

Removal needs division, which is multiplication by an inverse. Since `p`
is prime, Fermat's little theorem gives `a^(p-1) = 1`, so `a^(p-2)` is the
inverse of `a`. `p - 2` is 3051 ones followed by a short tail, and a chain
of runs of ones computes it in 3071 squarings and 25 multiplications.
Core used this chain until it switched to a faster variant of the safegcd
algorithm from libsecp256k1, which the pinned `muhash.cpp` has. An inverse
is unique, so both give the same number; the slower chain is shorter to
write and to check, and a node inverts once per hash it reads.

<!-- source: coax/lib/bitcoin/muhash.coax inverse -->
```jai
// a = a^(2^n) * m in place: n squarings, then one multiplication.
mh_square_n_mul :: (a: [] u64, n: int, m: [] u64) {
  for k: 0..<n { mh_mul_into(a, a, a); }
  mh_mul_into(a, m, a);
}
// 1 / a mod p as a^(p - 2) (Fermat). In binary, p - 2 is 3051 ones, then
// 0 1111 00 1 0 1 000 1 00 11 00 1. Write r(k) for a^(2^k - 1), a run of k
// ones: r(2k) = r(k)^(2^k) * r(k), and r(j + k) = r(j)^(2^k) * r(k). The
// table holds r(1), r(2), r(4), ..., r(2048): 2047 squarings. Then r(3051)
// = r(2048 + 512 + 256 + 128 + 64 + 32 + 8 + 2 + 1), and each tail run
// shifts in its zeros and multiplies in its ones. 3071 squarings and 25
// multiplications in all. a = 0 gives 0, which has no inverse.
mh_inverse :: (a: Num3072) -> Num3072 {
  p: [12 * 48] u64;
  for i: 0..<MH_LIMBS { p[i] = a.limb[i]; }
  for i: 0..<11 {
    for k: 0..<MH_LIMBS { p[48*(i+1) + k] = p[48*i + k]; }
    mh_square_n_mul(p[48*(i+1)..<48*(i+2)], 1 << i, p[48*i..<48*(i+1)]);
  }
  r: Num3072;
  for k: 0..<MH_LIMBS { r.limb[k] = p[48*11 + k]; }
  x := r.limb[..];
  mh_square_n_mul(x, 512, p[48*9..<48*10]);
  mh_square_n_mul(x, 256, p[48*8..<48*9]);
  mh_square_n_mul(x, 128, p[48*7..<48*8]);
  mh_square_n_mul(x, 64, p[48*6..<48*7]);
  mh_square_n_mul(x, 32, p[48*5..<48*6]);
  mh_square_n_mul(x, 8, p[48*3..<48*4]);
  mh_square_n_mul(x, 2, p[48*1..<48*2]);
  mh_square_n_mul(x, 1, p[0..<48]);       // r(3051)
  mh_square_n_mul(x, 5, p[48*2..<48*3]);  // 0 1111
  mh_square_n_mul(x, 3, p[0..<48]);       // 00 1
  mh_square_n_mul(x, 2, p[0..<48]);       // 0 1
  mh_square_n_mul(x, 4, p[0..<48]);       // 000 1
  mh_square_n_mul(x, 4, p[48..<96]);      // 00 11
  mh_square_n_mul(x, 3, p[0..<48]);       // 00 1
  return r;
}
```

### The set and its hash

<!-- source: coax/lib/bitcoin/muhash.coax set -->
```jai
// The empty set: 1 / 1.
muhash_new :: () -> MuHash3072 {
  r: MuHash3072;
  r.numerator.limb[0] = 1; r.denominator.limb[0] = 1;
  return r;
}
// Insert multiplies the element into the numerator; Remove multiplies it
// into the denominator. Neither divides, so both cost one multiplication.
muhash_insert_vartime :: (m: MuHash3072, data: [] u8) -> MuHash3072 {
  r := m;
  r.numerator = mh_multiply(r.numerator, mh_element_vartime(data));
  return r;
}
muhash_remove_vartime :: (m: MuHash3072, data: [] u8) -> MuHash3072 {
  r := m;
  r.denominator = mh_multiply(r.denominator, mh_element_vartime(data));
  return r;
}
// Core's operator*= (the union of two disjoint sets) and operator/= (one
// set taken out of another): fractions multiplied and divided.
muhash_combine :: (m: MuHash3072, other: MuHash3072) -> MuHash3072 {
  return MuHash3072.{numerator = mh_multiply(m.numerator, other.numerator),
    denominator = mh_multiply(m.denominator, other.denominator)};
}
muhash_divide :: (m: MuHash3072, other: MuHash3072) -> MuHash3072 {
  return MuHash3072.{numerator = mh_multiply(m.numerator, other.denominator),
    denominator = mh_multiply(m.denominator, other.numerator)};
}
```

The hash of a set is SHA-256 of the 384 little-endian bytes of
`numerator / denominator`, fully reduced. Core prints it reversed, as it
prints every hash.

<!-- source: coax/lib/bitcoin/muhash.coax finalize -->
```jai
// The 32-byte hash, in hash order (Core prints it reversed): numerator /
// denominator, reduced below p, as 384 little-endian bytes, then SHA-256.
// The one inversion happens here. A denominator of 1 needs none, and its
// inverse is 1, so skipping it changes nothing.
muhash_finalize_vartime :: (m: MuHash3072, out: [] u8) -> int {
  if out.count != 32 { return MUHASH_CAPACITY; }
  n := m.numerator; d := m.denominator;
  if mh_is_overflow(n) { n = mh_full_reduce(n); }
  if mh_is_overflow(d) { d = mh_full_reduce(d); }
  one: Num3072; one.limb[0] = 1;
  same := true;
  for i: 0..<MH_LIMBS { if d.limb[i] != one.limb[i] { same = false; } }
  if !same { n = mh_multiply(n, mh_inverse(d)); }
  if mh_is_overflow(n) { n = mh_full_reduce(n); }
  b: [384] u8;
  #assert mh_to_bytes(n, b[..]) == MUHASH_OK;
  #assert sha256_hash_checked(b[..], out, false) == SHA_OK;
  return MUHASH_OK;
}
```

A node that keeps the hash current over the whole chain divides once per
read, not once per spent coin. Without the coin statistics index,
`gettxoutsetinfo` inserts every coin of the set as it is, so its
denominator is 1 and it does not divide at all. With the index
(`-coinstatsindex`), Core keeps one MuHash per block and removes spent
coins as this chapter's spend does.

### Worked example: Core's vector

`crypto_tests.cpp` builds a set from three 32-byte elements: the number 0
inserted, 1 inserted and 2 removed (`FromInt(i)` is 32 bytes, `i` in the
first). The hash is
`10d312b100cbd32ada024a6646e40d3482fcff103668d2625f10002a607d5863` as Core
prints it. Both the library's checks and the example reproduce it, and so
does Core's own Python `muhash.py`, which shares no code with Coax.

## The result codes

| Code | Core's error | Returned by |
| --- | --- | --- |
| `COIN_OK`, `MUHASH_OK` | none | all |
| `COIN_TRUNCATED` | "end of data" | every reader |
| `COIN_VARINT_LARGE` | "ReadVarInt(): size too large" | the VARINT, record and key readers |
| `COIN_CAPACITY` | none: the caller's buffer | the writers and readers |
| `COIN_BAD_AMOUNT` | none: Core's precondition | the record writer |
| `COIN_BAD_HEIGHT` | none: Core asserts | the record writer |
| `COIN_BAD_KEY` | none | the key reader |
| `MUHASH_CAPACITY` | none | finalize, serialize |
| `MUHASH_BAD_STATE` | none | deserialize |

A writer that refuses returns minus its code and leaves the rest of the
caller's buffer alone. A reader's result carries the code, and where
reading stopped when it succeeds.

## The example

The example is [bitcoin/examples/utxo-set.coax](../examples/utxo-set.coax).
It prints one coin with its key, its record and its `TxOutSer`, VARINTs,
compressed amounts and scripts, then both hashes of the two regtest sets
Core's functional tests publish, a spend and its undo, Core's MuHash
vector, and four refusals.

<!-- source: bitcoin/examples/utxo-set.coax example -->
```jai
#load "../../coax/lib/bitcoin/coins.coax";
#load "../../coax/lib/nostr/hex.coax";
#import "Basic";

CODES :: [7] string.["ok", "end of data", "size too large", "capacity", "bad amount", "bad height", "bad key"];
// Bytes as lowercase hex.
show :: (label: string, bytes: [] u8) {
  text: [400] u8;
  r := hex_encode_checked(cast(string) bytes, text[..], false);
  print("%: %\n", label, cast(string) text[0..<r.written]);
}
// A hash as Core prints it: the digest reversed.
show_id :: (label: string, digest: [] u8) {
  shown: [32] u8;
  for i: 0..<32 { shown[i] = digest[31 - i]; }
  show(label, shown[..]);
}
// One vector's txid and script into the caller's buffers; the script.
vector :: (v: UtxoVector, txid: [] u8, script: [] u8) -> [] u8 {
  t := cn_unhex(v.txid, txid);
  return cn_unhex(v.script, script);
}
coin_of :: (v: UtxoVector) -> Coin { return Coin.{value = v.value, height = v.height, coinbase = v.coinbase}; }
// A script and its compressed form.
squeeze :: (label: string, hex: string) {
  s: [80] u8; out: [33] u8;
  script := cn_unhex(hex, s[..]);
  n := coin_compress_script_vartime(script, out[..]);
  if n == 0 { print("%: % bytes, no short form\n", label, script.count); return; }
  print("%: % bytes -> % bytes, kind %\n", label, script.count, n, out[0]);
}

main :: () {
  txid: [32] u8; sb: [40] u8; rec: [64] u8; key: [40] u8; ser: [80] u8; out: [32] u8;

  // One coin: the coinbase of regtest block 100 in rpc_dumptxoutset.py's chain.
  print("A coin\n");
  v := UTXO_DUMP[0];
  for i: 0..<UTXO_DUMP_COUNT { if UTXO_DUMP[i].height == 100 { v = UTXO_DUMP[i]; } }
  script := vector(v, txid[..], sb[..]);
  show_id("  txid", txid[..]);
  print("  output %, height %, coinbase %, value % sat\n", v.n, v.height, v.coinbase, v.value);
  show("  scriptPubKey", script);
  k := coin_key_write_vartime(txid[..], v.n, key[..]);
  show("  chainstate key", key[0..<k]);
  n := coin_write_vartime(coin_of(v), script, rec[..]);
  show("  record", rec[0..<n]);
  print("  % bytes: VARINT(201) = 8049, VARINT(50) = 32, kind 00, the key hash\n", n);
  m := coin_txout_ser_vartime(txid[..], v.n, coin_of(v), script, ser[..]);
  show("  TxOutSer", ser[0..<m]);

  print("VARINT\n");
  for x: [6] u64.[0, 127, 128, 300, 16511, 16512] {
    b: [10] u8;
    w := coin_varint_write_vartime(b[..], 0, x);
    text: [20] u8;
    r := hex_encode_checked(cast(string) b[0..<w], text[..], false);
    print("  %: %\n", x, cast(string) text[0..<r.written]);
  }

  print("Compressed amounts\n");
  for a: [7] u64.[0, 1, 1000000, 100000000, 5000000000, 2100000000000000, 123456789] {
    c := coin_compress_amount(a);
    print("  % sat -> % -> %\n", a, c, coin_decompress_amount(c));
  }

  print("Compressed scripts\n");
  squeeze("  P2PKH", "76a914751e76e8199196d454941c45d1b3a323f1433bd688ac");
  squeeze("  P2SH", "a914b472a266d0bd89c13706a4132ccfb16f7c3b9fcb87");
  squeeze("  P2PK, compressed key", "210279be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798ac");
  squeeze("  P2PK, uncompressed key", CN_G_UNCOMPRESSED_P2PK);
  squeeze("  P2WPKH", "0014751e76e8199196d454941c45d1b3a323f1433bd6");
  squeeze("  P2TR", "51202b6a3e4c9ec15e4d4a38c17d6d8d8a9a7bce3ae1751e76e8199196d454941c45");

  // The two hashes of rpc_dumptxoutset.py's 100 coins.
  print("rpc_dumptxoutset.py: 100 coins\n");
  st: [112] u8;
  #assert utxo_hash_init(st[..]);
  for i: 0..<UTXO_DUMP_COUNT {
    s := vector(UTXO_DUMP[i], txid[..], sb[..]);
    #assert utxo_hash_add_vartime(st[..], txid[..], UTXO_DUMP[i].n, coin_of(UTXO_DUMP[i]), s);
  }
  #assert utxo_hash_final(st[..], out[..]);
  show_id("  hash_serialized_3", out[..]);
  print("  Core's txoutset_hash matches: %\n", mh_shown_is(out[..], UTXO_DUMP_TXOUTSET_HASH));

  // feature_utxo_set_hash.py's 101 coins: both hashes.
  print("feature_utxo_set_hash.py: 101 coins\n");
  #assert utxo_hash_init(st[..]);
  set := muhash_new();
  for i: 0..<UTXO_SET_COUNT {
    s := vector(UTXO_SET[i], txid[..], sb[..]);
    #assert utxo_hash_add_vartime(st[..], txid[..], UTXO_SET[i].n, coin_of(UTXO_SET[i]), s);
    set = utxo_muhash_apply_vartime(set, txid[..], UTXO_SET[i].n, coin_of(UTXO_SET[i]), s, false);
  }
  #assert utxo_hash_final(st[..], out[..]);
  show_id("  hash_serialized_3", out[..]);
  print("  Core's matches: %\n", mh_shown_is(out[..], UTXO_SET_HASH_SERIALIZED));
  #assert muhash_finalize_vartime(set, out[..]) == MUHASH_OK;
  show_id("  muhash", out[..]);
  print("  Core's matches: %\n", mh_shown_is(out[..], UTXO_SET_MUHASH));

  // Spend the self-transfer's output, then add it back.
  print("Spend one coin, then restore it\n");
  last := UTXO_SET[0];
  for i: 0..<UTXO_SET_COUNT { if !UTXO_SET[i].coinbase { last = UTXO_SET[i]; } }
  s := vector(last, txid[..], sb[..]);
  spent := utxo_muhash_apply_vartime(set, txid[..], last.n, coin_of(last), s, true);
  #assert muhash_finalize_vartime(spent, out[..]) == MUHASH_OK;
  show_id("  without it", out[..]);
  back := utxo_muhash_apply_vartime(spent, txid[..], last.n, coin_of(last), s, false);
  #assert muhash_finalize_vartime(back, out[..]) == MUHASH_OK;
  show_id("  with it again", out[..]);
  print("  restored: %\n", mh_shown_is(out[..], UTXO_SET_MUHASH));

  // crypto_tests.cpp: FromInt(0) * FromInt(1) / FromInt(2).
  print("Core's MuHash3072 vector\n");
  one: [32] u8; two: [32] u8; zero: [32] u8; one[0] = 1; two[0] = 2;
  acc := muhash_insert_vartime(muhash_new(), zero[..]);
  acc = muhash_insert_vartime(acc, one[..]);
  acc = muhash_remove_vartime(acc, two[..]);
  #assert muhash_finalize_vartime(acc, out[..]) == MUHASH_OK;
  show_id("  {0} + {1} - {2}", out[..]);

  // Broken records.
  print("Refusals\n");
  n = coin_write_vartime(coin_of(v), script, rec[..]);
  r := coin_read_vartime(rec[0..<n-1], 0, sb[..]);
  print("  the record cut one byte short: %\n", CODES[r.code]);
  big := [6] u8.[0x8e, 0xfe, 0xfe, 0xff, 0x00, 0x00];
  r = coin_read_vartime(big[..], 0, sb[..]);
  print("  a height code of 2^32: %\n", CODES[r.code]);
  print("  a value above 21,000,000 BTC: %\n", CODES[-coin_write_vartime(Coin.{value = 2100000000000001}, script, rec[..])]);
  print("  a key with an index above 2^32 - 1: %\n", coin_key_write_vartime(txid[..], 0x100000000, key[..]));
}
```

It prints:

```text
A coin
  txid: 3fa99e31ecb8219c50d67ac45bdcf3441b0c5f09d58fba1135f6dd6596ebc55b
  output 0, height 100, coinbase true, value 5000000000 sat
  scriptPubKey: 76a9142b4569203694fc997e13f2c0a1383b9e16c77a0d88ac
  chainstate key: 435bc5eb9665ddf63511ba8fd5095f0c1b44f3dc5bc47ad6509c21b8ec319ea93f00
  record: 804932002b4569203694fc997e13f2c0a1383b9e16c77a0d
  24 bytes: VARINT(201) = 8049, VARINT(50) = 32, kind 00, the key hash
  TxOutSer: 5bc5eb9665ddf63511ba8fd5095f0c1b44f3dc5bc47ad6509c21b8ec319ea93f00000000c900000000f2052a010000001976a9142b4569203694fc997e13f2c0a1383b9e16c77a0d88ac
VARINT
  0: 00
  127: 7f
  128: 8000
  300: 812c
  16511: ff7f
  16512: 808000
Compressed amounts
  0 sat -> 0 -> 0
  1 sat -> 1 -> 1
  1000000 sat -> 7 -> 1000000
  100000000 sat -> 9 -> 100000000
  5000000000 sat -> 50 -> 5000000000
  2100000000000000 sat -> 21000000 -> 2100000000000000
  123456789 sat -> 1111111101 -> 123456789
Compressed scripts
  P2PKH: 25 bytes -> 21 bytes, kind 0
  P2SH: 23 bytes -> 21 bytes, kind 1
  P2PK, compressed key: 35 bytes -> 33 bytes, kind 2
  P2PK, uncompressed key: 67 bytes -> 33 bytes, kind 4
  P2WPKH: 22 bytes, no short form
  P2TR: 34 bytes, no short form
rpc_dumptxoutset.py: 100 coins
  hash_serialized_3: 771d773b5c27b6f35f598ce764652a2cf28fbc268341eb1827844e416c629c7d
  Core's txoutset_hash matches: true
feature_utxo_set_hash.py: 101 coins
  hash_serialized_3: 396058cfd7f3b4c05dcdfaf360be9986d859d2c8315082d3aadda6d2d16add25
  Core's matches: true
  muhash: 97f341d4226cb4451dd9da2856759fb97659cfc238a0b8d5452e64af6032bd3d
  Core's matches: true
Spend one coin, then restore it
  without it: f71eabc81159aac9a1b1f759939e3f66acbd6542fcfba12afc659e6301a95658
  with it again: 97f341d4226cb4451dd9da2856759fb97659cfc238a0b8d5452e64af6032bd3d
  restored: true
Core's MuHash3072 vector
  {0} + {1} - {2}: 10d312b100cbd32ada024a6646e40d3482fcff103668d2625f10002a607d5863
Refusals
  the record cut one byte short: end of data
  a height code of 2^32: size too large
  a value above 21,000,000 BTC: bad amount
  a key with an index above 2^32 - 1: -1
```

The book's checks run this program in debug and release builds and
compare the output with
[bitcoin/fixtures/utxo-set.out](../fixtures/utxo-set.out).

### Where the two sets come from

No node runs here, so the coins are rebuilt from what Core's tests say.
`rpc_dumptxoutset.py` mines 100 regtest blocks to one address with mock
time and publishes the `txoutset_hash` of the result and the SHA-256 of
the `dumptxoutset` file. `feature_utxo_set_hash.py` mines with MiniWallet,
sends one transaction, and publishes both `hash_serialized_3` and
`muhash`. [utxo.py](../../coax/tests/bitcoin/utxo.py) rebuilds both chains
from the pinned `miner.cpp`, `wallet.py`, `address.py` and `test_node.py`:
the coinbase layout, the mock clock and each block's time, the nonce
search and the witness commitment. It then checks that the rebuilt chain
gives every value the tests publish, including the 5,788-byte snapshot
file byte for byte, before it writes the coins to
[utxo_vectors.coax](../../coax/tests/bitcoin/utxo_vectors.coax). The Coax
code reproduces the same published values from those coins.

## How the code is checked

### Checks in the example

The checks are in
[bitcoin/examples/checks/utxo-set.coax](../examples/checks/utxo-set.coax),
which loads the example:

<!-- source: bitcoin/examples/checks/utxo-set.coax check -->
```jai
// Every coin of both regtest sets survives a write and a read: the record,
// the key and the TxOutSer all come back to the same coin.
utxo_round_trip_ok :: (v: UtxoVector) -> bool {
  txid: [32] u8; sb: [80] u8; rec: [64] u8; back: [80] u8; key: [40] u8;
  script := vector(v, txid[..], sb[..]);
  c := coin_of(v);
  n := coin_write_vartime(c, script, rec[..]);
  if n <= 0 { return false; }
  r := coin_read_vartime(rec[0..<n], 0, back[..]);
  if r.code != COIN_OK || r.next != n || r.coin.value != c.value { return false; }
  if r.coin.height != c.height || r.coin.coinbase != c.coinbase { return false; }
  if r.script_n != script.count { return false; }
  for i: 0..<script.count { if back[i] != script[i] { return false; } }
  k := coin_key_write_vartime(txid[..], v.n, key[..]);
  kr := coin_key_read_vartime(key[0..<k]);
  if kr.code != COIN_OK || kr.n != v.n { return false; }
  for i: 0..<32 { if key[kr.txid + i] != txid[i] { return false; } }
  return true;
}
#forall "rpc_dumptxoutset.py's coins round-trip" i: 0..<UTXO_DUMP_COUNT #steps 20000000 {
  #assert utxo_round_trip_ok(UTXO_DUMP[i]);
}
#forall "feature_utxo_set_hash.py's coins round-trip" i: 0..<UTXO_SET_COUNT #steps 20000000 {
  #assert utxo_round_trip_ok(UTXO_SET[i]);
}
// MuHash ignores order: inserting the coins backwards gives Core's value.
#test "feature_utxo_set_hash.py's muhash, coins in reverse order" #steps 4000000000 {
  txid: [32] u8; sb: [80] u8; out: [32] u8;
  set := muhash_new();
  for j: 0..<UTXO_SET_COUNT {
    v := UTXO_SET[UTXO_SET_COUNT - 1 - j];
    s := vector(v, txid[..], sb[..]);
    set = utxo_muhash_apply_vartime(set, txid[..], v.n, coin_of(v), s, false);
  }
  #assert muhash_finalize_vartime(set, out[..]) == MUHASH_OK;
  #assert mh_shown_is(out[..], UTXO_SET_MUHASH);
}
// hash_serialized_3 depends on order: swapping the first two coins changes it.
#test "hash_serialized_3 depends on key order" #steps 400000000 {
  txid: [32] u8; sb: [80] u8; out: [32] u8; st: [112] u8;
  #assert utxo_hash_init(st[..]);
  for j: 0..<UTXO_DUMP_COUNT {
    i := ifx j == 0 then 1 else ifx j == 1 then 0 else j;
    s := vector(UTXO_DUMP[i], txid[..], sb[..]);
    #assert utxo_hash_add_vartime(st[..], txid[..], UTXO_DUMP[i].n, coin_of(UTXO_DUMP[i]), s);
  }
  #assert utxo_hash_final(st[..], out[..]);
  #assert !mh_shown_is(out[..], UTXO_DUMP_TXOUTSET_HASH);
}
```

```shell
coax/bin/coax test bitcoin/examples/checks/utxo-set.coax
```

On 2026-11-03 `coax test` runs 153 checks, including those of the files it
loads. Mutation testing: not run (owner speed order).

### The library's own checks

`muhash.coax` and `coins.coax` are `#authority` files. Their checks cover:

- one column step, small products and products that wrap past `p`, and
  exactly which numbers overflow;
- the inverse of 1, of 2 and of `p - 1`, each multiplied back to one;
- an element as SHA-256 then ChaCha20's keystream, and Core's
  `crypto_tests.cpp` vectors: the `{0} + {1} - {2}` hash, by fractions and
  by insert and remove, the serialized state, and the overflow vector;
- the empty set, insertion in either order, and the byte interface with
  its refusals;
- VARINT: `serialize.h`'s examples, every byte-length boundary, and the
  refusals of a value too large and of the end of the data;
- `compress_tests.cpp`: Core's pairs, every amount from 1 to 10,000 also
  as cents and as whole bitcoin, and every compressed value below 20,000;
- the six script forms, the near misses that keep no short form, and the
  `x` with no point;
- the coin record of a regtest coinbase, a raw script, an empty one and
  one over 10,000 bytes, every cut of a record, and each refusal;
- the key, its order and `TxOutSer`; both hashes of both regtest sets,
  the `dumptxoutset` file's SHA-256 and a spend; and the C interface.

```shell
coax/bin/coax test coax/lib/bitcoin/muhash.coax
coax/bin/coax test coax/lib/bitcoin/coins.coax
```

On 2026-11-03 `muhash.coax` runs 58 checks and `coins.coax` 146,
including the files they load. The inverse dominates: each inverse check
takes 10 to 30 seconds in the checker. Mutation testing: not run (owner
speed order).

### Core's own Python, natively

[utxo.py](../../coax/tests/bitcoin/utxo.py) holds a transliteration of
`compressor.cpp`, `serialize.h`'s VARINT, `coinstats.cpp`'s `TxOutSer`
and the coin record, and uses Core's own `test_framework/crypto/muhash.py`
and `chacha20.py` from the pins for MuHash. It compiles `coins.coax` to C
in checked and release builds and compares them through ctypes. In each
build:

- MuHash: Core's `{0} + {1} - {2}` set, the empty set and 24 seeded sets
  of insertions and removals, half of them with an element added and
  removed again. The hash and both halves of the state must equal
  `muhash.py`'s;
- amounts: 320,002 cases: every amount up to 100,000 satoshi, 10,000
  multiples of a cent and of a bitcoin, multiples of 50 bitcoin up to
  `MAX_MONEY`, random amounts, and decompression of 120,001 numbers up to
  `2^64 - 1`;
- VARINTs: 12,026 writes and reads into both `uint32_t` and `uint64_t`,
  including 3,000 seeded byte strings, each with ReadVarInt's outcome;
- scripts and records: 485 scripts of every kind, near misses and random
  bytes, each compressed, written with a seeded coin and read back at
  several cuts;
- corrupted records: 1,500 seeded edits of real records and a kind 4 with
  no point, each giving Core's error or Core's coin;
- the 201 coins of the two rebuilt chains, with both hashes and the
  `dumptxoutset` file built from Coax's records.

Every output buffer has a guard byte past its end that must stay
untouched.

```shell
COAX_TEST_COMPILER=coax/bin/coax python3 coax/tests/bitcoin/utxo.py
```

On 2026-11-03 all five tests pass, with 668,482 cases across the two
builds, in about 90 seconds.

## What this chapter leaves out

- The chainstate database itself: chapter 62 builds the store, and
  Core's obfuscation key is explained there.
- The coins cache, its flushes and crash safety: chapter 63.
- Connecting and disconnecting blocks, and the undo data that restores
  spent coins: chapter 58.
- Loading a `dumptxoutset` snapshot and assumeutxo: the node chapters in
  Part X. The file's format is used here only to check the rebuilt coins.
- The constant-time MuHash. Every element here is public, so the
  variable-time arithmetic is enough.
- The `bogosize` and `total_amount` fields of `gettxoutsetinfo`, which
  are counts, not hashes.

## Summary

- A coin is an outpoint, an amount, a script, a height and a coinbase
  flag. The chainstate key is `'C'`, the txid and a VARINT of the index.
- Core's record is a VARINT of `height * 2 + coinbase`, a compressed
  amount and a compressed script: 24 bytes for a P2PKH coinbase.
- `hash_serialized_3` is SHA-256d over every `TxOutSer` in key order.
  `muhash` is the product of each `TxOutSer`'s number modulo
  `2^3072 - 1103717`, hashed with SHA-256.
- MuHash updates in one multiplication per coin, in any order. Removal
  multiplies the denominator, and the single inverse waits until the hash
  is read.
- Both hashes of both sets match the values Core's tests publish, which
  is the comparison chapter 80's node will make against a running Core.

## Dependencies

- [2. Big integers in fixed limbs](ch002-big-integers-in-fixed-limbs.md)
- [4. SHA-256](ch004-sha-256.md)
- [11. ChaCha20](ch011-chacha20.md)
- [17. Point encodings: compressed, uncompressed and x-only](ch017-point-encodings-compressed-uncompressed-and-x-only.md)
- [34. Transactions and their serialization](ch034-transactions-and-their-serialization.md)

## Sources

- Bitcoin Core at revision `ddea9196`: `crypto/muhash.cpp`,
  `crypto/muhash.h`, `test/crypto_tests.cpp`, `compressor.cpp`,
  `compressor.h`, `test/compress_tests.cpp`, `coins.h`,
  `kernel/coinstats.cpp`, `txdb.cpp`, `node/utxo_snapshot.h`,
  `node/miner.cpp` and `serialize.h`, and the functional tests
  `feature_utxo_set_hash.py` and `rpc_dumptxoutset.py` with
  `test_framework/crypto/muhash.py`, `test_framework/crypto/chacha20.py`,
  `test_framework/wallet.py`, `test_framework/address.py` and
  `test_framework/test_node.py`, pinned in
  [coax/tests/vectors/upstream/bitcoin-core/](../../coax/tests/vectors/upstream/bitcoin-core/muhash.cpp).
- Mihir Bellare and Daniele Micciancio, "A New Paradigm for
  Collision-free Hashing: Incrementality at Reduced Cost", Eurocrypt 1997.
- All are listed in [SOURCES.md](../../coax/tests/vectors/SOURCES.md).
