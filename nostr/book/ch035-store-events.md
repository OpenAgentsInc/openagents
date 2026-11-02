# 35. Store events

A relay must keep what it acknowledged. This chapter builds the store:
in-memory decisions, an append-only file, replacement rules and recovery after
a crash.

Part IX, A relay. Result: Verified originals with one copy per ID, replaceable
and addressable replacement with the same-second lowest-ID rule, an fsync
before `OK`, and crash recovery that cuts only a partial last write.

## What this chapter teaches

- Admission of verified events only, and deduplication by event ID that
  keeps the lowest exact original when the same event arrives as different
  bytes.
- Replacement for kinds 0, 3 and 10000–19999 by kind and author, and for
  30000–39999 by kind, author and `d` tag: the newer `created_at` wins, and
  on the same second the lowest ID wins.
- Ephemeral kinds (20000–29999): accepted and never stored.
- Fixed bounds: a record table and a byte arena. A full table is compacted
  first; only when nothing can be dropped does the store answer `full`.
  The relay sizes both once, at start: 2,048 records and 4 MiB (`RSD_MAX`,
  `RSD_ARENA`), or `--store-capacity EVENTS` records at 2 KiB each, 128 to
  1,048,576 (`rr_capacity`, `rr_arena` in `relay_route.coax`). The file
  format doesn't depend on the size, but a file that holds more than the
  store refuses to start, unchanged.
- The file: one frame per stored original, a 4-byte big-endian length and the
  exact bytes, written and synced before the relay sends `OK true`.
- Recovery: replay re-verifies every frame, cuts a crash's partial last
  append, and refuses to start on any other damage.
