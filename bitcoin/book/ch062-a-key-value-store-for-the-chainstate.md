# 62. A key-value store for the chainstate

Chapter 56 gave each coin a key and a record. A node needs somewhere to
keep millions of them that survives a restart and, more important, a
crash. Bitcoin Core uses LevelDB, a general-purpose log-structured
database. This chapter builds a much smaller store of its own. It is not
compatible with LevelDB and is not meant to be: it does the four things a
chainstate needs and states exactly what it guarantees.

- `get`, `put` and `delete` on byte-string keys and values.
- Batches: a set of puts and deletes that lands entirely or not at all.
- Ordered iteration, from the first key or from any key.
- Crash safety: after a crash at any moment, the store reopens to exactly
  the last batch it committed.

The chapter also explains the obfuscation key that Core applies to every
value it writes, and why it exists.

## What this chapter teaches

- An append-only log as a database: one checksummed frame per batch, and
  replay on open.
- What "committed" means: the frame is appended and `fsync` has returned.
- Why a crash can damage only the last frame, what that damage looks
  like, and how replay tells it apart from damage to committed bytes.
- Compaction by copy-on-write: a new file, `fsync`, `rename`, and an
  `fsync` of the directory.
- Bitcoin Core's obfuscation key: what it is, what it is for, and why it
  is not encryption.
- Seven result codes.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [kvstore.coax](../../coax/lib/bitcoin/kvstore.coax) | `kv_compare` | byte order |
| | `kv_find_vartime`, `kv_get_vartime`, `kv_set_vartime`, `kv_delete_vartime` | the sorted table in memory |
| | `kv_entry_vartime`, `kv_seek_vartime` | ordered iteration |
| | `kv_batch_new`, `kv_batch_put_vartime`, `kv_batch_delete_vartime` | building a batch |
| | `kv_op_vartime`, `kv_batch_check_vartime`, `kv_apply_vartime` | reading, checking and applying a batch |
| | `kv_mask`, `kv_mask_batch` | Core's obfuscation |
| | `kv_header_vartime`, `kv_seal_vartime`, `kv_commit_vartime` | the log's header and frames |
| | `kv_open_vartime` | replay, with the torn-tail rule |
| | `kv_snapshot_vartime` | compaction |
| [hash.coax](../../coax/lib/bitcoin/hash.coax) | `sha256d_checked` | the checksums (chapter 5) |

The store is pure logic over byte views. It builds the bytes a host must
write and reads the bytes a host gives it. It never touches a file: Coax
checks run in an interpreter that cannot, and keeping I/O out makes every
rule checkable. A host does the four system calls the design needs:
`write`, `fsync`, `rename` and `ftruncate`.
[kvstore.py](../../coax/tests/bitcoin/kvstore.py) is such a host, on real
files.

Keys and values are public chain data, so everything runs in variable
time.

<!-- source: coax/lib/bitcoin/kvstore.coax codes -->
```jai
KV_OK :: 0;
KV_NOT_FOUND :: 1;   // no such key, or the end of iteration
KV_FULL :: 2;        // the caller's table or output is too small
KV_TOO_LONG :: 3;    // an empty key, or a key or value over its limit
KV_BAD_BATCH :: 4;   // a batch whose bytes do not read as one
KV_CORRUPT :: 5;     // committed bytes of the log are damaged: refuse to open
KV_BAD_HEADER :: 6;  // not a log of this store, or its header is damaged

KV_KEY_MAX :: 1024;
KV_VALUE_MAX :: 1048576;
KV_HEADER :: 20;          // the log's header
KV_FRAME_HEAD :: 8;       // a frame's length and checksum
KV_BODY_HEAD :: 12;       // a batch's sequence number and count
KV_FRAME_MAX :: 67108864; // the largest body a frame may claim
KV_ENTRY_HEAD :: 6;       // a table entry's key and value lengths
KV_PUT :: 1;
KV_DELETE :: 2;
KV_VERSION :: 1;
```

<!-- source: coax/lib/bitcoin/kvstore.coax types -->
```jai
// Where a key is in the table, or where it would go.
KvFind :: struct { at: int; found: bool; }
// A value read: its length, which is set even when out is too small.
KvGet :: struct { code: int; n: int; }
// One table entry, for iteration: its key, its value and the next entry.
KvEntry :: struct { code: int; next: int; key: [] u8; value: [] u8; }
// One operation of a batch.
KvOp :: struct { code: int; kind: int; key: [] u8; value: [] u8; value_at: int; next: int; }
// A whole batch checked: its sequence number, its operations, and the most
// the table can grow by applying it.
KvBatch :: struct { code: int; seq: u64; count: int; grow: int; }
// A commit: the table's new length and the frame's length in the output.
KvCommit :: struct { code: int; used: int; n: int; }
// A log opened: the table's length, the last sequence number, how many
// frames replayed, where the file's good bytes end and whether a torn tail
// must be cut there, and the obfuscation key.
KvOpen :: struct { code: int; used: int; seq: u64; frames: int; keep: int; cut: bool; mask: u64; }
```

## The table

In memory the store is one sorted array of entries, packed end to end:
a 2-byte key length, a 4-byte value length, the key, then the value. A
lookup walks it from the start and stops at the first key that is not
smaller. A put moves the entries after it to make room or close a gap.
Every operation is linear in the table's size. That is slow for a full
UTXO set, and it is the simplest structure that keeps keys in order with
no allocation. Chapter 63's cache sits in front of it, and a faster
index can replace it without changing the file format.

Keys sort as LevelDB's default comparator sorts them: by bytes, with a
prefix before any longer key.

