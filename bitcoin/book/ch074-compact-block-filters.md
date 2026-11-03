# 74. Compact block filters

A light client holds a few keys and wants the transactions that pay to
them or spend from them. It can ask a full node to filter blocks for it,
as BIP-37 bloom filters did, but then the node learns which scripts the
client cares about. BIP-157 and BIP-158 turn this around: the node
publishes one small filter per block, the same for every client, and the
client tests its own scripts against it. On a match it downloads the
whole block. The node learns nothing beyond which blocks the client
fetched.

This chapter builds the filter that BIP-158 calls basic, which Bitcoin
Core serves with `-blockfilterindex` and `-peerblockfilters`, and the
filter headers of BIP-157 that chain the filters together.

## What this chapter teaches

- A Golomb-coded set (GCS): a probabilistic set that hashes each element
  to a number below N * M, sorts the numbers and stores their
  differences with Golomb-Rice coding.
- Why the hash is SipHash-2-4 keyed by the block hash, and how a
  multiply maps a 64-bit hash into a range without division.
- Golomb-Rice coding with P = 19, the parameter that goes with
  M = 784,931.
- Which scripts the basic filter holds, and why it needs data that is not
  in the block: the scripts the block's inputs spend.
- Filter headers: a hash chain over the filters, so that a client can
  check filters from one peer against headers from another.
- Five result codes, and Core's strict decode check.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [blockfilter.coax](../../coax/lib/bitcoin/blockfilter.coax) | `bf_key`, `bf_range`, `bf_hash_vartime` | the SipHash key and the range map |
| | `bf_sort` | heapsort of the hashes, with their element indices |
| | `bf_put_bits`, `bf_get_bit`, `bf_golomb_put`, `bf_golomb_get` | the bit stream and Golomb-Rice codes |
| | `bf_build_vartime` | a GCS filter of a set of elements |
| | `bf_read_vartime`, `bf_match_any_vartime`, `bf_match_vartime` | decoding and matching |
| | `bf_gather_vartime`, `bf_block_hash` | a block's basic-filter elements |
| | `bf_filter_header` | BIP-157's filter header |
| | `bf_basic_vartime` | the whole pipeline for one block |
| [siphash.coax](../../coax/lib/crypto/siphash.coax) | `siphash24` | SipHash-2-4 (chapter 10) |
| [tx.coax](../../coax/lib/bitcoin/tx.coax) | `tx_parse_vartime`, `tx_compact_read_vartime` | reading the block's transactions |

Every input is public chain data, so everything runs in variable time.

<!-- source: coax/lib/bitcoin/blockfilter.coax codes -->
```jai
BF_OK :: 0;
BF_CAPACITY :: 1;    // a caller's buffer is too small
BF_BAD_BLOCK :: 2;   // the block's bytes do not read
BF_BAD_PREVOUTS :: 3; // the spent scripts do not read, or are not one per input
BF_BAD_FILTER :: 4;  // the filter's bytes are not N Golomb-Rice codes, exactly

BF_P :: 19;                       // Golomb-Rice parameter
BF_M :: cast(u64) 784931;         // 1 / false-positive rate
BF_HEADER :: 80;
BF_LOW :: cast(u64) 0x7ffff;         // the low P bits
```

<!-- source: coax/lib/bitcoin/blockfilter.coax types -->
```jai
// The SipHash key: the first 16 bytes of the block hash, as two
// little-endian words.
BfKey :: struct { k0: u64; k1: u64; }
// One element: n bytes at at, in a pool of element bytes.
BfSpan :: struct { at: int; n: int; }
// A filter built: its length in the output and its element count N.
BfFilter :: struct { code: int; size: int; n: int; }
// Elements gathered from a block: how many, and how much of the pool.
BfGather :: struct { code: int; n: int; used: int; at: int; }
// A filter read: N, and where the Golomb-Rice bits start.
BfRead :: struct { code: int; n: int; bits: int; }
// A match: whether any query is in the set.
BfMatch :: struct { code: int; found: bool; }
```

## Hashing into a range

A GCS with N elements and false-positive rate 1/M hashes each element
to a number in [0, N * M). Two different elements collide with
probability 1/(N * M), so a query that is not in the set matches one of
the N values with probability about N/(N * M) = 1/M.

The hash is SipHash-2-4 (chapter 10). Its 128-bit key is the first 16
bytes of the block hash, in the order SHA-256d produced them, read as two
little-endian words. A different key per block means an attacker cannot
pick scripts that collide in every filter.

