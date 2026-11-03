# 73. Compact blocks

When a node learns of a new block, it has usually seen most of the
block's transactions already: they were relayed to it one by one and sit
in its mempool. Sending the whole block again wastes bandwidth, and the
time to send it delays the block on every hop. BIP-152's compact blocks
send the block's header and a 6-byte short ID for each transaction
instead. The receiver finds the transactions in its mempool by short ID,
asks for the few it lacks, and puts the block back together.

A block of 1 MB becomes a message of about 9 KB. On a node that has the
transactions, the block arrives with no further round trip.

This chapter computes short IDs as Bitcoin Core does, writes and reads
the `cmpctblock`, `getblocktxn` and `blocktxn` messages, and rebuilds
blocks from a mempool with Core's `PartiallyDownloadedBlock`, including
what happens when two transactions share a short ID. It ends with how a
node picks the three peers that announce blocks to it in high-bandwidth
mode.

## What this chapter teaches

- What `sendcmpct`'s two fields mean, and why only version 2 is used.
- How a short ID is computed: SipHash-2-4 of the wtxid, keyed from
  SHA-256 of the header and a nonce, cut to 48 bits.
- How a `cmpctblock` sends some transactions whole, at differentially
  encoded indexes.
- How a receiver fills the block from its mempool and an extra pool, asks
  for the rest with `getblocktxn`, and gets them in `blocktxn`.
- What a short-ID collision does, and why the rebuilt block goes through
  chapter 49's `IsBlockMutated`.
- The difference between high-bandwidth and low-bandwidth mode, and how
  Core chooses its three high-bandwidth peers.

## Where Bitcoin uses compact blocks

| Where | What it uses | Chapter |
| --- | --- | --- |
| The handshake | `sendcmpct` after `verack`, with the high-bandwidth flag off | 68 |
| Block download | `MSG_CMPCT_BLOCK` in `getdata`, and the fallback to a whole block | 72 |
| The mempool | the transactions a node rebuilds blocks from | 82 |
| Orphans and replacements | the extra pool of transactions no longer in the mempool | 83, 84 |

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [cmpctblock.coax](../../coax/lib/bitcoin/cmpctblock.coax) | `cmpct_keys(header, nonce)`, `cmpct_short_id(k, wtxid)` | the SipHash keys and a short ID |
| | `cmpct_write_vartime`, `cmpct_read_vartime` | the `cmpctblock` message |
| | `cmpct_init_vartime` | `PartiallyDownloadedBlock::InitData`: match short IDs |
| | `cmpct_missing_vartime` | the indexes to ask for |
| | `cmpct_request_write_vartime`, `cmpct_request_read_vartime` | `getblocktxn` |
| | `cmpct_blocktxn_write_vartime`, `cmpct_blocktxn_read_vartime` | `blocktxn` |
| | `cmpct_fill_vartime` | `PartiallyDownloadedBlock::FillBlock` |
| | `cmpct_hb_choose_vartime` | `MaybeSetPeerAsAnnouncingHeaderAndIDs` |
| | `cmpct_getdata_reply`, `cmpct_getblocktxn_reply` | which reply a request gets, by depth |
| [siphash.coax](../../coax/lib/crypto/siphash.coax) | `siphash24` | SipHash-2-4 (chapter 10) |
| [witness.coax](../../coax/lib/bitcoin/witness.coax) | `witness_mutated_vartime` | `IsBlockMutated` (chapter 49) |
| [tx.coax](../../coax/lib/bitcoin/tx.coax) | `tx_parse_vartime`, `tx_wtxid_vartime` | transactions and wtxids (chapters 34 and 35) |

Blocks and the messages about them are public, so every procedure runs
in variable time and those that decide end in `_vartime` (chapter 3).
Readers copy nothing: a read message is positions in the caller's bytes,
and a rebuilt block is written into the caller's buffer.

The constants come from Core's `blockencodings.h` and
`net_processing.cpp`:

<!-- source: coax/lib/bitcoin/cmpctblock.coax constants -->
```jai
CMPCT_VERSION :: 2;              // CMPCTBLOCKS_VERSION: wtxids and witnesses (BIP-152 version 2)
CMPCT_SHORT_ID :: 6;             // SHORTTXIDS_LENGTH: bytes in a short ID
CMPCT_MAX_INDEX :: 0xffff;       // indexes are uint16_t: no block of more transactions
CMPCT_MAX_TXS :: 100000;         // MAX_BLOCK_WEIGHT / MIN_SERIALIZABLE_TRANSACTION_WEIGHT
CMPCT_HB_PEERS :: 3;             // BIP-152: high-bandwidth mode to at most three peers
CMPCT_MAX_DEPTH :: 5;            // MAX_CMPCTBLOCK_DEPTH: answer getdata with cmpctblock this near the tip
CMPCT_MAX_BLOCKTXN_DEPTH :: 10;  // MAX_BLOCKTXN_DEPTH: answer getblocktxn with blocktxn this near the tip
```

The result codes are Core's `ReadStatus` followed by the refusals of the
readers, and the five places a transaction of the block can come from:

<!-- source: coax/lib/bitcoin/cmpctblock.coax codes -->
```jai
// ReadStatus, then the refusals of the readers and writers. Core's words
// are in quotes.
CMPCT_OK :: 0;
CMPCT_INVALID :: 1;        // READ_STATUS_INVALID: malformed, "invalid compact block"; the peer is penalized
CMPCT_FAILED :: 2;         // READ_STATUS_FAILED: a short-ID collision, or a mutated result; ask for the block
CMPCT_BAD :: 3;            // the bytes don't read; tx_code says why (a deserialization exception in Core)
CMPCT_INDEX_RANGE :: 4;    // a prefilled index above 0xffff: "CompactSize exceeds limit of type"
CMPCT_TOO_MANY :: 5;       // more than 0xffff transactions: "indexes overflowed 16 bits"
CMPCT_DIFF_OVERFLOW :: 6;  // a getblocktxn index past 0xffff: "differential value overflow"
CMPCT_OUT_OF_BOUNDS :: 7;  // a getblocktxn index past the block: "getblocktxn with out-of-bounds tx indices"
CMPCT_CAPACITY :: 8;       // more entries than the caller's views hold, or no room to write

// Where InitData found each transaction of the block.
CMPCT_MISSING :: 0;        // not found: ask for it with getblocktxn
CMPCT_PREFILLED :: 1;      // in the cmpctblock itself
CMPCT_MEMPOOL :: 2;        // in the mempool
CMPCT_EXTRA :: 3;          // in the extra pool: recent orphans and replaced transactions
CMPCT_COLLIDED :: 4;       // two different transactions have its short ID: ask for it
```

## Negotiation: `sendcmpct`

After `verack`, each side sends `sendcmpct` (chapter 68). Its payload is a
one-byte boolean and an 8-byte little-endian version:

```text
1 byte    announce: 1 asks for high-bandwidth mode, 0 for low-bandwidth
8 bytes   version: 1 identifies transactions by txid, 2 by wtxid
```

Version 1 predates segwit. Its short IDs are computed from txids, and its
transactions are sent without witnesses. Version 2, added to BIP-152 with
segwit, uses wtxids and sends witnesses. Core sends only version 2 and
ignores a `sendcmpct` of any other version, as BIP-152 requires: a node
treats such a peer "as if they had not received the message". Every
message in this chapter is version 2.

The first field is the mode. A node sends `sendcmpct(0, 2)` during the
handshake and later sends `sendcmpct(1, 2)` to the peers it wants new
blocks from first. The last section of this chapter shows which peers
those are.

## Short IDs

A short ID has to be short, and an attacker must not be able to make two
transactions with the same one. A fixed hash of the wtxid would be short
but not safe: anyone could grind a pair of transactions that collide and
make every node fail to rebuild every block that contains them. BIP-152
keys the hash with a value the sender of the block chooses:

1. Take SHA-256 of the block's 80-byte header and an 8-byte nonce,
   little-endian. The sender picks the nonce, so each connection gets
   different short IDs.
2. k0 is the first 8 bytes of the digest as a little-endian number, and
   k1 the next 8.
3. The short ID of a transaction is SipHash-2-4 of its 32-byte wtxid with
   keys k0 and k1, with the two most significant bytes dropped.

The hash is single SHA-256, not SHA-256d, and the result is 6 bytes.
BIP-152 works out that 48 bits suffice for blocks of up to 10,000
transactions and mempools of up to 100,000, with a failure to rebuild in
at most one block in 281,474. An attacker can't grind a collision ahead
of time, because the keys depend on the header of a block that doesn't
exist yet and on the nonce. A failure only means the node falls back to
fetching the whole block.