<!-- source: coax/lib/bitcoin/kvstore.coax compare -->
```jai
// Byte order, as LevelDB's default comparator: the first differing byte
// decides, and a key that is a prefix of another sorts first.
kv_compare :: (a: [] u8, b: [] u8) -> int {
  n := ifx a.count < b.count then a.count else b.count;
  for i: 0..<n {
    if a[i] != b[i] { return ifx a[i] < b[i] then -1 else 1; }
  }
  if a.count == b.count { return 0; }
  return ifx a.count < b.count then -1 else 1;
}
```

<!-- source: coax/lib/bitcoin/kvstore.coax table -->
```jai
// The table is entries in key order, packed from byte 0 to used: each is
// a 2-byte key length, a 4-byte value length, the key, then the value.
kv_entry_size :: (key_n: int, value_n: int) -> int { return KV_ENTRY_HEAD + key_n + value_n; }
kv_key_ok :: (key: [] u8) -> bool { return key.count >= 1 && key.count <= KV_KEY_MAX; }

// The first entry whose key is at least key: its offset, and whether its
// key is key itself.
kv_find_vartime :: (table: [] u8, used: int, key: [] u8) -> KvFind {
  at := 0;
  while at < used {
    kn := kv_get16(table, at); vn := kv_get32(table, at + 2);
    c := kv_compare(table[at+KV_ENTRY_HEAD..<at+KV_ENTRY_HEAD+kn], key);
    if c >= 0 { return KvFind.{at = at, found = c == 0}; }
    at += kv_entry_size(kn, vn);
  }
  return KvFind.{at = used};
}
// Get: the value of key into out.
kv_get_vartime :: (table: [] u8, used: int, key: [] u8, out: [] u8) -> KvGet {
  f := kv_find_vartime(table, used, key);
  if !f.found { return KvGet.{code = KV_NOT_FOUND}; }
  kn := kv_get16(table, f.at); vn := kv_get32(table, f.at + 2);
  if vn > out.count { return KvGet.{code = KV_FULL, n = vn}; }
  from := f.at + KV_ENTRY_HEAD + kn;
  for i: 0..<vn { out[i] = table[from+i]; }
  return KvGet.{n = vn};
}
// Move table[from..<used] to start at to, in either direction.
kv_move :: (table: [] u8, from: int, to: int, used: int) {
  n := used - from;
  if to > from { i := n - 1; while i >= 0 { table[to+i] = table[from+i]; i -= 1; } }
  if to < from { for i: 0..<n { table[to+i] = table[from+i]; } }
}
// Put: insert key or replace its value, keeping the order. Returns the
// table's new length, or minus a code with the table unchanged.
kv_set_vartime :: (table: [] u8, used: int, key: [] u8, value: [] u8) -> int {
  if !kv_key_ok(key) || value.count > KV_VALUE_MAX { return -KV_TOO_LONG; }
  f := kv_find_vartime(table, used, key);
  old := 0;
  if f.found { old = kv_entry_size(kv_get16(table, f.at), kv_get32(table, f.at + 2)); }
  size := kv_entry_size(key.count, value.count);
  if used - old + size > table.count { return -KV_FULL; }
  kv_move(table, f.at + old, f.at + size, used);
  kv_put16(table, f.at, key.count); kv_put32(table, f.at + 2, value.count);
  for i: 0..<key.count { table[f.at+KV_ENTRY_HEAD+i] = key[i]; }
  for i: 0..<value.count { table[f.at+KV_ENTRY_HEAD+key.count+i] = value[i]; }
  return used - old + size;
}
// Delete: remove key if it is there. Returns the table's new length.
kv_delete_vartime :: (table: [] u8, used: int, key: [] u8) -> int {
  f := kv_find_vartime(table, used, key);
  if !f.found { return used; }
  size := kv_entry_size(kv_get16(table, f.at), kv_get32(table, f.at + 2));
  kv_move(table, f.at + size, f.at, used);
  return used - size;
}
```

A put that does not fit returns `KV_FULL` and leaves the table as it was.
The caller owns the table's memory and decides how large it is.

Iteration hands back views into the table. A seek finds the first key at
or after a given one, which is how a node walks every coin of one
transaction: seek to `'C'` and the txid, then read while the prefix
matches.

<!-- source: coax/lib/bitcoin/kvstore.coax iterate -->
```jai
// The entry at offset at: KV_NOT_FOUND at the end. Start at 0 for the whole
// table, or at kv_seek_vartime's offset for the keys from a given one.
kv_entry_vartime :: (table: [] u8, used: int, at: int) -> KvEntry {
  if at >= used { return KvEntry.{code = KV_NOT_FOUND, next = used}; }
  kn := kv_get16(table, at); vn := kv_get32(table, at + 2);
  k := at + KV_ENTRY_HEAD;
  return KvEntry.{next = k + kn + vn, key = table[k..<k+kn], value = table[k+kn..<k+kn+vn]};
}
kv_seek_vartime :: (table: [] u8, used: int, key: [] u8) -> int {
  return kv_find_vartime(table, used, key).at;
}
```

## Batches

A batch is a list of operations, encoded as the body of a log frame: an
8-byte sequence number, a 4-byte count, then each operation.

```text
seq (8)  count (4)  [01 klen(2) vlen(4) key value]  [02 klen(2) key]  ...
                     put                             delete
```