SipHash gives 64 bits. To map them onto [0, F) Core uses `FastRange64`:
multiply by F as a 128-bit product and keep the high 64 bits. That is
`floor(h * F / 2^64)`, which spreads [0, 2^64) evenly over [0, F) with no
division and no bias worth counting.

<!-- source: coax/lib/bitcoin/blockfilter.coax hash -->
```jai
// The key from a block hash in hash order (the bytes SHA-256d produced).
bf_key :: (block_hash: [] u8) -> BfKey {
  return BfKey.{k0 = cb_load_le64(block_hash, 0).value, k1 = cb_load_le64(block_hash, 8).value};
}
// HashToRange: SipHash-2-4, then a 64x64 multiply keeping the high word,
// which maps [0, 2^64) onto [0, f) without a division (FastRange64).
bf_range :: (h: u64, f: u64) -> u64 { return mul_wide(h, f).hi; }
bf_hash_vartime :: (key: BfKey, element: [] u8, f: u64) -> u64 {
  return bf_range(siphash24(key.k0, key.k1, element), f);
}
```

## Sorting

The filter stores the values in increasing order, so the builder sorts
them. Heapsort needs no extra memory and no recursion. It carries an
index array along so that each sorted value still knows which element it
came from: the builder needs that to drop duplicates.

<!-- source: coax/lib/bitcoin/blockfilter.coax sort -->
```jai
// Heapsort of values, carrying idx along: n log n with no extra memory.
bf_sift :: (v: [] u64, idx: [] int, start: int, end: int) {
  root := start;
  while 2 * root + 1 < end {
    child := 2 * root + 1;
    if child + 1 < end && v[child] < v[child+1] { child += 1; }
    if v[root] >= v[child] { return; }
    t := v[root]; v[root] = v[child]; v[child] = t;
    j := idx[root]; idx[root] = idx[child]; idx[child] = j;
    root = child;
  }
}
bf_sort :: (v: [] u64, idx: [] int, n: int) {
  s := n / 2 - 1;
  while s >= 0 { bf_sift(v, idx, s, n); s -= 1; }
  e := n - 1;
  while e > 0 {
    t := v[0]; v[0] = v[e]; v[e] = t;
    j := idx[0]; idx[0] = idx[e]; idx[e] = j;
    bf_sift(v, idx, 0, e);
    e -= 1;
  }
}
```

## Golomb-Rice coding

Sorted values below N * M are about M apart on average. Their
differences are small numbers with a geometric distribution, and
Golomb-Rice coding is close to optimal for those. Each difference x is
written as its quotient `x >> P` in unary (that many 1 bits and a 0),
then its low P bits as they are. BIP-158 picks P = 19 and
M = 784,931, about 1.497 * 2^19: for a false-positive rate near 2^-19,
that M gives the smallest filters with P = 19.

A difference below 2^19 takes 20 bits; one between 2^19 and 2^20 takes
21. On average a code takes about 21 bits.

Bits go most significant first, and the last byte is padded with zeros.

<!-- source: coax/lib/bitcoin/blockfilter.coax bits -->
```jai
// Bits are written and read most significant first, as Core's
// BitStreamWriter and BitStreamReader do; the last byte is padded with
// zeros. pos counts bits from the start of out.
bf_put_bits :: (out: [] u8, pos: int, value: u64, nbits: int) -> int {
  for i: 0..<nbits {
    bit := (value >> cast(u64) (nbits - 1 - i)) & 1;
    if bit == 1 { out[(pos+i)/8] |= cast(u8) (0x80 >> ((pos + i) % 8)); }
  }
  return pos + nbits;
}
bf_get_bit :: (b: [] u8, pos: int) -> int { return cast(int) ((b[pos/8] >> cast(u8) (7 - pos % 8)) & 1); }
// GolombRiceEncode: the quotient x >> P in unary (that many ones, then a
// zero), then the low P bits. Returns the new bit position, or -1 when out
// is too small.
bf_golomb_put :: (out: [] u8, pos: int, x: u64) -> int {
  q := cast(int) (x >> cast(u64) BF_P);
  if pos + q + 1 + BF_P > out.count * 8 { return -1; }
  p := pos;
  for i: 0..<q { p = bf_put_bits(out, p, 1, 1); }
  p = bf_put_bits(out, p, 0, 1);
  return bf_put_bits(out, p, x & BF_LOW, BF_P);
}
// GolombRiceDecode at bit pos of b: the value, and where the next code
// starts (-1 when the bits run out first, Core's "end of data").
BfCode :: struct { value: u64; next: int; }
bf_golomb_get :: (b: [] u8, pos: int) -> BfCode {
  limit := b.count * 8; p := pos; q := cast(u64) 0;
  while true {
    if p >= limit { return BfCode.{next = -1}; }
    if bf_get_bit(b, p) == 0 { break; }
    q += 1; p += 1;
  }
  p += 1;
  if p + BF_P > limit { return BfCode.{next = -1}; }
  r := cast(u64) 0;
  for i: 0..<BF_P { r = (r << 1) | cast(u64) bf_get_bit(b, p + i); }
  return BfCode.{value = add_wrap(mul_wrap(q, BF_LOW + 1), r), next = p + BF_P};
}
```