<!-- source: coax/lib/bitcoin/cmpctblock.coax keys -->
```jai
// The SipHash-2-4 keys of one cmpctblock: SHA-256 of the 80-byte header
// and the nonce (8 bytes, little-endian), whose first two 8-byte words,
// little-endian, are k0 and k1.
CmpctKeys :: struct { k0: u64; k1: u64; }
cmpct_keys :: (header: [] u8, nonce: u64) -> CmpctKeys {
  input: [88] u8; d: [32] u8;
  for i: 0..<80 { input[i] = header[i]; }
  stored := cb_store_le64(input[..], 80, nonce);
  #assert sha256_hash_checked(input[..], d[..], false) == SHA_OK;
  return CmpctKeys.{k0=cb_load_le64(d[..], 0).value, k1=cb_load_le64(d[..], 8).value};
}
// A wtxid's short ID: SipHash-2-4 of its 32 bytes (hash order), with the
// two high bytes dropped.
cmpct_short_id :: (k: CmpctKeys, wtxid: [] u8) -> u64 {
  return siphash24(k.k0, k.k1, wtxid) & 0xffffffffffff;
}
```

`siphash24` is chapter 10's, which takes the wtxid in hash order, the
bytes as computed.

### Worked example: block 413,567

Bitcoin Core's benchmark block 413,567 has 1,557 transactions. It predates
segwit, so each wtxid is the transaction's txid. With the nonce
`000b1e55ed413567`, the keys and the first short IDs are:

```text
block 413,567, nonce 000b1e55ed413567
  SHA-256(header || nonce): 1766e76b63a0629e1678d7992cab38c0e77f875fa4bcd2d916779e2765586e04
  k0: 9e62a0636be76617, k1: c038ab2c99d77816
  short IDs of transactions 1 to 3: c8c57540116d 89921a6298b8 803c423f68f5
  cmpctblock: 1556 short IDs and the 185-byte coinbase, 9614 bytes
```

The block is 999,887 bytes. Its `cmpctblock`, with the coinbase sent whole,
is 9,614 bytes: 88 for the header and nonce, 3 for the count, 6 × 1,556 =
9,336 for the short IDs, and 187 for the coinbase with its count and
index.

## The `cmpctblock` message

BIP-152's `HeaderAndShortIDs`:

```text
80 bytes      the block header
8 bytes       the nonce, little-endian
CompactSize   the number of short IDs
6 bytes each  the short IDs, little-endian, in block order
CompactSize   the number of prefilled transactions
each          a CompactSize index and the transaction, with its witness
```

A prefilled transaction is one the sender expects the receiver to lack.
Core always prefills the coinbase, which no mempool holds, and nothing
else. The short IDs and the prefilled transactions together cover every
transaction of the block once, so the short IDs skip the prefilled
positions.

Prefilled indexes are differential. The first is the absolute index, and
each later one is the distance from the previous one, minus one. Indexes
that ascend become small numbers, each a single CompactSize byte. Indexes
0, 5, 6 and 18 are sent as 0, 4, 0 and 11:

<!-- source: coax/lib/bitcoin/cmpctblock.coax write -->
```jai
// A transaction's bytes: n at at in some buffer.
CmpctTx :: struct { at: int; n: int; }
// What a reader read: code, the byte where it refused (or next, where
// reading would go on), the transaction reader's code, and how many.
CmpctList :: struct { code: int; at: int; tx_code: int; count: int; next: int; }
// The transactions of a serialized block: where each is, and its wtxid.
cmpct_block_txs_vartime :: (b: [] u8, txs: [] CmpctTx, wtxids: [] u8, ins: [] TxIn, outs: [] TxOut, items: [] TxItem) -> CmpctList {
  if b.count < 80 { return CmpctList.{code=CMPCT_BAD, tx_code=TX_TRUNCATED}; }
  c := tx_compact_read_vartime(b, 80);
  if c.code != TX_OK { return CmpctList.{code=CMPCT_BAD, at=80, tx_code=c.code}; }
  if c.value > txs.count || 32 * c.value > wtxids.count { return CmpctList.{code=CMPCT_CAPACITY, at=80}; }
  at := c.next;
  for k: 0..<c.value {
    t := tx_parse_vartime(b[at..<b.count], true, ins, outs, items);
    if t.code != TX_OK { return CmpctList.{code=ifx t.code == TX_CAPACITY then CMPCT_CAPACITY else CMPCT_BAD, at=at+t.at, tx_code=t.code, count=k}; }
    txs[k] = CmpctTx.{at=at, n=t.total};
    #assert tx_wtxid_vartime(b[at..<at+t.total], t, wtxids[32*k..<32*k+32]);
    at += t.total;
  }
  return CmpctList.{count=c.value, next=at};
}
// The cmpctblock payload: the header, the nonce, the short IDs of every
// transaction not in prefill (ascending block indexes), then each
// prefilled transaction with its index as the difference from the last
// one plus one, minus one. wtxids holds every transaction's wtxid; txs[i]
// is where transaction i is in src, read only for the prefilled ones.
// Core's own constructor prefills the coinbase alone. Returns the length,
// or -1 if out is too small or prefill is not ascending and in the block.
cmpct_write_vartime :: (header: [] u8, nonce: u64, wtxids: [] u8, src: [] u8, txs: [] CmpctTx, prefill: [] int, out: [] u8) -> int {
  count := wtxids.count / 32;
  for p: 0..<prefill.count {
    if prefill[p] >= count || (p > 0 && prefill[p] <= prefill[p-1]) || prefill[p] < 0 { return -1; }
  }
  nshort := count - prefill.count;
  room := 88 + 9 + 6 * nshort + 9;
  for p: 0..<prefill.count { room += 3 + txs[prefill[p]].n; }
  if out.count < room || header.count != 80 { return -1; }
  for i: 0..<80 { out[i] = header[i]; }
  stored := cb_store_le64(out, 80, nonce);
  at := 88;
  at = tx_compact_write_vartime(out, at, nshort);
  k := cmpct_keys(header, nonce);
  p := 0;
  for i: 0..<count {
    if p < prefill.count && prefill[p] == i { p += 1; continue; }
    id := cmpct_short_id(k, wtxids[32*i..<32*i+32]);
    for j: 0..<6 { out[at+j] = cast(u8) ((id >> cast(u64) (8 * j)) & 255); }
    at += 6;
  }
  at = tx_compact_write_vartime(out, at, prefill.count);
  last := -1;
  for q: 0..<prefill.count {
    at = tx_compact_write_vartime(out, at, prefill[q] - last - 1);
    last = prefill[q];
    t := txs[prefill[q]];
    for j: 0..<t.n { out[at+j] = src[t.at+j]; }
    at += t.n;
  }
  return at;
}
```

The reader follows Core's serializer. A differential index above 0xffff
is refused while it is read, because Core reads it into a `uint16_t`. A
block of more than 0xffff transactions in all is refused after the read,
as "indexes overflowed 16 bits". Each prefilled transaction is read with
its witness:

<!-- source: coax/lib/bitcoin/cmpctblock.coax read -->
```jai
// A read cmpctblock: the nonce, nshort short IDs of 6 bytes from byte
// ids_at, and nprefilled prefilled transactions in the caller's array.
CmpctBlock :: struct { code: int; at: int; tx_code: int; nonce: u64; nshort: int; ids_at: int; nprefilled: int; next: int; }
// A prefilled transaction: its index as sent (differential), and where its
// bytes are. empty is CTransaction::IsNull: no inputs and no outputs.
CmpctPrefilled :: struct { diff: int; at: int; n: int; empty: bool; }
// Reads a cmpctblock payload as Core's serializer does: each prefilled
// index must fit 16 bits, each transaction is read with its witness, and
// a block of more than 0xffff transactions refuses after the whole read.
// Bytes after the message are left alone, as Core leaves them.
cmpct_read_vartime :: (b: [] u8, pre: [] CmpctPrefilled, ins: [] TxIn, outs: [] TxOut, items: [] TxItem) -> CmpctBlock {
  if b.count < 88 { return CmpctBlock.{code=CMPCT_BAD, at=b.count, tx_code=TX_TRUNCATED}; }
  k := CmpctBlock.{nonce=cb_load_le64(b, 80).value};
  c := tx_compact_read_vartime(b, 88);
  if c.code != TX_OK { k.code = CMPCT_BAD; k.at = 88; k.tx_code = c.code; return k; }
  k.nshort = c.value; k.ids_at = c.next;
  if b.count - c.next < 6 * c.value { k.code = CMPCT_BAD; k.at = b.count; k.tx_code = TX_TRUNCATED; return k; }
  at := c.next + 6 * c.value;
  p := tx_compact_read_vartime(b, at);
  if p.code != TX_OK { k.code = CMPCT_BAD; k.at = at; k.tx_code = p.code; return k; }
  if p.value > pre.count { k.code = CMPCT_CAPACITY; k.at = at; return k; }
  at = p.next;
  for i: 0..<p.value {
    d := tx_compact_read_vartime(b, at);
    if d.code != TX_OK { k.code = CMPCT_BAD; k.at = at; k.tx_code = d.code; return k; }
    if d.value > CMPCT_MAX_INDEX { k.code = CMPCT_INDEX_RANGE; k.at = at; return k; }
    at = d.next;
    t := tx_parse_vartime(b[at..<b.count], true, ins, outs, items);
    if t.code != TX_OK {
      k.code = ifx t.code == TX_CAPACITY then CMPCT_CAPACITY else CMPCT_BAD;
      k.at = at + t.at; k.tx_code = t.code; return k;
    }
    pre[i] = CmpctPrefilled.{diff=d.value, at=at, n=t.total, empty=t.nin == 0 && t.nout == 0};
    at += t.total;
    k.nprefilled += 1;
  }
  if k.nshort + k.nprefilled > CMPCT_MAX_INDEX { k.code = CMPCT_TOO_MANY; k.at = at; return k; }
  k.next = at;
  return k;
}
// Short ID i of a read cmpctblock: 6 bytes, little-endian.
cmpct_short_at :: (b: [] u8, k: CmpctBlock, i: int) -> u64 {
  v: u64 = 0;
  for j: 0..<6 { v |= cast(u64) b[k.ids_at + 6*i + j] << cast(u64) (8 * j); }
  return v;
}
```