- What is not solved: rollback to an older copy of the whole file (#966b).

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [store_policy.coax](../../coax/lib/nostr/store_policy.coax) | `ns_class`, `ns_before`, `ns_expired` | chapter 22's kind classes, the byte order of two IDs, and NIP-40 expiry (`#authority`) |
| [store_core.coax](../../coax/lib/nostr/store_core.coax) | `ns_store_plan`, `ns_store_apply`, `ns_store_admit` | every decision: verified only, one record per ID, replacement, ephemeral, room; split into a plan and its application so a file can record the plan first (`#authority`) |
| | `ns_store_duplicate`, `ns_store_compact`, `ns_store_room` | the same ID again; dropping replaced records and expired regular events when the table or arena is full |
| [store_dedup_policy.coax](../../coax/lib/nostr/store_dedup_policy.coax) | `ns_original_before` | the order of two exact originals: byte by byte, then the shorter first (`#authority`) |
| [store.coax](../../coax/lib/nostr/store.coax) | `rsd_rec`, `rsd_verified`, `rsd_admit`, `rsd_query` | the relay's face: copies chapter 34's `RsAdmit` into a record and calls store_core |
| [store_file.coax](../../coax/lib/nostr/store_file.coax) | `ns_file_header`, `ns_file_next`, `ns_file_replay_reason` | the file's framing and replay's reading rule (`#authority`) |
| [store_recover.coax](../../coax/lib/nostr/store_recover.coax) | `nsr_region`, `nsr_frame`, `nsr_replayed` | what replay does at each frame: read on, stop, cut or refuse (`#authority`) |
| [store_saved.coax](../../coax/lib/nostr/store_saved.coax) | `rsd_admit_saved`, `rsd_replay` | the glue: save before apply, and the replay loop over the host's file |
| [store-file-native.c](../../coax/platform/store-file-native.c) | `cxstore_load`, `cxstore_commit`, `cxstore_truncate` | the C host: read, append with `pwrite` and `fsync`, truncate |

The registry also lists
[store_snapshot.coax](../../coax/lib/nostr/store_snapshot.coax) and
[store_dedup_rewrite.coax](../../coax/lib/nostr/store_dedup_rewrite.coax).
They frame and rewrite a bounded snapshot (an `NSLG` header, a count, then
length-prefixed originals) from the store's first ingress work. The relay
doesn't use them: its file is the append-only log below.

`store_saved.coax` declares the host's file calls with `#foreign`, and
`coax run` doesn't link a C host. The example therefore repeats
`rsd_admit_saved` and `rsd_replay` line for line over a byte buffer. Every
decision it makes is a call into store_core, store_file and store_recover.

## Admission

`rsd_admit(s, r, text)` takes chapter 34's admission result `r` for the exact
bytes `text`. `rsd_verified` requires `r.reason == RS_ACCEPT`, no read error,
a passed signature check and `r.e.original == text`. Anything else is
refused with `RS_BAD_SIG` before the store looks at it, so a forged copy of
a stored event never reaches the duplicate check.

`ns_store_plan` then decides, in this order:

1. **Ephemeral.** A kind in 20000–29999 is accepted (`OK true`) and not
   stored. The relay sends it to matching subscriptions and forgets it.
2. **The same ID.** If a live record has this ID, the answer is `duplicate`.
   If a replaced record that is not yet compacted has it, the answer is
   also `duplicate`.
3. **An older version.** For a replaceable or addressable kind, if the live
   record at the same address is newer (`ns_store_newer`: a later
   `created_at`, or the same second and a lower ID), the answer is
   `superseded`.
4. **Room.** If the table or the arena is full, `ns_store_compact` drops
   replaced records and expired regular events, and moves the kept
   originals down to close the gaps. If there is still no room, the answer
   is `full`.
5. Otherwise the event is appended and the record it replaces, if any, is
   retired: still in the table, not live, so its ID answers `duplicate`
   until the next compaction.

The relay answers `duplicate` and `superseded` with `OK true` and
`duplicate:`, and `full` with `OK false` and `error: relay storage is full`
(chapter 34's reasons table).

### The same event as different bytes

An event's ID is the SHA-256 of its serialization, not of the JSON the
client sent. The same signed event can arrive with its members in another
order or with a character escaped, and it still verifies. The store keeps
one record per ID and keeps the original that is lowest by
`ns_original_before`: byte by byte, then the shorter first. So every relay
that saw the same copies holds the same bytes, whatever order they arrived
in.

`ns_store_duplicate` writes a lower original over the stored one when it is
no longer, appends it and retires the old record when it is longer, and
keeps the old one when a longer copy doesn't fit. The answer is always
`duplicate`, and it never takes another record.

### The file

`rsd_admit_saved` makes the plan, and when the plan writes bytes (a new
event, or a lower original of a stored one), appends the original's frame
to the file and syncs it. Only then does it apply the plan. A failed write
applies nothing and answers `full`, so `OK true` is never sent for an event
that isn't on disk.

A frame is the original's length as 4 big-endian bytes (1 to 262,144) and the
exact original. There is no file header, no checksum and no index. The
host's `cxstore_commit` writes with `pwrite` and calls `fsync`. A new file is
created with `O_EXCL` and its directory is synced, so the first acknowledged
events can't vanish with the file's directory entry after a power loss.

<!-- source: nostr/examples/store-events.coax save -->
```jai
// store_saved.coax's rsd_save and rsd_admit_saved, with the file in memory:
// decide, append the frame (4-byte big-endian length, then the exact
// original) when the decision writes, and only then apply it.
save :: (st: [] Store, text: string) -> bool {
  head: [4] u8;
  if !ns_file_header(text.count, head[..]) || st[0].total + 4 + text.count > st[0].file.count { return false; }
  for i: 0..<4 { st[0].file[st[0].total + i] = head[i]; }
  for i: 0..<text.count { st[0].file[st[0].total + 4 + i] = text[i]; }
  st[0].total += 4 + text.count;   // the host's cxstore_commit: pwrite, then fsync
  return true;
}
admit_saved :: (st: [] Store, r: RsAdmit, text: string) -> int {
  c := rsd_rec(r);
  p := ns_store_plan(st[0].s[..], c, text, rsd_verified(r, text));
  if p.write != NS_W_NONE && !save(st, text) { return RS_FULL; }
  return ns_store_apply(st[0].s[..], p, c, text);
}
```

## Recovery

On start, `rsd_replay` reads the file frame by frame and, for each whole
frame, reads the event as admission does, verifies its signature again and
re-admits it. Nothing in the file is trusted. Admission is order
independent (lowest original per ID, newest version per address), so
replaying the whole file rebuilds the store. One rule differs from live
admission: an event that expired since it was saved is still admitted, so
an expired replaceable head keeps an older version from coming back.

Each append is one frame synced before the `OK`, so a crash can damage only
the last frame, and only the way an interrupted append does: the file ends
inside it, or the file grew and some new bytes are still zero. A JSON event
never holds a zero byte, and every frame header starts with one (lengths
are at most 262,144 = 0x00040000). So a crash leaves one shape: non-zero
bytes, then zeros to the end of the file. `nsr_frame` cuts that shape and
nothing else:

| At a frame | Decision |
| --- | --- |
| The end of the file | done |
| The file ends inside the 4-byte header | cut |
| The body runs past the end, and the bytes after the header are non-zero then zeros | cut |
| A length of 0 or over 262,144, and every byte from it to the end is zero | cut |
| A whole last frame whose body ends in zeros | cut, unread |
| Any other whole frame | read, verify, re-admit |
| ...the store takes it: stored, duplicate or superseded | read on |
| ...anything else: not an event, a bad signature, a refused kind or time, the store full | refuse |
| Anything else | refuse |

Refuse means the relay prints `relay: refusing to start: the --store file is
damaged at byte N (reason R); it was not changed` and exits before it
listens. Skipping a damaged frame could lose an acknowledged event or
bring back a version it replaced, so the operator decides. A cut truncates
the file at the frame. That frame's `OK` was never sent, because the sync
didn't finish.

<!-- source: nostr/examples/store-events.coax replay -->
```jai
// store_saved.coax's rsd_replay line for line, with cxstore_byte and
// cxstore_truncate replaced by the buffer. Each frame's fate is
// store_recover.coax's nsr_frame.
Replay :: struct { ok: bool; frames: int; stored: int; kept: int; cut: bool; refused: bool; at: int; reason: int; }
replay :: (st: [] Store) -> Replay {
  out: Replay;
  total := st[0].total; file := st[0].file[..];
  at := 0;
  while true {
    head: [4] u8; k := 0;
    while k < 4 && at + k < total { head[k] = file[at + k]; k += 1; }
    f := ns_file_next(head[0..<k], at, total);
    to := ifx f.status == NSF_OK then f.to else at;
    p := nsr_region(f.status, at, f.from, to, total);
    zero := p; while zero < total && file[zero] != 0 { zero += 1; }
    nonzero := zero; while nonzero < total && file[nonzero] == 0 { nonzero += 1; }
    d := nsr_frame(NsrFrame.{status = f.status, at = at, from = f.from, to = to, total = total, zero = zero, nonzero = nonzero});
    if d == NSR_DONE { break; }
    if d == NSR_CUT { out.cut = true; st[0].total = at; break; }
    if d != NSR_NEXT || f.to - f.from > st[0].text.count { out.refused = true; out.at = at; out.reason = -1 - f.status; return out; }
    for i: f.from..<f.to { st[0].text[i - f.from] = file[i]; }
    t := cast(string) st[0].text[0..<f.to - f.from];
    out.frames += 1;
    // rsd_replay_read: admission's reading, but an event expired since it
    // was saved is kept.
    r := rs_admit_read(t, NOW, false, false, st[0].scratch[..]);
    r.reason = ns_file_replay_reason(r.reason);
    if r.reason == RS_ACCEPT { r = rs_admit_verified(r, nostr_event_verify_vartime(t, st[0].scratch[..], false)); }
    reason := r.reason;
    if reason == RS_ACCEPT { reason = rsd_admit(st[0].s[..], r, t); }
    if nsr_replayed(reason) != NSR_NEXT { out.refused = true; out.at = at; out.reason = reason; return out; }
    if reason == RS_ACCEPT { out.stored += 1; } else { out.kept += 1; }
    at = f.to;
  }
  out.ok = true;
  return out;
}
```

The rule has two known limits. A damaged length on the last frame that runs
past the end looks like a torn append and is cut, losing that one committed
event: no format without redundancy can tell them apart. And an interrupted
append whose zeros are not all at the end (a filesystem writing a later page
first) is refused, not cut: safe, but the operator has to act.

## Worked example: 19 insertions by hand

All events are signed with the BIP-340 vector 1 test key, so they share one
author. The relay's clock is NOW = 1793491200. The store has 6 record slots
and a 4,096-byte arena, so only the table limits it. Each event is named by
the first 4 bytes of its ID; the ids come from Python's `hashlib` (below).

| Event | `created_at` | ID | Bytes |
| --- | --- | --- | --- |
| note, kind 1 `first note` | NOW | `362a72d6` | 352 |
| the same note, `content` member first | NOW | `362a72d6` | 352 |
| kind 0 v1 | NOW-10 | `08ed00f9` | 359 |
| kind 0 v0 | NOW-20 | `5bba6c75` | 359 |
| kind 0 v2 | NOW | `1cda1d89` | 359 |
| kind 0 tie a | NOW | `a6e33212` | 362 |
| kind 0 tie h | NOW | `023de96f` | 362 |
| kind 30023 `d`=alpha | NOW-5 | `725dc10b` | 368 |
| kind 30023 `d`=beta | NOW-5 | `06443566` | 366 |
| kind 30023 `d`=alpha | NOW-100 | `1623bf7b` | 369 |
| kind 30023 `d`=alpha | NOW | `fd2009e2` | 368 |
| kind 20001 | NOW | `f032419d` | 352 |
| notes b, c, d, kind 1 | NOW | `bc9ce755`, `1175dbd5`, `a6fc2430` | 348 each |

The steps, with the store after each (live records only; `+` marks the
record added, `-` the one retired):

1. **note.** Empty store: stored at #0. The file gets its first frame at
   byte 0: the header 352 = 0x160 is `00 00 01 60`, then the 352 bytes.
   File: 356 bytes.
2. **note again.** Its ID is live at #0: `duplicate`. Nothing is written.
3. **note, content first.** The same ID, other bytes. The stored original
   begins `{"id"` and this one `{"content"`; the third bytes are `i` (0x69)
   and `c` (0x63), so this one is lower. It is no longer (352 bytes), so it
   is written over #0 in place: `duplicate`, the record count unchanged.
   The plan writes bytes, so the file gets a second frame at 356. File: 712.
4. **note as signed again.** Now higher than the stored original:
   `duplicate`, nothing written.
5. **kind 0 v1 at NOW-10.** No kind 0 from this author: stored at #1
   (+363, file 1,075).
6. **kind 0 v0 at NOW-20.** The head #1 is newer (NOW-10 > NOW-20):
   `superseded`, nothing written.
7. **kind 0 v2 at NOW.** Newer than #1: stored at #2, #1 retired (+363,
   file 1,438). Records: 3, live 2.
8. **kind 0 v1 again.** Its ID is on retired #1, which is not compacted
   yet: `duplicate`.
9. **kind 0 tie a at NOW.** The same second as the head #2. NIP-01 keeps the
   lowest ID: `1cda...` < `a6e3...` (0x1c < 0xa6), so the head stays:
   `superseded`.
10. **kind 0 tie h at NOW.** `023d...` < `1cda...` (0x02 < 0x1c): tie h wins.
    Stored at #3, #2 retired (+366, file 1,804). Records 4, live 2.
11. **30023 alpha at NOW-5.** A new address (kind, author, `alpha`): stored
    at #4 (+372, file 2,176).
12. **30023 beta at NOW-5.** Another `d`, another address: stored at #5
    (+370, file 2,546). The table is full: 6 records, 4 live.
13. **30023 alpha at NOW-100.** The alpha head #4 is newer: `superseded`.
14. **30023 alpha at NOW.** Newer than #4, but there is no slot. Compaction
    drops retired #1 and #2, moves the rest down (the arena goes from 2,166
    to 1,448 bytes used) and tracks the head: #4 becomes #2. The new alpha
    is stored at #4 and #2 retired (+372, file 2,918). Records 5, live 4.