## Building a filter

`bf_build_vartime` takes the elements as spans of a byte pool. Core keeps
them in a `std::unordered_set`, so an element that appears twice in a
block counts once. Here duplicates are found after sorting by the raw
SipHash: equal bytes give equal hashes, so equal elements end up next to
each other, and the range map keeps the order. Only elements with equal
hashes are compared byte by byte.

The filter is CompactSize(N) followed by the codes. N counts distinct
elements, and the range is N * M, so the builder learns N before it maps
any value.

<!-- source: coax/lib/bitcoin/blockfilter.coax build -->
```jai
// A GCS filter of the elements spans[0..<n] (bytes in pool). Equal
// elements count once, as Core's ElementSet does. values and idx are
// scratch, n long. Writes CompactSize(N), then the coded differences of
// the sorted hashes.
//
// SipHash comes first and the range second: elements with the same bytes
// have the same 64-bit hash, so sorting by it puts duplicates side by side,
// and the range map keeps the order, so the sorted hashes give the sorted
// values once N is known.
bf_build_vartime :: (key: BfKey, pool: [] u8, spans: [] BfSpan, n: int, values: [] u64, idx: [] int, out: [] u8) -> BfFilter {
  if values.count < n || idx.count < n || spans.count < n { return BfFilter.{code = BF_CAPACITY}; }
  for i: 0..<n {
    values[i] = siphash24(key.k0, key.k1, pool[spans[i].at..<spans[i].at+spans[i].n]);
    idx[i] = i;
  }
  bf_sort(values, idx, n);
  // Keep one of each run of equal elements.
  unique := 0;
  for i: 0..<n {
    keep := true;
    j := unique - 1;
    while keep && j >= 0 && values[j] == values[i] {
      if bf_same(pool, spans[idx[j]], spans[idx[i]]) { keep = false; }
      j -= 1;
    }
    if keep { values[unique] = values[i]; idx[unique] = idx[i]; unique += 1; }
  }
  head := tx_compact_size_vartime(unique);
  if out.count < head { return BfFilter.{code = BF_CAPACITY}; }
  for i: 0..<out.count { out[i] = 0; }
  #assert tx_compact_write_vartime(out, 0, unique) == head;
  f := cast(u64) unique * BF_M;
  pos := head * 8; last := cast(u64) 0;
  for i: 0..<unique {
    v := bf_range(values[i], f);
    pos = bf_golomb_put(out, pos, v - last);
    if pos < 0 { return BfFilter.{code = BF_CAPACITY}; }
    last = v;
  }
  return BfFilter.{size = (pos + 7) / 8, n = unique};
}
bf_same :: (pool: [] u8, a: BfSpan, b: BfSpan) -> bool {
  if a.n != b.n { return false; }
  for i: 0..<a.n { if pool[a.at+i] != pool[b.at+i] { return false; } }
  return true;
}
```

## Reading and matching

Core's `GCSFilter` constructor checks every filter it is given: N must be
below 2^32, the stream must hold exactly N codes, and no byte may follow
the one the last code ends in. `bf_read_vartime` does the same, so a
filter with a missing or an extra byte is refused, not half-read.

A match hashes the queries with the filter's N, sorts them, and walks the
filter's values and the queries together, as Core's `MatchInternal`
does. One pass answers any number of queries: a wallet tests all its
scripts against a filter at once.