<!-- source: coax/lib/bitcoin/kvstore.coax batch -->
```jai
// A batch is the body of a frame: an 8-byte sequence number (set when it
// is committed), a 4-byte count, then each operation: a kind byte, a
// 2-byte key length, for a put a 4-byte value length, the key, the value.
// Each builder returns where the next operation goes, or minus a code with
// the batch unchanged.
kv_batch_new :: (batch: [] u8) -> int #export_c {
  if batch.count < KV_BODY_HEAD { return -KV_FULL; }
  for i: 0..<KV_BODY_HEAD { batch[i] = 0; }
  return KV_BODY_HEAD;
}
kv_batch_put_vartime :: (batch: [] u8, at: int, key: [] u8, value: [] u8) -> int #export_c {
  if !kv_key_ok(key) || value.count > KV_VALUE_MAX { return -KV_TOO_LONG; }
  if at < KV_BODY_HEAD || at > batch.count || 7 + key.count + value.count > batch.count - at { return -KV_FULL; }
  batch[at] = KV_PUT; kv_put16(batch, at + 1, key.count); kv_put32(batch, at + 3, value.count);
  for i: 0..<key.count { batch[at+7+i] = key[i]; }
  for i: 0..<value.count { batch[at+7+key.count+i] = value[i]; }
  kv_put32(batch, 8, kv_get32(batch, 8) + 1);
  return at + 7 + key.count + value.count;
}
kv_batch_delete_vartime :: (batch: [] u8, at: int, key: [] u8) -> int #export_c {
  if !kv_key_ok(key) { return -KV_TOO_LONG; }
  if at < KV_BODY_HEAD || at > batch.count || 3 + key.count > batch.count - at { return -KV_FULL; }
  batch[at] = KV_DELETE; kv_put16(batch, at + 1, key.count);
  for i: 0..<key.count { batch[at+3+i] = key[i]; }
  kv_put32(batch, 8, kv_get32(batch, 8) + 1);
  return at + 3 + key.count;
}
// The operation at at, read back.
kv_op_vartime :: (body: [] u8, at: int) -> KvOp {
  bad := KvOp.{code = KV_BAD_BATCH};
  if body.count - at < 3 { return bad; }
  kind := cast(int) body[at]; kn := kv_get16(body, at + 1);
  if kn < 1 || kn > KV_KEY_MAX { return bad; }
  if kind == KV_DELETE {
    if kn > body.count - at - 3 { return bad; }
    return KvOp.{kind = kind, key = body[at+3..<at+3+kn], next = at + 3 + kn};
  }
  if kind != KV_PUT || body.count - at < 7 { return bad; }
  vn := kv_get32(body, at + 3);
  if vn > KV_VALUE_MAX || kn + vn > body.count - at - 7 { return bad; }
  v := at + 7 + kn;
  return KvOp.{kind = kind, key = body[at+7..<v], value = body[v..<v+vn], value_at = v, next = v + vn};
}
// The whole batch: its count must match its operations, which must end
// exactly at its end. grow bounds what applying it adds to the table.
kv_batch_check_vartime :: (body: [] u8) -> KvBatch {
  if body.count < KV_BODY_HEAD { return KvBatch.{code = KV_BAD_BATCH}; }
  count := kv_get32(body, 8);
  at := KV_BODY_HEAD; grow := 0;
  for i: 0..<count {
    op := kv_op_vartime(body, at);
    if op.code != KV_OK { return KvBatch.{code = KV_BAD_BATCH}; }
    if op.kind == KV_PUT { grow += kv_entry_size(op.key.count, op.value.count); }
    at = op.next;
  }
  if at != body.count { return KvBatch.{code = KV_BAD_BATCH}; }
  return KvBatch.{seq = kv_get64(body, 0), count = count, grow = grow};
}
// Apply a checked batch to the table, in order, after unmasking each value
// with mask (0 for a batch the caller built). Returns the new length, or
// minus KV_FULL with the table unchanged: the check comes first, so a batch
// lands whole or not at all.
kv_apply_vartime :: (table: [] u8, used: int, body: [] u8, mask: u64) -> int {
  b := kv_batch_check_vartime(body);
  if b.code != KV_OK { return -b.code; }
  if b.grow > table.count - used { return -KV_FULL; }
  at := KV_BODY_HEAD; u := used;
  for i: 0..<b.count {
    op := kv_op_vartime(body, at);
    if op.kind == KV_DELETE { u = kv_delete_vartime(table, u, op.key); }
    if op.kind == KV_PUT {
      u = kv_set_vartime(table, u, op.key, op.value);
      f := kv_find_vartime(table, u, op.key);
      kv_mask(table[f.at+KV_ENTRY_HEAD+op.key.count..<f.at+KV_ENTRY_HEAD+op.key.count+op.value.count], mask);
    }
    at = op.next;
  }
  return u;
}
```

A batch is atomic in memory because it is checked before it is applied.
The check reads every operation and sums, for each put, the size of the
entry it could add. If the table has that much room, no step of the batch
can fail; if not, nothing changes. The bound is conservative: a batch
that deletes as much as it adds is still refused by a nearly full table.

Atomic on disk comes from the log.

## The log

The file is a 20-byte header, then one frame per committed batch.

```text
header   "OAKV"  version 01  flags  00 00  mask (8)  checksum (4)
frame    length (4)  checksum (4)  body (length bytes)
```

Each checksum is the first 4 bytes of SHA-256d, the same construction as
a P2P message header (chapter 65). It is there to detect damage, not an
attacker: anyone who can write the file can recompute it.