15. **kind 0 v1 again.** Its record was compacted away, so the ID is not
    known any more. The head tie h (NOW) is newer than v1 (NOW-10):
    `superseded`. Step 8 answered `duplicate` to the same bytes. Both are
    `OK true`, and neither stores anything.
16. **kind 20001.** Ephemeral: accepted, not stored, not written.
17. **note b.** 5 records < 6: stored at #5 (+352, file 3,270).
18. **note c.** The table is full again. Compaction drops retired alpha #2:
    stored, and the table is full with 6 live records (+352, file 3,622).
19. **note d.** The table is full and compaction finds nothing to drop:
    `full`. Nothing is written.

At the end the store holds the content-first note, tie h, beta, the newest
alpha, note b and note c. The file holds 10 frames, starting at 0, 356, 712,
1075, 1438, 1804, 2176, 2546, 2918 and 3270. The superseded events, the
ephemeral event and the second and fourth note never reach it.

Replaying the file into an empty store re-admits the 10 frames in order: 9
are stored and frame 1 (the content-first note) is a lower duplicate written
in place, the same result as step 3. Then damage, each on a fresh copy of
the file. Note c's frame starts at 3270: a 4-byte header and 348 bytes.

- **The file ends at 3272, 2 bytes into note c's header.** `ns_file_next`
  has fewer than 4 header bytes: torn. 3272 − 3270 < 4, so `nsr_frame`
  cuts at 3270. Note c is gone; it was never acknowledged.
- **The file ends at 3374, 100 bytes into note c's body.** The header says
  348 bytes but only 100 follow: torn. The scan from 3274 finds no zero
  before the end: non-zero bytes, then (no) zeros. Cut at 3270.
- **The last 40 bytes are zeros (3582..3622).** The frame is whole and is
  the last one, and its body has a zero at 3582 and only zeros after: the
  file grew before its data was written. Cut at 3270, unread.
- **Frame 1's last signature digit changed (byte 709).** A whole frame in
  the middle: read, and verification fails. `RS_BAD_SIG` (5) is not a reason
  replay reads on: refused at byte 356.
- **Frame 2's first length byte set to `ff`.** The length is 0xff000167,
  over 262,144: bad. Byte 713 is zero but 712 is not, so this is not a run of
  zeros to the end: refused at byte 712, reason −1 − `NSF_BAD` = −4.
- **Frame 2's length 359 changed to 360 (byte 715).** The frame now ends at
  1076 and takes the next header's first byte, a zero. It is read as a whole
  middle frame, and the JSON reader refuses the trailing zero:
  `RS_BAD_EVENT` (3), refused at byte 712.

## The example

The example is [nostr/examples/store-events.coax](../examples/store-events.coax).
It loads the store, the file and recovery units and chapter 21's signer. It:

1. signs 14 events, rewrites the note with its `content` member first, and
   prints the 15 ids and sizes and the note in both member orders;
2. sends the 19 insertions through chapter 34's read, policy and signature
   steps into `admit_saved`, and prints the verdict, what the file got and
   the store's records after each;
3. lists the file's frames;
4. replays the file into an empty store and compares what both stores
   serve to a filter that matches everything;
5. replays the three crash shapes and the three kinds of committed damage.

The whole program:

<!-- source: nostr/examples/store-events.coax example -->
```jai
#load "../../coax/lib/nostr/store.coax";
#load "../../coax/lib/nostr/store_file.coax";
#load "../../coax/lib/nostr/store_recover.coax";
#load "../../coax/lib/nostr/event_sign.coax";
#import "Basic";

// The relay's clock for the whole run: 2026-11-01T00:00:00Z.
NOW :: 1793491200;
// BIP-340 vector 1's published test key and aux. Never sign with a real key
// in an example.
KEY_HEX :: "b7e151628aed2a6abf7158809cf4f3c762e7160f38b4da56a784d9045190cfef";
AUX_HEX :: "0000000000000000000000000000000000000000000000000000000000000001";

REASON :: [12] string.["accepted", "duplicate", "superseded", "bad event", "bad id", "bad signature",
  "auth kind", "tags", "time", "expired", "auth required", "full"];

// A store with room for six records, and its file. The file is a byte
// buffer here: coax/relay/relay.coax keeps it on disk through
// coax/platform/store-file-native.c, which `coax run` doesn't link.
Store :: struct {
  arena: [4096] u8; recs: [6] RsdRec;
  s: [1] RsdStore;
  file: [4096] u8; total: int;      // the append-only file and its size
  scratch: [4096] u8;               // the id preimage and the verifier's work
  text: [1024] u8;                  // one original read back from the file
}
store_init :: (st: [] Store) {
  st[0].s[0] = RsdStore.{}; st[0].s[0].arena = st[0].arena[..]; st[0].s[0].recs = st[0].recs[..];
}

// book:begin save
// store_saved.coax's rsd_save and rsd_admit_saved, with the file in memory:
// decide, append the frame (4-byte big-endian length, then the exact
// original) when the decision writes, and only then apply it.
save :: (st: [] Store, text: string) -> bool {
  head: [4] u8;
  if !ns_file_header(text.count, head[..]) || st[0].total + 4 + text.count > st[0].file.count { return false; }
  for i: 0..<4 { st[0].file[st[0].total + i] = head[i]; }
  for i: 0..<text.count { st[0].file[st[0].total + 4 + i] = text[i]; }
  st[0].total += 4 + text.count;   // the host's cxstore_commit: pwrite, then fsync
  return true;
}
admit_saved :: (st: [] Store, r: RsAdmit, text: string) -> int {
  c := rsd_rec(r);
  p := ns_store_plan(st[0].s[..], c, text, rsd_verified(r, text));
  if p.write != NS_W_NONE && !save(st, text) { return RS_FULL; }
  return ns_store_apply(st[0].s[..], p, c, text);
}
// book:end save

// One EVENT's text as relay.coax's ry_event handles it after the frame:
// read, policy, signature, then the store.
admit :: (st: [] Store, text: string) -> int {
  r := rs_admit_read(text, NOW, false, false, st[0].scratch[..]);
  if r.reason == RS_ACCEPT { r = rs_admit_verified(r, nostr_event_verify_vartime(text, st[0].scratch[..], false)); }
  if r.reason != RS_ACCEPT { return r.reason; }
  return admit_saved(st, r, text);
}

// book:begin replay
// store_saved.coax's rsd_replay line for line, with cxstore_byte and
// cxstore_truncate replaced by the buffer. Each frame's fate is
// store_recover.coax's nsr_frame.
Replay :: struct { ok: bool; frames: int; stored: int; kept: int; cut: bool; refused: bool; at: int; reason: int; }
replay :: (st: [] Store) -> Replay {
  out: Replay;
  total := st[0].total; file := st[0].file[..];
  at := 0;
  while true {
    head: [4] u8; k := 0;
    while k < 4 && at + k < total { head[k] = file[at + k]; k += 1; }
    f := ns_file_next(head[0..<k], at, total);
    to := ifx f.status == NSF_OK then f.to else at;
    p := nsr_region(f.status, at, f.from, to, total);
    zero := p; while zero < total && file[zero] != 0 { zero += 1; }
    nonzero := zero; while nonzero < total && file[nonzero] == 0 { nonzero += 1; }
    d := nsr_frame(NsrFrame.{status = f.status, at = at, from = f.from, to = to, total = total, zero = zero, nonzero = nonzero});
    if d == NSR_DONE { break; }
    if d == NSR_CUT { out.cut = true; st[0].total = at; break; }
    if d != NSR_NEXT || f.to - f.from > st[0].text.count { out.refused = true; out.at = at; out.reason = -1 - f.status; return out; }
    for i: f.from..<f.to { st[0].text[i - f.from] = file[i]; }
    t := cast(string) st[0].text[0..<f.to - f.from];
    out.frames += 1;
    // rsd_replay_read: admission's reading, but an event expired since it
    // was saved is kept.
    r := rs_admit_read(t, NOW, false, false, st[0].scratch[..]);
    r.reason = ns_file_replay_reason(r.reason);
    if r.reason == RS_ACCEPT { r = rs_admit_verified(r, nostr_event_verify_vartime(t, st[0].scratch[..], false)); }
    reason := r.reason;
    if reason == RS_ACCEPT { reason = rsd_admit(st[0].s[..], r, t); }
    if nsr_replayed(reason) != NSR_NEXT { out.refused = true; out.at = at; out.reason = reason; return out; }
    if reason == RS_ACCEPT { out.stored += 1; } else { out.kept += 1; }
    at = f.to;
  }
  out.ok = true;
  return out;
}
// book:end replay

// Sign one event with the test key into out; the event's text.
sign :: (created_at: int, kind: int, tags: string, content: string, out: [] u8) -> string {
  key: [32] u8; aux: [32] u8;
  #assert hex_decode_checked(KEY_HEX, key[..], false).error == NC_OK;
  #assert hex_decode_checked(AUX_HEX, aux[..], false).error == NC_OK;
  work: [1] NostrSignWork; scratch: [8192] u8;
  n := EVENT_MORE;
  while n == EVENT_MORE { n = nostr_event_sign_step(key[..], aux[..], created_at, kind, tags, content, work[..], scratch[..], out, false); }
  #assert n > 0;
  return cast(string) out[0..<n];
}
text_of :: (n: int, out: [] u8) -> string {
  #assert n > 0;
  return cast(string) out[0..<n];
}
// The same event with its "content" member moved first: other bytes, the
// same id and signature.
content_first :: (event: string, out: [] u8) -> string {
  at := -1; key := ",\"content\":\"";
  for i: 0..<event.count - key.count { if at < 0 && event[i..<i + key.count] == key { at = i; } }
  #assert at > 0;
  end := at + key.count;
  while event[end] != '"' { end += 1; }
  end += 1;   // the member is event[at + 1..<end]
  n := json_raw(out, 0, "{"); n = json_raw(out, n, event[at + 1..<end]); n = json_raw(out, n, ",");
  n = json_raw(out, n, event[1..<at]); n = json_raw(out, n, event[end..<event.count]);
  return text_of(n, out);
}
// Prints the first 4 bytes of bytes in hex.
print_hex4 :: (bytes: [] u8) {
  digits := "0123456789abcdef";
  out: [8] u8;
  for i: 0..<4 { out[2 * i] = digits[bytes[i] >> 4]; out[2 * i + 1] = digits[bytes[i] & 15]; }
  print("%", cast(string) out[..]);
}
show_time :: (t: int) {
  if t == NOW { print("NOW"); } else if t < NOW { print("NOW-%", NOW - t); } else { print("NOW+%", t - NOW); }
}
show_store :: (st: [] Store) {
  s := st[0].s[..];
  print("    store: % records, % live, % arena bytes used, % live bytes\n", s[0].n, s[0].live, s[0].used, s[0].bytes);
  for i: 0..<s[0].n {
    r := s[0].recs[i];
    print("      #% % kind % ", i, ifx r.live then "live   " else "retired", r.kind);
    show_time(r.created);
    if ns_class(r.kind) == NS_ADDRESSABLE { print(" d=%", cast(string) r.d[0..<r.d_n]); }
    id := r.id; print(" id "); print_hex4(id[..]);
    print(" %..% (% bytes) %...\n", r.from, r.to, r.to - r.from, rsd_text(s, i)[0..<12]);
  }
}
insert :: (st: [] Store, label: string, text: string) {
  before := st[0].total;
  result := admit(st, text);
  print("% -> %", label, REASON[result]);
  if st[0].total == before { print(", file unchanged\n"); }
  else { print(", file +% bytes: frame at % with header ", st[0].total - before, before); print_hex4(st[0].file[before..<before + 4]); print("\n"); }
  show_store(st);
}
// The texts a full query returns, newest first, joined: what the store serves.
served :: (st: [] Store, out: [] u8) -> string {
  f: NostrFilter; idx: [8] int;
  q := rsd_query(st[0].s[..], f, NOW, idx[..]);
  n := 0;
  for i: 0..<q { n = json_raw(out, n, rsd_text(st[0].s[..], idx[i])); n = json_raw(out, n, "\n"); }
  return cast(string) out[0..<n];
}
// Whether a live record holds exactly text.
holds :: (st: [] Store, text: string) -> bool {
  for i: 0..<st[0].s[0].n { if st[0].s[0].recs[i].live && rsd_text(st[0].s[..], i) == text { return true; } }
  return false;
}
// Copies a's file into b and replays it into b's empty store.
reload :: (a: [] Store, b: [] Store, total: int) -> Replay {
  store_init(b);
  for i: 0..<total { b[0].file[i] = a[0].file[i]; }
  b[0].total = total;
  return replay(b);
}
show_replay :: (label: string, rp: Replay, st: [] Store, before: int) {
  print("% -> ", label);
  if rp.refused {
    // an admission reason, or -1 - the frame's status (NSF_BAD is 3)
    why := ifx rp.reason >= 0 then REASON[rp.reason] else "no frame has this length";
    print("refused at byte %, reason % (%), file unchanged: %\n", rp.at, rp.reason, why, st[0].total == before);
    return;
  }
  print("ok: % frames, % stored, % kept", rp.frames, rp.stored, rp.kept);
  if rp.cut { print(", cut at byte % (was %)", st[0].total, before); }
  print("\n");
}

main :: () {
  a: [1] Store; store_init(a[..]);
  b1: [1024] u8; note := sign(NOW, 1, "[]", "first note", b1[..]);
  b2: [1024] u8; moved := content_first(note, b2[..]);
  b3: [1024] u8; p10 := sign(NOW - 10, 0, "[]", "{\"name\":\"v1\"}", b3[..]);
  b4: [1024] u8; p20 := sign(NOW - 20, 0, "[]", "{\"name\":\"v0\"}", b4[..]);
  b5: [1024] u8; p0 := sign(NOW, 0, "[]", "{\"name\":\"v2\"}", b5[..]);
  b6: [1024] u8; tie_a := sign(NOW, 0, "[]", "{\"name\":\"tie a\"}", b6[..]);
  b7: [1024] u8; tie_h := sign(NOW, 0, "[]", "{\"name\":\"tie h\"}", b7[..]);
  b8: [1024] u8; alpha5 := sign(NOW - 5, 30023, "[[\"d\",\"alpha\"]]", "alpha one", b8[..]);
  b9: [1024] u8; beta5 := sign(NOW - 5, 30023, "[[\"d\",\"beta\"]]", "beta one", b9[..]);
  b10: [1024] u8; alpha100 := sign(NOW - 100, 30023, "[[\"d\",\"alpha\"]]", "alpha zero", b10[..]);
  b11: [1024] u8; alpha0 := sign(NOW, 30023, "[[\"d\",\"alpha\"]]", "alpha two", b11[..]);
  b12: [1024] u8; typing := sign(NOW, 20001, "[]", "typing", b12[..]);
  b13: [1024] u8; nb := sign(NOW, 1, "[]", "note b", b13[..]);
  b14: [1024] u8; nc := sign(NOW, 1, "[]", "note c", b14[..]);
  b15: [1024] u8; nd := sign(NOW, 1, "[]", "note d", b15[..]);

  print("the events (the BIP-340 vector 1 test key), by the first 4 bytes of their ids\n");
  scratch: [4096] u8;
  names := [15] string.["note", "note, content first", "kind 0 v1 NOW-10", "kind 0 v0 NOW-20", "kind 0 v2 NOW",
    "kind 0 tie a NOW", "kind 0 tie h NOW", "30023 alpha NOW-5", "30023 beta NOW-5", "30023 alpha NOW-100",
    "30023 alpha NOW", "20001 typing", "note b", "note c", "note d"];
  texts := [15] string.[note, moved, p10, p20, p0, tie_a, tie_h, alpha5, beta5, alpha100, alpha0, typing, nb, nc, nd];
  for i: 0..<15 {
    e := nostr_event_read(texts[i], scratch[..], false);
    id := e.id; print("  %: id ", names[i]); print_hex4(id[..]); print(" (% bytes)\n", texts[i].count);
  }
  print("the note as signed:     %\n", note);
  print("the note content first: %\n", moved);

  print("\ninsertions, in order\n");
  insert(a[..], "1. note", note);
  insert(a[..], "2. note again", note);
  insert(a[..], "3. note, content first (lower bytes)", moved);
  insert(a[..], "4. note as signed again (higher bytes)", note);
  insert(a[..], "5. kind 0 v1 at NOW-10", p10);
  insert(a[..], "6. kind 0 v0 at NOW-20 (older)", p20);
  insert(a[..], "7. kind 0 v2 at NOW (newer)", p0);
  insert(a[..], "8. kind 0 v1 again (retired, not yet compacted)", p10);
  insert(a[..], "9. kind 0 tie a at NOW", tie_a);
  insert(a[..], "10. kind 0 tie h at NOW", tie_h);
  insert(a[..], "11. 30023 d=alpha at NOW-5", alpha5);
  insert(a[..], "12. 30023 d=beta at NOW-5", beta5);
  insert(a[..], "13. 30023 d=alpha at NOW-100 (older)", alpha100);
  insert(a[..], "14. 30023 d=alpha at NOW (newer; the table is full: compaction)", alpha0);
  insert(a[..], "15. kind 0 v1 again (compacted away)", p10);
  insert(a[..], "16. kind 20001 (ephemeral, never stored)", typing);
  insert(a[..], "17. note b", nb);
  insert(a[..], "18. note c (the table is full: compaction)", nc);
  insert(a[..], "19. note d (the table is full, nothing to drop)", nd);

  print("\nthe file: % bytes, each frame a 4-byte big-endian length and the original\n", a[0].total);
  at := 0; k := 0;
  while at < a[0].total {
    n := ns_u32(a[0].file[..], at);
    e := nostr_event_read(cast(string) a[0].file[at + 4..<at + 4 + n], scratch[..], false);
    id := e.id; print("  frame % at %: header ", k, at); print_hex4(a[0].file[at..<at + 4]); print(", % bytes, kind %, id ", n, e.kind); print_hex4(id[..]); print("\n");
    at += 4 + n; k += 1;
  }

  print("\nrestart: replay the file into an empty store\n");
  live: [8192] u8; want := served(a[..], live[..]);
  b: [1] Store;
  rp := reload(a[..], b[..], a[0].total);
  show_replay("whole file", rp, b[..], a[0].total);
  got: [8192] u8;
  print("serves what the live store served: %\n", served(b[..], got[..]) == want);
  show_store(b[..]);

  last := at - (4 + nc.count);   // where note c's frame starts
  print("\ncrash damage at the end of the file (an append that never finished)\n");
  rp = reload(a[..], b[..], last + 2);
  show_replay("file ends 2 bytes into note c's header", rp, b[..], last + 2);
  rp = reload(a[..], b[..], last + 4 + 100);
  show_replay("file ends 100 bytes into note c's body", rp, b[..], last + 4 + 100);
  store_init(b[..]);
  for i: 0..<a[0].total { b[0].file[i] = a[0].file[i]; }
  for i: a[0].total - 40..<a[0].total { b[0].file[i] = 0; }
  b[0].total = a[0].total;
  rp = replay(b[..]);
  show_replay("note c's last 40 bytes are zeros", rp, b[..], a[0].total);
  print("after the cut: % live records, note c held: %\n", b[0].s[0].live, holds(b[..], nc));

  print("\ndamage to committed bytes: the relay refuses to start\n");
  second := 4 + ns_u32(a[0].file[..], 0);     // frame 1 starts here
  sig := second + 4 + moved.count - 3;        // the last hex digit of its signature
  store_init(b[..]);
  for i: 0..<a[0].total { b[0].file[i] = a[0].file[i]; }
  b[0].total = a[0].total;
  b[0].file[sig] = ifx b[0].file[sig] == '0' then '1' else '0';
  rp = replay(b[..]);
  show_replay("frame 1's last signature digit changed", rp, b[..], a[0].total);
  b[0].file[sig] = a[0].file[sig];
  third := second + 4 + moved.count;          // frame 2's header
  b[0].file[third] = 0xff;
  rp = replay(b[..]);
  show_replay("frame 2's first length byte set to ff", rp, b[..], a[0].total);
  b[0].file[third] = 0;
  b[0].file[third + 3] += 1;
  rp = replay(b[..]);
  show_replay("frame 2's length one more (360 bytes)", rp, b[..], a[0].total);
}
```