## Rebuilding: `InitData`

`PartiallyDownloadedBlock::InitData` turns a `cmpctblock` into slots, one
per transaction of the block, and fills as many as it can:

1. A header with `nBits` of 0, or no transactions at all, is invalid.
2. Each prefilled transaction goes to its absolute index. A transaction
   with no inputs and no outputs, or an index past the transactions the
   short IDs leave room for, is invalid.
3. Each short ID stands for the next slot no prefilled transaction holds.
   Two equal short IDs in the message fail: the receiver can't tell which
   transaction is which, and asks for the whole block.
4. Each transaction of the mempool, in the order Core scans it, is hashed
   to a short ID with this block's keys. A match fills the slot. A second
   match for the same slot empties it again and marks it collided, since
   the node can't tell which of the two the block holds.
5. The extra pool works the same way, except that a second match with
   the same wtxid is the same transaction, not a collision.
6. Scanning stops once every short ID has a transaction.

`invalid` means the peer sent a malformed message, and Core penalizes it.
`failed` means the node can't rebuild the block this way and fetches it
whole. Core keeps the short IDs in a hash table and also fails a message
that puts more than 12 into one bucket of that table, which defends the
table itself. Here the short IDs are sorted and searched instead, with
the same answers, so that limit has no counterpart.

The extra pool holds transactions the node saw but no longer keeps:
orphans and transactions replaced by fee (chapters 83 and 84). A miner may still
include them. Its keys can differ from the wtxids of the transactions
stored under them, which is how the example below tests a collision.

<!-- source: coax/lib/bitcoin/cmpctblock.coax init -->
```jai
// Where each transaction of the block is: source (CMPCT_MISSING to
// CMPCT_COLLIDED) and its index in the prefilled list, the mempool or the
// extra pool.
CmpctSlot :: struct { source: int; from: int; }
// A short ID and the block position it stands for.
CmpctEntry :: struct { id: u64; slot: int; }
// InitData's result: the status, the block's transaction count, and how
// many came from each source.
CmpctInit :: struct { status: int; count: int; prefilled: int; mempool: int; extra: int; available: int; }
// PartiallyDownloadedBlock::InitData for the cmpctblock b, read as k.
// pool_ids are the mempool's wtxids in the order the mempool is scanned
// (Core's txns_randomized). extra_keys are the extra pool's wtxids as
// keyed, extra_ids the wtxids of the transactions stored under them. slots
// and map hold the block's transactions and its short IDs. Core keeps the
// short IDs in a hash table; here they are sorted and searched, with the
// same answers. Core's further limit of 12 entries per hash-table bucket,
// a defense of its own table, has no counterpart here.
cmpct_init_vartime :: (b: [] u8, k: CmpctBlock, pre: [] CmpctPrefilled, pool_ids: [] u8, extra_keys: [] u8, extra_ids: [] u8, slots: [] CmpctSlot, map: [] CmpctEntry) -> CmpctInit {
  count := k.nshort + k.nprefilled;
  r := CmpctInit.{count=count};
  if tx_le_vartime(b, 72, 4) == 0 || count == 0 || count > CMPCT_MAX_TXS { r.status = CMPCT_INVALID; return r; }
  if slots.count < count || map.count < k.nshort { r.status = CMPCT_CAPACITY; return r; }
  for i: 0..<count { slots[i] = CmpctSlot.{source=CMPCT_MISSING}; }
  // The prefilled transactions, at their absolute indexes.
  last := -1;
  for i: 0..<k.nprefilled {
    if pre[i].empty { r.status = CMPCT_INVALID; return r; }
    last += pre[i].diff + 1;
    if last > CMPCT_MAX_INDEX || last > k.nshort + i { r.status = CMPCT_INVALID; return r; }
    slots[last] = CmpctSlot.{source=CMPCT_PREFILLED, from=i};
  }
  r.prefilled = k.nprefilled;
  // Each short ID stands for the next position no prefilled one holds.
  offset := 0;
  for i: 0..<k.nshort {
    while slots[i + offset].source == CMPCT_PREFILLED { offset += 1; }
    map[i] = CmpctEntry.{id=cmpct_short_at(b, k, i), slot=i + offset};
  }
  cmpct_sort_vartime(map[0..<k.nshort]);
  for i: 1..<k.nshort { if map[i].id == map[i-1].id { r.status = CMPCT_FAILED; return r; } }   // a short-ID collision
  keys := cmpct_keys(b[0..<80], k.nonce);
  available := 0;
  // The mempool. Two matches for one short ID: ask for it instead.
  for m: 0..<pool_ids.count / 32 {
    e := cmpct_find_vartime(map[0..<k.nshort], cmpct_short_id(keys, pool_ids[32*m..<32*m+32]));
    if e >= 0 {
      s := map[e].slot;
      if slots[s].source == CMPCT_MISSING { slots[s] = CmpctSlot.{source=CMPCT_MEMPOOL, from=m}; available += 1; }
      else if slots[s].source != CMPCT_COLLIDED { slots[s] = CmpctSlot.{source=CMPCT_COLLIDED}; available -= 1; }
    }
    if available == k.nshort { break; }
  }
  // The extra pool. A second match collides only if it is a different
  // transaction: the same one in the mempool and the extra pool is not.
  for x: 0..<extra_keys.count / 32 {
    e := cmpct_find_vartime(map[0..<k.nshort], cmpct_short_id(keys, extra_keys[32*x..<32*x+32]));
    if e >= 0 {
      s := map[e].slot;
      if slots[s].source == CMPCT_MISSING { slots[s] = CmpctSlot.{source=CMPCT_EXTRA, from=x}; available += 1; }
      else if slots[s].source != CMPCT_COLLIDED && !cmpct_same_tx(slots[s], pool_ids, extra_ids, extra_ids[32*x..<32*x+32]) {
        slots[s] = CmpctSlot.{source=CMPCT_COLLIDED}; available -= 1;
      }
    }
    if available == k.nshort { break; }
  }
  for i: 0..<count {
    if slots[i].source == CMPCT_MEMPOOL { r.mempool += 1; }
    if slots[i].source == CMPCT_EXTRA { r.extra += 1; }
  }
  r.available = r.prefilled + r.mempool + r.extra;
  return r;
}
// Is the transaction already in slot the one whose wtxid is id?
cmpct_same_tx :: (slot: CmpctSlot, pool_ids: [] u8, extra_ids: [] u8, id: [] u8) -> bool {
  have := ifx slot.source == CMPCT_MEMPOOL then pool_ids[32*slot.from..<32*slot.from+32] else extra_ids[32*slot.from..<32*slot.from+32];
  for i: 0..<32 { if have[i] != id[i] { return false; } }
  return true;
}
// Sort entries by short ID: Shell's sort with Ciura's gaps.
cmpct_sort_vartime :: (map: [] CmpctEntry) {
  gaps := [8] int.[701, 301, 132, 57, 23, 10, 4, 1];
  for g: 0..<8 {
    gap := gaps[g];
    for i: gap..<map.count {
      e := map[i];
      j := i;
      while j >= gap && map[j-gap].id > e.id { map[j] = map[j-gap]; j -= gap; }
      map[j] = e;
    }
  }
}
// The entry with short ID id in the sorted map, or -1.
cmpct_find_vartime :: (map: [] CmpctEntry, id: u64) -> int {
  lo := 0; hi := map.count;
  while lo < hi {
    mid := (lo + hi) / 2;
    if map[mid].id < id { lo = mid + 1; } else { hi = mid; }
  }
  return ifx lo < map.count && map[lo].id == id then lo else -1;
}
// The block positions InitData didn't fill, ascending, into out; how many.
cmpct_missing_vartime :: (slots: [] CmpctSlot, out: [] int) -> int {
  n := 0;
  for i: 0..<slots.count {
    if slots[i].source == CMPCT_MISSING || slots[i].source == CMPCT_COLLIDED {
      if n == out.count { return -1; }
      out[n] = i; n += 1;
    }
  }
  return n;
}
```

