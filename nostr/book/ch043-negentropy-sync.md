# 43. Negentropy sync

Two relays that hold mostly the same events can find the few they don't
share without sending every ID. This chapter builds Negentropy protocol V1,
range-based set reconciliation, as the appendix of NIP-77 specifies it, and
the four NIP-77 frames that carry its messages: `NEG-OPEN`, `NEG-MSG`,
`NEG-CLOSE` and `NEG-ERR`.

Part IX, A relay. Result: NIP-77's `NEG-OPEN`, `NEG-MSG`, `NEG-CLOSE` and
`NEG-ERR`, and Negentropy protocol V1: range-based set reconciliation over
`(created_at, id)` with fingerprints.

## What this chapter teaches

- Items are `(created_at, id)` pairs, sorted by timestamp and then by ID.
- A message is the version byte `0x61` followed by ranges. Each range has
  an upper bound, a mode (`Skip`, `Fingerprint` or `IdList`) and a payload.
- Varints, and bounds written as a timestamp step and a short ID prefix.
- The fingerprint of a range: the IDs added mod 2^256 as little-endian
  numbers, then the count as a varint, SHA-256, first 16 bytes.
- One round for either side, and rounds repeated until the client knows
  the IDs it has and the server lacks (`have`) and the reverse (`need`).