Run it from the repository root:

```shell
coax/bin/coax run nostr/examples/store-events.coax --run-steps 250000000
```

The output has the store's records after every step; the full transcript is
[nostr/fixtures/store-events.out](../fixtures/store-events.out). Steps 3,
10, 14, 15 and 19, the file and the replays:

```text
3. note, content first (lower bytes) -> duplicate, file +356 bytes: frame at 356 with header 00000160
    store: 1 records, 1 live, 352 arena bytes used, 352 live bytes
      #0 live    kind 1 NOW id 362a72d6 0..352 (352 bytes) {"content":"...
10. kind 0 tie h at NOW -> accepted, file +366 bytes: frame at 1438 with header 0000016a
    store: 4 records, 2 live, 1432 arena bytes used, 714 live bytes
      #0 live    kind 1 NOW id 362a72d6 0..352 (352 bytes) {"content":"...
      #1 retired kind 0 NOW-10 id 08ed00f9 352..711 (359 bytes) {"id":"08ed0...
      #2 retired kind 0 NOW id 1cda1d89 711..1070 (359 bytes) {"id":"1cda1...
      #3 live    kind 0 NOW id 023de96f 1070..1432 (362 bytes) {"id":"023de...
14. 30023 d=alpha at NOW (newer; the table is full: compaction) -> accepted, file +372 bytes: frame at 2546 with header 00000170
    store: 5 records, 4 live, 1816 arena bytes used, 1448 live bytes
      #0 live    kind 1 NOW id 362a72d6 0..352 (352 bytes) {"content":"...
      #1 live    kind 0 NOW id 023de96f 352..714 (362 bytes) {"id":"023de...
      #2 retired kind 30023 NOW-5 d=alpha id 725dc10b 714..1082 (368 bytes) {"id":"725dc...
      #3 live    kind 30023 NOW-5 d=beta id 06443566 1082..1448 (366 bytes) {"id":"06443...
      #4 live    kind 30023 NOW d=alpha id fd2009e2 1448..1816 (368 bytes) {"id":"fd200...
15. kind 0 v1 again (compacted away) -> superseded, file unchanged
    store: 5 records, 4 live, 1816 arena bytes used, 1448 live bytes
      #0 live    kind 1 NOW id 362a72d6 0..352 (352 bytes) {"content":"...
      #1 live    kind 0 NOW id 023de96f 352..714 (362 bytes) {"id":"023de...
      #2 retired kind 30023 NOW-5 d=alpha id 725dc10b 714..1082 (368 bytes) {"id":"725dc...
      #3 live    kind 30023 NOW-5 d=beta id 06443566 1082..1448 (366 bytes) {"id":"06443...
      #4 live    kind 30023 NOW d=alpha id fd2009e2 1448..1816 (368 bytes) {"id":"fd200...
19. note d (the table is full, nothing to drop) -> full, file unchanged
    store: 6 records, 6 live, 2144 arena bytes used, 2144 live bytes
      #0 live    kind 1 NOW id 362a72d6 0..352 (352 bytes) {"content":"...
      #1 live    kind 0 NOW id 023de96f 352..714 (362 bytes) {"id":"023de...
      #2 live    kind 30023 NOW-5 d=beta id 06443566 714..1080 (366 bytes) {"id":"06443...
      #3 live    kind 30023 NOW d=alpha id fd2009e2 1080..1448 (368 bytes) {"id":"fd200...
      #4 live    kind 1 NOW id bc9ce755 1448..1796 (348 bytes) {"id":"bc9ce...
      #5 live    kind 1 NOW id 1175dbd5 1796..2144 (348 bytes) {"id":"1175d...

the file: 3622 bytes, each frame a 4-byte big-endian length and the original
  frame 0 at 0: header 00000160, 352 bytes, kind 1, id 362a72d6
  frame 1 at 356: header 00000160, 352 bytes, kind 1, id 362a72d6
  frame 2 at 712: header 00000167, 359 bytes, kind 0, id 08ed00f9
  frame 3 at 1075: header 00000167, 359 bytes, kind 0, id 1cda1d89
  frame 4 at 1438: header 0000016a, 362 bytes, kind 0, id 023de96f
  frame 5 at 1804: header 00000170, 368 bytes, kind 30023, id 725dc10b
  frame 6 at 2176: header 0000016e, 366 bytes, kind 30023, id 06443566
  frame 7 at 2546: header 00000170, 368 bytes, kind 30023, id fd2009e2
  frame 8 at 2918: header 0000015c, 348 bytes, kind 1, id bc9ce755
  frame 9 at 3270: header 0000015c, 348 bytes, kind 1, id 1175dbd5

restart: replay the file into an empty store
whole file -> ok: 10 frames, 9 stored, 1 kept
serves what the live store served: true
    store: 6 records, 6 live, 2144 arena bytes used, 2144 live bytes
      #0 live    kind 1 NOW id 362a72d6 0..352 (352 bytes) {"content":"...
      #1 live    kind 0 NOW id 023de96f 352..714 (362 bytes) {"id":"023de...
      #2 live    kind 30023 NOW-5 d=beta id 06443566 714..1080 (366 bytes) {"id":"06443...
      #3 live    kind 30023 NOW d=alpha id fd2009e2 1080..1448 (368 bytes) {"id":"fd200...
      #4 live    kind 1 NOW id bc9ce755 1448..1796 (348 bytes) {"id":"bc9ce...
      #5 live    kind 1 NOW id 1175dbd5 1796..2144 (348 bytes) {"id":"1175d...

crash damage at the end of the file (an append that never finished)
file ends 2 bytes into note c's header -> ok: 9 frames, 8 stored, 1 kept, cut at byte 3270 (was 3272)
file ends 100 bytes into note c's body -> ok: 9 frames, 8 stored, 1 kept, cut at byte 3270 (was 3374)
note c's last 40 bytes are zeros -> ok: 9 frames, 8 stored, 1 kept, cut at byte 3270 (was 3622)
after the cut: 5 live records, note c held: false

damage to committed bytes: the relay refuses to start
frame 1's last signature digit changed -> refused at byte 356, reason 5 (bad signature), file unchanged: true
frame 2's first length byte set to ff -> refused at byte 712, reason -4 (no frame has this length), file unchanged: true
frame 2's length one more (360 bytes) -> refused at byte 712, reason 3 (bad event), file unchanged: true
```