## Asking for the rest: `getblocktxn` and `blocktxn`

The slots still empty or collided are requested with `getblocktxn`:

```text
32 bytes      the block hash
CompactSize   the number of indexes
CompactSize   each index, differential, as in cmpctblock
```

Core reads it with `DifferenceFormatter`, which refuses the first index
that passes 0xffff with "differential value overflow". The deltas
0x7000, 0x8ffe and 0 reach 0x7000, then 0xffff, then 0x10000, which is one
too many.

<!-- source: coax/lib/bitcoin/cmpctblock.coax request -->
```jai
// getblocktxn: the block hash, a count, then each index as the difference
// from the previous one, minus one. Returns the length, or -1 if out is too
// small or the indexes don't ascend from 0 to 0xffff.
cmpct_request_write_vartime :: (hash: [] u8, indexes: [] int, out: [] u8) -> int {
  if out.count < 32 + 9 + 3 * indexes.count || hash.count != 32 { return -1; }
  for i: 0..<32 { out[i] = hash[i]; }
  at := tx_compact_write_vartime(out, 32, indexes.count);
  last := -1;
  for i: 0..<indexes.count {
    if indexes[i] <= last || indexes[i] > CMPCT_MAX_INDEX { return -1; }
    at = tx_compact_write_vartime(out, at, indexes[i] - last - 1);
    last = indexes[i];
  }
  return at;
}
// Reads a getblocktxn into absolute indexes. DifferenceFormatter refuses
// the first index that passes 0xffff, as "differential value overflow".
cmpct_request_read_vartime :: (b: [] u8, indexes: [] int) -> CmpctList {
  if b.count < 32 { return CmpctList.{code=CMPCT_BAD, at=b.count, tx_code=TX_TRUNCATED}; }
  c := tx_compact_read_vartime(b, 32);
  if c.code != TX_OK { return CmpctList.{code=CMPCT_BAD, at=32, tx_code=c.code}; }
  r := CmpctList.{};
  at := c.next; shift := 0;
  for i: 0..<c.value {
    if i == indexes.count { r.code = CMPCT_CAPACITY; r.at = at; return r; }
    d := tx_compact_read_vartime(b, at);
    if d.code != TX_OK { r.code = CMPCT_BAD; r.at = at; r.tx_code = d.code; return r; }
    shift += d.value;
    if shift > CMPCT_MAX_INDEX { r.code = CMPCT_DIFF_OVERFLOW; r.at = at; return r; }
    indexes[i] = shift; shift += 1;
    at = d.next; r.count += 1;
  }
  r.next = at;
  return r;
}
```

The reply is `blocktxn`: the block hash, a count, and the requested
transactions, with witnesses, in the order requested. An index past the
block is misbehavior, and the sender answers nothing:

<!-- source: coax/lib/bitcoin/cmpctblock.coax respond -->
```jai
// What a blocktxn writer did: code and the length.
CmpctWrite :: struct { code: int; n: int; }
// SendBlockTransactions: the block hash, then the requested transactions
// of the block b (txs says where each is), in the order requested. An
// index past the block is Core's Misbehaving, and nothing is sent.
cmpct_blocktxn_write_vartime :: (hash: [] u8, b: [] u8, txs: [] CmpctTx, indexes: [] int, out: [] u8) -> CmpctWrite {
  room := 32 + 9;
  for i: 0..<indexes.count {
    if indexes[i] < 0 || indexes[i] >= txs.count { return CmpctWrite.{code=CMPCT_OUT_OF_BOUNDS}; }
    room += txs[indexes[i]].n;
  }
  if out.count < room || hash.count != 32 { return CmpctWrite.{code=CMPCT_CAPACITY}; }
  for i: 0..<32 { out[i] = hash[i]; }
  at := tx_compact_write_vartime(out, 32, indexes.count);
  for i: 0..<indexes.count {
    t := txs[indexes[i]];
    for j: 0..<t.n { out[at+j] = b[t.at+j]; }
    at += t.n;
  }
  return CmpctWrite.{n=at};
}
// Reads a blocktxn: the hash, a count, then each transaction with its
// witness; txs gets where each one is.
cmpct_blocktxn_read_vartime :: (b: [] u8, txs: [] CmpctTx, ins: [] TxIn, outs: [] TxOut, items: [] TxItem) -> CmpctList {
  if b.count < 32 { return CmpctList.{code=CMPCT_BAD, at=b.count, tx_code=TX_TRUNCATED}; }
  c := tx_compact_read_vartime(b, 32);
  if c.code != TX_OK { return CmpctList.{code=CMPCT_BAD, at=32, tx_code=c.code}; }
  if c.value > txs.count { return CmpctList.{code=CMPCT_CAPACITY, at=32}; }
  at := c.next;
  for k: 0..<c.value {
    t := tx_parse_vartime(b[at..<b.count], true, ins, outs, items);
    if t.code != TX_OK { return CmpctList.{code=ifx t.code == TX_CAPACITY then CMPCT_CAPACITY else CMPCT_BAD, at=at+t.at, tx_code=t.code, count=k}; }
    txs[k] = CmpctTx.{at=at, n=t.total};
    at += t.total;
  }
  return CmpctList.{count=c.value, next=at};
}
```

## Putting the block together: `FillBlock`

`FillBlock` writes the block from the slots, taking each empty one from
`blocktxn` in order. Too few or too many transactions in `blocktxn` is
invalid.

A short ID that matched the wrong transaction can't be caught by
`InitData`: the wrong transaction has the right short ID. The rebuilt
block catches it. `FillBlock` runs `IsBlockMutated` from chapter 49, with
the witness rules on once segwit is active. The wrong transaction changes
the Merkle root, which no longer matches the header. A transaction with
the right txid and a different witness keeps the root and breaks the
witness commitment. Either way the result is `failed`, not invalid: the
peer did nothing wrong, and the node fetches the whole block.

<!-- source: coax/lib/bitcoin/cmpctblock.coax fill -->
```jai
// FillBlock's result: the status, the block's length in out, and for
// CMPCT_FAILED the WITNESS_ code IsBlockMutated gave.
CmpctFill :: struct { status: int; n: int; reason: int; }
// PartiallyDownloadedBlock::FillBlock: the block, with each transaction
// from its slot (prefilled ones from the cmpctblock b, mempool and extra
// ones from store at pool_txs and extra_txs) and each missing one, in
// order, from missing (in bytes, a blocktxn). Too few or too many is
// invalid. The rebuilt block then goes through IsBlockMutated, with segwit
// as check_witness_root: a mutated block most likely means a short ID
// matched the wrong transaction, so it fails, and the node asks for the
// whole block. hashes holds 96 bytes per transaction.
cmpct_fill_vartime :: (b: [] u8, pre: [] CmpctPrefilled, slots: [] CmpctSlot, store: [] u8, pool_txs: [] CmpctTx, extra_txs: [] CmpctTx,
                       bytes: [] u8, missing: [] CmpctTx, segwit: bool, out: [] u8,
                       ins: [] TxIn, outs: [] TxOut, items: [] TxItem, hashes: [] u8) -> CmpctFill {
  if tx_le_vartime(b, 72, 4) == 0 { return CmpctFill.{status=CMPCT_INVALID}; }
  count := slots.count;
  if out.count < 89 || hashes.count < 96 * count { return CmpctFill.{status=CMPCT_CAPACITY}; }
  for i: 0..<80 { out[i] = b[i]; }
  at := tx_compact_write_vartime(out, 80, count);
  m := 0;
  for i: 0..<count {
    src := bytes; t: CmpctTx;
    s := slots[i];
    if s.source == CMPCT_PREFILLED { src = b; t = CmpctTx.{at=pre[s.from].at, n=pre[s.from].n}; }
    else if s.source == CMPCT_MEMPOOL { src = store; t = pool_txs[s.from]; }
    else if s.source == CMPCT_EXTRA { src = store; t = extra_txs[s.from]; }
    else {
      if m >= missing.count { return CmpctFill.{status=CMPCT_INVALID}; }
      t = missing[m]; m += 1;
    }
    if out.count - at < t.n { return CmpctFill.{status=CMPCT_CAPACITY}; }
    for j: 0..<t.n { out[at+j] = src[t.at+j]; }
    at += t.n;
  }
  if m != missing.count { return CmpctFill.{status=CMPCT_INVALID}; }
  w := witness_mutated_vartime(out[0..<at], segwit, ins, outs, items,
    hashes[0..<32*count], hashes[32*count..<64*count], hashes[64*count..<96*count]);
  if w.code == WITNESS_CAPACITY { return CmpctFill.{status=CMPCT_CAPACITY, n=at}; }
  if w.code != WITNESS_OK { return CmpctFill.{status=CMPCT_FAILED, n=at, reason=w.code}; }
  return CmpctFill.{n=at};
}
```