<!-- source: coax/lib/bitcoin/kvstore.coax frame -->
```jai
// The log's header: "OAKV", version 1, a flags byte (bit 0: values are
// masked), two zero bytes, the mask, then a checksum of those 16 bytes.
// Every checksum here is the first 4 bytes of SHA-256d, as in a P2P
// message header.
kv_checksum :: (data: [] u8, out: [] u8) {
  d: [32] u8;
  #assert sha256d_checked(data, d[..], false) == SHA_OK;
  for i: 0..<4 { out[i] = d[i]; }
}
kv_checksum_ok :: (data: [] u8, sum: [] u8) -> bool {
  c: [4] u8;
  kv_checksum(data, c[..]);
  for i: 0..<4 { if c[i] != sum[i] { return false; } }
  return true;
}
kv_header_vartime :: (mask: u64, out: [] u8) -> int #export_c {
  if out.count < KV_HEADER { return -KV_FULL; }
  out[0] = 'O'; out[1] = 'A'; out[2] = 'K'; out[3] = 'V';
  out[4] = KV_VERSION; out[5] = ifx mask != 0 then cast(u8) 1 else 0; out[6] = 0; out[7] = 0;
  kv_put64(out, 8, mask);
  kv_checksum(out[0..<16], out[16..<20]);
  return KV_HEADER;
}
// A frame: a 4-byte body length, the body's checksum, then the body. Seal
// the plain batch already in frame[8..<8+n]: set its sequence number, mask
// its values, then fill in the length and checksum.
kv_seal_vartime :: (frame: [] u8, n: int, seq: u64, mask: u64) {
  body := frame[KV_FRAME_HEAD..<KV_FRAME_HEAD+n];
  kv_put64(body, 0, seq);
  kv_mask_batch(body, mask);
  kv_put32(frame, 0, n);
  kv_checksum(body, frame[4..<8]);
}
// Commit a batch: apply it to the table and write the frame to append.
// The batch is committed when the host has appended the frame and fsync
// has returned, and not before. Nothing changes on a refusal.
kv_commit_vartime :: (table: [] u8, used: int, seq: u64, batch: [] u8, mask: u64, frame: [] u8) -> KvCommit {
  b := kv_batch_check_vartime(batch);
  if b.code != KV_OK { return KvCommit.{code = b.code}; }
  if batch.count > KV_FRAME_MAX { return KvCommit.{code = KV_TOO_LONG}; }
  if frame.count < KV_FRAME_HEAD + batch.count || b.grow > table.count - used { return KvCommit.{code = KV_FULL}; }
  for i: 0..<batch.count { frame[KV_FRAME_HEAD+i] = batch[i]; }
  kv_seal_vartime(frame, batch.count, seq + 1, mask);
  u := kv_apply_vartime(table, used, batch, 0);
  return KvCommit.{used = u, n = KV_FRAME_HEAD + batch.count};
}
```

A commit has two halves. `kv_commit_vartime` checks the batch, writes the
frame into the caller's buffer with the next sequence number and applies
the batch to the table. Then the host appends the frame and calls
`fsync`. The batch is committed when `fsync` returns, and not before: a
node must not tell anyone it has a block's coins until then. If the
append or the `fsync` fails, the host must drop the table and reopen the
file, because the table is ahead of the disk.

### Why only the last frame can be torn

`fsync` returns only when the file's bytes and its length are on the
disk. The host appends one frame and waits for `fsync` before it appends
another. So when the power fails, every frame but the one being appended
is complete on disk, and the last one is in one of these states:

- not there at all;
- a prefix of its bytes, and the file ends there;
- a prefix of its bytes, then zeros up to its full length, when the file
  system grew the file before the data reached the disk.

That gives the replay rule. A frame whose length runs past the end of the
file is torn. So is a frame whose checksum fails when only zeros follow
it, and a length that is not a valid one when only zeros follow its 4
bytes (part of the length was written). Replay stops at a torn frame and
asks the host to cut the file there, which loses only a batch that was
never committed. Any other damage is to bytes that were committed, and
replay refuses to open the store with `KV_CORRUPT` instead of skipping a
batch and serving a chainstate that is silently wrong.

<!-- source: coax/lib/bitcoin/kvstore.coax open -->
```jai
// Are the bytes from at to the end all zero? A file that grew in a crash
// but whose new bytes were never written reads as zeros.
kv_zero_from :: (file: [] u8, at: int) -> bool {
  for i: at..<file.count { if file[i] != 0 { return false; } }
  return true;
}
// Open a log: check the header and replay every frame into the table.
// Only the last frame can be damaged by a crash, because each append is
// fsynced before the next, and a torn append has one shape: a prefix of
// the frame, then nothing or zeros to the end of the file. So replay cuts
// a frame that runs past the end, a length that is not one with only
// zeros after its 4 bytes (part of the length was written), and a frame
// whose checksum fails with only zeros after it. It stops there and asks
// the host to cut the file at keep. Damage anywhere else is in committed
// bytes, and replay refuses rather than guess.
kv_open_vartime :: (file: [] u8, table: [] u8) -> KvOpen {
  if file.count < KV_HEADER || file[0] != 'O' || file[1] != 'A' || file[2] != 'K' || file[3] != 'V' { return KvOpen.{code = KV_BAD_HEADER}; }
  if file[4] != KV_VERSION || !kv_checksum_ok(file[0..<16], file[16..<20]) { return KvOpen.{code = KV_BAD_HEADER}; }
  r := KvOpen.{mask = kv_get64(file, 8)};
  at := KV_HEADER;
  while at < file.count {
    if file.count - at < KV_FRAME_HEAD { r.cut = true; break; }
    n := kv_get32(file, at);
    if n >= KV_BODY_HEAD && n <= KV_FRAME_MAX && n > file.count - at - KV_FRAME_HEAD { r.cut = true; break; }
    if n < KV_BODY_HEAD || n > KV_FRAME_MAX {
      if kv_zero_from(file, at + 4) { r.cut = true; break; }
      return KvOpen.{code = KV_CORRUPT, keep = at};
    }
    end := at + KV_FRAME_HEAD + n;
    body := file[at+KV_FRAME_HEAD..<end];
    if !kv_checksum_ok(body, file[at+4..<at+8]) {
      if kv_zero_from(file, end) { r.cut = true; break; }
      return KvOpen.{code = KV_CORRUPT, keep = at};
    }
    b := kv_batch_check_vartime(body);
    if b.code != KV_OK || b.seq == 0 || (r.frames > 0 && b.seq != r.seq + 1) { return KvOpen.{code = KV_CORRUPT, keep = at}; }
    u := kv_apply_vartime(table, r.used, body, r.mask);
    if u < 0 { return KvOpen.{code = -u, keep = at}; }
    r.used = u; r.seq = b.seq; r.frames += 1;
    at = end;
  }
  r.keep = at;
  return r;
}
```