Each record line gives its index, whether it is live, its kind, its
`created_at`, the `d` value for an addressable kind, the first 4 bytes of
its ID, its span in the arena and the first 12 bytes of its original. The
results match the trace above step for step. Step 3 shows the in-place
write: the span stays 0..352 and the original now starts `{"content"`.
Step 14 shows the compaction: the arena's spans move down and alpha's old
record is now #2.

## How the code is checked

### Checks in the example

The checks are in
[nostr/examples/checks/store-events.coax](../examples/checks/store-events.coax),
which loads the example, so running the example doesn't run them:

- the note and the two tie events signed by Coax are byte for byte the
  events that `coax/tests/nostr/nip59_reference.py` writes and signs, and
  the content-first note is the one Python's `json` writes. That script is
  pure Python (BIP-340 from the BIP, `hashlib`, `json`) and shares no code
  with Coax;
- the 19 insertions of Python's events get the verdicts worked out by hand
  above from NIP-01 and Python's ids;
- the file they write equals the file built by hand from the 10 originals
  with their 4-byte lengths, and its SHA-256 is the one Python computed for
  the same 3,622 bytes;
- the hand-built file replays to the same 6 originals, and each kind of
  damage gives the cut or refusal worked out above, at the same byte, with
  the file unchanged on refusal.