<!-- source: coax/lib/bitcoin/blockfilter.coax read -->
```jai
// Core's GCSFilter from encoded bytes, with its decode check: N below 2^32
// and exactly N codes, with nothing after the byte the last one ends in.
bf_read_vartime :: (filter: [] u8) -> BfRead {
  c := tx_compact_read_vartime(filter, 0);
  if c.code != TX_OK || c.value > 0xffffffff { return BfRead.{code = BF_BAD_FILTER}; }
  pos := c.next * 8;
  for i: 0..<c.value {
    g := bf_golomb_get(filter, pos);
    if g.next < 0 { return BfRead.{code = BF_BAD_FILTER}; }
    pos = g.next;
  }
  if (pos + 7) / 8 != filter.count { return BfRead.{code = BF_BAD_FILTER}; }
  return BfRead.{n = c.value, bits = c.next * 8};
}
// MatchAny: is any of the n query elements in the filter? The queries are
// hashed into values (scratch, n long) and sorted, then one pass over the
// filter walks both lists in order, as Core's MatchInternal does.
bf_match_any_vartime :: (key: BfKey, filter: [] u8, pool: [] u8, spans: [] BfSpan, n: int, values: [] u64, idx: [] int) -> BfMatch {
  r := bf_read_vartime(filter);
  if r.code != BF_OK { return BfMatch.{code = r.code}; }
  if values.count < n || idx.count < n || spans.count < n { return BfMatch.{code = BF_CAPACITY}; }
  f := cast(u64) r.n * BF_M;
  for i: 0..<n { values[i] = bf_hash_vartime(key, pool[spans[i].at..<spans[i].at+spans[i].n], f); idx[i] = i; }
  bf_sort(values, idx, n);
  pos := r.bits; value := cast(u64) 0; q := 0;
  for i: 0..<r.n {
    g := bf_golomb_get(filter, pos);
    pos = g.next; value = add_wrap(value, g.value);
    while q < n && values[q] < value { q += 1; }
    if q == n { return BfMatch.{}; }
    if values[q] == value { return BfMatch.{found = true}; }
  }
  return BfMatch.{};
}
// Match: one element.
bf_match_vartime :: (key: BfKey, filter: [] u8, element: [] u8) -> BfMatch {
  spans: [1] BfSpan; values: [1] u64; idx: [1] int;
  spans[0] = BfSpan.{at = 0, n = element.count};
  return bf_match_any_vartime(key, filter, element, spans[..], 1, values[..], idx[..]);
}
```

## The basic filter's elements

BIP-158's basic filter holds two kinds of script:

- Every output script of every transaction, except empty scripts and
  scripts that start with `OP_RETURN` (0x6a). Those can never be spent,
  so no wallet watches them.
- Every script that an input of the block spends, except empty ones.
  These are not in the block. A full node has them in its undo data,
  written when it connected the block (chapter 56's coins are where they
  came from).

The second kind is what lets a wallet find its spends by script alone,
with no outpoints to remember. It also means that only a node with the
block's undo data can build the filter. Here the caller passes the spent
scripts as `prevouts`: one CompactSize-prefixed script per non-coinbase
input, in block order. If their number does not match the block's inputs,
the result is `BF_BAD_PREVOUTS`.

<!-- source: coax/lib/bitcoin/blockfilter.coax basic -->
```jai
// The basic filter's elements (BIP-158, Core's BasicFilterElements): every
// output script that is not empty and does not start with OP_RETURN, then
// every script the block's inputs spend that is not empty. The spent
// scripts are not in the block: they come from the node's undo data, here
// as prevouts, one CompactSize-prefixed script per input after the
// coinbase, in block order. Each element is copied into pool.
bf_gather_vartime :: (block: [] u8, prevouts: [] u8, ins: [] TxIn, outs: [] TxOut, items: [] TxItem, pool: [] u8, spans: [] BfSpan) -> BfGather {
  if block.count < BF_HEADER { return BfGather.{code = BF_BAD_BLOCK}; }
  c := tx_compact_read_vartime(block, BF_HEADER);
  if c.code != TX_OK { return BfGather.{code = BF_BAD_BLOCK, at = c.next}; }
  g := BfGather.{};
  at := c.next; inputs := 0;
  for t: 0..<c.value {
    tx := tx_parse_vartime(block[at..<block.count], true, ins, outs, items);
    if tx.code != TX_OK {
      g.code = ifx tx.code == TX_CAPACITY then BF_CAPACITY else BF_BAD_BLOCK; g.at = at + tx.at;
      return g;
    }
    if t > 0 { inputs += tx.nin; }
    for o: 0..<tx.nout {
      s := block[at+outs[o].script..<at+outs[o].script+outs[o].script_n];
      if s.count > 0 && s[0] != 0x6a {
        g = bf_add(s, pool, spans, g);
        if g.code != BF_OK { return g; }
      }
    }
    at += tx.total;
  }
  if at != block.count { return BfGather.{code = BF_BAD_BLOCK, at = at}; }
  p := 0;
  for i: 0..<inputs {
    s := tx_compact_read_vartime(prevouts, p);
    if s.code != TX_OK || s.value > prevouts.count - s.next { return BfGather.{code = BF_BAD_PREVOUTS, at = p}; }
    script := prevouts[s.next..<s.next+s.value];
    p = s.next + s.value;
    if script.count > 0 {
      g = bf_add(script, pool, spans, g);
      if g.code != BF_OK { return g; }
    }
  }
  if p != prevouts.count { return BfGather.{code = BF_BAD_PREVOUTS, at = p}; }
  return g;
}
// One element into the pool and the span list.
bf_add :: (s: [] u8, pool: [] u8, spans: [] BfSpan, g: BfGather) -> BfGather {
  if g.n >= spans.count || s.count > pool.count - g.used { return BfGather.{code = BF_CAPACITY, n = g.n, used = g.used}; }
  for i: 0..<s.count { pool[g.used+i] = s[i]; }
  spans[g.n] = BfSpan.{at = g.used, n = s.count};
  return BfGather.{n = g.n + 1, used = g.used + s.count};
}
// The block hash: SHA-256d of the 80-byte header.
bf_block_hash :: (block: [] u8, out: [] u8) {
  #assert sha256d_checked(block[0..<BF_HEADER], out, false) == SHA_OK;
}
```