## High-bandwidth and low-bandwidth mode

In low-bandwidth mode, the default, a peer announces a block with
`headers` or `inv` after validating it. The node asks for it with
`getdata(MSG_CMPCT_BLOCK)` and gets a `cmpctblock`. Rebuilding then costs
one and a half round trips, or two and a half if transactions are
missing.

In high-bandwidth mode the peer sends the `cmpctblock` unasked, as soon
as it has checked the header and before it has validated the whole block.
Rebuilding then costs half a round trip. BIP-152 limits this mode to a
few peers, because each one sends every block, and Core to three.
Because the block may turn out invalid, BIP-152 says a node should not
penalize a peer for an invalid block whose header is valid when the block
came in high-bandwidth mode.

Core picks its three peers with `MaybeSetPeerAsAnnouncingHeaderAndIDs`.
Each time a peer delivers a new block first, that peer moves to the end
of the list. A new peer joins with `sendcmpct(1, 2)`, and when the list
grows past three, the oldest peer leaves with `sendcmpct(0, 2)`. One rule
protects against an attacker who makes many inbound connections: an
inbound peer never pushes out the last outbound one. When the list is
full, holds one outbound peer and that peer is the oldest, the first two
swap, so the inbound peer next in line leaves instead.

A node in blocks-only mode has no mempool to rebuild from, and a peer
that never sent `sendcmpct(…, 2)` can't send version 2 blocks, so neither
is chosen.

Two replies depend on depth. `getdata(MSG_CMPCT_BLOCK)` gets a
`cmpctblock` for a block within 5 of the tip, and the whole block deeper.
`getblocktxn` gets `blocktxn` for a block within 10 of the tip and the
whole block deeper, so that a few bytes of request can't make a node read
old blocks from disk piece by piece. A `getblocktxn` with no indexes
disconnects the peer.

<!-- source: coax/lib/bitcoin/cmpctblock.coax modes -->
```jai
// MaybeSetPeerAsAnnouncingHeaderAndIDs: after peer delivers a new block,
// ask it for high-bandwidth mode. list[0..<n] are the high-bandwidth peers,
// oldest first; inbound says which peers connected to us. on is the peer
// sent sendcmpct(1, 2) and off the one sent sendcmpct(0, 2), or -1.
CmpctHb :: struct { n: int; on: int; off: int; }
cmpct_hb_choose_vartime :: (list: [] int, n: int, peer: int, inbound: [] bool, provides: bool, blocksonly: bool) -> CmpctHb {
  r := CmpctHb.{n=n, on=-1, off=-1};
  // A blocks-only node's mempool can't rebuild blocks, and a peer that
  // never sent sendcmpct(…, 2) can't send them.
  if blocksonly || !provides { return r; }
  outbound := 0;
  for i: 0..<n {
    if list[i] == peer {
      for j: i..<n-1 { list[j] = list[j+1]; }
      list[n-1] = peer;
      return r;
    }
    if !inbound[list[i]] { outbound += 1; }
  }
  // An inbound peer never pushes out the last outbound one.
  if inbound[peer] && n >= CMPCT_HB_PEERS && outbound == 1 && !inbound[list[0]] {
    first := list[0]; list[0] = list[1]; list[1] = first;
  }
  list[n] = peer; r.on = peer; r.n = n + 1;
  if r.n > CMPCT_HB_PEERS {
    r.off = list[0];
    for j: 0..<r.n-1 { list[j] = list[j+1]; }
    r.n -= 1;
  }
  return r;
}
// The answer to a getdata for MSG_CMPCT_BLOCK: a cmpctblock for a block
// within 5 of the tip when the node can fetch directly, else the block.
CMPCT_REPLY_NONE :: 0; CMPCT_REPLY_CMPCTBLOCK :: 1; CMPCT_REPLY_BLOCK :: 2; CMPCT_REPLY_BLOCKTXN :: 3; CMPCT_REPLY_DISCONNECT :: 4;
cmpct_getdata_reply :: (height: int, tip: int, direct: bool) -> int {
  return ifx direct && height >= tip - CMPCT_MAX_DEPTH then CMPCT_REPLY_CMPCTBLOCK else CMPCT_REPLY_BLOCK;
}
// The answer to a getblocktxn of count indexes: none is a disconnect; a
// block the node lacks gets nothing; within 10 of the tip a blocktxn, and
// deeper the whole block, so a peer can't make the node read old blocks
// from disk for a few bytes of request.
cmpct_getblocktxn_reply :: (count: int, have: bool, height: int, tip: int) -> int {
  if count == 0 { return CMPCT_REPLY_DISCONNECT; }
  if !have { return CMPCT_REPLY_NONE; }
  return ifx height >= tip - CMPCT_MAX_BLOCKTXN_DEPTH then CMPCT_REPLY_BLOCKTXN else CMPCT_REPLY_BLOCK;
}
```

## The example

The example is
[bitcoin/examples/compact-blocks.coax](../examples/compact-blocks.coax).
It computes block 413,567's short IDs, rebuilds mainnet block 514,999
from a mempool with three transactions missing, sends prefilled
transactions, tries four collisions and two malformed messages, and runs
the high-bandwidth peer choice and the replies.