Python's events are constants at the top of the checks file, above the
listing.

<!-- source: nostr/examples/checks/store-events.coax check -->
```jai
// The 19 insertions of the example, as Python's events, and the verdict
// worked out by hand for each from NIP-01 and Python's ids.
SESSION :: [19] string.[PY_NOTE, PY_NOTE, PY_MOVED, PY_NOTE, PY_P10, PY_P20, PY_P0, PY_P10, PY_TIE_A, PY_TIE_H,
  PY_ALPHA5, PY_BETA5, PY_ALPHA100, PY_ALPHA0, PY_P10, PY_TYPING, PY_NB, PY_NC, PY_ND];
HAND :: [19] int.[RS_ACCEPT, RS_DUP, RS_DUP, RS_DUP, RS_ACCEPT, RS_SUPERSEDED, RS_ACCEPT, RS_DUP, RS_SUPERSEDED, RS_ACCEPT,
  RS_ACCEPT, RS_ACCEPT, RS_SUPERSEDED, RS_ACCEPT, RS_SUPERSEDED, RS_ACCEPT, RS_ACCEPT, RS_ACCEPT, RS_FULL];
// The originals that reach the file, in order, and what the store holds at the end.
WRITTEN :: [10] string.[PY_NOTE, PY_MOVED, PY_P10, PY_P0, PY_TIE_H, PY_ALPHA5, PY_BETA5, PY_ALPHA0, PY_NB, PY_NC];
HELD :: [6] string.[PY_MOVED, PY_TIE_H, PY_BETA5, PY_ALPHA0, PY_NB, PY_NC];
// SHA-256 of the 3,622-byte file Python built from WRITTEN.
PY_FILE_SHA256 :: "0c011d88f83ce61471b2d4002c3d438ff7352334e65a9c3af37cc9c55b03a0e3";

// The file written by hand into st, a new Store whose bytes are all zero:
// each original after its length in 4 big-endian bytes (every length is
// below 65,536, so the first two are zero). Frames start at 0, 356, 712,
// 1075, 1438, 1804, 2176, 2546, 2918 and 3270; the file is 3,622 bytes.
hand_file :: (st: [] Store) {
  store_init(st);
  n := 0;
  for t: WRITTEN {
    st[0].file[n + 2] = cast,trunc(u8) (t.count >> 8); st[0].file[n + 3] = cast,trunc(u8) t.count;
    for i: 0..<t.count { st[0].file[n + 4 + i] = t[i]; }
    n += 4 + t.count;
  }
  st[0].total = n;
}
// Whether st holds exactly HELD, all live, in index order.
holds_all :: (st: [] Store) -> bool {
  if st[0].s[0].n != 6 || st[0].s[0].live != 6 { return false; }
  for i: 0..<6 { if rsd_text(st[0].s[..], i) != HELD[i] { return false; } }
  return true;
}

#test "holds_all refuses a record that is not live and a different first original" {
  a: [1] Store; store_init(a[..]); c: RsdRec;
  for t: HELD { ns_store_append(a[0].s[..], c, t); }
  #assert holds_all(a[..]);
  a[0].s[0].live = 5; #assert !holds_all(a[..]);
  b: [1] Store; store_init(b[..]);
  ns_store_append(b[0].s[..], c, PY_NOTE);
  for i: 1..<6 { ns_store_append(b[0].s[..], c, HELD[i]); }
  #assert !holds_all(b[..]);
}
#test "the example's events are Python's bytes" #steps 2000000000 {
  b: [1024] u8; m: [1024] u8;
  #assert sign(NOW, 1, "[]", "first note", b[..]) == PY_NOTE;
  #assert content_first(PY_NOTE, m[..]) == PY_MOVED;
  #assert sign(NOW, 0, "[]", "{\"name\":\"tie a\"}", b[..]) == PY_TIE_A;
  #assert sign(NOW, 0, "[]", "{\"name\":\"tie h\"}", b[..]) == PY_TIE_H;
}
#test "Python's events get the hand verdicts and write the hand file" #steps 4000000000 {
  a: [1] Store; store_init(a[..]);
  for i: 0..<19 { #assert admit(a[..], SESSION[i]) == HAND[i]; }
  #assert holds_all(a[..]);
  h: [1] Store; hand_file(h[..]);
  #assert a[0].total == 3622 && h[0].total == 3622 && cast(string) a[0].file[0..<3622] == cast(string) h[0].file[0..<3622];
}
#test "the hand file is Python's and replays to the same store" #steps 2000000000 {
  h: [1] Store; hand_file(h[..]);
  digest: [32] u8; text: [64] u8;
  #assert sha256_hash_checked(h[0].file[0..<h[0].total], digest[..], false) == 0;
  #assert hex_encode_checked(cast(string) digest[..], text[..], false).written == 64 && cast(string) text[..] == PY_FILE_SHA256;
  r := replay(h[..]);
  #assert r.ok && r.frames == 10 && r.stored == 9 && r.kept == 1 && !r.cut && !r.refused && holds_all(h[..]);
}
#test "a partial header at the end is cut" #steps 2000000000 {
  h: [1] Store; hand_file(h[..]); h[0].total = 3272;
  r := replay(h[..]);
  #assert r.ok && r.cut && h[0].total == 3270 && r.frames == 9 && !holds(h[..], PY_NC) && holds(h[..], PY_NB);
}
#test "a partial body at the end is cut" #steps 2000000000 {
  h: [1] Store; hand_file(h[..]); h[0].total = 3374;
  r := replay(h[..]);
  #assert r.ok && r.cut && h[0].total == 3270 && r.frames == 9;
}
#test "a last frame ending in zeros is cut" #steps 2000000000 {
  h: [1] Store; hand_file(h[..]);
  for i: 3582..<3622 { h[0].file[i] = 0; }
  r := replay(h[..]);
  #assert r.ok && r.cut && h[0].total == 3270 && r.frames == 9;
}
#test "damage to committed frames refuses and changes nothing" #steps 2000000000 {
  h: [1] Store; hand_file(h[..]);
  #assert h[0].file[0] == 0 && h[0].file[1] == 0 && h[0].file[2] == 1 && h[0].file[3] == 0x60;
  // frame 1 ends at 712 with its last signature digit, a quote and a brace
  digit := h[0].file[709]; h[0].file[709] = ifx digit == '0' then '1' else '0';
  r := replay(h[..]);
  #assert !r.ok && r.refused && r.at == 356 && r.reason == RS_BAD_SIG && h[0].total == 3622;
  g: [1] Store; hand_file(g[..]); g[0].file[712] = 0xff;
  r = replay(g[..]);
  #assert !r.ok && r.refused && r.at == 712 && r.reason == -1 - NSF_BAD && g[0].total == 3622;
  k: [1] Store; hand_file(k[..]); k[0].file[715] = 0x68;   // 359 becomes 360
  r = replay(k[..]);
  #assert !r.ok && r.refused && r.at == 712 && r.reason == RS_BAD_EVENT && k[0].total == 3622;
}
```