## Filter headers

A client that downloads filters from one peer cannot tell whether that
peer left a script out. BIP-157 gives each filter a header, the SHA-256d
of the filter's own SHA-256d and the previous block's filter header. A
client asks several peers for the headers, which are small. Where the
peers agree, it accepts the filters that hash to them. Where they
disagree, it downloads the block, builds the filter itself and finds out
which peer lied.

The genesis block's previous filter header is 32 zero bytes.

<!-- source: coax/lib/bitcoin/blockfilter.coax header -->
```jai
// BIP-157's filter header: SHA-256d of the filter's hash and the previous
// filter header, both in hash order. The genesis block's previous header
// is 32 zero bytes.
bf_filter_header :: (filter: [] u8, prev: [] u8, out: [] u8) {
  pair: [64] u8;
  #assert sha256d_checked(filter, pair[0..<32], false) == SHA_OK;
  for i: 0..<32 { pair[32+i] = prev[i]; }
  #assert sha256d_checked(pair[..], out, false) == SHA_OK;
}
```

`bf_basic_vartime` puts the steps together: gather the elements, hash
the header to get the key, build the filter and chain its header.

<!-- source: coax/lib/bitcoin/blockfilter.coax filter -->
```jai
// The basic filter of a block and its filter header, from the block, its
// spent scripts and the previous filter header. The arrays are the
// caller's scratch: ins, outs and items for one transaction at a time, and
// pool, spans, values and idx for the block's elements.
bf_basic_vartime :: (block: [] u8, prevouts: [] u8, prev_header: [] u8, ins: [] TxIn, outs: [] TxOut, items: [] TxItem,
                     pool: [] u8, spans: [] BfSpan, values: [] u64, idx: [] int, out: [] u8, header_out: [] u8) -> BfFilter {
  if prev_header.count != 32 || header_out.count != 32 { return BfFilter.{code = BF_CAPACITY}; }
  g := bf_gather_vartime(block, prevouts, ins, outs, items, pool, spans);
  if g.code != BF_OK { return BfFilter.{code = g.code}; }
  hash: [32] u8;
  bf_block_hash(block, hash[..]);
  f := bf_build_vartime(bf_key(hash[..]), pool, spans, g.n, values, idx, out);
  if f.code != BF_OK { return f; }
  bf_filter_header(out[0..<f.size], prev_header, header_out);
  return f;
}
```

## The result codes

| Code | Meaning | Returned by |
| --- | --- | --- |
| `BF_OK` | done | all |
| `BF_CAPACITY` | a caller's buffer is too small | build, match, gather, the pipeline |
| `BF_BAD_BLOCK` | the block's bytes do not read as a header and transactions | gather, the pipeline |
| `BF_BAD_PREVOUTS` | the spent scripts do not read, or are not one per input | gather, the pipeline |
| `BF_BAD_FILTER` | the filter is not exactly N Golomb-Rice codes | read, match |

## The example

The example is
[bitcoin/examples/block-filters.coax](../examples/block-filters.coax).
It builds a small set, then the basic filter of testnet block 49,291 from
BIP-158's test vectors, matches scripts against it, shows the decode
check, and chains the genesis filter header.