One case the rule cannot tell apart: a flipped bit in a committed frame's
length that makes it point past the end of the file looks like a torn
append, and replay cuts from there. Telling the two apart needs a
committed length kept somewhere else (LevelDB's MANIFEST plays that
role). The chapter accepts that limit and states it. The damage tests
include such flips, and the Coax and Python readers must agree on each.

Sequence numbers catch a different mistake. Each frame carries the next
number, and replay refuses a frame that does not follow. A frame copied
from another log, or left over from an old one, does not replay.

### Compaction

The log only grows: a coin that is created and spent leaves two
operations behind. Compaction writes the table as a fresh log: a header
and one frame of puts, carrying the last sequence number so the next
commit follows on.

<!-- source: coax/lib/bitcoin/kvstore.coax snapshot -->
```jai
// Compaction: the whole table as a new log, a header and one frame of puts
// carrying the last sequence number, so that the next commit follows on.
// The host writes it to a new file, fsyncs it, renames it over the old log
// and fsyncs the directory. Returns its length, or minus a code.
kv_snapshot_vartime :: (table: [] u8, used: int, seq: u64, mask: u64, out: [] u8) -> int #export_c {
  if kv_header_vartime(mask, out) < 0 { return -KV_FULL; }
  if seq == 0 { return ifx used == 0 then KV_HEADER else -KV_BAD_BATCH; }
  body := out[KV_HEADER+KV_FRAME_HEAD..<out.count];
  at := kv_batch_new(body);
  if at < 0 { return -KV_FULL; }
  e := kv_entry_vartime(table, used, 0);
  while e.code == KV_OK {
    at = kv_batch_put_vartime(body, at, e.key, e.value);
    if at < 0 { return at; }
    e = kv_entry_vartime(table, used, e.next);
  }
  if at > KV_FRAME_MAX { return -KV_TOO_LONG; }
  kv_seal_vartime(out[KV_HEADER..<out.count], at, seq, mask);
  return KV_HEADER + KV_FRAME_HEAD + at;
}
```

The host makes the switch crash-safe by copy-on-write:

1. Write the snapshot to a new file, `chainstate.tmp`.
2. `fsync` it.
3. `rename` it over `chainstate.log`. POSIX `rename` is atomic: any
   process, and the disk after a crash, sees either the old file or the
   new one.
4. `fsync` the directory, so that the rename itself is on the disk.

A crash before step 3 leaves the old log, and the host deletes the
temporary file when it next opens the store. A crash after step 3 leaves
the new one. The first header is written the same way.

## Bitcoin Core's obfuscation key

Core's chainstate stores coins, and coins hold scriptPubKeys, and anyone
can put bytes in a scriptPubKey. People have paid to put byte strings
that antivirus software recognizes as malware signatures into outputs.
A scanner that finds such a pattern in a LevelDB file may quarantine or
delete the file, and the node's database is then broken.

Since 2015 (Core 0.12), Core masks every value it writes to the
chainstate with an 8-byte key drawn at random when the database is
created. It stores the key itself, unmasked, under the key
`"\0obfuscate_key"`. Byte `i` of a value is XORed with byte `i mod 8` of
the key. `util/obfuscation.h` does it a word at a time with rotated
copies of the key, and the effect is the same. An attacker who does not
know the key cannot choose the bytes that land on disk.

It is not encryption. The key is in the same database, an XOR with a
repeating 8-byte key is trivial to undo, and keys are not masked at all.
It protects the node from its own antivirus software, nothing more.

<!-- source: coax/lib/bitcoin/kvstore.coax mask -->
```jai
// Bitcoin Core's obfuscation (util/obfuscation.h): every value is XORed
// with an 8-byte key, byte i of the value with byte i mod 8 of the key.
// Core stores the key, unmasked, under "\0obfuscate_key". Here it is in
// the header, as a little-endian u64. A key of zero changes nothing, and
// masking twice unmasks.
kv_mask :: (value: [] u8, mask: u64) {
  for i: 0..<value.count { value[i] ^= cast(u8) ((mask >> cast(u64) (8 * (i % 8))) & 255); }
}
// Mask every value of a batch in place.
kv_mask_batch :: (body: [] u8, mask: u64) {
  count := kv_get32(body, 8); at := KV_BODY_HEAD;
  for i: 0..<count {
    op := kv_op_vartime(body, at);
    kv_mask(body[op.value_at..<op.value_at+op.value.count], mask);
    at = op.next;
  }
}
```

This store keeps the key in its header and masks the values in each
frame. A key of zero masks nothing, as in Core, where a database created
before 0.12 has no key.

## The result codes

| Code | Meaning | Returned by |
| --- | --- | --- |
| `KV_OK` | done | all |
| `KV_NOT_FOUND` | no such key; the end of iteration | get, iteration |
| `KV_FULL` | the caller's table or buffer is too small | put, the batch builders, commit, open, snapshot |
| `KV_TOO_LONG` | an empty key, a key over 1,024 bytes or a value over 1 MiB | put, the batch builders, commit |
| `KV_BAD_BATCH` | the bytes are not a batch | the batch check, apply, commit |
| `KV_CORRUPT` | committed bytes of the log are damaged | open |
| `KV_BAD_HEADER` | not this store's log, or its header is damaged | open |