To run them:

```shell
coax/bin/coax test nostr/examples/checks/store-events.coax --run-steps 250000000
```

On 2026-11-01 `coax test` ran 471 checks, including the loaded units' own
checks, and all passed in 151 s. The interpreter has a fixed limit on
aggregate traversal per check, and one BIP-340 verification uses a large
share of it, so the replays are split into one check each: the 19
insertions take 1.5 billion steps and about 57 s. `coax mutate
nostr/examples/checks/store-events.coax --root-only` killed 41 of 41
mutants in the checks' own procedures. A first run left 6 survivors: the
hand file wrote the two zero header bytes into a reused store (now each
damage case gets a new, zeroed store), and `holds_all` was only ever asked
about the right store (now a check gives it a store with a record that is
not live and one with a different first original).

### The library's own checks

`store_core.coax` checks verified-only admission, malformed spans, one
record per ID with the lowest original in place or appended, replacement
on tied timestamps, expiry, compaction under both bounds and the query
order. `store_file.coax` checks the framing at lengths 0, 1, 262,144 and
262,145 and at offset 0. `store_recover.coax` checks each row of the table
above and has a `#forall` over 107,520 cases: every cut is a crash's shape,
`done` only at the end, `read on` only for a whole frame.

These records were not rerun for this chapter, because the files are
unchanged since:

- [2026-10-29-nostr-store-r1.md](../../docs/verification/2026-10-29-nostr-store-r1.md):
  `coax mutate` on `store_core.coax` killed 590 of 590 mutants, and on
  `store_file.coax` 242 of 242.
- [2026-10-30-nostr-store-recovery.md](../../docs/verification/2026-10-30-nostr-store-recovery.md)
  (files at `092c3e44a4`): `store_recover.coax` 72 of 72;
  `coax/tests/nostr/store_file.py`, the real file under ASan and UBSan, 38 of
  38, including eight kinds of committed damage refused with the file
  unchanged; and `coax/tests/nostr/store_crash.py`, 27 of 27.

### The SIGKILL test

`store_crash.py` is the evidence for "every acknowledged event survives a
crash". Each round starts the relay on the same `--store` file, streams
signed writes (notes, kind 0 and kind 30617 versions, and acknowledged
events again) over one NIP-42-authenticated WebSocket, and kills the relay
with SIGKILL after 0.1 to 3 s. Before the next start it damages the file
as an interrupted append can. After every restart every acknowledged note
is served, nothing unsent is served, the replaceable heads are never older
than the newest acknowledged version, and the file is whole frames. The
2026-10-30 run had 18 kills, 57 of 70 events acknowledged, 10 restarts that
cut a partial write, and no loss. It takes about 6 minutes and was not
rerun.

SIGKILL tests the process dying; the kernel still writes its page cache.
Power loss and filesystem reordering are emulated only by the tail damage,
not by a real power cut.

### What is not solved

An older copy of the whole file, or a deleted file, replays as a valid older
or empty store. Detecting that needs a rollback-resistant checkpoint held
outside the file (a store identity, a generation and a digest). Choosing
that provider is an owner decision, and #966b stays open. The file also has
no per-frame checksum and no compaction: replaced versions stay in the file
and are replayed on every start.

## Summary

- The store admits only events whose signature was verified over the exact
  bytes it keeps.
- One record per ID, holding the lowest exact original; a duplicate never
  takes another record.
- Replaceable kinds are keyed by kind and author, addressable kinds also by
  `d`; the newer `created_at` wins, then the lowest ID. Ephemeral kinds are
  never stored.
- A full table is compacted; `full` means nothing could be dropped.
- Every write is a 4-byte big-endian length and the original, synced before
  `OK true`. Replay re-verifies every frame, cuts only a crash's partial
  last append, and refuses any other damage without changing the file.

## Dependencies

- [34. Admit an event](ch034-admit-an-event.md)
- [22. Kinds and tags](ch022-kinds-and-tags.md)

## Sources

- [NIP-01](../nips/official/01.md): replaceable, ephemeral and addressable
  kinds, the same-timestamp lowest-ID rule, and `OK` with `duplicate:` and
  `error:`.
- [NIP-40](../nips/official/40.md), Expiration Timestamp.
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