<!-- source: bitcoin/examples/compact-blocks.coax example -->
```jai
#load "../../coax/lib/bitcoin/cmpctblock.coax";
#load "../../coax/lib/nostr/hex.coax";
#import "Basic";

// Print a label and bytes as lowercase hex.
show :: (label: string, bytes: [] u8) {
  text: [400] u8;
  r := hex_encode_checked(cast(string) bytes, text[..], false);
  print("%: %\n", label, cast(string) text[0..<r.written]);
}
// A hash as explorers show it: the digest reversed.
show_id :: (label: string, digest: [] u8) {
  shown: [32] u8;
  #assert bitcoin_reverse32(digest, shown[..]) == SHA_OK;
  show(label, shown[..]);
}
// Print the low digits hex digits of v.
hex :: (v: u64, digits: int) {
  text: [16] u8;
  for i: 0..<digits {
    d := cast(int) ((v >> cast(u64) (4 * (digits - 1 - i))) & 15);
    text[i] = cast(u8) (ifx d < 10 then '0' + d else 'a' + d - 10);
  }
  print("%", cast(string) text[0..<digits]);
}
// Decode hex into out and return the filled part. Every literal here and
// in the vector tables is valid lowercase hex, so an error stops the program.
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
STATUS :: [9] string.["ok", "invalid", "failed", "bad", "index out of range", "too many",
  "differential value overflow", "out of bounds", "capacity"];
REPLY :: [5] string.["nothing", "cmpctblock", "block", "blocktxn", "disconnect"];
SOURCE :: [5] string.["missing", "prefilled", "mempool", "extra pool", "collided"];
// Print a list of numbers after a label.
numbers :: (label: string, v: [] int) {
  print("%:", label);
  for i: 0..<v.count { print(" %", v[i]); }
  print("\n");
}

// The node rebuilding block 514,999: its mempool and extra pool are
// transactions in store; the ones at pool and extra.
Node :: struct {
  store: [12000] u8; used: int;
  pool: [24] CmpctTx; npool: int; pool_ids: [32 * 24] u8;
  extra: [4] CmpctTx; nextra: int; extra_keys: [32 * 4] u8; extra_ids: [32 * 4] u8;
}
// Add a transaction to the mempool, or under key to the extra pool.
add :: (n: [] Node, tx: [] u8, to_extra: bool, key: [] u8) {
  ins: [8] TxIn; outs: [8] TxOut; items: [16] TxItem; id: [32] u8;
  t := tx_read_vartime(tx, true, ins[..], outs[..], items[..]);
  #assert t.code == TX_OK && tx_wtxid_vartime(tx, t, id[..]);
  at := n[0].used;
  for i: 0..<tx.count { n[0].store[at+i] = tx[i]; }
  n[0].used += tx.count;
  if to_extra {
    x := n[0].nextra;
    n[0].extra[x] = CmpctTx.{at=at, n=tx.count};
    for i: 0..<32 { n[0].extra_keys[32*x+i] = ifx key.count == 32 then key[i] else id[i]; n[0].extra_ids[32*x+i] = id[i]; }
    n[0].nextra += 1;
  } else {
    p := n[0].npool;
    n[0].pool[p] = CmpctTx.{at=at, n=tx.count};
    for i: 0..<32 { n[0].pool_ids[32*p+i] = id[i]; }
    n[0].npool += 1;
  }
}
// Receive the cmpctblock c; ask for what is missing from the sender's
// block blk (with txs where its transactions are); rebuild. Prints each
// step and returns FillBlock's status.
receive :: (n: [] Node, c: [] u8, blk: [] u8, txs: [] CmpctTx, quiet: bool) -> int {
  ins: [8] TxIn; outs: [8] TxOut; items: [16] TxItem; pre: [24] CmpctPrefilled;
  slots: [24] CmpctSlot; map: [24] CmpctEntry; idx: [24] int; hash: [32] u8;
  req: [200] u8; got: [24] int; reply: [12000] u8; missing: [24] CmpctTx; out: [12000] u8; hashes: [96 * 24] u8;
  k := cmpct_read_vartime(c, pre[..], ins[..], outs[..], items[..]);
  s := cmpct_init_vartime(c, k, pre[..], n[0].pool_ids[0..<32*n[0].npool], n[0].extra_keys[0..<32*n[0].nextra],
    n[0].extra_ids[0..<32*n[0].nextra], slots[..], map[..]);
  print("  init: %", STATUS[s.status]);
  if s.status != CMPCT_OK { print("\n"); return s.status; }
  print(", % prefilled, % from the mempool, % from the extra pool\n", s.prefilled, s.mempool, s.extra);
  m := cmpct_missing_vartime(slots[0..<s.count], idx[..]);
  if m > 0 { numbers("  missing", idx[0..<m]); }
  // getblocktxn, answered by the sender with blocktxn.
  #assert sha256d_checked(c[0..<80], hash[..], false) == SHA_OK;
  bytes := reply[0..<0];
  if m > 0 {
    r := cmpct_request_write_vartime(hash[..], idx[0..<m], req[..]);
    if !quiet { show("  getblocktxn", req[0..<r]); }
    l := cmpct_request_read_vartime(req[0..<r], got[..]);
    w := cmpct_blocktxn_write_vartime(hash[..], blk, txs, got[0..<l.count], reply[..]);
    if !quiet { print("  blocktxn: % transactions, % bytes\n", l.count, w.n); }
    bytes = reply[0..<w.n];
    b := cmpct_blocktxn_read_vartime(bytes, missing[..], ins[..], outs[..], items[..]);
    #assert b.code == CMPCT_OK && b.count == m;
  }
  f := cmpct_fill_vartime(c, pre[..], slots[0..<s.count], n[0].store[..], n[0].pool[0..<n[0].npool], n[0].extra[0..<n[0].nextra],
    bytes, missing[0..<m], true, out[..], ins[..], outs[..], items[..], hashes[..]);
  print("  fill: %", STATUS[f.status]);
  if f.status == CMPCT_FAILED { print(", %", WITNESS_REASONS[f.reason]); }
  if f.status == CMPCT_OK { print(", the block byte for byte: %", equal_bytes(out[0..<f.n], blk)); }
  print("\n");
  return f.status;
}

main :: () {
  ins: [12] TxIn; outs: [8] TxOut; items: [16] TxItem;

  // Short IDs: block 413,567, 1,557 transactions from Bitcoin Core's
  // benchmark data. It predates segwit, so its wtxids are its txids.
  h: [80] u8; leaves: [32 * 1557] u8; cbuf: [400] u8; c: [12000] u8; d: [32] u8; input: [88] u8;
  header := decode(MK_HEADER_413567, h[..]);
  ids := decode(MK_LEAVES_413567, leaves[..]);
  coinbase := decode(CB_COINBASE_413567, cbuf[..]);
  print("block 413,567, nonce "); hex(CB_NONCE_413567, 16); print("\n");
  for i: 0..<80 { input[i] = header[i]; }
  stored := cb_store_le64(input[..], 80, CB_NONCE_413567);
  #assert sha256_hash_checked(input[..], d[..], false) == SHA_OK;
  show("  SHA-256(header || nonce)", d[..]);
  k := cmpct_keys(header, CB_NONCE_413567);
  print("  k0: "); hex(k.k0, 16); print(", k1: "); hex(k.k1, 16); print("\n");
  print("  short IDs of transactions 1 to 3:");
  for i: 1..<4 { print(" "); hex(cmpct_short_id(k, ids[32*i..<32*i+32]), 12); }
  print("\n");
  txs: [1557] CmpctTx;
  txs[0] = CmpctTx.{at=0, n=coinbase.count};
  zero := [1] int.[0];
  n := cmpct_write_vartime(header, CB_NONCE_413567, ids, coinbase, txs[..], zero[..], c[..]);
  print("  cmpctblock: 1556 short IDs and the %-byte coinbase, % bytes\n", coinbase.count, n);

  // Block 514,999 from a mempool that lacks three of its transactions,
  // one of which is in the extra pool.
  print("block 514,999, nonce "); hex(CB_NONCE_514999, 16); print("\n");
  b: [6000] u8; wids: [32 * 20] u8; btx: [20] CmpctTx;
  blk := decode(MK_BLOCK_514999, b[..]);
  w := cmpct_block_txs_vartime(blk, btx[..], wids[..], ins[..], outs[..], items[..]);
  n = cmpct_write_vartime(blk[0..<80], CB_NONCE_514999, wids[0..<32*w.count], blk, btx[0..<w.count], zero[..], c[..]);
  print("  cmpctblock: % short IDs and the coinbase, % bytes; the block, % bytes\n", w.count - 1, n, blk.count);
  node: [1] Node;
  t: [400] u8;
  unrelated := decode(CB_NON_BLOCK_TX, t[..]);
  none: [32] u8;
  for j: 0..<18 {
    i := 18 - j;
    if i == 4 || i == 9 || i == 15 { continue; }
    add(node[..], blk[btx[i].at..<btx[i].at+btx[i].n], false, none[0..<0]);
  }
  add(node[..], unrelated, false, none[0..<0]);
  add(node[..], blk[btx[9].at..<btx[9].at+btx[9].n], true, none[0..<0]);
  print("  mempool: % transactions, 15 of them in the block; extra pool: transaction 9\n", node[0].npool);
  status := receive(node[..], c[0..<n], blk, btx[0..<w.count], false);

  // Prefilled transactions at absolute indexes 0, 5, 6 and 18.
  print("prefilled transactions\n");
  four := [4] int.[0, 5, 6, 18];
  n = cmpct_write_vartime(blk[0..<80], CB_NONCE_514999, wids[0..<32*w.count], blk, btx[0..<w.count], four[..], c[..]);
  pre: [8] CmpctPrefilled;
  r := cmpct_read_vartime(c[0..<n], pre[..], ins[..], outs[..], items[..]);
  print("  indexes 0 5 6 18 are sent as");
  for i: 0..<r.nprefilled { print(" %", pre[i].diff); }
  print("; % short IDs, % bytes\n", r.nshort, n);

  // Collisions.
  print("collisions\n");
  n = cmpct_write_vartime(blk[0..<80], CB_NONCE_514999, wids[0..<32*w.count], blk, btx[0..<w.count], zero[..], c[..]);
  // The same short ID twice in the message.
  dup: [12000] u8;
  for i: 0..<n { dup[i] = c[i]; }
  for i: 0..<6 { dup[95+i] = dup[89+i]; }
  print("  one short ID twice in the cmpctblock\n");
  status = receive(node[..], dup[0..<n], blk, btx[0..<w.count], true);
  // A transaction in the mempool and a different one in the extra pool
  // whose key is the same wtxid: neither is trusted.
  other: [1] Node;
  for j: 1..<19 { if j != 4 { add(other[..], blk[btx[j].at..<btx[j].at+btx[j].n], false, none[0..<0]); } }
  add(other[..], unrelated, true, wids[32*3..<32*4]);
  print("  transaction 3 in the mempool and another under its wtxid in the extra pool\n");
  status = receive(other[..], c[0..<n], blk, btx[0..<w.count], true);
  // The wrong transaction under transaction 4's short ID.
  wrong: [1] Node;
  for j: 1..<19 { if j != 4 { add(wrong[..], blk[btx[j].at..<btx[j].at+btx[j].n], false, none[0..<0]); } }
  add(wrong[..], unrelated, true, wids[32*4..<32*5]);
  print("  another transaction under transaction 4's wtxid\n");
  status = receive(wrong[..], c[0..<n], blk, btx[0..<w.count], true);
  // Transaction 10 with one witness byte changed: the same txid.
  witness: [1] Node;
  for j: 1..<19 { if j != 10 { add(witness[..], blk[btx[j].at..<btx[j].at+btx[j].n], false, none[0..<0]); } }
  t10: [400] u8;
  tx10 := blk[btx[10].at..<btx[10].at+btx[10].n];
  for i: 0..<tx10.count { t10[i] = tx10[i]; }
  x := tx_read_vartime(tx10, true, ins[..], outs[..], items[..]);
  t10[items[ins[0].first].at + 10] ^= 1;
  add(witness[..], t10[0..<tx10.count], true, wids[32*10..<32*11]);
  print("  transaction 10 with a witness byte changed, under its wtxid\n");
  status = receive(witness[..], c[0..<n], blk, btx[0..<w.count], true);

  // Malformed messages.
  print("malformed\n");
  for i: 0..<n { dup[i] = c[i]; }
  dup[89 + 6 * 18 + 1] = 19;
  print("  a prefilled index past the short IDs\n");
  status = receive(node[..], dup[0..<n], blk, btx[0..<w.count], true);
  idx: [4] int; q: [64] u8;
  s := decode(CB_REQUEST_MAX, q[..]);
  l := cmpct_request_read_vartime(s, idx[..]);
  print("  getblocktxn for index 65535: %\n", STATUS[l.code]);
  s = decode(CB_REQUEST_OVERFLOW, q[..]);
  l = cmpct_request_read_vartime(s, idx[..]);
  print("  getblocktxn with deltas 28672, 36862, 0: %\n", STATUS[l.code]);

  // High-bandwidth peers.
  print("high-bandwidth peers\n");
  list: [4] int; inbound: [8] bool;
  inbound[3] = true; inbound[4] = true; inbound[5] = true;
  order := [6] int.[1, 3, 4, 5, 4, 2];
  hb := CmpctHb.{};
  for i: 0..<6 {
    p := order[i];
    hb = cmpct_hb_choose_vartime(list[..], hb.n, p, inbound[..], true, false);
    print("  peer % (%) delivers a block; high-bandwidth:", p, ifx inbound[p] then "inbound" else "outbound");
    for j: 0..<hb.n { print(" %", list[j]); }
    if hb.on >= 0 { print("; sendcmpct(1) to %", hb.on); }
    if hb.off >= 0 { print(", sendcmpct(0) to %", hb.off); }
    print("\n");
  }

  // Replies by depth, with the tip at 900,000.
  print("replies, tip at 900000\n");
  print("  getdata(MSG_CMPCT_BLOCK) for 899995: %, for 899994: %\n",
    REPLY[cmpct_getdata_reply(899995, 900000, true)], REPLY[cmpct_getdata_reply(899994, 900000, true)]);
  print("  getblocktxn for 899990: %, for 899989: %, with no indexes: %\n",
    REPLY[cmpct_getblocktxn_reply(1, true, 899990, 900000)], REPLY[cmpct_getblocktxn_reply(1, true, 899989, 900000)],
    REPLY[cmpct_getblocktxn_reply(0, true, 900000, 900000)]);
}
```