## The example

The example is
[bitcoin/examples/chainstate-store.coax](../examples/chainstate-store.coax).
It keeps the log in a byte array instead of a file, commits two batches,
reads and iterates, reopens, crashes at every byte of the second append,
damages a committed byte and compacts.

<!-- source: bitcoin/examples/chainstate-store.coax example -->
```jai
#load "../../coax/lib/bitcoin/kvstore.coax";
#load "../../coax/lib/nostr/hex.coax";
#import "Basic";

CODES :: [7] string.["ok", "not found", "full", "too long", "bad batch", "corrupt", "bad header"];
// Bytes as lowercase hex.
show :: (label: string, bytes: [] u8) {
  text: [600] u8;
  r := hex_encode_checked(cast(string) bytes, text[..], false);
  print("%: %\n", label, cast(string) text[0..<r.written]);
}
// A string's bytes in out.
bytes_of :: (s: string, out: [] u8) -> [] u8 {
  for i: 0..<s.count { out[i] = s[i]; }
  return out[0..<s.count];
}
// Every key and value of the table, in order.
dump :: (label: string, table: [] u8, used: int) {
  print("%:", label);
  e := kv_entry_vartime(table, used, 0);
  while e.code == KV_OK {
    print(" %=%", cast(string) e.key, cast(string) e.value);
    e = kv_entry_vartime(table, used, e.next);
  }
  print("\n");
}
// Add a put or a delete to a batch, by string.
put :: (batch: [] u8, at: int, key: string, value: string) -> int {
  k: [32] u8; v: [32] u8;
  return kv_batch_put_vartime(batch, at, bytes_of(key, k[..]), bytes_of(value, v[..]));
}
del :: (batch: [] u8, at: int, key: string) -> int {
  k: [32] u8;
  return kv_batch_delete_vartime(batch, at, bytes_of(key, k[..]));
}

main :: () {
  // The "disk": the log's bytes. The host would write them to a file.
  file: [512] u8; table: [512] u8; batch: [128] u8;
  mask := cast(u64) 0x2bd7d3a1c06f5a19;   // fixed here; Core draws it at random
  n := kv_header_vartime(mask, file[..]);
  show("header", file[0..<n]);

  // Batch 1: three keys.
  at := kv_batch_new(batch[..]);
  at = put(batch[..], at, "coin:a", "50");
  at = put(batch[..], at, "coin:c", "25");
  at = put(batch[..], at, "coin:b", "12.5");
  c := kv_commit_vartime(table[..], 0, 0, batch[0..<at], mask, file[n..<file.count]);
  print("batch 1: %, frame of % bytes\n", CODES[c.code], c.n);
  show("  frame", file[n..<n+c.n]);
  n += c.n;
  used := c.used;
  dump("  table", table[..], used);

  // Batch 2: spend coin:a and add coin:d, together.
  at = kv_batch_new(batch[..]);
  at = del(batch[..], at, "coin:a");
  at = put(batch[..], at, "coin:d", "6.25");
  c = kv_commit_vartime(table[..], used, 1, batch[0..<at], mask, file[n..<file.count]);
  print("batch 2: %, frame of % bytes\n", CODES[c.code], c.n);
  first := n;
  n += c.n;
  used = c.used;
  dump("  table", table[..], used);

  // Reads and ordered iteration from a key.
  value: [16] u8; key: [16] u8;
  g := kv_get_vartime(table[..], used, bytes_of("coin:c", key[..]), value[..]);
  print("get coin:c: % %\n", CODES[g.code], cast(string) value[0..<g.n]);
  g = kv_get_vartime(table[..], used, bytes_of("coin:a", key[..]), value[..]);
  print("get coin:a: %\n", CODES[g.code]);
  e := kv_entry_vartime(table[..], used, kv_seek_vartime(table[..], used, bytes_of("coin:bb", key[..])));
  print("first key from coin:bb: %\n", cast(string) e.key);

  // The value bytes on disk are masked: batch 1's "50" is at the first
  // put's value.
  show("  \"50\" on disk", file[20+8+12+7+6..<20+8+12+7+6+2]);

  // Reopen the log from scratch.
  fresh: [512] u8;
  r := kv_open_vartime(file[0..<n], fresh[..]);
  print("reopen: %, sequence %, % frames\n", CODES[r.code], r.seq, r.frames);
  dump("  table", fresh[..], r.used);

  // A crash during batch 2's append: the file ends at any byte of it.
  one := 0; two := 0; cuts := 0;
  for end: first..<n+1 {
    q := kv_open_vartime(file[0..<end], fresh[..]);
    if q.seq == 1 { one += 1; }
    if q.seq == 2 { two += 1; }
    if q.cut { cuts += 1; }
  }
  print("crash at each of % lengths: % reopen to batch 1 (% cut), % to batch 2\n", n + 1 - first, one, cuts, two);
  // The same, with the rest of the frame left as zeros.
  zeros: [512] u8;
  for i: 0..<first+5 { zeros[i] = file[i]; }
  q := kv_open_vartime(zeros[0..<n], fresh[..]);
  print("five bytes of batch 2, then zeros: %, sequence %, cut at %\n", CODES[q.code], q.seq, q.keep);

  // Damage to a committed byte: refused, not skipped.
  file[30] ^= 1;
  q = kv_open_vartime(file[0..<n], fresh[..]);
  print("a flipped bit in batch 1: % at byte %\n", CODES[q.code], q.keep);
  file[30] ^= 1;

  // Compaction: the table as a new log of one frame.
  snap: [512] u8;
  s := kv_snapshot_vartime(table[..], used, 2, mask, snap[..]);
  q = kv_open_vartime(snap[0..<s], fresh[..]);
  print("compacted: % bytes instead of %, reopens %, sequence %\n", s, n, CODES[q.code], q.seq);
  dump("  table", fresh[..], q.used);
}
```

