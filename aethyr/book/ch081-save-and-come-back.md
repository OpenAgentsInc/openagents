# 81. Save and come back

> **Status: partly implemented.**
> [apps/openagents-web/save.coax](../../apps/openagents-web/save.coax) keeps
> a realm's save as a versioned `AESV` container with a CRC-32, in two slots,
> in the browser and, for a player with an account, on the server
> (`/api/playtest/progress`, which checks the same container). A load tries
> the current save, then the last good one; another world's save and a schema
> newer than the build are refused; a save from before containers opens as
> schema 0. The Garden, the Haunted House, Everglade's arranging view and
> every walkable realm's place use it. A made realm's document is saved by
> revision and a stale save is refused with `409`
> ([chapter 52](ch052-edit-a-realm-in-place.md)), and so is a player's
> progress (#12719): the server keeps a progress save only from the revision
> there now, and no decision reads a device's clock. The example below runs
> natively in debug and release. Not built yet: the layer as
> `aethyr/save.coax`, declared save fields, retries that are applied once,
> and a merge or a conflict copy in place of taking the server's save. See
> [Still to do](#still-to-do).

A save keeps what a player made or reached, so that a reload or a later
visit brings it back. It is not a replay: a replay keeps inputs and
recomputes ([chapter 5](ch005-replay-a-session-exactly.md)), and a save
keeps a chosen part of the state. This chapter makes that part versioned and
checked, and keeps the rules that `save.coax` already enforces.

## What this chapter teaches

- How the container and its checksum make a broken write detectable.
- How two slots make a broken write fall back to the last good save.
- How to move an old save forward, and refuse a save from a newer build.
- Why a conflict is found by revision, not by clock time.

## The container

`save_pack` writes the bytes: `"AESV"`, the container version
`SAVE_VERSION` (2), the layer `SAVE_PROGRESS` (2), two reserved bytes, the
payload's schema as a little-endian `u32`, a varint and the world's ID, a
varint and the payload, then a CRC-32 (polynomial `0xEDB88320`) of
everything before it:

<!-- source: apps/openagents-web/save.coax pack -->
```jai
// payload packed into out as a progress container; its length, or -1 when it doesn't fit.
save_pack :: (world: string, schema: int, payload: [] u8, n: int, out: [] u8) -> int #export_c {
  if out.count < 16 + world.count + n || n < 0 || n > payload.count { return -1; }
  out[0] = 65; out[1] = 69; out[2] = 83; out[3] = 86;
  out[4] = SAVE_VERSION; out[5] = SAVE_PROGRESS; out[6] = 0; out[7] = 0;
  for k: 0..<4 { out[8 + k] = cast(u8) ((schema >> (8 * k)) & 255); }
  i := save_varint(out, 12, world.count);
  if i < 0 || i + world.count > out.count { return -1; }
  for k: 0..<world.count { out[i + k] = world[k]; }
  i = save_varint(out, i + world.count, n);
  if i < 0 || i + n + 4 > out.count { return -1; }
  for k: 0..<n { out[i + k] = payload[k]; }
  i += n;
  crc := save_crc32(out, i);
  for k: 0..<4 { out[i + k] = cast(u8) ((crc >> cast(u32) (8 * k)) & 255); }
  return i + 4;
}
```

`save_open` takes a container back only when everything about it holds: the
magic, a container version it knows (1 or 2), the layer, the CRC, a schema no
newer than the build's, the world, and lengths that end exactly at the CRC:

<!-- source: apps/openagents-web/save.coax open -->
```jai
// A container's payload, when its magic, version, layer, CRC and world are
// right and its schema is one this build knows (at most max_schema).
save_open :: (b: [] u8, n: int, world: string, max_schema: int) -> SaveOpened #export_c {
  no: SaveOpened;
  if n < 18 || n > b.count || b[0] != 65 || b[1] != 69 || b[2] != 83 || b[3] != 86 { return no; }
  if (b[4] != 1 && b[4] != 2) || b[5] != SAVE_PROGRESS { return no; }
  want := cast(u32) b[n - 4] | (cast(u32) b[n - 3] << 8) | (cast(u32) b[n - 2] << 16) | (cast(u32) b[n - 1] << 24);
  if save_crc32(b, n - 4) != want { return no; }
  schema := cast(int) b[8] | (cast(int) b[9] << 8) | (cast(int) b[10] << 16) | (cast(int) b[11] << 24);
  if schema > max_schema { return no; }
  at: [1] int; at[0] = 12;
  wn := save_read_varint(b, n - 4, at[..]);
  if wn != world.count || at[0] + wn > n - 4 { return no; }
  for k: 0..<wn { if b[at[0] + k] != world[k] { return no; } }
  at[0] += wn;
  pn := save_read_varint(b, n - 4, at[..]);
  if pn < 0 || at[0] + pn != n - 4 { return no; }
  return SaveOpened.{ok = true, schema = schema, at = at[0], count = pn};
}
```

### Worked by hand

The Garden keeps its vase under the world `garden`, schema 1. The payload
`vase 3` is six bytes:

| Bytes | Value | Hex |
| --- | --- | --- |
| 0–3 | `AESV` | `41 45 53 56` |
| 4–7 | container version 2, layer 2, two reserved | `02 02 00 00` |
| 8–11 | schema 1, little-endian | `01 00 00 00` |
| 12 | world length 6, one varint byte | `06` |
| 13–18 | `garden` | `67 61 72 64 65 6e` |
| 19 | payload length 6 | `06` |
| 20–25 | `vase 3` | `76 61 73 65 20 33` |
| 26–29 | CRC-32 of bytes 0–25 | `ab c9 74 36` |

That is 12 + 1 + 6 + 1 + 6 + 4 = 30 bytes. The CRC is 0x3674C9AB, written
low byte first. Python's `zlib.crc32` of bytes 0–25 gives the same
0x3674C9AB, and `save.coax`'s own check holds its table to the standard check
value, `crc32("123456789")` = 0xCBF43926.

## Two slots

The record the browser keeps is a line each: `verse-save 1`, the current
save's time in milliseconds and its container in base64, then the previous
save's time and container. `save_record` makes the old current save the
previous one only when it opens; otherwise the old previous one stays, so a
damaged save is never kept as the fallback. A load tries the current save,
then the last good one:

<!-- source: apps/openagents-web/save.coax pick -->
```jai
SavePick :: struct { which: int; at: int; count: int; schema: int; time: f64; }   // which: 0 none, 1 current, 2 previous
// The save to load, trying the current one first (from `from`: 1, or 2 when
// the current one opened but its house didn't), then the last good one.
save_pick :: (bytes: [] u8, s: SaveSlots, world: string, max_schema: int, from: int) -> SavePick #export_c {
  none: SavePick;
  if from <= 1 && s.cur_n > 0 {
    if s.legacy { return SavePick.{which = 1, at = s.cur, count = s.cur_n, schema = 0, time = 0}; }
    o := save_open(bytes[s.cur..<s.cur + s.cur_n], s.cur_n, world, max_schema);
    if o.ok { return SavePick.{which = 1, at = s.cur + o.at, count = o.count, schema = o.schema, time = s.cur_at}; }
  }
  if s.prev_n > 0 {
    o := save_open(bytes[s.prev..<s.prev + s.prev_n], s.prev_n, world, max_schema);
    if o.ok { return SavePick.{which = 2, at = s.prev + o.at, count = o.count, schema = o.schema, time = s.prev_at}; }
  }
  return none;
}
```

A record from before containers (the Haunted House's save as bare hex) reads
as a legacy current save and opens as schema 0; the realm migrates it and
keeps it again as a container of its own schema.

## Save the vase twice

The program is
[aethyr/examples/save-and-come-back.coax](../examples/save-and-come-back.coax):

<!-- source: aethyr/examples/save-and-come-back.coax example -->
```jai
#load "../../apps/openagents-web/save.coax";
#import "Basic";

hex :: (b: [] u8, n: int) {
  digits := "0123456789abcdef";
  for i: 0..<n {
    hi := cast(int) (b[i] >> 4); lo := cast(int) (b[i] & 15);
    print("%", digits[hi..<hi + 1]); print("%", digits[lo..<lo + 1]);
  }
  print("\n");
}

// Two saves of the Garden's vase kept as the browser keeps them, then loaded back: the current
// one, the last good one when the current one is damaged, and refusals of another world's save
// and of a schema newer than this build.
main :: () {
  first := "vase 3"; second := "vase 4";
  p: [16] u8;
  for k: 0..<first.count { p[k] = first[k]; }
  a: [64] u8; an := save_pack("garden", 1, p[..], first.count, a[..]);
  print("container: % bytes: ", an); hex(a[..], an);
  for k: 0..<second.count { p[k] = second[k]; }
  b: [64] u8; bn := save_pack("garden", 1, p[..], second.count, b[..]);
  none: SaveSlots; empty: [1] u8;
  r1: [512] u8; n1 := save_record(a[..], an, 1000, empty[..], none, "garden", 1, r1[..]);
  slots: [256] u8; s1 := save_read(cast(string) r1[0..<n1], slots[..]);
  r2: [512] u8; n2 := save_record(b[..], bn, 2000, slots[..], s1, "garden", 1, r2[..]);
  print("record:\n%", cast(string) r2[0..<n2]);
  back: [256] u8; s := save_read(cast(string) r2[0..<n2], back[..]);
  k := save_pick(back[..], s, "garden", 1, 1);
  print("load: slot %, schema %, saved at %, payload \"%\"\n", k.which, k.schema, cast(int) k.time, cast(string) back[k.at..<k.at + k.count]);
  back[s.cur + 20] ^= 1;   // the low bit of the current save's first payload byte: "vase 4" reads "wase 4"
  k = save_pick(back[..], s, "garden", 1, 1);
  print("current damaged -> load: slot %, saved at %, payload \"%\"\n", k.which, cast(int) k.time, cast(string) back[k.at..<k.at + k.count]);
  print("another world's save opens: %\n", save_open(a[..], an, "haunted-house", 1).ok);
  newer: [64] u8; nn := save_pack("garden", 2, p[..], second.count, newer[..]);
  print("a schema 2 save in a schema 1 build opens: %; in a schema 2 build: %\n", save_open(newer[..], nn, "garden", 1).ok, save_open(newer[..], nn, "garden", 2).ok);
  old: [16] u8; o := save_read("0a0b0cff", old[..]);
  k = save_pick(old[..], o, "garden", 1, 1);
  print("a bare-hex save from before containers: slot %, schema %, % bytes\n", k.which, k.schema, k.count);
}
```

```shell
coax/bin/coax run aethyr/examples/save-and-come-back.coax
```

```text
container: 30 bytes: 4145535602020000010000000667617264656e06766173652033abc97436
record:
verse-save 1
2000
QUVTVgICAAABAAAABmdhcmRlbgZ2YXNlIDQIXBCo
1000
QUVTVgICAAABAAAABmdhcmRlbgZ2YXNlIDOryXQ2
load: slot 1, schema 1, saved at 2000, payload "vase 4"
current damaged -> load: slot 2, saved at 1000, payload "vase 3"
another world's save opens: false
a schema 2 save in a schema 1 build opens: false; in a schema 2 build: true
a bare-hex save from before containers: slot 1, schema 0, 4 bytes
```

The first line is the hand-worked container. One flipped bit in the current
save's payload breaks its CRC, so the load falls back to the previous save,
`vase 3` from time 1000. The refusals are the broken cases: another world's
save, and a schema newer than the build, which a newer build opens.

## Where it is used

- [apps/openagents-web/garden.coax](../../apps/openagents-web/garden.coax):
  the vase's save (at most every `KEEP_EVERY`, 1.0 s, while you arrange), its
  restore, and a test that refuses another world's save.
- [apps/openagents-web/world.coax](../../apps/openagents-web/world.coax): the
  Haunted House, whose old bare-hex save opens as schema 0. Its slot is
  `haunted-house`, and `realms.json`'s `slot_was` (`realm`) names the slot it
  read before, read once when the new one is empty.
- [apps/openagents-web/place.coax](../../apps/openagents-web/place.coax):
  where you are in every walkable realm, so a reload puts you back
  (`coax/web/place-reload.ts`).
- [apps/openagents-web/everglade-arrange.coax](../../apps/openagents-web/everglade-arrange.coax):
  what you arranged in Everglade's close-up view.
- [coax/web/api/player.c](../../coax/web/api/player.c): the progress routes
  and `container_ok`, the same container checked on the server (at most
  16 MiB).
- [verse/fairy-forest/life.coax](../../verse/fairy-forest/life.coax): Fairy
  Forest packs its own payload (`ffl_save`, read by `ffl_open`).

## Checks

- `coax/bin/coax test apps/openagents-web/save.coax --library`: a save packs
  into a container that opens again; another world's save and a newer schema
  don't open; the CRC table's check value; two saves in the record, the
  current one corrupted, falling back to the last good one, which a new save
  keeps as the previous; both broken, nothing to load; the bare-hex save as
  schema 0; the server's answer and body both ways in base64.
- The independent checks: `zlib.crc32` computes the example's CRC, and the
  server's `container_ok` in C checks every container the page sends.

## Conflicts are found by revision

A made realm's document works this way: a save names the revision it was read
at, and a stale one gets `409` with the revision there now
([chapter 52](ch052-edit-a-realm-in-place.md)). A player's progress works the
same way. The rule is
[coax/web/api/progress-policy.coax](../../coax/web/api/progress-policy.coax),
an `#authority` unit that `coax mutate` leaves with no surviving mutant. The
API's `player.c` reads the slot under a lock, asks the policy, and SQL
([0119_progress_revisions.sql](../../coax/web/api/migrations/0119_progress_revisions.sql))
does what it decides.

No decision orders saves by clock time. The server keeps a save's time from
its own clock, to show, and reads the device's time for nothing.

Each slot has a revision, starting at 0. `GET` answers `{rev, current,
previous}`. A save sends `{bytes, rev}`, where `rev` is the revision it was
made from. The server keeps it only when `rev` is the revision there now, as
revision `rev + 1`. Otherwise it answers 409 with the revision and the saves
there now, and changes nothing. A `rev` that isn't a whole number is a 400.
A change made without naming a revision, such as a sign-in that moves a
guest's saves into an account or an erase, gets a revision no device has
read, so a device still on the old one gets a 409.

A page built before revisions names none. The transition rule keeps such a
save only while no page with revisions has saved the slot, and then the save
that arrives last is kept. Once a page with revisions saves a slot, a save
without a revision gets a 409 there. Existing saves become revision 1, and a
save time that a fast clock put in the future is brought back to now.

On the page, [save.coax](../../apps/openagents-web/save.coax)'s `SaveSync`
keeps the revision the server last answered and sends one save at a time; a
save made while one is in flight waits for its answer. What a 409 does
depends on what the slot holds:

- **Progress** (the Haunted House, Fairy Forest) takes the server's save and
  its revision: it is newer by revision. The change made on this device since
  is dropped, and the page says so.
- **Where you are and the spell bar** ([place.coax](../../apps/openagents-web/place.coax),
  Everglade's arranging view) send this device's copy again from the server's
  revision, because what you just did on the device you play is what you
  meant last.

At load, a server with revisions has the account's copy: the page opens it
unless it is the save this browser already has. A server from before
revisions answers none, and the page falls back to the newer copy by time.

The three cases this handles, each checked in `progress-policy.coax` and
against the running API in `coax/web/api/test_player.py`:

- **Skewed clocks.** Device A's clock is a day ahead of B's. B saves
  revision 7 → 8. A, which loaded 7, saves from 7 and gets 409, whatever its
  clock says. A device a day behind that saves from the revision there now is
  kept.
- **Two sessions at once.** Two tabs load 3. The first is kept as 4; the
  second, from 3, gets 409 with the first's save.
- **Old pages.** A page without revisions keeps working on a slot that only
  such pages saved, and can't overwrite one that a page with revisions saved.

Room-server state is not a progress save. Everglade's XP (#12739) and HP
live in `verse.room_vitals`. Only `oa-room` writes them, signed with the
seat-ticket key (`POST /api/playtest/room-vitals`), and XP only rises
(`greatest`), so no stale page's save can lower it. When a guest joins with an
email or signs in, that row moves to the account with them, and the larger
XP stays.

## Still to do

- **`aethyr/save.coax`.** Move the container, the slots, the migrations and
  the refusal of newer schemas from `apps/openagents-web/save.coax` into
  Aethyr, and have every realm (Fairy Forest's payload included) load its
  saves through it.
- **Declared save fields.** A realm marks the fields it saves and Aethyr
  derives the payload from them in a fixed order, with a layout fingerprint
  per schema; entities in a save by chapter 6's durable IDs. Today each realm
  writes its payload by hand.
- **Retries applied once.** A save carries a `session` ID and a `sequence`
  number, and the server answers a repeat with the first attempt's answer.
  Today a lost reply leaves the page on the old revision; its next save gets
  a 409 and takes the server's save, which is its own.
- **Merge or conflict copy.** On a 409 a realm that declares a merge for its
  saved fields (a set of found things merges by union) merges them and sends
  the result from the server's revision. A realm without one keeps the local
  save as a conflict copy in a separate slot, and the player chooses. Today
  progress takes the server's save and drops the local change, with a
  message.
- **The browser's revision.** The record this browser keeps doesn't carry
  the revision it was based on. A save that never reached the server
  (offline) gives way to the server's on the next load.

### Acceptance checks still open

- Every realm that saves today loads its existing saves through
  `aethyr/save.coax`, and no realm keeps its own container code.
- A save written by schema N opens in a build with schema N + 1 through that
  realm's migration (today the only migration is the bare-hex save to
  schema 0).
- `place-reload.ts` passes for every walkable realm through the new layer.
- A save and its restore give the same digest of the saved fields, natively
  and in the browser.
- An offline device's queued saves produce exactly one conflict on reconnect,
  and the other device's progress is still there. The refused changes end up
  merged or in a conflict copy. A resent save increments the revision once.

## Dependencies

Chapters [3](ch003-state-starts-from-its-defaults.md) (defaults),
[5](ch005-replay-a-session-exactly.md) (digests),
[6](ch006-entities-are-born-and-die.md) (durable IDs) and
[51](ch051-a-realm-is-a-document.md) (the realm document, which a save never
replaces). Coax: R5 derivation and R16 snapshots in the
[roadmap](../../coax/ROADMAP.md), and
[chapter 18](../../coax/book/ch18-protocol.md) for versioned records.