```shell
coax/bin/coax run bitcoin/examples/compact-blocks.coax
```

It prints:

```text
block 413,567, nonce 000b1e55ed413567
  SHA-256(header || nonce): 1766e76b63a0629e1678d7992cab38c0e77f875fa4bcd2d916779e2765586e04
  k0: 9e62a0636be76617, k1: c038ab2c99d77816
  short IDs of transactions 1 to 3: c8c57540116d 89921a6298b8 803c423f68f5
  cmpctblock: 1556 short IDs and the 185-byte coinbase, 9614 bytes
block 514,999, nonce 000b1e55ed514999
  cmpctblock: 18 short IDs and the coinbase, 449 bytes; the block, 5044 bytes
  mempool: 16 transactions, 15 of them in the block; extra pool: transaction 9
  init: ok, 1 prefilled, 15 from the mempool, 1 from the extra pool
  missing: 4 15
  getblocktxn: 9710a60eebdc424c688520330526b7da0531bbe5a0fd0800000000000000000002040a
  blocktxn: 2 transactions, 481 bytes
  fill: ok, the block byte for byte: true
prefilled transactions
  indexes 0 5 6 18 are sent as 0 4 0 11; 15 short IDs, 1405 bytes
collisions
  one short ID twice in the cmpctblock
  init: failed
  transaction 3 in the mempool and another under its wtxid in the extra pool
  init: ok, 1 prefilled, 16 from the mempool, 0 from the extra pool
  missing: 3 4
  fill: ok, the block byte for byte: true
  another transaction under transaction 4's wtxid
  init: ok, 1 prefilled, 17 from the mempool, 1 from the extra pool
  fill: failed, bad-txnmrklroot
  transaction 10 with a witness byte changed, under its wtxid
  init: ok, 1 prefilled, 17 from the mempool, 1 from the extra pool
  fill: failed, bad-witness-merkle-match
malformed
  a prefilled index past the short IDs
  init: invalid
  getblocktxn for index 65535: ok
  getblocktxn with deltas 28672, 36862, 0: differential value overflow
high-bandwidth peers
  peer 1 (outbound) delivers a block; high-bandwidth: 1; sendcmpct(1) to 1
  peer 3 (inbound) delivers a block; high-bandwidth: 1 3; sendcmpct(1) to 3
  peer 4 (inbound) delivers a block; high-bandwidth: 1 3 4; sendcmpct(1) to 4
  peer 5 (inbound) delivers a block; high-bandwidth: 1 4 5; sendcmpct(1) to 5, sendcmpct(0) to 3
  peer 4 (inbound) delivers a block; high-bandwidth: 1 5 4
  peer 2 (outbound) delivers a block; high-bandwidth: 5 4 2; sendcmpct(1) to 2, sendcmpct(0) to 1
replies, tip at 900000
  getdata(MSG_CMPCT_BLOCK) for 899995: cmpctblock, for 899994: block
  getblocktxn for 899990: blocktxn, for 899989: block, with no indexes: disconnect
```

Block 514,999 has 19 transactions in 5,044 bytes, and its `cmpctblock`
is 449. The mempool holds 15 of the 18 transactions after the coinbase,
in reverse order, and one transaction that isn't in the block.
Transaction 9 is in the extra pool, so only 4 and 15 are missing. The
`getblocktxn` asks for them as the deltas 4 and 10, and the rebuilt block
equals the original byte for byte.

The collisions show each outcome:

- The same short ID twice in the message fails at once.
- Transaction 3 in the mempool and a different transaction under its
  wtxid in the extra pool collide. Slot 3 is asked for with slot 4, and
  the block is rebuilt.
- A different transaction stored under transaction 4's wtxid passes
  `InitData` and fails in `FillBlock` with `bad-txnmrklroot`.
- Transaction 10 with one witness byte changed has the same txid, so the
  Merkle root still matches. The witness commitment doesn't:
  `bad-witness-merkle-match`.

In the high-bandwidth run, peers 3, 4 and 5 are inbound. When peer 5
arrives, the list is 1, 3, 4 with peer 1 the only outbound peer and the
oldest. Peers 1 and 3 swap, and peer 3 leaves instead of peer 1. Peer 4
delivering again only moves it to the end. Outbound peer 2 then pushes
out the oldest peer, which is now peer 1.

## How the code is checked

### Checks in the example

The checks are in
[bitcoin/examples/checks/compact-blocks.coax](../examples/checks/compact-blocks.coax),
which loads the example. They rebuild block 514,999 with each of its
transactions prefilled beside the coinbase, round-trip `getblocktxn` for
every subset of six indexes, and check that short IDs fit 48 bits and
change with the nonce:

<!-- source: bitcoin/examples/checks/compact-blocks.coax check -->
```jai
// Any prefilled set rebuilds block 514,999 byte for byte from a mempool
// that holds every other transaction: the coinbase and transaction i are
// sent whole, and the differential indexes put i back where it was.
#forall "any prefilled transaction rebuilds" i: 1..<19 #steps 400000000 {
  ins: [12] TxIn; outs: [8] TxOut; items: [16] TxItem;
  b: [6000] u8; wids: [32 * 20] u8; btx: [20] CmpctTx; c: [6000] u8;
  pre: [24] CmpctPrefilled; slots: [24] CmpctSlot; map: [24] CmpctEntry; out: [6000] u8; hashes: [96 * 24] u8;
  blk := decode(MK_BLOCK_514999, b[..]);
  w := cmpct_block_txs_vartime(blk, btx[..], wids[..], ins[..], outs[..], items[..]);
  two := [2] int.[0, i];
  n := cmpct_write_vartime(blk[0..<80], CB_NONCE_514999, wids[0..<32*w.count], blk, btx[0..<w.count], two[..], c[..]);
  k := cmpct_read_vartime(c[0..<n], pre[..], ins[..], outs[..], items[..]);
  #assert k.code == CMPCT_OK && k.nprefilled == 2 && k.nshort == w.count - 2;
  #assert pre[0].diff == 0 && pre[1].diff == i - 1;
  s := cmpct_init_vartime(c[0..<n], k, pre[..], wids[0..<32*w.count], wids[0..<0], wids[0..<0], slots[..], map[..]);
  #assert s.status == CMPCT_OK && s.prefilled == 2 && s.mempool == w.count - 2;
  f := cmpct_fill_vartime(c[0..<n], pre[..], slots[0..<s.count], blk, btx[0..<w.count], btx[0..<0],
    blk[0..<0], btx[0..<0], true, out[..], ins[..], outs[..], items[..], hashes[..]);
  #assert f.status == CMPCT_OK && equal_bytes(out[0..<f.n], blk);
}
// getblocktxn's differential indexes round-trip for every subset of the
// first six indexes, read back in increasing order.
#forall "getblocktxn round-trips" m: 1..<64 {
  hash: [32] u8; idx: [6] int; got: [6] int; q: [64] u8;
  count := 0;
  for j: 0..<6 { if (m >> j) & 1 == 1 { idx[count] = j * 1000; count += 1; } }
  r := cmpct_request_write_vartime(hash[..], idx[0..<count], q[..]);
  l := cmpct_request_read_vartime(q[0..<r], got[..]);
  #assert l.code == CMPCT_OK && l.count == count;
  for j: 0..<count { #assert got[j] == idx[j]; }
}
// Short IDs are 48 bits, and the nonce changes them: each nonce gives new
// SipHash keys, so a collision under one nonce is unlikely under the next.
#forall "short IDs are 48 bits and move with the nonce" nonce: 0..<32 #steps 200000000 {
  b: [6000] u8; wids: [32 * 20] u8; btx: [20] CmpctTx;
  ins: [12] TxIn; outs: [8] TxOut; items: [16] TxItem;
  blk := decode(MK_BLOCK_514999, b[..]);
  w := cmpct_block_txs_vartime(blk, btx[..], wids[..], ins[..], outs[..], items[..]);
  a := cmpct_keys(blk[0..<80], cast(u64) nonce);
  z := cmpct_keys(blk[0..<80], cast(u64) nonce + 1);
  for i: 1..<w.count {
    s := cmpct_short_id(a, wids[32*i..<32*i+32]);
    #assert s >> 48 == 0 && s != cmpct_short_id(z, wids[32*i..<32*i+32]);
  }
}
```

```shell
coax/bin/coax test bitcoin/examples/checks/compact-blocks.coax
```

`coax test` runs 112 checks, including the loaded files' own checks, in
about 16 seconds on 2026-11-03.

### The library's own checks

`cmpctblock.coax` is an `#authority` file. Its checks port the cases of
Bitcoin Core's `src/test/blockencodings_tests.cpp`:
`SimpleRoundTripTest`, `NonCoinbasePreforwardRTTest`,
`SufficientPreforwardRTTest`, `EmptyBlockRoundTripTest`,
`ReceiveWithExtraTransactions`, and the three `TransactionsRequest`
tests. Core's test blocks draw their hashes from `FastRandomContext(42)`;
here each draw is a SHA-256 stand-in in the same order, because the tests'
outcomes depend only on which transactions are where. The checks also
compare block 413,567's keys, first short IDs and whole message with
`messages.py`'s, the test block's `cmpctblock` and a `blocktxn` with
`messages.py`'s bytes, and run malformed messages, a duplicate short ID,
the readers' refusals, the high-bandwidth choice and the replies by
depth.

```shell
coax/bin/coax test coax/lib/bitcoin/cmpctblock.coax
```

`coax test` runs 106 checks, including those of the files it loads, in
about 5 seconds on 2026-11-03. Mutation testing: not run (owner
speed order).

### Bitcoin Core's code and functional-test framework

[make_cmpct_vectors.py](../../coax/tests/bitcoin/make_cmpct_vectors.py)
builds the vectors with Bitcoin Core's `test_framework/messages.py`:
`HeaderAndShortIDs` computes the keys and short IDs, with `siphash256`
from the pinned `test_framework/crypto/siphash.py`, and
`P2PHeaderAndShortWitnessIDs`, `BlockTransactionsRequest` and
`BlockTransactions` write the messages. It builds Core's test blocks as
`blockencodings_tests.cpp` builds them, and writes everything into
[cmpct_vectors.coax](../../coax/tests/bitcoin/cmpct_vectors.coax).

[cmpct.py](../../coax/tests/bitcoin/cmpct.py) computes every line the
example prints. It transliterates `InitData` and `FillBlock` from the
pinned `blockencodings.cpp`, with chapter 49's `IsBlockMutated`, and
`MaybeSetPeerAsAnnouncingHeaderAndIDs` and the depth limits from
`net_processing.cpp`, reads every message back with `messages.py`, and
compares the transcript with the fixture:

```shell
python3 -I coax/tests/bitcoin/cmpct.py --coax coax/bin/coax
```

On 2026-11-03 all 43 lines agree, and the example's output in native
debug and release builds equals them. Wasm was not run.

## What this chapter leaves out

- Version 1 messages, with txids and without witnesses: no current node
  sends them.
- The 12-entry bucket limit of Core's short-ID hash table, a defense of
  that table rather than a rule of the protocol.
- Downloading a block from several peers at once, and the timeouts that
  switch to another peer: chapter 72.
- Relaying a `cmpctblock` before the block is fully validated, which
  needs the block's header checks: chapters 47 and 57.
- The mempool and the extra pool themselves: chapters 82 to 84.

## Summary

- `sendcmpct(announce, 2)` turns on compact blocks with wtxids; announce
  picks high-bandwidth (1) or low-bandwidth (0) mode.
- A short ID is SipHash-2-4 of the wtxid, keyed by SHA-256 of the header
  and a nonce, cut to 6 bytes, so that no one can grind collisions ahead
  of time.
- A `cmpctblock` is the header, the nonce, the short IDs and a few
  prefilled transactions at differential indexes; Core prefills the
  coinbase.
- The receiver matches short IDs against its mempool and extra pool,
  asks for the rest with `getblocktxn` and gets them in `blocktxn`.
- A collision found in matching is asked for; one missed there is caught
  by `IsBlockMutated` on the rebuilt block, and the node fetches the
  whole block.
- At most three peers announce in high-bandwidth mode, and an inbound
  peer never pushes out the last outbound one.

## Dependencies

- [10. SipHash-2-4](ch010-siphash-2-4.md)
- [35. Txids, wtxids and malleability](ch035-txids-wtxids-and-malleability.md)
- [49. The witness commitment](ch049-the-witness-commitment.md)
- [65. P2P messages and checksums](ch065-p2p-messages-and-checksums.md)
- [68. The handshake and feature negotiation](ch068-the-handshake-and-feature-negotiation.md)

## Sources

- [BIP-152](https://github.com/bitcoin/bips/blob/master/bip-0152.mediawiki),
  pinned in
  [coax/tests/vectors/upstream/bip152/](../../coax/tests/vectors/upstream/bip152/bip-0152.mediawiki)
  at revision `927b6de9`.
- Bitcoin Core at revision `ddea9196`: `blockencodings.h`,
  `blockencodings.cpp` (`PartiallyDownloadedBlock`),
  `test/blockencodings_tests.cpp` and
  `test_framework/crypto/siphash.py`, pinned in
  [coax/tests/vectors/upstream/bitcoin-core/](../../coax/tests/vectors/upstream/bitcoin-core/blockencodings.cpp)
  by this chapter; `net_processing.cpp`
  (`MaybeSetPeerAsAnnouncingHeaderAndIDs`, `MAX_CMPCTBLOCK_DEPTH`,
  `MAX_BLOCKTXN_DEPTH`), `test_framework/messages.py` and
  `src/bench/data/block413567.raw`, pinned there by earlier chapters.
- Mainnet block 514,999, pinned in
  [coax/tests/vectors/upstream/mainnet/](../../coax/tests/vectors/upstream/mainnet/)
  by chapter 46.
- All pinned files are listed in [SOURCES.md](../../coax/tests/vectors/SOURCES.md).