It prints:

```text
header: 4f414b5601010000195a6fc0a1d3d72b912a8b86
batch 1: ok, frame of 67 bytes
  frame: 3b000000020e4c2101000000000000000300000001060002000000636f696e3a612c6a01060002000000636f696e3a632b6f01060004000000636f696e3a62286841f5
  table: coin:a=50 coin:b=12.5 coin:c=25
batch 2: ok, frame of 46 bytes
  table: coin:b=12.5 coin:c=25 coin:d=6.25
get coin:c: ok 25
get coin:a: not found
first key from coin:bb: coin:c
  "50" on disk: 2c6a
reopen: ok, sequence 2, 2 frames
  table: coin:b=12.5 coin:c=25 coin:d=6.25
crash at each of 47 lengths: 46 reopen to batch 1 (45 cut), 1 to batch 2
five bytes of batch 2, then zeros: ok, sequence 1, cut at 87
a flipped bit in batch 1: corrupt at byte 20
compacted: 89 bytes instead of 133, reopens ok, sequence 2
  table: coin:b=12.5 coin:c=25 coin:d=6.25
```

The frame starts with its length, `3b000000` (59), and the checksum. The
body's sequence number is 1 and its count 3. The value `"50"` (`3530`)
is `2c6a` on disk: `35 XOR 19` and `30 XOR 5a`, the mask's first two
bytes in little-endian order.

The book's checks run this program in debug and release builds and
compare the output with
[bitcoin/fixtures/chainstate-store.out](../fixtures/chainstate-store.out).

## How the code is checked

### Checks in the example

The checks are in
[bitcoin/examples/checks/chainstate-store.coax](../examples/checks/chainstate-store.coax),
which loads the example:

<!-- source: bitcoin/examples/checks/chainstate-store.coax check -->
```jai
// A log of `count` batches over eight keys, each batch a seeded mix of puts
// and deletes. Returns the log's length; the expected value of key k after
// the last batch is in want[k] (0 for absent, else its one-byte value).
cs_log :: (file: [] u8, count: int, mask: u64, want: [] u8) -> int {
  table: [512] u8; batch: [128] u8; key: [2] u8; value: [1] u8;
  n := kv_header_vartime(mask, file);
  used := 0; x := cast(u64) 62;
  for k: 0..<8 { want[k] = 0; }
  for b: 0..<count {
    at := kv_batch_new(batch[..]);
    for j: 0..<3 {
      x = add_wrap(mul_wrap(x, 6364136223846793005), 1442695040888963407);
      k := cast(int) ((x >> 33) % 8);
      key[0] = 'k'; key[1] = cast(u8) ('0' + k);
      if ((x >> 40) & 3) == 0 {
        at = kv_batch_delete_vartime(batch[..], at, key[..]);
        want[k] = 0;
      } else {
        value[0] = cast(u8) (1 + ((x >> 45) % 200));
        at = kv_batch_put_vartime(batch[..], at, key[..], value[..]);
        want[k] = value[0];
      }
    }
    c := kv_commit_vartime(table[..], used, cast(u64) b, batch[0..<at], mask, file[n..<file.count]);
    used = c.used; n += c.n;
  }
  return n;
}
// The table holds exactly want.
cs_holds :: (table: [] u8, used: int, want: [] u8) -> bool {
  key: [2] u8; out: [4] u8; count := 0;
  for k: 0..<8 {
    key[0] = 'k'; key[1] = cast(u8) ('0' + k);
    g := kv_get_vartime(table, used, key[..], out[..]);
    if want[k] == 0 && g.code != KV_NOT_FOUND { return false; }
    if want[k] != 0 && (g.code != KV_OK || g.n != 1 || out[0] != want[k]) { return false; }
    if want[k] != 0 { count += 1; }
  }
  // And nothing else, in order.
  e := kv_entry_vartime(table, used, 0); seen := 0; last: [2] u8;
  while e.code == KV_OK {
    if seen > 0 && kv_compare(last[..], e.key) >= 0 { return false; }
    last[0] = e.key[0]; last[1] = e.key[1];
    seen += 1; e = kv_entry_vartime(table, used, e.next);
  }
  return seen == count;
}
// Every log of 1 to 12 batches reopens to its last batch, masked or not.
#forall "a log of n batches reopens to batch n" n: 1..<13 #steps 400000000 {
  file: [1024] u8; table: [512] u8; want: [8] u8;
  for mask: [2] u64.[0, 0x2bd7d3a1c06f5a19] {
    size := cs_log(file[..], n, mask, want[..]);
    r := kv_open_vartime(file[0..<size], table[..]);
    #assert r.code == KV_OK && r.seq == cast(u64) n && r.frames == n && !r.cut;
    #assert cs_holds(table[..], r.used, want[..]);
  }
}
// Cut a log of 6 batches at every byte of its last frame: each reopens to batch 5.
#test "a crash at every byte of the last append reopens to the batch before" #steps 4000000000 {
  file: [1024] u8; table: [512] u8; want5: [8] u8; want6: [8] u8;
  first := cs_log(file[..], 5, 7, want5[..]);
  n := cs_log(file[..], 6, 7, want6[..]);
  for end: first..<n {
    r := kv_open_vartime(file[0..<end], table[..]);
    #assert r.code == KV_OK && r.seq == 5 && r.keep == first && r.cut == (end > first);
    #assert cs_holds(table[..], r.used, want5[..]);
  }
}
// Compacting at any point gives the same table, and commits follow on.
#test "compaction keeps the table and the sequence" #steps 4000000000 {
  file: [1024] u8; table: [512] u8; snap: [512] u8; again: [512] u8; want: [8] u8;
  n := cs_log(file[..], 9, 0x55, want[..]);
  r := kv_open_vartime(file[0..<n], table[..]);
  s := kv_snapshot_vartime(table[..], r.used, r.seq, 0x55, snap[..]);
  q := kv_open_vartime(snap[0..<s], again[..]);
  #assert q.code == KV_OK && q.seq == 9 && q.frames == 1 && cs_holds(again[..], q.used, want[..]);
  #assert s < n;
}
```