<!-- source: bitcoin/examples/block-filters.coax example -->
```jai
#load "../../coax/lib/bitcoin/blockfilter.coax";
#load "../../coax/lib/nostr/hex.coax";
#import "Basic";

CODES :: [5] string.["ok", "buffer too small", "bad block", "bad spent scripts", "bad filter"];
// Bytes as lowercase hex.
show :: (label: string, bytes: [] u8) {
  text: [600] u8;
  r := hex_encode_checked(cast(string) bytes, text[..], false);
  print("%: %\n", label, cast(string) text[0..<r.written]);
}
// A hash as Core prints it: the bytes reversed.
show_hash :: (label: string, h: [] u8) {
  r: [32] u8;
  for i: 0..<32 { r[i] = h[31-i]; }
  show(label, r[..]);
}

main :: () {
  // A set of three scripts, keyed by a made-up block hash.
  hash: [32] u8;
  for i: 0..<32 { hash[i] = cast(u8) i; }
  key := bf_key(hash[..]);
  pool := [9] u8.[0x51, 0x52, 0x53, 0x00, 0x14, 0xaa, 0xbb, 0xcc, 0xdd];
  spans := [4] BfSpan.[BfSpan.{at = 0, n = 1}, BfSpan.{at = 1, n = 2}, BfSpan.{at = 3, n = 6}, BfSpan.{at = 0, n = 1}];
  values: [64] u64; idx: [64] int; out: [256] u8;
  f := bf_build_vartime(key, pool[..], spans[..], 4, values[..], idx[..], out[..]);
  print("four elements, one repeated: %, N = %, % bytes\n", CODES[f.code], f.n, f.size);
  show("  filter", out[0..<f.size]);
  for s: spans[0..<3] {
    print("  match %: %\n", s.n, bf_match_vartime(key, out[0..<f.size], pool[s.at..<s.at+s.n]).found);
  }
  print("  match 0x5253 under another key: %\n", bf_match_vartime(BfKey.{k0 = 1}, out[0..<f.size], pool[1..<3]).found);

  // A real testnet block: height 49291, from BIP-158's test vectors. The
  // spent scripts come from the node's undo data for the block.
  v := BF_VECTORS[4];
  block: [2048] u8; prev: [1024] u8; ph: [32] u8; shown: [32] u8; header: [32] u8;
  ins: [16] TxIn; outs: [32] TxOut; items: [32] TxItem; bspans: [64] BfSpan; bpool: [2048] u8;
  b := tx_unhex(v.block, block[..]);
  p := tx_unhex(v.prevouts, prev[..]);
  s := tx_unhex(v.prev_header, shown[..]);
  for i: 0..<32 { ph[i] = s[31-i]; }
  bf_block_hash(b, hash[..]);
  print("block %: % bytes\n", v.height, b.count);
  show_hash("  hash", hash[..]);
  g := bf_gather_vartime(b, p, ins[..], outs[..], items[..], bpool[..], bspans[..]);
  print("  elements: % scripts, % bytes\n", g.n, g.used);
  f = bf_basic_vartime(b, p, ph[..], ins[..], outs[..], items[..], bpool[..], bspans[..], values[..], idx[..], out[..], header[..]);
  print("  basic filter: %, N = %, % bytes\n", CODES[f.code], f.n, f.size);
  show("  filter", out[0..<f.size]);
  show_hash("  filter header", header[..]);
  print("  same as BIP-158: filter %, header %\n", tx_hex_is(out[0..<f.size], v.filter), bf_shown_is(header[..], v.header));

  // A wallet with one of the block's spent scripts finds it; one with
  // another script does not.
  key = bf_key(hash[..]);
  mine := [3] u8.[0x51, 0x52, 0x53];
  print("  match a spent script: %\n", bf_match_vartime(key, out[0..<f.size], p[1..<1+cast(int) p[0]]).found);
  print("  match OP_1 OP_2 OP_3: %\n", bf_match_vartime(key, out[0..<f.size], mine[..]).found);

  // Decoding is strict: a missing byte or an extra one is refused.
  r := bf_read_vartime(out[0..<f.size]);
  print("read: %, % codes from bit %\n", CODES[r.code], r.n, r.bits);
  print("  one byte short: %\n", CODES[bf_read_vartime(out[0..<f.size-1]).code]);
  print("  one byte extra: %\n", CODES[bf_read_vartime(out[0..<f.size+1]).code]);

  // The chain of headers starts from 32 zero bytes at genesis.
  zero: [32] u8; gf: [16] u8;
  v = BF_VECTORS[0];
  filter := tx_unhex(v.filter, gf[..]);
  bf_filter_header(filter, zero[..], header[..]);
  show("genesis filter", filter);
  show_hash("  genesis filter header", header[..]);
}
```