- The frame limit, and how a message that would pass it ends early.
- What is not built: the relay process (`coax/relay/relay.coax`) does not
  answer `NEG-OPEN` yet. Holding a session per connection, `NEG-ERR`
  `blocked:` over a record limit and `closed:` after a timeout, and
  replication between Coax relays are out of scope; see
  [What is not built](#what-is-not-built).

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [nip77.coax](../../coax/lib/nostr/nip77.coax) | `neg_put_varint`, `neg_get_varint` | Varints |
| | `neg_put_bound`, `neg_get_bound`, `neg_between` | Bounds, and the shortest bound between two neighbouring items |
| | `neg_add`, `neg_fingerprint` | The 256-bit sum and the fingerprint of a range |
| | `neg_sort`, `neg_lower`, `neg_init` | Sorting items, finding a bound, and one side's state |
| | `neg_initiate`, `neg_split` | The client's first message, and a range split into buckets |
| | `neg_reconcile`, `neg_compare`, `neg_text` | One round for either side, the client's reading of an IdList, and what each refusal means |
| | `n77_read`, `n77_write_open`, `n77_write_msg`, `n77_write_close`, `n77_write_err` | The NIP-77 frames |
| [nip77_fixtures.coax](../../coax/tests/nostr/nip77_fixtures.coax) | | Items, whole syncs and the Python transcription's values, for the library's checks |
| [negentropy_ref.py](../../coax/tests/nostr/negentropy_ref.py) | | A Python transcription of the protocol, the independent check |

`nip77.coax` is new. It loads only `json.coax`, `hex.coax` and chapter 4's
SHA-256, so it carries none of the relay's code. It decides neither money
nor admission, so it is not `#authority`. Its checks were written first,
and `coax mutate --root-only` on it kills 751 of 751 mutants.

The protocol's types and error codes:

<!-- source: coax/lib/nostr/nip77.coax codes -->
```jai
NEG_V1 :: 0x61;
NEG_SKIP :: 0; NEG_FINGERPRINT :: 1; NEG_IDLIST :: 2;
NEG_BUCKETS :: 16;
NEG_INFINITY :: 9223372036854775807;
NEG_FRAME_MIN :: 4096;
// What neg_init, neg_initiate and neg_reconcile refuse.
NEG_OK :: 0;
NEG_SHORT :: 1;        // the message ends inside a field, or is empty
NEG_VARINT :: 2;       // a varint over 2^63 - 2
NEG_VERSION_BYTE :: 3; // the first byte is not 0x60 to 0x6f
NEG_VERSION :: 4;      // a version this side can't speak (the client's refusal)
NEG_MODE :: 5;         // a mode other than 0, 1, 2
NEG_BOUND :: 6;        // an id prefix over 32 bytes, or a timestamp past the range
NEG_FRAME :: 7;        // a frame limit from 1 to 4095
NEG_SPACE :: 8;        // the output, have or need buffer is full
NEG_ORDER :: 9;        // items not strictly ascending, or a timestamp out of range
NEG_ROLE :: 10;        // neg_initiate on the server's side

NegItem :: struct { created: int; id: [32] u8; }
NegId :: struct { b: [32] u8; }
// A bound: a timestamp and the first n bytes of an id (the rest are zero).
NegBound :: struct { created: int; n: int; prefix: [32] u8; }
// One side of a sync. items are sorted; have and need are the client's
// answers (ids it has and the server lacks, and the reverse).
Neg :: struct {
  items: [] NegItem; initiator: bool; frame_limit: int;
  have: [] NegId; have_n: int; need: [] NegId; need_n: int;
  error: int;
}
// A message being written: the bytes so far and the last timestamp written.
NegOut :: struct { buf: [] u8; n: int; last: int; full: bool; }
// A message being read.
NegIn :: struct { msg: string; at: int; last: int; error: int; }
```

## Items and ranges

Each side starts with the events that match the sync's filter, as items: a
64-bit timestamp and a 32-byte ID. Both sides sort them the same way:
by `created_at`, and by ID, byte by byte, when timestamps are equal.

A message covers the whole timestamp and ID space with adjacent ranges.
The first range starts at timestamp 0 and an empty ID. Each range names
only its upper bound, which is exclusive; its lower bound is the previous
range's upper bound. A message whose last range doesn't end at infinity
has an implied `Skip` from there to infinity.

`neg_lower` finds where a bound falls in the sorted items. Ranges arrive in
ascending order, so one message walks the items once. `neg_init` refuses
items that are not strictly ascending (a duplicate included) and
timestamps outside `0` to `2^63 - 2`.

<!-- source: coax/lib/nostr/nip77.coax items -->
```jai
neg_less :: (a: NegItem, b: NegItem) -> bool {
  if a.created < b.created { return true; }
  if a.created > b.created { return false; }
  for i: 0..<32 { if a.id[i] < b.id[i] { return true; } if a.id[i] > b.id[i] { return false; } }
  return false;
}
// item < bound: the bound's missing id bytes are zero.
neg_below :: (a: NegItem, b: NegBound) -> bool {
  if a.created < b.created { return true; }
  if a.created > b.created { return false; }
  for i: 0..<b.n { if a.id[i] < b.prefix[i] { return true; } if a.id[i] > b.prefix[i] { return false; } }
  return false;
}
// The first item at or after from that is not below the bound. Ranges come
// in ascending order, so one message walks the items once.
neg_lower :: (items: [] NegItem, from: int, b: NegBound) -> int {
  i := from;
  while i < items.count && neg_below(items[i], b) { i += 1; }
  return i;
}
// Heap sort by (created_at, id).
neg_sift :: (v: [] NegItem, root: int, n: int) {
  at := root;
  while 2 * at + 1 < n {
    c := 2 * at + 1;
    if c + 1 < n && neg_less(v[c], v[c + 1]) { c += 1; }
    if !neg_less(v[at], v[c]) { return; }
    t := v[at]; v[at] = v[c]; v[c] = t; at = c;
  }
}
neg_sort :: (v: [] NegItem) {
  for k: 0..<v.count / 2 { neg_sift(v, v.count / 2 - 1 - k, v.count); }
  for k: 1..<v.count { end := v.count - k; t := v[0]; v[0] = v[end]; v[end] = t; neg_sift(v, 0, end); }
}
// One side over sorted items. frame_limit is 0 (none) or at least 4096.
neg_init :: (n: [] Neg, items: [] NegItem, initiator: bool, frame_limit: int) -> int {
  n[0] = Neg.{items = items, initiator = initiator, frame_limit = frame_limit};
  if frame_limit != 0 && frame_limit < NEG_FRAME_MIN { n[0].error = NEG_FRAME; return NEG_FRAME; }
  for i: 0..<items.count {
    if items[i].created < 0 || items[i].created >= NEG_INFINITY || (i > 0 && !neg_less(items[i - 1], items[i])) {
      n[0].error = NEG_ORDER; return NEG_ORDER;
    }
  }
  return NEG_OK;
}
```

## Varints

NIP-77 writes every integer as a varint: base-128 digits, most significant
first, with the high bit set on every byte but the last.

### Worked example: four varints by hand

1. `127` is one digit: `7f`.
2. `128 = 1·128 + 0`: digits 1 and 0, so `81 00`.
3. `300 = 2·128 + 44`: digits 2 and 44 (`0x2c`), so `82 2c`.
4. `1793491200 = 6·128^4 + 87·128^3 + 26·128^2 + 2·128 + 0`, the
   example's clock (2026-11-01T00:00:00Z): digits 6, 87, 26, 2 and 0. With
   the high bit on the first four, `86 d7 9a 82 00`.

The example prints the same bytes. `neg_get_varint` refuses a value of
2^63 − 1 or more (`NEG_VARINT`), since this file keeps 2^63 − 1 for
infinity. Nostr timestamps are far below that.

<!-- source: coax/lib/nostr/nip77.coax varint -->
```jai
neg_put_byte :: (w: [] NegOut, b: int) {
  if w[0].n >= w[0].buf.count { w[0].full = true; return; }
  w[0].buf[w[0].n] = cast(u8) b; w[0].n += 1;
}
// Base-128 digits, most significant first; the high bit on all but the last.
neg_put_varint :: (w: [] NegOut, v: int) {
  if v < 0 { w[0].full = true; return; }
  d: [10] u8; k := 0; x := v;
  while true { d[k] = cast(u8) (x & 127); k += 1; x >>= 7; if x == 0 { break; } }
  if w[0].n + k > w[0].buf.count { w[0].full = true; return; }
  while k > 1 { k -= 1; neg_put_byte(w, cast(int) d[k] | 128); }
  neg_put_byte(w, cast(int) d[0]);
}
neg_get_varint :: (r: [] NegIn) -> int {
  v := 0;
  while r[0].error == NEG_OK {
    if r[0].at >= r[0].msg.count { r[0].error = NEG_SHORT; break; }
    c := cast(int) r[0].msg[r[0].at]; r[0].at += 1;
    if v > (NEG_INFINITY >> 7) { r[0].error = NEG_VARINT; break; }
    v = (v << 7) | (c & 127);
    if c < 128 {
      if v == NEG_INFINITY { r[0].error = NEG_VARINT; break; }
      return v;
    }
  }
  return 0;
}
```

## Bounds

A bound is an encoded timestamp, an ID prefix length and the prefix:

- The timestamp is `0` for infinity. Any other is written as `1 +` the step
  from the previous timestamp written in the same message. The first step
  is from 0.
- The prefix is as long as it must be to separate the last item of the
  previous range from the first item of this one: empty when their
  timestamps differ, otherwise their common prefix plus one byte. Missing
  bytes are zeros.

The protocol's infinity is the timestamp 2^64 − 1. This file uses ints, so
`NEG_INFINITY` (2^63 − 1) stands for it, and a decoded step that would reach
it is refused (`NEG_BOUND`). After an infinite bound, every later bound in
the message reads as infinity, as the reference implementation reads it.

<!-- source: coax/lib/nostr/nip77.coax bound -->
```jai
// A bound: 0 for infinity, else 1 + the step from the last timestamp
// written; then the prefix's length and bytes.
neg_put_bound :: (w: [] NegOut, b: NegBound) {
  if b.created == NEG_INFINITY { neg_put_varint(w, 0); }
  else { neg_put_varint(w, b.created - w[0].last + 1); }
  w[0].last = b.created;
  neg_put_varint(w, b.n);
  for i: 0..<b.n { neg_put_byte(w, cast(int) b.prefix[i]); }
}
neg_get_bound :: (r: [] NegIn) -> NegBound {
  b: NegBound;
  code := neg_get_varint(r);
  if code == 0 || r[0].last == NEG_INFINITY { b.created = NEG_INFINITY; }
  else if code - 1 >= NEG_INFINITY - r[0].last { r[0].error = NEG_BOUND; }
  else { b.created = r[0].last + code - 1; }
  r[0].last = b.created;
  b.n = neg_get_varint(r);
  if b.n > 32 { r[0].error = NEG_BOUND; }
  if r[0].error != NEG_OK { return b; }
  if r[0].at + b.n > r[0].msg.count { r[0].error = NEG_SHORT; return b; }
  for i: 0..<b.n { b.prefix[i] = r[0].msg[r[0].at + i]; }
  r[0].at += b.n;
  return b;
}
// The shortest bound above a and not above b, neighbours in sort order:
// b's timestamp alone, or with b's id up to the first byte that differs.
neg_between :: (a: NegItem, b: NegItem) -> NegBound {
  m: NegBound; m.created = b.created;
  if a.created != b.created { return m; }
  while a.id[m.n] == b.id[m.n] { m.prefix[m.n] = b.id[m.n]; m.n += 1; }
  m.prefix[m.n] = b.id[m.n]; m.n += 1;
  return m;
}
```

## Fingerprints

A fingerprint is a digest of the IDs in a range. Each ID is read as a
32-byte little-endian number. The numbers are added mod 2^256, the count is
appended as a varint, and the first 16 bytes of the SHA-256 of those bytes
are the fingerprint. Addition doesn't depend on order, so both sides get the
same fingerprint for the same set, however they found it.

### Worked example: the fingerprint of two IDs by hand

Take `a = ff ff 00 … 00` and `b = 01 00 … 00` (32 bytes each).

1. Little-endian, `a` is `0xffff` and `b` is `0x0001`.
2. `0xffff + 0x0001 = 0x10000`. Written back as 32 little-endian bytes:
   `00 00 01 00 … 00`. The carry moved from byte 0 through byte 1 into
   byte 2.
3. The count is 2: varint `02`. The hashed input is the 32 sum bytes and
   `02`, 33 bytes.
4. SHA-256 of those 33 bytes is
   `47178f396ea8b5434d8ed8aa88bbbb2324037414f06a94fcb362280ca5121e50`.
5. The fingerprint is its first 16 bytes:
   `47178f396ea8b5434d8ed8aa88bbbb23`.

The sum wraps. `ff … ff` (2^256 − 1) plus `01 00 … 00` is 0 mod 2^256, and
with count 2 the fingerprint is `58cc2f44d3a27866874701fbad573da9`. The
empty range's fingerprint is that of 32 zero bytes and `00`:
`7f9c9e31ac8256ca2f258583df262dbc`. The checks hold all three values.

<!-- source: coax/lib/nostr/nip77.coax fingerprint -->
```jai
// sum += id, both 32-byte little-endian integers, mod 2^256.
neg_add :: (sum: [] u8, id: [32] u8) {
  carry := 0;
  for i: 0..<32 {
    s := cast(int) sum[i] + cast(int) id[i] + carry;
    sum[i] = cast(u8) (s & 255); carry = s >> 8;
  }
}
// The first 16 bytes of SHA-256(sum of the ids || varint(count)).
neg_fingerprint :: (items: [] NegItem, from: int, to: int, out: [] u8) {
  sum: [32] u8; buf: [42] u8;
  for i: from..<to { neg_add(sum[..], items[i].id); }
  for i: 0..<32 { buf[i] = sum[i]; }
  wb: [1] NegOut; w := wb[..]; w[0] = NegOut.{buf = buf[..], n = 32};
  neg_put_varint(w, to - from);
  h: [32] u8;
  #assert sha256_hash_checked(buf[0..<w[0].n], h[..], false) == SHA_OK;
  for i: 0..<16 { out[i] = h[i]; }
}
```

## A round

The client sends its first message with `neg_initiate`. A range of fewer
than 32 items goes as an `IdList`. A larger one is split into 16 buckets of
nearly equal size, each sent as its fingerprint and the bound that ends it.
NIP-77 doesn't fix these numbers; they are the reference implementation's
(hoytech/negentropy).

<!-- source: coax/lib/nostr/nip77.coax split -->
```jai
// Items [lo, hi) up to bound ub: an IdList under 32 items, otherwise 16
// buckets, each with its fingerprint and the bound that ends it.
neg_split :: (n: [] Neg, lo: int, hi: int, ub: NegBound, w: [] NegOut) {
  items := n[0].items;
  count := hi - lo;
  if count < 2 * NEG_BUCKETS {
    neg_put_bound(w, ub); neg_put_varint(w, NEG_IDLIST); neg_put_varint(w, count);
    for i: lo..<hi { for k: 0..<32 { neg_put_byte(w, cast(int) items[i].id[k]); } }
    return;
  }
  per := count / NEG_BUCKETS; extra := count % NEG_BUCKETS; at := lo;
  for k: 0..<NEG_BUCKETS {
    size := ifx k < extra then per + 1 else per;
    fp: [16] u8; neg_fingerprint(items, at, at + size, fp[..]);
    at += size;
    b := ub;
    if at < hi { b = neg_between(items[at - 1], items[at]); }
    neg_put_bound(w, b); neg_put_varint(w, NEG_FINGERPRINT);
    for i: 0..<16 { neg_put_byte(w, cast(int) fp[i]); }
  }
}
neg_over :: (n: [] Neg, bytes: int) -> bool { return n[0].frame_limit != 0 && bytes > n[0].frame_limit - 200; }
// The client's first message: everything, split.
neg_initiate :: (n: [] Neg, out: [] u8) -> int {
  if !n[0].initiator { n[0].error = NEG_ROLE; return -1; }
  wb: [1] NegOut; w := wb[..]; w[0] = NegOut.{buf = out};
  neg_put_byte(w, NEG_V1);
  inf: NegBound; inf.created = NEG_INFINITY;
  neg_split(n, 0, n[0].items.count, inf, w);
  if w[0].full { n[0].error = NEG_SPACE; return -1; }
  return w[0].n;
}
```

`neg_reconcile` answers one message. For each range it reads, it finds the
same range in its own items and acts on the mode:

| Mode received | Server | Client |
| --- | --- | --- |
| `Skip` | nothing to do | nothing to do |
| `Fingerprint`, equal to ours | skip | skip |
| `Fingerprint`, different | split the range (buckets or an `IdList`) | split the range |
| `IdList` | answer with its own IDs in the range | record `have` and `need`; nothing more to say |

Skips are not written at once. A run of skipped ranges is written as one
`Skip` range just before the next range that has something to say, and a
trailing run is left out: the implied `Skip` to infinity covers it. A
client whose answer would be the version byte alone is done, and
`neg_reconcile` returns 0.

<!-- source: coax/lib/nostr/nip77.coax reconcile -->
```jai
// Answers one message into out: its length; 0 when the client is done; -1
// with n.error set on a refusal. The client collects have and need.
neg_reconcile :: (n: [] Neg, msg: string, out: [] u8) -> int {
  n[0].error = NEG_OK;
  items := n[0].items;
  wb: [1] NegOut; w := wb[..]; w[0] = NegOut.{buf = out};
  rb: [1] NegIn; r := rb[..]; r[0] = NegIn.{msg = msg};
  neg_put_byte(w, NEG_V1);
  if msg.count == 0 { return neg_refuse(n, NEG_SHORT); }
  v := cast(int) msg[0]; r[0].at = 1;
  if v < 0x60 || v > 0x6f { return neg_refuse(n, NEG_VERSION_BYTE); }
  if v != NEG_V1 {
    if n[0].initiator { return neg_refuse(n, NEG_VERSION); }
    if w[0].full { return neg_refuse(n, NEG_SPACE); }
    return 1;
  }
  prev: NegBound; prev_at: int; skip := false; sent_at: int;
  while r[0].at < msg.count {
    mark := w[0].n;
    b := neg_get_bound(r);
    mode := neg_get_varint(r);
    if r[0].error != NEG_OK { return neg_refuse(n, r[0].error); }
    lo := prev_at; hi := neg_lower(items, prev_at, b);
    if mode == NEG_SKIP { skip = true; }
    else if mode == NEG_FINGERPRINT {
      if r[0].at + 16 > msg.count { return neg_refuse(n, NEG_SHORT); }
      ours: [16] u8; neg_fingerprint(items, lo, hi, ours[..]);
      same := true;
      for i: 0..<16 { if ours[i] != msg[r[0].at + i] { same = false; } }
      r[0].at += 16;
      if same { skip = true; }
      else {
        if skip { skip = false; neg_put_bound(w, prev); neg_put_varint(w, NEG_SKIP); }
        neg_split(n, lo, hi, b, w);
      }
    } else if mode == NEG_IDLIST {
      count := neg_get_varint(r);
      if r[0].error != NEG_OK || count > (msg.count - r[0].at) / 32 { return neg_refuse(n, NEG_SHORT); }
      ids := msg[r[0].at..<r[0].at + 32 * count];
      r[0].at += 32 * count;
      if n[0].initiator {
        skip = true;
        if !neg_compare(n, lo, hi, ids, count) { return neg_refuse(n, NEG_SPACE); }
      } else {
        if skip { skip = false; neg_put_bound(w, prev); neg_put_varint(w, NEG_SKIP); }
        // send our ids, as many as the frame limit allows
        end := b; k := 0;
        while lo + k < hi && !neg_over(n, mark + 32 * k) { k += 1; }
        if lo + k < hi {
          end.created = items[lo + k].created; end.n = 32; end.prefix = items[lo + k].id;
          hi = lo + k;
        }
        neg_put_bound(w, end); neg_put_varint(w, NEG_IDLIST); neg_put_varint(w, k);
        for i: lo..<hi { for j: 0..<32 { neg_put_byte(w, cast(int) items[i].id[j]); } }
        mark = w[0].n; sent_at = hi;
      }
    } else { return neg_refuse(n, NEG_MODE); }
    if neg_over(n, w[0].n) {
      // over the frame limit: drop this range's output and fingerprint
      // everything from the last bound sent to infinity (an infinite bound
      // doesn't depend on the last timestamp written)
      w[0].n = mark;
      inf: NegBound; inf.created = NEG_INFINITY;
      fp: [16] u8; neg_fingerprint(items, sent_at, items.count, fp[..]);
      neg_put_bound(w, inf); neg_put_varint(w, NEG_FINGERPRINT);
      for i: 0..<16 { neg_put_byte(w, cast(int) fp[i]); }
      break;
    }
    if w[0].n > mark { sent_at = hi; }
    prev_at = hi; prev = b;
  }
  if w[0].full { return neg_refuse(n, NEG_SPACE); }
  if w[0].n == 1 && n[0].initiator { return 0; }
  return w[0].n;
}
neg_refuse :: (n: [] Neg, e: int) -> int { n[0].error = e; return -1; }
```

The client reads the server's `IdList` with `neg_compare`. Its own IDs in
the range that the list lacks are `have`; the list's IDs it lacks are
`need`. An ID listed twice is recorded once.

<!-- source: coax/lib/nostr/nip77.coax idlist -->
```jai
// The client's reading of the server's IdList for items [lo, hi): ours not
// in theirs are have, theirs not in ours are need (each once).
neg_has :: (ids: string, count: int, id: [32] u8) -> bool {
  for j: 0..<count {
    same := true;
    for k: 0..<32 { if ids[32 * j + k] != id[k] { same = false; break; } }
    if same { return true; }
  }
  return false;
}
neg_compare :: (n: [] Neg, lo: int, hi: int, ids: string, count: int) -> bool {
  for i: lo..<hi {
    if !neg_has(ids, count, n[0].items[i].id) {
      if n[0].have_n >= n[0].have.count { return false; }
      n[0].have[n[0].have_n].b = n[0].items[i].id; n[0].have_n += 1;
    }
  }
  for j: 0..<count {
    id: [32] u8; for k: 0..<32 { id[k] = ids[32 * j + k]; }
    if neg_has(ids, j, id) { continue; }
    mine := false;
    for i: lo..<hi { if !mine && neg_has(ids[32 * j..<32 * j + 32], 1, n[0].items[i].id) { mine = true; } }
    if mine { continue; }
    if n[0].need_n >= n[0].need.count { return false; }
    n[0].need[n[0].need_n].b = id; n[0].need_n += 1;
  }
  return true;
}
```

### Versions

The version byte is `0x61` for V1. A byte outside `0x60` to `0x6f` is not a
Negentropy message (`NEG_VERSION_BYTE`). A server that gets another
version, such as `0x62`, answers with the single byte `0x61`, the highest
it speaks, so the client can retry. A client that gets one refuses
(`NEG_VERSION`).

### The frame limit

A side may cap its messages with a frame limit `L`, 0 for none or at least
4096 bytes. When a range would take the message past `L − 200` bytes, the
side drops that range's output and ends the message with one `Fingerprint`
range from the last bound it wrote to infinity. The other side then splits
that range in its answer, and the sync continues over more rounds. A
server answering an `IdList` sends as many IDs as fit and ends its list at
the first ID it didn't send.

One choice differs from the reference implementation. After a cut, the
reference's JavaScript code (`js/Negentropy.js`) fingerprints the items from the end of the dropped
range, but the range it sends starts at the last bound written. If the
other side's items in that range happened to equal the shorter set, the
fingerprints would match and the differences in the dropped part would be
lost. This file fingerprints from the last bound written. Messages
without a cut are byte for byte the reference's.

## NIP-77 frames

The binary messages travel as lowercase hex in four JSON frames:

```text
["NEG-OPEN", <subscription ID>, <filter>, <initial message, hex>]
["NEG-MSG", <subscription ID>, <message, hex>]
["NEG-CLOSE", <subscription ID>]
["NEG-ERR", <subscription ID>, <reason>]
```

`n77_read` reads any of them. It takes the largest binary message this
side accepts, and refuses longer hex before decoding (`N77_FRAME`). The
subscription ID has 1 to 64 characters, as in `REQ`. The filter of a
`NEG-OPEN` is returned as its bytes in the frame, for the filter reader of
[chapter 23](ch023-filters.md); the reason of a `NEG-ERR` likewise, with the optional fourth
element, the relay's record limit. Hex must be plain digits, so a string
with escapes is refused (`N77_HEX`); either case is accepted.

<!-- source: coax/lib/nostr/nip77.coax frames -->
```jai
N77_OPEN :: 1; N77_MSG :: 2; N77_CLOSE :: 3; N77_ERR :: 4;
// What n77_read refuses.
N77_OK :: 0; N77_JSON :: 1; N77_SHAPE :: 2; N77_VERB :: 3; N77_SUB :: 4;
N77_HEX :: 5; N77_FRAME :: 6; N77_FILTER :: 7;
N77_SUB_MAX :: 64;
// A frame as read: the verb, the subscription id, the filter's bytes in the
// text (NEG-OPEN), the message decoded into the caller's buffer (NEG-OPEN,
// NEG-MSG), the reason's raw bytes and the optional record limit (NEG-ERR).
N77Frame :: struct {
  error: int = N77_SHAPE; verb: int;
  sub: [256] u8; sub_n: int;
  filter_from: int; filter_to: int;
  msg_n: int;
  reason_from: int; reason_to: int; max_records: int = -1;
}
```

<!-- source: coax/lib/nostr/nip77.coax read -->
```jai
// One NEG-* frame. max is the largest binary message this side accepts.
n77_read :: (text: string, msg: [] u8, max: int) -> N77Frame {
  f: N77Frame;
  if !json_valid(text) { return n77_fail(f, N77_JSON); }
  j: [1] Json; json_start(j[..], text);
  if json_next(j[..]) != JSON_ARRAY || json_next(j[..]) != JSON_STRING { return f; }
  verb := json_text(j[..]);
  if json_is(verb, "NEG-OPEN") { f.verb = N77_OPEN; } else if json_is(verb, "NEG-MSG") { f.verb = N77_MSG; }
  else if json_is(verb, "NEG-CLOSE") { f.verb = N77_CLOSE; } else if json_is(verb, "NEG-ERR") { f.verb = N77_ERR; }
  else { return n77_fail(f, N77_VERB); }
  if json_next(j[..]) != JSON_STRING { return f; }
  f.sub_n = json_unescape(json_text(j[..]), f.sub[..]);
  c := n77_chars(f.sub[0..<f.sub_n]);
  if c < 1 || c > N77_SUB_MAX { return n77_fail(f, N77_SUB); }
  t := json_next(j[..]);
  if f.verb == N77_OPEN {
    if t != JSON_OBJECT { return n77_fail(f, N77_FILTER); }
    f.filter_from = j[0].from; skipped := json_skip(j[..]); f.filter_to = j[0].to;
    t = json_next(j[..]);
  }
  if f.verb == N77_OPEN || f.verb == N77_MSG {
    if t != JSON_STRING { return f; }
    k := n77_hex(json_text(j[..]), msg, max);
    if k < 0 { return n77_fail(f, -k); }
    f.msg_n = k; t = json_next(j[..]);
  }
  if f.verb == N77_ERR {
    if t != JSON_STRING { return f; }
    f.reason_from = j[0].from; f.reason_to = j[0].to; t = json_next(j[..]);
    if t == JSON_NUMBER {
      m := json_int(json_text(j[..]));
      if !m.ok || m.value < 0 { return f; }
      f.max_records = m.value; t = json_next(j[..]);
    }
  }
  if t != JSON_ARRAY_END { return f; }
  return n77_fail(f, N77_OK);
}
```

<!-- source: coax/lib/nostr/nip77.coax write -->
```jai
n77_head :: (out: [] u8, verb: string, sub: string) -> int {
  n := json_raw(out, 0, "[\""); n = json_raw(out, n, verb); n = json_raw(out, n, "\",");
  return json_write_string(out, n, sub);
}
n77_put_hex :: (out: [] u8, at: int, msg: string) -> int {
  n := json_raw(out, at, ",\"");
  if n == -1 { return -1; }
  r := hex_encode_checked(msg, out[n..<out.count], false);
  if r.error != NC_OK { return -1; }
  return json_raw(out, n + r.written, "\"");
}
n77_write_open :: (out: [] u8, sub: string, filter: string, msg: string) -> int {
  n := n77_head(out, "NEG-OPEN", sub); n = json_raw(out, n, ","); n = json_raw(out, n, filter);
  return json_raw(out, n77_put_hex(out, n, msg), "]");
}
n77_write_msg :: (out: [] u8, sub: string, msg: string) -> int {
  return json_raw(out, n77_put_hex(out, n77_head(out, "NEG-MSG", sub), msg), "]");
}
n77_write_close :: (out: [] u8, sub: string) -> int { return json_raw(out, n77_head(out, "NEG-CLOSE", sub), "]"); }
n77_write_err :: (out: [] u8, sub: string, reason: string) -> int {
  n := json_raw(out, n77_head(out, "NEG-ERR", sub), ",");
  return json_raw(out, json_write_string(out, n, reason), "]");
}
```

## The example

The example is
[nostr/examples/negentropy-sync.coax](../examples/negentropy-sync.coax). It
runs offline: two relays are stores with chapter 36's index, and frames pass
between the client side of relay X and the server side of relay Y as
strings. Each side selects its items with the sync's filter through
`nsi_query_limit`; the server reads the filter from the `NEG-OPEN` frame.

<!-- source: nostr/examples/negentropy-sync.coax example -->
```jai
#load "../../coax/lib/nostr/nip77.coax";
#load "../../coax/lib/nostr/store_index.coax";
#load "../../coax/lib/nostr/event_sign.coax";
#import "Basic";

// The relays' clock for the whole run: 2026-11-01T00:00:00Z.
NOW :: 1793491200;
// BIP-340 vectors 0 to 3's published test keys and aux. Never sign with a
// real key in an example.
KEY_A :: "b7e151628aed2a6abf7158809cf4f3c762e7160f38b4da56a784d9045190cfef";
AUX_A :: "0000000000000000000000000000000000000000000000000000000000000001";
KEY_B :: "c90fdaa22168c234c4c6628b80dc1cd129024e088a67cc74020bbea63b14e5c9";
AUX_B :: "c87aa53824b4d7ae2eb035a2b5bbbccc080e76cdc6d1692c4b0b62d798e6d906";
KEY_C :: "0000000000000000000000000000000000000000000000000000000000000003";
AUX_C :: "0000000000000000000000000000000000000000000000000000000000000000";

// A relay: a store of up to 1024 records and its index (chapter 36).
Relay :: struct {
  arena: [131072] u8; recs: [1024] RsdRec; s: [1] RsdStore;
  by_id: [1024] int; by_ak: [1024] int; by_time: [1024] int; tag_key: [2048] int; tag_rec: [2048] int;
  ix: [1] NsiIndex;
  scratch: [4096] u8;
}
relay_init :: (r: [] Relay) {
  r[0].s[0] = RsdStore.{}; r[0].s[0].arena = r[0].arena[..]; r[0].s[0].recs = r[0].recs[..];
  r[0].ix[0] = NsiIndex.{by_id = r[0].by_id[..], by_ak = r[0].by_ak[..], by_time = r[0].by_time[..], tag_key = r[0].tag_key[..], tag_rec = r[0].tag_rec[..]};
}
// One EVENT through the relay's admission: read, signature, store.
admit :: (r: [] Relay, text: string) -> int {
  a := rs_admit_read(text, NOW, false, false, r[0].scratch[..]);
  if a.reason == RS_ACCEPT { a = rs_admit_verified(a, nostr_event_verify_vartime(text, r[0].scratch[..], false)); }
  if a.reason != RS_ACCEPT { return a.reason; }
  return rsd_admit(r[0].s[..], a, text);
}
sign :: (key_hex: string, aux_hex: string, created_at: int, kind: int, content: string, out: [] u8) -> string {
  key: [32] u8; aux: [32] u8;
  #assert hex_decode_checked(key_hex, key[..], false).error == NC_OK;
  #assert hex_decode_checked(aux_hex, aux[..], false).error == NC_OK;
  work: [1] NostrSignWork; scratch: [8192] u8;
  n := EVENT_MORE;
  while n == EVENT_MORE { n = nostr_event_sign_step(key[..], aux[..], created_at, kind, "[]", content, work[..], scratch[..], out, false); }
  #assert n > 0;
  return cast(string) out[0..<n];
}
to_hex :: (b: [] u8, out: [] u8) -> string {
  digits := "0123456789abcdef";
  for i: 0..<b.count { out[2 * i] = digits[b[i] >> 4]; out[2 * i + 1] = digits[b[i] & 15]; }
  return cast(string) out[0..<2 * b.count];
}

// book:begin items
// The records of a store that match a NIP-01 filter, as sorted items.
Side :: struct { items: [1024] NegItem; n: int; neg: [1] Neg; have: [512] NegId; need: [512] NegId; }
select_items :: (r: [] Relay, filter: string, side: [] Side) -> bool {
  strs: [64] string; ints: [16] int; tags: [16] NostrTagFilter; bytes: [2048] u8;
  sp: [1] FrSpace; sp[0] = FrSpace.{strs = strs[..], ints = ints[..], tags = tags[..], bytes = bytes[..]};
  f: [1] NostrFilter;
  if filter_read(filter, f[..], sp[..]) != FR_OK { return false; }
  at: [1024] int;
  q := nsi_query_limit(r[0].s[..], r[0].ix[..], f[0], NOW, 1024, at[..]);
  if q.n < 0 { return false; }
  for i: 0..<q.n {
    side[0].items[i].created = r[0].recs[at[i]].created; side[0].items[i].id = r[0].recs[at[i]].id;
  }
  side[0].n = q.n;
  neg_sort(side[0].items[0..<q.n]);
  return true;
}
// book:end items

// book:begin sync
// Client x syncs with relay y over NIP-77 frames, with frame limit limit on
// both sides. Each frame is written, read back by the other side, and
// answered. Prints one line per frame unless quiet.
run_sync :: (x: [] Relay, y: [] Relay, filter: string, limit: int, cx: [] Side, sy: [] Side, quiet: bool) -> int {
  frame: [65536] u8; msg: [32768] u8; got: [32768] u8;
  #assert select_items(x, filter, cx);
  #assert neg_init(cx[0].neg[..], cx[0].items[0..<cx[0].n], true, limit) == NEG_OK;
  cx[0].neg[0].have = cx[0].have[..]; cx[0].neg[0].need = cx[0].need[..];
  m := neg_initiate(cx[0].neg[..], msg[..]);
  k := n77_write_open(frame[..], "sync-1", filter, cast(string) msg[0..<m]);
  frames := 0; sizes := 0;
  while true {
    // the relay: a NEG-OPEN starts a session over its own matching records
    f := n77_read(cast(string) frame[0..<k], got[..], 32768);
    #assert f.error == N77_OK;
    frames += 1; sizes += m;
    show_frame(cast(string) frame[0..<k], cast(string) got[0..<f.msg_n], frames, quiet);
    if f.verb == N77_OPEN {
      #assert select_items(y, cast(string) frame[f.filter_from..<f.filter_to], sy);
      #assert neg_init(sy[0].neg[..], sy[0].items[0..<sy[0].n], false, limit) == NEG_OK;
    }
    m = neg_reconcile(sy[0].neg[..], cast(string) got[0..<f.msg_n], msg[..]);
    #assert m > 0;
    k = n77_write_msg(frame[..], "sync-1", cast(string) msg[0..<m]);
    // the client
    f = n77_read(cast(string) frame[0..<k], got[..], 32768);
    #assert f.error == N77_OK && f.verb == N77_MSG;
    frames += 1; sizes += m;
    show_frame(cast(string) frame[0..<k], cast(string) got[0..<f.msg_n], frames, quiet);
    m = neg_reconcile(cx[0].neg[..], cast(string) got[0..<f.msg_n], msg[..]);
    #assert m >= 0;
    if m == 0 { break; }
    k = n77_write_msg(frame[..], "sync-1", cast(string) msg[0..<m]);
  }
  k = n77_write_close(frame[..], "sync-1");
  if quiet { print("\n"); } else { print("  client -> %\n", cast(string) frame[0..<k]); }
  print("  % frames, % message bytes; have %, need %\n", frames, sizes, cx[0].neg[0].have_n, cx[0].neg[0].need_n);
  return frames;
}
// book:end sync
// A frame, its hex cut to 24 digits, and its first ranges decoded; or,
// when quiet, only the message's size.
show_frame :: (frame: string, msg: string, i: int, quiet: bool) {
  if quiet { if i == 1 { print("  message sizes:"); } print(" %", msg.count); return; }
  who := ifx i % 2 == 1 then "client" else "relay ";
  at := frame.count - 3; while frame[at] != '"' { at -= 1; }
  if frame.count - 2 - (at + 1) > 24 { print("  % -> %...\"] (% bytes)\n", who, cast(string) frame[0..<at + 25], msg.count); }
  else { print("  % -> % (% bytes)\n", who, frame, msg.count); }
  explain(msg);
}
// book:begin explain
// The ranges of a message, the first four in full.
explain :: (msg: string) {
  r: [1] NegIn; r[0] = NegIn.{msg = msg, at = 1};
  h: [64] u8; k := 0;
  while r[0].at < msg.count && r[0].error == NEG_OK {
    b := neg_get_bound(r[..]); mode := neg_get_varint(r[..]);
    if k < 4 {
      print("       range %: below ", k + 1);
      if b.created == NEG_INFINITY { print("infinity"); } else if b.n == 0 { print("%", b.created); }
      else { print("(%, id prefix %)", b.created, to_hex(b.prefix[0..<b.n], h[..])); }
    }
    if mode == NEG_FINGERPRINT {
      fp: [16] u8; for i: 0..<16 { fp[i] = msg[r[0].at + i]; }
      if k < 4 { print(", fingerprint %\n", to_hex(fp[..], h[..])); }
      r[0].at += 16;
    } else if mode == NEG_IDLIST {
      c := neg_get_varint(r[..]);
      if k < 4 { print(", IdList of % ids\n", c); }
      r[0].at += 32 * c;
    } else if k < 4 { print(", Skip\n"); }
    k += 1;
  }
  if k > 4 { print("       ... % ranges in all\n", k); }
}
// book:end explain
// The record i of the generated relays: id sha256("negentropy i").
gen :: (r: [] Relay, i: int) {
  c: RsdRec; gen_id(i, c.id[..]);
  c.author[0] = 1; c.kind = 1; c.created = NOW - (i * 37) % 500; c.expiry = -1; c.tags_to = 2;
  #assert ns_store_admit(r[0].s[..], c, "[]", true) == NSS_ACCEPT;
}
eq32 :: (a: [32] u8, b: [32] u8) -> bool { for k: 0..<32 { if a[k] != b[k] { return false; } } return true; }
// The ids in a set that the other set lacks, by a full comparison.
only_in :: (a: [] Side, b: [] Side) -> int {
  n := 0;
  for i: 0..<a[0].n {
    found := false;
    for j: 0..<b[0].n { if !found && eq32(a[0].items[i].id, b[0].items[j].id) { found = true; } }
    if !found { n += 1; }
  }
  return n;
}
same_ids :: (got: [] NegId, n: int, a: [] Side, b: [] Side) -> bool {
  // every id reported is in a and not in b, and each appears once
  for i: 0..<n {
    in_a := false; in_b := false;
    for j: 0..<a[0].n { if eq32(a[0].items[j].id, got[i].b) { in_a = true; } }
    for j: 0..<b[0].n { if eq32(b[0].items[j].id, got[i].b) { in_b = true; } }
    if !in_a || in_b { return false; }
    for j: 0..<i { if eq32(got[j].b, got[i].b) { return false; } }
  }
  return n == only_in(a, b);
}
// Digest of the sorted ids, to compare with the Python transcription.
ids_sha256 :: (got: [] NegId, n: int, out: [] u8) -> string {
  v: [512] NegItem; for i: 0..<n { v[i].id = got[i].b; }
  neg_sort(v[0..<n]);
  bytes: [16384] u8; for i: 0..<n { for k: 0..<32 { bytes[32 * i + k] = v[i].id[k]; } }
  h: [32] u8; #assert sha256_hash_checked(bytes[0..<32 * n], h[..], false) == SHA_OK;
  return to_hex(h[..], out);
}
neg_refused :: (what: string, n: [] Neg, hex: string) {
  b: [256] u8; o: [4096] u8;
  r := hex_decode_checked(hex, b[..], false); #assert r.error == NC_OK;
  k := neg_reconcile(n, cast(string) b[0..<r.written], o[..]);
  if k < 0 { print("  %: % -> refused, %\n", what, hex, neg_text(n[0].error)); }
  else { h: [64] u8; print("  %: % -> answered %\n", what, hex, to_hex(o[0..<k], h[..])); }
}
// A relay that refuses a frame answers NEG-ERR, when it has a subscription id.
frame_refused :: (what: string, text: string, max: int) {
  m: [64] u8; f := n77_read(text, m[..], max);
  names := [8] string.["N77_OK", "N77_JSON", "N77_SHAPE", "N77_VERB", "N77_SUB", "N77_HEX", "N77_FRAME", "N77_FILTER"];
  print("  %: % -> %", what, text, names[f.error]);
  if f.error != N77_OK && f.sub_n > 0 {
    o: [256] u8; k := n77_write_err(o[..], cast(string) f.sub[0..<f.sub_n], ifx f.error == N77_FRAME then "blocked: message over the frame limit" else "invalid: bad NEG frame");
    print(", relay answers %", cast(string) o[0..<k]);
  }
  print("\n");
}

main :: () {
  h: [64] u8; h2: [64] u8;
  print("varints, by hand: base 128, most significant digit first\n");
  vals := [6] int.[0, 127, 128, 300, 16384, NOW];
  for v: vals {
    o: [16] u8; w: [1] NegOut; w[0] = NegOut.{buf = o[..]}; neg_put_varint(w[..], v);
    print("  % -> %\n", v, to_hex(o[0..<w[0].n], h[..]));
  }

  print("\nthe fingerprint of two ids, by hand\n");
  two: [2] NegItem; two[0].id[0] = 0xff; two[0].id[1] = 0xff; two[1].id[0] = 0x01;
  sum: [33] u8; neg_add(sum[0..<32], two[0].id); neg_add(sum[0..<32], two[1].id); sum[32] = 2;
  print("  a = %...  (0xffff little-endian)\n  b = %...  (0x0001)\n", to_hex(two[0].id[0..<4], h[..]), to_hex(two[1].id[0..<4], h2[..]));
  print("  a + b mod 2^256 = %... (0x010000)\n", to_hex(sum[0..<4], h[..]));
  d: [32] u8; #assert sha256_hash_checked(sum[..], d[..], false) == SHA_OK;
  print("  sha256(sum || varint(2)) = %\n", to_hex(d[..], h2[..]));
  fp: [16] u8; neg_fingerprint(two[..], 0, 2, fp[..]);
  print("  fingerprint = first 16 bytes = %\n", to_hex(fp[..], h[..]));

  print("\ntwo relays, five signed notes, three on each (A B C D E; X has A B C, Y has C D E)\n");
  x: [1] Relay; y: [1] Relay; relay_init(x[..]); relay_init(y[..]);
  keys := [5] string.[KEY_A, KEY_B, KEY_C, KEY_A, KEY_B];
  auxs := [5] string.[AUX_A, AUX_B, AUX_C, AUX_A, AUX_B];
  notes := [5] string.["note A", "note B", "note C", "note D", "note E"];
  for i: 0..<5 {
    b: [1024] u8; e := sign(keys[i], auxs[i], NOW - 100 + 10 * (i / 2), 1, notes[i], b[..]);
    if i <= 2 { #assert admit(x[..], e) == RS_ACCEPT; }
    if i >= 2 { #assert admit(y[..], e) == RS_ACCEPT; }
    id := nostr_event_read(e, h[..], false).id;
    print("  % created_at % id %\n", notes[i], NOW - 100 + 10 * (i / 2), to_hex(id[..], h2[..]));
  }
  // a reaction on both relays, outside the filter
  b7: [1024] u8; r7 := sign(KEY_C, AUX_C, NOW - 5, 7, "+", b7[..]);
  #assert admit(x[..], r7) == RS_ACCEPT && admit(y[..], r7) == RS_ACCEPT;
  #assert nsi_build(x[0].s[..], x[0].ix[..]) && nsi_build(y[0].s[..], y[0].ix[..]);
  cx: [1] Side; sy: [1] Side;
  frames := run_sync(x[..], y[..], "{\"kinds\":[1]}", 0, cx[..], sy[..], false);
  for i: 0..<cx[0].neg[0].have_n { print("  have %... (the relay lacks it)\n", to_hex(cx[0].have[i].b[0..<8], h[..])); }
  for i: 0..<cx[0].neg[0].need_n { print("  need %... (the client lacks it)\n", to_hex(cx[0].need[i].b[0..<8], h[..])); }

  print("\ntwo relays of generated records: 1,000 ids sha256(\"negentropy i\"), X keeps those whose\n");
  print("first byte mod 5 is not 0, Y those whose second byte mod 5 is not 0\n");
  relay_init(x[..]); relay_init(y[..]);
  for i: 0..<1000 {
    c: [32] u8; gen_id(i, c[..]);
    if c[0] % 5 != 0 { gen(x[..], i); }
    if c[1] % 5 != 0 { gen(y[..], i); }
  }
  #assert nsi_build(x[0].s[..], x[0].ix[..]) && nsi_build(y[0].s[..], y[0].ix[..]);
  for limit: [2] int.[0, 4096] {
    print("frame limit %:\n", limit);
    frames = run_sync(x[..], y[..], "{\"kinds\":[1]}", limit, cx[..], sy[..], limit != 0);
    hv := cx[0].neg[0].have_n; nd := cx[0].neg[0].need_n;
    print("  X % records, Y %; a full comparison finds % only on X and % only on Y\n", cx[0].n, sy[0].n, only_in(cx[..], sy[..]), only_in(sy[..], cx[..]));
    print("  have is exactly X - Y: %; need is exactly Y - X: %\n", same_ids(cx[0].have[..], hv, cx[..], sy[..]), same_ids(cx[0].need[..], nd, sy[..], cx[..]));
    print("  sha256 of the sorted have ids %\n", ids_sha256(cx[0].have[..], hv, h[..]));
    print("  sha256 of the sorted need ids %\n", ids_sha256(cx[0].need[..], nd, h[..]));
  }

  print("\nrefusals: Negentropy messages (the relay's side, then the client's)\n");
  srv: [1] Neg; cli: [1] Neg;
  #assert neg_init(srv[..], sy[0].items[0..<sy[0].n], false, 4096) == NEG_OK;
  #assert neg_init(cli[..], cx[0].items[0..<cx[0].n], true, 4096) == NEG_OK;
  neg_refused("an empty IdList cut short", srv[..], "61000002");
  neg_refused("a fingerprint cut short", srv[..], "6100000100112233");
  neg_refused("mode 3", srv[..], "6100000300");
  neg_refused("an id prefix of 33 bytes", srv[..], "610021");
  neg_refused("a varint past 2^63 - 2", srv[..], "61ffffffffffffffff7f");
  neg_refused("version byte 0x50", srv[..], "50");
  neg_refused("version 2 (0x62), to the relay", srv[..], "62");
  neg_refused("version 2 (0x62), to the client", cli[..], "62");
  bad: [1] Neg;
  print("  a frame limit of 1000: %\n", neg_text(neg_init(bad[..], cx[0].items[0..<1], false, 1000)));

  print("\nrefusals: NIP-77 frames (a relay that takes messages of at most 4 bytes for the last two)\n");
  frame_refused("odd hex", "[\"NEG-MSG\",\"s1\",\"610\"]", 4096);
  frame_refused("not hex", "[\"NEG-MSG\",\"s1\",\"61zz\"]", 4096);
  frame_refused("no message", "[\"NEG-MSG\",\"s1\"]", 4096);
  frame_refused("a filter that is an array", "[\"NEG-OPEN\",\"s1\",[],\"61\"]", 4096);
  frame_refused("an empty subscription id", "[\"NEG-CLOSE\",\"\"]", 4096);
  frame_refused("5 bytes", "[\"NEG-MSG\",\"s1\",\"6100000200\"]", 4);
  frame_refused("4 bytes", "[\"NEG-MSG\",\"s1\",\"61000000\"]", 4);
}
gen_id :: (i: int, out: [] u8) {
  t := "negentropy "; b: [32] u8; n := 0;
  for c: t { b[n] = c; n += 1; }
  d: [24] u8; k := 0; x := i;
  while true { d[k] = cast(u8) ('0' + x % 10); k += 1; x /= 10; if x == 0 { break; } }
  while k > 0 { k -= 1; b[n] = d[k]; n += 1; }
  #assert sha256_hash_checked(b[0..<n], out, false) == SHA_OK;
}
```

### Worked example: five signed notes

Relay X holds notes A, B and C; relay Y holds C, D and E. A and B were
created at `1793491100`, C and D at `1793491110`, E at `1793491120`. Both
relays also hold a reaction (kind 7) that the filter `{"kinds":[1]}` leaves
out.

1. X has three items, fewer than 32, so its first message is one range:
   `61`, then the bound `00 00` (infinity, no prefix), `02` (`IdList`),
   `03` and the IDs of A, B and C: 1 + 2 + 1 + 1 + 96 = 101 bytes.
2. Y reads an `IdList` as the server and answers with its own IDs in the
   same range, C, D and E, sorted by time and ID: 101 bytes.
3. X compares. A and B are not in Y's list: `have`. D and E are not in X:
   `need`. X has nothing more to say, so it sends `NEG-CLOSE`.

To finish, X would send A and B with `EVENT` and fetch D and E with `REQ`.

### Generated relays

The second part uses 1,000 generated records. Item `i` has the ID
`sha256("negentropy i")` and the timestamp `1793491200 − (37·i mod 500)`,
so many items share a timestamp and bounds need ID prefixes. X keeps an
item when its ID's first byte mod 5 isn't 0, Y when the second byte mod 5
isn't 0: 771 and 804 items.

X's first message has 16 ranges. 771 = 16·48 + 3, so the first three
buckets hold 49 items and the rest 48. The first bound is
`86 d7 99 fe 2b 01 d3`: the varint is 1793490731, so the timestamp is
1793490730, and the prefix is the one byte `d3`, because items 49 and 50
share that timestamp and differ in their first ID byte. The next bound is
written `22`: 34, a step of 33 to `1793490763`. Y's fingerprints differ in
all 16 ranges, so it splits each into 16 (256 ranges); the client splits
those into `IdList`s; the server answers with its IDs. The client ends with
143 `have` and 176 `need`, and a full comparison of the two sets finds the
same IDs.

With a frame limit of 4096 on both sides, the same sync takes 16 frames,
none longer than 3,967 bytes, and finds the same IDs.

These sets differ in about a third of their items, so the sync sends more
bytes (43,980) than the client's IDs alone (24,672). Reconciliation pays when
sets mostly agree. In the checks' `near` case, 1,197 and 1,196 items that
differ in 7, the four messages total 4,131 bytes, against 38,304 bytes for
the client's IDs.

### The deliberately broken cases

The last part sends broken messages to a server and a client, and broken
frames to a relay. Each Negentropy refusal is printed with `neg_text`. A
relay answers a broken frame that has a subscription ID with `NEG-ERR`:
`invalid:` for a malformed frame, and `blocked:` for one over its frame
limit, here a relay that takes messages of at most 4 bytes.

| Input | Result |
| --- | --- |
| `61000002` (an `IdList` with no count) | `NEG_SHORT` |
| `6100000100112233` (a fingerprint of 4 bytes) | `NEG_SHORT` |
| `6100000300` (mode 3) | `NEG_MODE` |
| `610021` (an ID prefix of 33 bytes) | `NEG_BOUND` |
| `61ffffffffffffffff7f` (a varint of 2^63 − 1) | `NEG_VARINT` |
| `50` | `NEG_VERSION_BYTE` |
| `62` to the server | answered `61` |
| `62` to the client | `NEG_VERSION` |
| frame limit 1000 | `NEG_FRAME` |
| `["NEG-MSG","s1","610"]` | `N77_HEX`, `NEG-ERR` `invalid:` |
| `["NEG-MSG","s1","6100000200"]` to a relay that takes 4 bytes | `N77_FRAME`, `NEG-ERR` `blocked:` |

To run it:

```shell
coax/bin/coax run nostr/examples/negentropy-sync.coax --run-steps 250000000
```

The output:

```text
varints, by hand: base 128, most significant digit first
  0 -> 00
  127 -> 7f
  128 -> 8100
  300 -> 822c
  16384 -> 818000
  1793491200 -> 86d79a8200

the fingerprint of two ids, by hand
  a = ffff0000...  (0xffff little-endian)
  b = 01000000...  (0x0001)
  a + b mod 2^256 = 00000100... (0x010000)
  sha256(sum || varint(2)) = 47178f396ea8b5434d8ed8aa88bbbb2324037414f06a94fcb362280ca5121e50
  fingerprint = first 16 bytes = 47178f396ea8b5434d8ed8aa88bbbb23

two relays, five signed notes, three on each (A B C D E; X has A B C, Y has C D E)
  note A created_at 1793491100 id 00cc823a7c0512cfee5c563e73a47895c768309b6604ec156066a5d85225c9b3
  note B created_at 1793491100 id c6b4df51484211c8101a70258f72ad884f903b399fb18c9706ff2cb3ad0157b1
  note C created_at 1793491110 id 49cae413bf579f99c2e3075854aebf5401a8a2c3fa7a3b21762c5a1395838ad9
  note D created_at 1793491110 id ff93c26285153749f7bd6ffbb82b6f16f2413c1f6ed5bdcf2cd818dba7cee481
  note E created_at 1793491120 id 31db86c7c0eb586469ff288d717b21e54dc9ace6443f0b61d4e0cee2486244cd
  client -> ["NEG-OPEN","sync-1",{"kinds":[1]},"610000020300cc823a7c0512..."] (101 bytes)
       range 1: below infinity, IdList of 3 ids
  relay  -> ["NEG-MSG","sync-1","610000020349cae413bf579f..."] (101 bytes)
       range 1: below infinity, IdList of 3 ids
  client -> ["NEG-CLOSE","sync-1"]
  2 frames, 202 message bytes; have 2, need 2
  have 00cc823a7c0512cf... (the relay lacks it)
  have c6b4df51484211c8... (the relay lacks it)
  need ff93c26285153749... (the client lacks it)
  need 31db86c7c0eb5864... (the client lacks it)

two relays of generated records: 1,000 ids sha256("negentropy i"), X keeps those whose
first byte mod 5 is not 0, Y those whose second byte mod 5 is not 0
frame limit 0:
  client -> ["NEG-OPEN","sync-1",{"kinds":[1]},"6186d799fe2b01d301b5a235..."] (316 bytes)
       range 1: below (1793490730, id prefix d3), fingerprint b5a2355016e82bd591c839c29086fb36
       range 2: below 1793490763, fingerprint 33777e988edf0a65111e635c0d7f56a0
       range 3: below 1793490795, fingerprint ab2b1c7068272e978467f8a459ee850c
       range 4: below 1793490826, fingerprint 44167b6f00904fe5fb4e78c2ad5978de
       ... 16 ranges in all
  relay  -> ["NEG-MSG","sync-1","6186d799fe0f01d30178ca47..."] (4974 bytes)
       range 1: below (1793490702, id prefix d3), fingerprint 78ca47d004d8703d19cbd67f7a58b7d5
       range 2: below 1793490707, fingerprint e0d3f1027a4571109156dc7e9f6f190e
       range 3: below 1793490709, fingerprint 76fd92a24c842177fd422559d662e776
       range 4: below 1793490711, fingerprint 835d679a6bc5646181801fd863c93368
       ... 256 ranges in all
  client -> ["NEG-MSG","sync-1","6186d799fe0f01d300060002..."] (18817 bytes)
       range 1: below (1793490702, id prefix d3), Skip
       range 2: below 1793490707, IdList of 8 ids
       range 3: below 1793490709, IdList of 3 ids
       range 4: below 1793490711, IdList of 3 ids
       ... 236 ranges in all
  relay  -> ["NEG-MSG","sync-1","6186d799fe0f01d300060002..."] (19873 bytes)
       range 1: below (1793490702, id prefix d3), Skip
       range 2: below 1793490707, IdList of 3 ids
       range 3: below 1793490709, IdList of 3 ids
       range 4: below 1793490711, IdList of 3 ids
       ... 236 ranges in all
  client -> ["NEG-CLOSE","sync-1"]
  4 frames, 43980 message bytes; have 143, need 176
  X 771 records, Y 804; a full comparison finds 143 only on X and 176 only on Y
  have is exactly X - Y: true; need is exactly Y - X: true
  sha256 of the sorted have ids 834f8d0dcc6b8b6aa6bf955b211d910ab818e328b931f443591499ddeda47eed
  sha256 of the sorted need ids 92f0c8158da7febf06b8055327a5d34395df2da15d22b6936d89f6d26c83c785
frame limit 4096:
  message sizes: 316 3752 3908 3967 352 3798 3886 3921 323 3816 788 3459 318 3710 318 3308
  16 frames, 39940 message bytes; have 143, need 176
  X 771 records, Y 804; a full comparison finds 143 only on X and 176 only on Y
  have is exactly X - Y: true; need is exactly Y - X: true
  sha256 of the sorted have ids 834f8d0dcc6b8b6aa6bf955b211d910ab818e328b931f443591499ddeda47eed
  sha256 of the sorted need ids 92f0c8158da7febf06b8055327a5d34395df2da15d22b6936d89f6d26c83c785

refusals: Negentropy messages (the relay's side, then the client's)
  an empty IdList cut short: 61000002 -> refused, the message ends inside a field
  a fingerprint cut short: 6100000100112233 -> refused, the message ends inside a field
  mode 3: 6100000300 -> refused, unknown mode
  an id prefix of 33 bytes: 610021 -> refused, bad bound
  a varint past 2^63 - 2: 61ffffffffffffffff7f -> refused, a varint is too large
  version byte 0x50: 50 -> refused, not a Negentropy version byte
  version 2 (0x62), to the relay: 62 -> answered 61
  version 2 (0x62), to the client: 62 -> refused, unsupported Negentropy version
  a frame limit of 1000: frame limit below 4096

refusals: NIP-77 frames (a relay that takes messages of at most 4 bytes for the last two)
  odd hex: ["NEG-MSG","s1","610"] -> N77_HEX, relay answers ["NEG-ERR","s1","invalid: bad NEG frame"]
  not hex: ["NEG-MSG","s1","61zz"] -> N77_HEX, relay answers ["NEG-ERR","s1","invalid: bad NEG frame"]
  no message: ["NEG-MSG","s1"] -> N77_SHAPE, relay answers ["NEG-ERR","s1","invalid: bad NEG frame"]
  a filter that is an array: ["NEG-OPEN","s1",[],"61"] -> N77_FILTER, relay answers ["NEG-ERR","s1","invalid: bad NEG frame"]
  an empty subscription id: ["NEG-CLOSE",""] -> N77_SUB
  5 bytes: ["NEG-MSG","s1","6100000200"] -> N77_FRAME, relay answers ["NEG-ERR","s1","blocked: message over the frame limit"]
  4 bytes: ["NEG-MSG","s1","61000000"] -> N77_OK
```

## How the code is checked

### Checks in the library

`nip77.coax` has 78 checks:

- varints of 0, 1, 127, 128, 300, 16383, 16384, the clock and 2^63 − 2, and
  their refusals;
- bounds written and read back, the step that would reach infinity, and
  prefixes of 32 and 33 bytes;
- the three fingerprints above, and the sum by hand;
- sorting, the order `neg_init` insists on, `neg_lower` and `neg_between`;
- the three-item first message, byte for byte;
- 24 pairs of small sets, with and without a frame limit, whose `have` and
  `need` equal the symmetric difference computed directly;
- four syncs over 300 and 600 items, three of them cut by a frame limit
  (in an `IdList`, in fingerprint ranges, and just after a skipped range),
  whose messages, `have` and `need` equal the Python transcription's;
- a fingerprint that differs in any one of its 16 bytes, the 32-item line,
  the frame limit's 200 bytes, and every refusal;
- the NIP-77 frames written and read back, and their refusals.

```shell
coax/bin/coax test coax/lib/nostr/nip77.coax
coax/bin/coax mutate coax/lib/nostr/nip77.coax --root-only
```

On 2026-11-01, `coax test` ran 78 checks, including the loaded files' own, in about 6 seconds. Root-only mutation killed 751 of 751 mutants in about 2 minutes on 28 cores. The first run's survivors were rewritten or checked: comparisons that could not tell `<` from `<=` (rewritten), a binary search whose midpoint did not matter (a linear walk now), a timestamp restored before an infinite bound that ignores it (removed), and paths no check reached (the cut after a skip, the empty server range, exact fits), which now have checks.

### Checks in the example

The checks are in
[nostr/examples/checks/negentropy-sync.coax](../examples/checks/negentropy-sync.coax),
which loads the example, so running the example doesn't run them. No
published Negentropy test vectors are pinned in this repository, and NIP-77
gives none. The independent check is
[negentropy_ref.py](../../coax/tests/nostr/negentropy_ref.py), a Python
transcription of NIP-77's appendix and of the reference implementation's
choices, written for this chapter from `js/Negentropy.js` (read, not run)
with the cut fixed as above. It shares no code with `nip77.coax`. For six
syncs over up to 1,200 items it prints the SHA-256 of every message in
order and of the sorted `have` and `need` IDs; the Coax code must match
each one.

<!-- source: nostr/examples/checks/negentropy-sync.coax check -->
```jai
// Values printed by coax/tests/nostr/negentropy_ref.py, a Python
// transcription of NIP-77's appendix and of the reference implementation's
// choices (16 buckets, IdLists under 32 items, the frame limit check
// n > L - 200). It shares no code with nip77.coax. Items come from one
// universe of 1,200: id sha256("negentropy i"), created_at
// 1793491200 - (37 i mod 500). The client keeps an item when its id's first
// byte mod 5 is not 0, the server when its second byte mod 5 is not 0.
CASES :: 6;
// name, sha256 of every message in order, sha256 of the sorted have ids,
// then of the sorted need ids.
WANT :: [24] string.[
  "random",            "31d99b84ee00676e5c02c6f18e93f05adac8432cb15e3ba3eea660a3a0909e23",
  "2defa301f84f07e65d369d8849826e0766d3e137f8de0bc854988c75148c1789", "d7b9405e8c9ee924592744c128e98a5119870c85b2b2ddb3c053eb7fd5a87b84",
  "random limit 4096", "40c1e33a481b9ed335ea37c0da69daa100013b1a3644113427b17c88faa8bf92",
  "2defa301f84f07e65d369d8849826e0766d3e137f8de0bc854988c75148c1789", "d7b9405e8c9ee924592744c128e98a5119870c85b2b2ddb3c053eb7fd5a87b84",
  "near",              "cc8e2575081d5afd0e99371c27dda10152f7348d272cbf54755f2cdef823e78f",
  "94a490dec16cee01e46cfcfadedc74e1a2e89e3f4875f5a2803b2b70b2d9ea89", "ae0f7f048dc9e2b6c805eb4feef7cd126d2778346e7dfe547d6bdb534d2117e5",
  "equal",             "9d36b959b4e0a8ec6a6e643e8b023e2613c124fdfc9cc84ff1108b14fc08648b",
  "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855", "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
  "empty client",      "7f40d5c0b184a480d1707002e5b014c01475a6b245917acb65b31ca6de12df93",
  "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855", "60ae6d578ebeb2e832d7ecb52bf0ce126c847cd539cef1e8c729dbc3388b1a85",
  "relays limit 4096", "8ca143182f6322010757a993c8e6be377eb6212feb6a86cb2fddc9a52c9f1528",
  "834f8d0dcc6b8b6aa6bf955b211d910ab818e328b931f443591499ddeda47eed", "92f0c8158da7febf06b8055327a5d34395df2da15d22b6936d89f6d26c83c785"];
// messages sent, have and need counts
COUNTS :: [18] int.[4, 180, 208,  22, 180, 208,  4, 4, 3,  2, 0, 0,  2, 0, 100,  16, 143, 176];
LIMITS :: [6] int.[0, 4096, 0, 0, 0, 4096];
universe_item :: (i: int) -> NegItem { x: NegItem; x.created = NOW - (i * 37) % 500; gen_id(i, x.id[..]); return x; }
// Which universe items each side keeps in case c.
keeps :: (c: int, client: bool, i: int, x: NegItem) -> bool {
  if c == 3 { return true; }
  if c == 4 { return !client && i < 100; }
  if c == 2 { if client { return i != 5 && i != 600 && i != 1100; } return i != 7 && i != 8 && i != 333 && i != 1199; }
  if c == 5 && i >= 1000 { return false; }
  return ifx client then x.id[0] % 5 != 0 else x.id[1] % 5 != 0;
}
Run :: struct { a: [1200] NegItem; b: [1200] NegItem; have: [600] NegId; need: [600] NegId; q: [65536] u8; r: [65536] u8; }
#forall "each case: messages byte for byte, have and need, as the Python transcription" c: 0..<CASES #steps 4000000000 {
  w: [1] Run; na := 0; nb := 0;
  for i: 0..<1200 {
    x := universe_item(i);
    if keeps(c, true, i, x) { w[0].a[na] = x; na += 1; }
    if keeps(c, false, i, x) { w[0].b[nb] = x; nb += 1; }
  }
  neg_sort(w[0].a[0..<na]); neg_sort(w[0].b[0..<nb]);
  cl: [1] Neg; sv: [1] Neg;
  #assert neg_init(cl[..], w[0].a[0..<na], true, LIMITS[c]) == NEG_OK && neg_init(sv[..], w[0].b[0..<nb], false, LIMITS[c]) == NEG_OK;
  cl[0].have = w[0].have[..]; cl[0].need = w[0].need[..];
  st: [112] u8; #assert sha256_init(st[..]) == SHA_OK;
  qn := neg_initiate(cl[..], w[0].q[..]); sent := 1;
  #assert sha256_update_checked(st[..], w[0].q[0..<qn], false) == SHA_OK;
  while qn > 0 {
    rn := neg_reconcile(sv[..], cast(string) w[0].q[0..<qn], w[0].r[..]); sent += 1;
    #assert rn > 0 && sha256_update_checked(st[..], w[0].r[0..<rn], false) == SHA_OK;
    qn = neg_reconcile(cl[..], cast(string) w[0].r[0..<rn], w[0].q[..]);
    #assert qn >= 0;
    if qn > 0 { sent += 1; #assert sha256_update_checked(st[..], w[0].q[0..<qn], false) == SHA_OK; }
  }
  d: [32] u8; h: [64] u8;
  #assert sha256_final_checked(st[..], d[..], false) == SHA_OK && to_hex(d[..], h[..]) == WANT[4 * c + 1];
  #assert sent == COUNTS[3 * c] && cl[0].have_n == COUNTS[3 * c + 1] && cl[0].need_n == COUNTS[3 * c + 2];
  #assert ids_sha256(w[0].have[..], cl[0].have_n, h[..]) == WANT[4 * c + 2];
  #assert ids_sha256(w[0].need[..], cl[0].need_n, h[..]) == WANT[4 * c + 3];
}
#test "hand values: varints, the two-id fingerprint, the three-item message" {
  // 300 = 2 * 128 + 44: digits 2, 44 -> 0x82 0x2c
  o: [16] u8; w: [1] NegOut; w[0] = NegOut.{buf = o[..]}; neg_put_varint(w[..], 300);
  #assert w[0].n == 2 && o[0] == 0x82 && o[1] == 0x2c;
  // 1793491200 = 6 * 128^4 + 87 * 128^3 + 26 * 128^2 + 2 * 128 + 0
  w[0] = NegOut.{buf = o[..]}; neg_put_varint(w[..], NOW);
  #assert w[0].n == 5 && o[0] == 0x86 && o[1] == 0xd7 && o[2] == 0x9a && o[3] == 0x82 && o[4] == 0x00;
  #assert 6 * 268435456 + 87 * 2097152 + 26 * 16384 + 2 * 128 == NOW;
  two: [2] NegItem; two[0].id[0] = 0xff; two[0].id[1] = 0xff; two[1].id[0] = 1;
  fp: [16] u8; h: [64] u8; neg_fingerprint(two[..], 0, 2, fp[..]);
  #assert to_hex(fp[..], h[..]) == "47178f396ea8b5434d8ed8aa88bbbb23";
  // the same from the sum written out: 00 00 01, 29 zeros, then varint 2
  sum: [33] u8; sum[2] = 1; sum[32] = 2; d: [32] u8;
  #assert sha256_hash_checked(sum[..], d[..], false) == SHA_OK;
  for i: 0..<16 { #assert d[i] == fp[i]; }
}
```

```shell
python3 -I coax/tests/nostr/negentropy_ref.py
coax/bin/coax test nostr/examples/checks/negentropy-sync.coax --run-steps 250000000
```

On 2026-11-01, `coax test` ran 477 checks, including the loaded files' own, in about 33 seconds, and the Python script ran in under a second. Root-only mutation of the checks file killed 58 of 58 mutants. Mutation of the example's own `main` was not run; its output is pinned by the fixture.

### What is not built

- Wiring NIP-77 into the relay of [chapter 34](ch034-admit-an-event.md) and
  `coax/relay/relay.coax`, which is out of scope for this chapter. A relay
  would keep one `Neg` per open subscription ID and connection, close the
  old one when a `NEG-OPEN` reuses an ID, refuse a query over a record
  limit with `NEG-ERR` `blocked:` (optionally with the limit as a fourth
  element), and close idle sessions with `closed:`. It would also
  advertise 77 in its NIP-11 document ([chapter 37](ch037-nip-11-relay-information.md)).
- Replication between Coax relays: a process that runs this sync on a
  schedule and then moves the `have` and `need` events with `EVENT` and
  `REQ`.
- Timestamps at or above 2^63 − 1. The protocol allows up to 2^64 − 2;
  this file refuses them.
- Interoperation with a running reference implementation. The messages
  were compared with the Python transcription only; the reference code
  was read, not run on this machine.

## Summary

- Negentropy sorts items by `(created_at, id)` and describes a set as
  adjacent ranges, each with an upper bound, a mode and a payload.
- A fingerprint is SHA-256 of the IDs' sum mod 2^256 and their count,
  truncated to 16 bytes; equal fingerprints let both sides skip a range.
- Ranges whose fingerprints differ are split into 16 buckets, and small
  ones are sent as ID lists, until the client knows `have` and `need`.
- The frame limit ends a message early with one fingerprint for the rest,
  at the cost of more rounds.
- NIP-77 carries the messages as hex in `NEG-OPEN`, `NEG-MSG`,
  `NEG-CLOSE` and `NEG-ERR`.

## Dependencies

- [36. Match filters with indexes](ch036-match-filters-with-indexes.md)
- [4. SHA-256](ch004-sha-256.md)

## Sources

- [NIP-77](../nips/official/77.md), pinned, with its appendix, Negentropy
  Protocol V1.
- [Range-Based Set Reconciliation](https://logperiodic.com/rbsr.html), the
  technique NIP-77 cites.
- [hoytech/negentropy](https://github.com/hoytech/negentropy), the
  reference implementation, `js/Negentropy.js` as read on 2026-11-01.
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