```shell
coax/bin/coax test bitcoin/examples/checks/chainstate-store.coax
```

On 2026-11-03 `coax test` runs 70 checks, including those of the files it
loads. Mutation testing: not run (owner speed order).

### The library's own checks

`kvstore.coax` is an `#authority` file. Its checks cover:

- little-endian fields and byte order, with prefixes and high bytes;
- put, replace with longer and shorter values, get with a short buffer,
  and delete, keeping the order; puts that do not fit or are too long,
  with the table unchanged;
- iteration from the start and seeks between keys and past the end;
- a batch built and read back, its count disagreeing with its operations,
  every cut of it, an unknown kind and a zero-length key; the builders'
  refusals;
- a batch that fits and one that cannot, which changes nothing;
- Core's obfuscation on ten bytes, unmasked by a second pass;
- the header, commit and reopen with and without a mask, and commit's
  refusals;
- a torn last frame at every length, zeros after a partial frame and
  after a partial length, and zeros after the last whole frame;
- damage to a committed body, checksum or length, a sequence number that
  does not follow, a frame whose checksum holds over bytes that are not a
  batch, and damaged headers;
- compaction with and without a mask, and its refusals; and the C
  interface.

```shell
coax/bin/coax test coax/lib/bitcoin/kvstore.coax
```

On 2026-11-03 it runs 64 checks, including those of the files it loads.

### Real files, natively

[kvstore.py](../../coax/tests/bitcoin/kvstore.py) compiles `kvstore.coax`
to C in checked and release builds. It has its own Python writer and
reader of the format, which uses a transliteration of Core's
`Obfuscation` class for the mask, and a host that does the I/O the
chapter describes on real files in a temporary directory.

- Workloads: three logs per build of 60 seeded batches each, with and
  without a mask, over 44 keys. After every commit the table, read in
  order, by key and by seek, must equal a Python dict, and the frame must
  equal the Python writer's bytes. Then 200 direct puts and deletes, and a
  snapshot that must equal the Python writer's and reopen to the same
  table.
- Crashes: a recorded workload of 16 steps on a real file, with `fsync`
  after each append, and two compactions. For every append, the file is
  rewritten as every prefix of the frame, and every prefix padded with
  zeros to the frame's length, then reopened. The store must hold exactly
  the last committed batch, the host cuts the torn tail with `ftruncate`
  and `fsync`, and the file must then equal the log before the append. A
  compaction that crashes with a partial temporary file must reopen to
  the old log.
- Damage: every byte of a five-batch log flipped three ways. The Coax
  reader and the Python reader must agree on each: open, cut or refuse,
  the table, the sequence number and where the good bytes end. All three
  outcomes must occur.

```shell
COAX_TEST_COMPILER=coax/bin/coax python3 coax/tests/bitcoin/kvstore.py
```

On 2026-11-03 the three tests pass in about 90 seconds, with 5,136 crash
points, 2,100 damaged logs, 360 commits and 4 compactions across the two
builds.

## What this chapter leaves out

- LevelDB itself: its sorted tables, levels, MANIFEST and background
  compaction. Core's chainstate is LevelDB, and this store does not read
  it.
- Concurrency. One thread owns the store; a reader during a commit is
  the caller's problem.
- Speed. Every operation is linear in the table's size, and the whole
  table is in memory. Chapter 63 puts the coins cache in front of the
  store and flushes it in batches, as Core's `CCoinsViewDB::BatchWrite`
  does with its `'H'` and `'B'` markers.
- A committed length kept outside the log, which would let replay tell a
  damaged length from a torn append.
- `fsync` on platforms where it does not reach the disk. On macOS,
  `fsync` does not flush the drive's write cache; `F_FULLFSYNC` does.

## Summary

- The store is a sorted table in memory and an append-only log on disk.
  A batch is one frame: a length, a 4-byte SHA-256d checksum and the
  operations.
- A batch is committed when its frame is appended and `fsync` returns.
  Open replays every frame.
- Only the frame being appended can be torn. Replay cuts a torn tail and
  refuses any other damage, so it never serves a wrong chainstate.
- Compaction writes the table as a new log and swaps it in with `fsync`,
  `rename` and a directory `fsync`.
- Core XORs every chainstate value with a random 8-byte key so that
  antivirus software cannot be made to flag its files. It is not
  encryption.

## Dependencies

- [1. Bytes, hex and base64](ch001-bytes-hex-and-base64.md)
- [5. SHA-256d and tagged hashes](ch005-sha-256d-and-tagged-hashes.md)

## Sources

- Bitcoin Core at revision `ddea9196`: `util/obfuscation.h`,
  `dbwrapper.cpp`, `dbwrapper.h` and `txdb.cpp`, pinned in
  [coax/tests/vectors/upstream/bitcoin-core/](../../coax/tests/vectors/upstream/bitcoin-core/obfuscation.h).
  The obfuscation key was added in Bitcoin Core pull request
  [6650](https://github.com/bitcoin/bitcoin/pull/6650).
- All are listed in [SOURCES.md](../../coax/tests/vectors/SOURCES.md).