It prints:

```text
four elements, one repeated: ok, N = 3, 9 bytes
  filter: 036c1748787d437470
  match 1: true
  match 2: true
  match 6: true
  match 0x5253 under another key: false
block 49291: 1597 bytes
  hash: 0000000018b07dca1b28b4b5a119f6d6e71698ce1ed96f143f54179ce177a19c
  elements: 10 scripts, 502 bytes
  basic filter: ok, N = 10, 28 bytes
  filter: 0afbc2920af1b027f31f87b592276eb4c32094bb4d3697021b4c6380
  filter header: b6d98692cec5145f67585f3434ec3c2b3030182e1cb3ec58b855c5c164dfaaa3
  same as BIP-158: filter true, header true
  match a spent script: true
  match OP_1 OP_2 OP_3: false
read: ok, 10 codes from bit 8
  one byte short: bad filter
  one byte extra: bad filter
genesis filter: 019dfca8
  genesis filter header: 21584579b7eb08997773e5aeff3a7f932700042d0ed2a6129012b7d7ae81b750
```

The small set's filter is 9 bytes: one for N = 3 and three codes of
about 21 bits each. Block 49,291's ten scripts take 28 bytes, where the
scripts themselves take 502. The genesis filter holds one script, the
coinbase output, in 4 bytes.

The book's checks run this program in debug and release builds and
compare the output with
[bitcoin/fixtures/block-filters.out](../fixtures/block-filters.out).

## How the code is checked

### Checks in the example

The checks are in
[bitcoin/examples/checks/block-filters.coax](../examples/checks/block-filters.coax),
which loads the example:

<!-- source: bitcoin/examples/checks/block-filters.coax check -->
```jai
// Every element of each BIP-158 block is in that block's filter: a filter
// has no false negatives. And every cut of the filter is refused.
#forall "each block's elements all match its filter" i: 0..<BF_VECTORS_COUNT #steps 4000000000 {
  v := BF_VECTORS[i];
  block: [2048] u8; prev: [1024] u8; pool: [2048] u8; hash: [32] u8; fb: [64] u8;
  ins: [16] TxIn; outs: [32] TxOut; items: [32] TxItem; spans: [64] BfSpan;
  b := tx_unhex(v.block, block[..]);
  p := tx_unhex(v.prevouts, prev[..]);
  filter := tx_unhex(v.filter, fb[..]);
  bf_block_hash(b, hash[..]);
  key := bf_key(hash[..]);
  g := bf_gather_vartime(b, p, ins[..], outs[..], items[..], pool[..], spans[..]);
  #assert g.code == BF_OK;
  for e: 0..<g.n {
    #assert bf_match_vartime(key, filter, pool[spans[e].at..<spans[e].at+spans[e].n]).found;
  }
  for cut: 0..<filter.count { #assert bf_read_vartime(filter[0..<cut]).code == BF_BAD_FILTER; }
}
// Sets of 1 to 40 one-byte elements: each element matches, and of the
// other 216 bytes almost none do (the rate is 1 in 784,931).
#forall "a built set matches its elements and rejects the rest" n: 1..<41 #steps 4000000000 {
  pool: [256] u8; spans: [256] BfSpan; values: [256] u64; idx: [256] int; out: [512] u8;
  for i: 0..<256 { pool[i] = cast(u8) i; spans[i] = BfSpan.{at = i, n = 1}; }
  key := BfKey.{k0 = cast(u64) n, k1 = 0x9e3779b97f4a7c15};
  f := bf_build_vartime(key, pool[..], spans[..], n, values[..], idx[..], out[..]);
  #assert f.code == BF_OK && f.n == n;
  false_hits := 0;
  for i: 0..<256 {
    found := bf_match_vartime(key, out[0..<f.size], pool[i..<i+1]).found;
    if i < n { #assert found; }
    if i >= 40 && found { false_hits += 1; }
  }
  #assert false_hits <= 1;
}
```

```shell
coax/bin/coax test bitcoin/examples/checks/block-filters.coax
```

On 2026-11-03 `coax test` runs 88 checks, including those of the files it
loads. Mutation testing: not run (owner speed order).

### The library's own checks

`blockfilter.coax` is an `#authority` file. It loads
[blockfilter_vectors.coax](../../coax/tests/bitcoin/blockfilter_vectors.coax),
the ten blocks of BIP-158's `testnet-19.json` with their spent scripts,
generated by `blockfilter.py --write`. Its checks cover:

- every BIP-158 block's basic filter and filter header, through the
  whole pipeline, and its decode;
- the bit stream, and Golomb-Rice codes with quotients 0, 1 and 3, out
  of room, and with the bits running out;
- heapsort with equal values, and the range map and key;
- an empty set, one element, and four elements of which two are equal
  and one is empty; matches of members and non-members; the builder's
  refusals;
- the decode check: every cut of a filter, an extra byte, and N of 2^32;
- matches of a block's spent script, and gathering's refusals: a short
  block, a bad transaction count, a missing byte, too few or too many
  spent scripts, and buffers too small;
- the genesis filter header, chained from zeros; and the C interface.

```shell
coax/bin/coax test coax/lib/bitcoin/blockfilter.coax
```

On 2026-11-03 it runs 83 checks, including those of the files it loads.

### Against Bitcoin Core and BIP-158, natively

[blockfilter.py](../../coax/tests/bitcoin/blockfilter.py) first checks
its own reference. It reads each BIP-158 block's scripts, hashes them
with Bitcoin Core's `test_framework/blockfilter.py` and
`crypto/siphash.py` (pinned), codes them with a transliteration of
`util/golombrice.h`, and must reach every published filter and filter
header. It also checks that `testnet-19.json` and Core's
`src/test/data/blockfilters.json` are the same file.

Then it compiles `blockfilter.coax` to C in checked and release builds
and compares against the reference:

- the basic filter and header of every BIP-158 block, every element of
  each block matching, and a missing spent-script byte refused;
- 150 seeded element sets per build, up to 400 elements with
  duplicates, under seeded keys, and six seeded match queries of members
  and strangers on each;
- 300 seeded damaged filters per build (cut, extended, a byte changed,
  or random bytes), each read with the decode check and, when it
  decodes, matched.

```shell
COAX_TEST_COMPILER=coax/bin/coax python3 coax/tests/bitcoin/blockfilter.py
```

On 2026-11-03 the three tests pass in about 3 seconds, with 20 BIP-158
blocks, 80 block elements, 300 element sets, 1,800 matches and 600
damaged filters across the two builds.

## What this chapter leaves out

- The P2P messages of BIP-157: `getcfilters`, `getcfheaders`,
  `getcfcheckpt` and their replies, and the `NODE_COMPACT_FILTERS`
  service bit.
- The filter index: Core stores filters in flat files and their hashes
  and headers in a LevelDB index, built in the background.
- A client's side: choosing peers, comparing their headers at
  checkpoints every 1,000 blocks, and what to do when they disagree.
- Filter types other than basic. BIP-158 reserves the type byte; 0 is the
  only one defined.
- Privacy limits. A client that fetches only matching blocks still tells
  its peers which blocks those are.

## Summary

- A basic filter is a Golomb-coded set of a block's output scripts
  (without `OP_RETURN` ones) and the scripts its inputs spend.
- Each element is hashed with SipHash-2-4 keyed by the block hash, and
  mapped onto [0, N * M) with M = 784,931 by a multiply.
- The sorted values' differences are Golomb-Rice coded with P = 19,
  about 21 bits each. False positives occur at a rate of about 1 in
  784,931; there are no false negatives.
- Decoding is strict: exactly N codes and no extra bytes.
- A filter header is SHA-256d of the filter's hash and the previous
  header, starting from 32 zero bytes at genesis.

## Dependencies

- [10. SipHash-2-4](ch010-siphash-2-4.md)
- [46. Merkle trees and the duplicate-leaf flaw](ch046-merkle-trees-and-the-duplicate-leaf-flaw.md)

## Sources

- [BIP-157](https://github.com/bitcoin/bips/blob/master/bip-0157.mediawiki)
  and [BIP-158](https://github.com/bitcoin/bips/blob/master/bip-0158.mediawiki),
  with BIP-158's `testnet-19.json`, at bips revision `5aadddb5`, pinned in
  [coax/tests/vectors/upstream/bip158/](../../coax/tests/vectors/upstream/bip158/testnet-19.json).
- Bitcoin Core at revision `ddea9196`: `blockfilter.cpp`,
  `util/golombrice.h`, `test/blockfilter_tests.cpp`,
  `test/data/blockfilters.json`, and `test_framework/blockfilter.py` and
  `crypto/siphash.py`, pinned in
  [coax/tests/vectors/upstream/bitcoin-core/](../../coax/tests/vectors/upstream/bitcoin-core/blockfilter.cpp).
- All are listed in [SOURCES.md](../../coax/tests/vectors/SOURCES.md).
