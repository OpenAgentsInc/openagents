# 30. Subscriptions and EOSE

A subscription returns stored events, then `EOSE`, then live events. This
chapter builds subscription bookkeeping and the rules for what `EOSE` does and
does not mean.

Part VIII, A client. Result: Subscription IDs that name a slot and its serial,
EOSE as the end of stored events and never a completeness claim, NIP-67
completeness hints, and paginated catch-up.

## What this chapter teaches

- IDs like `s3.17`: frames for a closed or old subscription are unknown.
- `EOSE` ends the stored phase; short pages and tied timestamps mean more may
  exist.
- NIP-67: the relay may say whether its stored results were complete; the
  client treats a missing hint as unknown.
- Paginating backward with `until` and handling `created_at` ties.

Chapter 29 built the connection: states, generations, backoff, and one
subscription and one publish carried across a reconnect. This chapter stays
on one connection most of the time and looks at the subscription side: how
ids are made, what each subscription state accepts, what `CLOSE` and
`CLOSED` do, how many subscriptions can be open, and how a client gets a
relay's whole stored set when the relay sends it in pages. Publishing and
`OK` are chapter 31's.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [relay.coax](../../coax/lib/nostr/relay.coax) | `rl_subscribe`, `rl_unsubscribe` | open a subscription in a free slot; end one (`#authority`) |
| | `rl_sub_text(slot, serial, out)`, `rl_sub_find(r, text)` | a slot's wire id, and the slot a wire id names |
| | `rl_next`, `rl_input` | the `REQ` and `CLOSE` frames to send; what each `EVENT`, `EOSE` and `CLOSED` does |
| [relay_read.coax](../../coax/lib/nostr/relay_read.coax) | `rl_read` | one relay text frame into `RlIn`, with the event's id and signature checked (`#authority`) |
| [message.coax](../../coax/lib/nostr/message.coax) | `nostr_message_read` | reads `EOSE` with up to 4 trailing values (`NM_MAX_EOSE_EXTRA`), and refuses a fifth |
| [filter.coax](../../coax/lib/nostr/filter.coax) | `nf_write` | chapter 23's filter writer, for the `since`, `until` and `limit` fields |
| [subscriptions.coax](../examples/subscriptions.coax) | `sx_hint`, `sx_complete` | the NIP-67 hint on an `EOSE` frame, and whether it says the stored set is complete |
| | `sx_catch_up` | paging backward with `until` through a relay that caps its pages |

The library has no NIP-67 reader and no pager. `message.coax` accepts the
hint array and counts it in `extra`, but nothing reads it. This chapter's
example writes both, `sx_hint` and `sx_catch_up`, on top of the library's
JSON reader and the machine. They are example code, not library code.

## Subscription ids

A `Relay` has 8 subscription slots (`RL_SUBS`). `rl_subscribe` takes the
first free slot, adds 1 to that slot's serial and returns the slot. When
every slot is taken it returns -1. The relay never sees a slot number; it
sees the wire id that `rl_sub_text` writes: `s`, the slot digit, a dot and
the serial. Slot 3's seventeenth subscription is `s3.17`.

NIP-01 lets a client choose any id up to 64 characters, and a `REQ` with an
id that is already open replaces that subscription. So a client must never
send the same id for two different requests, or events for the old one are
taken as answers to the new one. The serial makes that impossible here:
each `rl_subscribe` on a slot gives a new id, so the machine has no
"duplicate id" case to refuse.

`rl_sub_find(r, text)` returns the slot whose current id equals `text`, or
-1. It compares bytes. A frame for `s0.1` after slot 0 moved on to `s0.2`
names no slot, and neither does `s9.1`, `s0.10` or an id the relay escaped
differently. `rl_input` returns `RL_UNKNOWN` for an `EVENT`, `EOSE` or
`CLOSED` whose id names no slot.

## Subscription states

A slot is in one of seven states:

| State | Meaning | `EVENT`, `EOSE`, `CLOSED` for its id |
| --- | --- | --- |
| `RS_FREE` (0) | unused | unknown (no id matches a free slot) |
| `RS_WANT` (1) | its `REQ` waits for `rl_next` | `RL_UNKNOWN` |
| `RS_LIVE` (2) | `REQ` sent; stored events arriving | accepted |
| `RS_CAUGHT` (3) | `EOSE` seen; live events arriving | accepted |
| `RS_CLOSING` (4) | the caller unsubscribed; its `CLOSE` waits for `rl_next` | `RL_UNKNOWN` |
| `RS_REFUSED` (5) | the relay sent `CLOSED` | `RL_UNKNOWN` |
| `RS_AUTH` (6) | the relay sent `CLOSED` with `auth-required:` | `RL_UNKNOWN` |

Only `RS_LIVE` and `RS_CAUGHT` accept frames. For those two, `rl_input`
decides:

- `EOSE` sets the state to `RS_CAUGHT` and returns `RL_EOSE`. A second
  `EOSE` does the same again.
- `CLOSED` sets `RS_AUTH` when the message starts with `auth-required:` and
  the subscription has not been retried after an `AUTH` on this connection;
  otherwise `RS_REFUSED`. It returns `RL_CLOSED`.
- `EVENT` returns `RL_FORGED` when the signature didn't check,
  `RL_DUPLICATE` when the id is in the window, and otherwise `RL_DELIVER`.

## Stored and live events

`rl_input` returns `RL_DELIVER` for both stored and live events. The caller
tells them apart by the subscription's state before the call: a delivery
while the slot is `RS_LIVE` is stored, and one while it is `RS_CAUGHT` is
live. The example's `sx_recv` does this.

"Live" means the relay sent it after `EOSE`, not that it is newer. A relay
sends a new event when it accepts it, and the event's `created_at` is
whatever its author wrote. The worked example below has a live event one
second older than the stored one.

Each subscription keeps `newest`, the largest `created_at` it delivered.
`rl_input` changes it only when an event's `created_at` is at least
`newest + 1`, so an older live event leaves it alone. Chapter 29 showed what
`newest` is for: after a reconnect, the `REQ` carries `since` = `newest`.

## What EOSE means

`EOSE` means the relay finished sending what it holds for that `REQ`. It
doesn't mean the client has every matching event:

- Relays cap results. A relay that holds 100 matching events may send 37,
  then `EOSE`, whatever `limit` the client asked for.
- The client's own `limit` cuts the result too: `"limit":10` gives 10 events
  and `EOSE` even when the relay holds 1000.
- Another relay may hold events this one doesn't.

So a client that wants the whole stored set must page, and can call the set
complete only when the relay says so.

## NIP-67 completeness hints

NIP-67 adds an optional third element to `EOSE`: an array of hint strings.

| Hint | Meaning |
| --- | --- |
| `"finish"` | the relay sent every stored event that matches |
| `"more"` | the relay holds more matching stored events than it sent |
| `"auth"` | the relay may hold more if the client authenticates (NIP-42) |

Unknown hints are ignored. The NIP says a hint's presence is definitive and
its absence is not: with no hint, the client pages as usual. The deployed
relay at relay.openagents.com sends `["EOSE","repo",["finish"]]`; the
fixture `coax/lib/nostr/fixtures/coax_relay.coax`, captured on 2026-10-28,
holds such frames.

`sx_hint` reads the hint from a frame that `nostr_message_read` accepted. It
walks the frame with chapter 17's JSON reader and looks only at a third
element that is an array; inside it, it marks the three known strings and
skips everything else. `sx_complete` says the stored set is complete only
when `"finish"` is there and neither `"more"` nor `"auth"` is. A missing
hint, an empty array, a string where the array should be, `"FINISH"` in
capitals, contradictory hints and a frame `message.coax` refuses are all
"unknown".

<!-- source: nostr/examples/subscriptions.coax hint -->
```jai
// NIP-67's hints on an EOSE frame: the strings in a third element that is an
// array. `read` is false when message.coax refuses the frame. Unknown
// strings, other values and a missing array are no hint.
SxHint :: struct { read: bool; finish: bool; more: bool; auth: bool; }
sx_hint :: (text: string) -> SxHint {
  h: SxHint;
  m := nostr_message_read(text);
  if m.error != NM_OK || m.type != NM_EOSE { return h; }
  h.read = true;
  if m.extra == 0 { return h; }
  j: [1] Json; json_start(j[..], text);
  t := json_next(j[..]); t = json_next(j[..]); t = json_next(j[..]);   // [ "EOSE" <sub>
  if json_next(j[..]) != JSON_ARRAY { return h; }
  t = json_next(j[..]);
  while t != JSON_ARRAY_END {
    if t == JSON_STRING {
      raw := json_text(j[..]);
      if json_is(raw, "finish") { h.finish = true; }
      if json_is(raw, "more") { h.more = true; }
      if json_is(raw, "auth") { h.auth = true; }
    } else if t == JSON_ARRAY || t == JSON_OBJECT {
      skipped := json_skip(j[..]);
    }
    t = json_next(j[..]);
  }
  return h;
}
// The stored set is known to be complete only when the relay said "finish"
// and neither "more" nor "auth" (more may come after AUTH). Anything else,
// a missing hint included, is unknown.
sx_complete :: (h: SxHint) -> bool { return h.read && h.finish && !h.more && !h.auth; }
sx_hint_text :: (h: SxHint) -> string {
  if !h.read { return "frame refused"; }
  names := [8] string.["none", "finish", "more", "finish+more", "auth", "finish+auth", "more+auth", "finish+more+auth"];
  return names[(ifx h.finish then 1 else 0) + (ifx h.more then 2 else 0) + (ifx h.auth then 4 else 0)];
}
```

`"auth"` with `"finish"` is not complete: NIP-67's own example sends that
pair when the relay has sent everything this client may read without
authenticating.

## CLOSE and CLOSED

`rl_unsubscribe(r, slot)` ends a subscription. What it does depends on
whether the relay has the `REQ`:

- A `RS_LIVE` or `RS_CAUGHT` slot on an open connection becomes
  `RS_CLOSING`. `rl_next` sends its `CLOSE` before any `REQ` and then frees
  the slot.
- Any other slot is freed at once with no `CLOSE`: the relay never had the
  `REQ` (`RS_WANT`, or the connection is down), or it already ended the
  subscription itself (`RS_REFUSED`, `RS_AUTH`).

The id stops matching at once. A frame the relay sent before it read the
`CLOSE` arrives as `RL_UNKNOWN`, and so does any frame after the slot's
next subscription took a new serial. Because a `RS_CLOSING` slot is not
free, `rl_subscribe` called before the `CLOSE` goes out takes another slot.

A relay ends a subscription itself with `["CLOSED", <id>, <message>]`. The
message starts with a NIP-01 prefix such as `error:`, `restricted:`,
`rate-limited:` or `auth-required:`. The machine treats `auth-required:` as
"retry after `AUTH`" (chapter 38) and every other `CLOSED` as final:

- a `RS_REFUSED` slot stays refused across reconnects, and its `REQ` is not
  sent again;
- a `RS_AUTH` slot goes back to `RS_WANT` when an `AUTH` is accepted on the
  same connection (once), or when a new connection opens.

A second `CLOSED` for the same id is `RL_UNKNOWN`: the subscription is no
longer live. A caller that wants to try a refused filter again, for example
after `rate-limited:`, calls `rl_unsubscribe` and `rl_subscribe`, which
gives a new id.

## Limits on open subscriptions

There are two limits. The client's is 8 slots: the ninth `rl_subscribe`
returns -1, and the caller must end one first. The relay's is its own and
unknown in advance. A relay over its limit answers a `REQ` with `CLOSED`
and a reason, such as `restricted: subscription limit reached`, and that
subscription becomes `RS_REFUSED`.

## Deduplication

The machine keeps the ids of the last 64 delivered events (`RL_SEEN`), one
window for the whole `Relay`. Three things follow:

- An event already delivered is `RL_DUPLICATE`, whichever subscription it
  comes on. Two subscriptions whose filters both match an event deliver it
  once, to whichever saw it first; a caller that needs it under both must
  route it itself.
- The window survives reconnects. The boundary event that `since` asks for
  again (chapter 29) is a duplicate.
- The window is a bound. An id delivered more than 64 deliveries ago is
  forgotten and delivered again. A pager must keep a page and its boundary
  events inside 64 ids; the checks use pages of up to 60.

`rl_input` doesn't check that an event matches the subscription's filter. A
relay can send any signed event under any live id. A caller that relies on
the filter checks the event with chapter 23's `nf_match`.

## Paginating backward

A relay that caps its pages gives the newest events that match, up to the
cap, then `EOSE`. To get older ones, the client sends another `REQ` with
`until` set to the oldest `created_at` it got. `until` is inclusive, so the
events at that second come again; the window drops them as duplicates.

When to stop is the hard part. Comparing the page size with the requested
`limit` fails when the relay's cap is lower than the limit: a client that
asks for 500 and gets 37 would stop after one page. `sx_catch_up` uses
these rules instead, after each page:

1. If the `EOSE` hint says complete, stop: the set is complete.
2. If the page brought new events, page again with `until` = the oldest
   `created_at` on the page.
3. If the page brought nothing new and was smaller than the largest page so
   far, stop. Every event at or before `until` was already delivered. The
   set is not known to be complete.
4. If the page brought nothing new and was as large as the largest page so
   far, the relay holds at least a full page of events at the `until`
   second, all already delivered. `until` can't get past them. Record a gap
   at that second, set `until` to one second earlier and page again.

A gap means "events with this `created_at` may be missing", not that some
are. NIP-67 asks relays to include every event at the boundary second in
one response for this reason.

The pager opens one subscription per page and unsubscribes it after its
`EOSE`. A page of old events has nothing live to wait for. Because the next
`rl_subscribe` comes before the `CLOSE` is sent, pages alternate between
slots 0 and 1: `s0.1`, `s1.1`, `s0.2`, `s1.2`.

<!-- source: nostr/examples/subscriptions.coax pager -->
```jai
// A model relay's stored events for one filter: created_at per event, in
// nondecreasing order, so event i's id is rl_id_of(i). The relay sends at
// most `cap` events per REQ, whatever limit the client asks for, and sends
// NIP-67 hints only when `hints` is set.
SX_STORE :: 128;
SxStore :: struct { at: [SX_STORE] int; n: int; cap: int; hints: bool; }
// What one catch-up got: the REQs, events sent and new per page, which
// events arrived, and what is known about completeness.
// `stopped` is set when the pager gave up after SX_PAGES REQs.
SX_PAGES :: 128;
SxCatch :: struct {
  reqs: int; sent: [SX_PAGES] int; fresh: [SX_PAGES] int; delivered: int; got: [SX_STORE] bool;
  complete: bool; gap: bool; gap_at: int; stopped: bool;
}
SX_ASK_LIMIT :: 500;

// The model relay's answer to one REQ: indices newest first (ties by index,
// highest first), at most cap of them. Returns how many it sent and, in
// eligible[0], how many stored events match.
sx_serve :: (s: SxStore, has_until: bool, until: int, picks: [] int, eligible: [] int) -> int {
  n := 0; eligible[0] = 0;
  i := s.n - 1;
  while i >= 0 {
    if !has_until || s.at[i] <= until {
      eligible[0] += 1;
      if n < s.cap { picks[n] = i; n += 1; }
    }
    i -= 1;
  }
  return n;
}

// Writes the pager's frames: CLOSEs, and each REQ with `until` when set and
// the limit the client asks for.
sx_page_send :: (r: [] Relay, has_until: bool, until: int, loud: bool) {
  while true {
    o := rl_next(r, 0);
    if o.kind == RL_OUT_NONE { return; }
    out: [256] u8; n := 0; sb: [24] u8; id := rl_sub_text(o.slot, r[0].subs[o.slot].serial, sb[..]);
    if o.kind == RL_OUT_CLOSE { n = nostr_message_write_close(id, out[..], false); }
    if o.kind == RL_OUT_REQ {
      kinds := [1] int.[1];
      f: NostrFilter; f.has_kinds = true; f.kinds = kinds[..];
      f.has_until = has_until; f.until = until; f.has_limit = true; f.limit = SX_ASK_LIMIT;
      filters := [1] NostrFilter.[f];
      n = nostr_message_write_req(id, filters[..], out[..], false);
    }
    if loud { print("  send %\n", cast(string) out[0..<n]); }
  }
}

// Pages backward through a relay's stored events with `until`, one
// subscription per page, until a page brings nothing new or the relay says
// "finish". `until` is inclusive, so the boundary events come again and the
// id window drops them. A page that is full and holds nothing new is stuck
// on one created_at: the pager records the gap and steps past it.
sx_catch_up :: (s: SxStore, loud: bool) -> SxCatch {
  c: SxCatch; r: [1] Relay; g := rl_open_at(r[..], 0);
  has_until := false; until := 0; widest := 0;
  while true {
    if c.reqs == SX_PAGES { c.stopped = true; break; }
    slot := rl_subscribe(r[..]);
    sb: [24] u8; sub := rl_sub_text(slot, r[0].subs[slot].serial, sb[..]);
    sx_page_send(r[..], has_until, until, loud);   // the previous page's CLOSE, then this REQ
    picks: [SX_STORE] int; eligible: [1] int;
    sent := sx_serve(s, has_until, until, picks[..], eligible[..]);
    fresh := 0; oldest := until;
    for k: 0..<sent {
      i := picks[k];
      in := RlIn.{kind = RL_IN_EVENT, sub = sub, id = rl_id_of(i), created_at = s.at[i], verified = true};
      if rl_input(r[..], g, in) == RL_DELIVER { fresh += 1; c.got[i] = true; }
      oldest = s.at[i];
    }
    // The EOSE, with a hint when the relay gives them.
    eb: [64] u8;
    e := json_raw(eb[..], 0, "[\"EOSE\",");
    e = json_write_string(eb[..], e, sub);
    if s.hints { e = json_raw(eb[..], e, ifx eligible[0] > sent then ",[\"more\"]" else ",[\"finish\"]"); }
    e = json_raw(eb[..], e, "]");
    eose := cast(string) eb[0..<e];
    scratch: [64] u8;
    d := rl_input(r[..], g, rl_read(eose, scratch[..], false));
    h := sx_hint(eose);
    ok := rl_unsubscribe(r[..], slot);
    c.sent[c.reqs] = sent; c.fresh[c.reqs] = fresh; c.reqs += 1; c.delivered += fresh;
    if loud { print("  recv % events, % new, oldest created_at %; % -> % (hint %)\n", sent, fresh, oldest, eose, sx_input_name(d), sx_hint_text(h)); }
    if sx_complete(h) { c.complete = true; break; }
    if fresh == 0 {
      if sent > 0 && sent == widest {
        c.gap = true; c.gap_at = until; until -= 1;
        if loud { print("  a full page with nothing new: stuck on created_at %; events there may be missing\n", c.gap_at); }
        continue;
      }
      break;
    }
    if sent > widest { widest = sent; }
    has_until = true; until = oldest;
  }
  sx_page_send(r[..], has_until, until, loud);   // the last page's CLOSE
  return c;
}

sx_report :: (s: SxStore, c: SxCatch) {
  print("  % REQs, % of % events", c.reqs, c.delivered, s.n);
  print(", stored set %", ifx c.complete then "complete (the relay said finish)" else "not known to be complete");
  if c.gap { print(", gap at created_at %", c.gap_at); }
  print("\n");
}
// 100 events, two per second from 1000 to 1049.
sx_store_pairs :: (cap: int, hints: bool) -> SxStore {
  s: SxStore; s.n = 100; s.cap = cap; s.hints = hints;
  for i: 0..<100 { s.at[i] = 1000 + i / 2; }
  return s;
}
// 45 events: 5 at 1999, then 40 at 2000.
sx_store_tie :: (cap: int) -> SxStore {
  s: SxStore; s.n = 45; s.cap = cap;
  for i: 0..<45 { s.at[i] = ifx i < 5 then 1999 else 2000; }
  return s;
}
```

The model relay holds events as a list of `created_at` values. Event `i`
has the id `rl_id_of(i)`, a 32-byte id with `i` in its first byte, and is
passed to `rl_input` as already verified. Real events would cost a Schnorr
verification each and add nothing to the page arithmetic; the session below
uses real signed events. Each page's `EOSE` is a real frame: it goes
through `rl_read` and `sx_hint`.

## Worked example: a session by hand

The session opens three subscriptions on one relay at t=0: slot 0 for
repository announcements (kind 30617), slot 1 for repository state (kind
30621) and slot 2 for comments (kind 1111). Their ids are `s0.1`, `s1.1`
and `s2.1`. The events are four captured relay.openagents.com events from
`COAX_RELAY`: `813d9fb0...` (kind 30617, `created_at` 1793214821),
`375b0d29...` (kind 30621), and comments `5f377761...` (1793219505) and
`fd6757b7...` (1793219504). Each is rewrapped with the session's id by
`sx_event`.

**t=0.** The connection opens as generation 1. `rl_next` gives three `REQ`
frames, one per slot, none with `since`. All three slots are `RS_LIVE`.

**t=100.** The relay sends nine frames:

| Frame | Answer | Why |
| --- | --- | --- |
| `EVENT s0.1 813d...` | `RL_DELIVER`, stored | `s0.1` is `RS_LIVE` |
| `["EOSE","s0.1",["finish"]]` | `RL_EOSE` | `s0.1` is `RS_CAUGHT`; the hint is `finish`, so the stored set is complete |
| `EVENT s2.1 5f37...` | `RL_DELIVER`, stored | `newest` for `s2.1` becomes 1793219505 |
| `["EOSE","s2.1"]` | `RL_EOSE` | no hint: the stored set is unknown |
| `EVENT s2.1 fd67...` | `RL_DELIVER`, live | after `EOSE`; 1793219504 is not at least 1793219505 + 1, so `newest` stays |
| `EVENT s2.1 5f37...` | `RL_DUPLICATE` | its id is in the window |
| `CLOSED s1.1 "auth-required: ..."` | `RL_CLOSED` | not retried yet, so `s1.1` is `RS_AUTH` |
| `EVENT s1.1 375b...` | `RL_UNKNOWN` | `s1.1` is not live |
| `CLOSED s1.1 "error: closed again"` | `RL_UNKNOWN` | the same |

**t=200.** The caller calls `rl_unsubscribe(0)`. `s0.1` is caught up and the
connection is open, so the slot becomes `RS_CLOSING`. An `["EOSE","s0.1"]`
that arrives now is `RL_UNKNOWN`. `rl_next` sends `["CLOSE","s0.1"]` and
frees the slot. `rl_subscribe` takes slot 0 again; its serial becomes 2, so
the id is `s0.2`, and `rl_next` sends its `REQ`.

**t=250.** The relay sends two frames:

| Frame | Answer | Why |
| --- | --- | --- |
| `EVENT s0.1 813d...` | `RL_UNKNOWN` | slot 0's id is now `s0.2`; no slot is named `s0.1` |
| `CLOSED s0.2 "restricted: subscription limit reached"` | `RL_CLOSED` | not `auth-required:`, so `s0.2` is `RS_REFUSED` |

The slots are now `s0.2` refused, `s1.1` waiting for `AUTH`, and `s2.1`
caught up with 2 deliveries and `newest` 1793219505.

**t=300 to t=900: a reconnect.** The connection drops at t=300. With
attempt 0 and jitter 0 the backoff is 500 ms (chapter 29), so generation 2
starts at t=800 and opens at t=850. `rl_opened` moves `s1.1` (`RS_AUTH`)
and `s2.1` (`RS_CAUGHT`) back to `RS_WANT` and leaves `s0.2` refused.
`rl_next` sends `REQ s1.1` with no `since` (it delivered nothing), then
`REQ s2.1` with `"since":1793219505`. Nothing goes for `s0.2`. At t=900:

| Frame | Answer | Why |
| --- | --- | --- |
| `EVENT s2.1 5f37...` | `RL_DUPLICATE` | the boundary event that `since` asked for again; the window survived the reconnect |
| `["EOSE","s2.1",["finish"]]` | `RL_EOSE` | complete |
| `CLOSED s1.1 "auth-required: ..."` | `RL_CLOSED` | a new connection reset the retry, so `s1.1` is `RS_AUTH` again |

As letters, `'a'` plus each answer's code (a `RL_DELIVER`, b
`RL_DUPLICATE`, d `RL_UNKNOWN`, f `RL_EOSE`, h `RL_CLOSED`), the fifteen
answers are `afafabhddddhbfh`. The three deliveries are stored, stored and
live: `ssl`.

## Worked example: pages by hand

**A: 100 events, cap 37, no hints.** Event `i` (0 to 99) has `created_at`
1000 + `i` / 2, so each second from 1000 to 1049 has two events. The relay
sends newest first: event 99, 98, 97 and so on.

| Page | `until` | Events the relay sends | Sent | New | Oldest |
| --- | --- | --- | --- | --- | --- |
| 1 | none | 99 down to 63 | 37 | 37 | 1000 + 63 / 2 = 1031 |
| 2 | 1031 | 63 down to 27 (event 63 again) | 37 | 36 | 1000 + 27 / 2 = 1013 |
| 3 | 1013 | 27 down to 0 (event 27 again) | 28 | 27 | 1000 |
| 4 | 1000 | 1 and 0, both again | 2 | 0 | 1000 |

Page 4 brought nothing new and is smaller than 37, so the pager stops:
37 + 36 + 27 = 100 events in 4 `REQ`s. No hint came, so the set is not known
to be complete, though every event arrived.

**B: the same relay with NIP-67 hints.** The relay says `"more"` when more
events match than it sent and `"finish"` otherwise. Page 1 has 100
matching, page 2 has 64 (events 63 to 0) and page 3 has 28. So pages 1 and
2 say `"more"`, and page 3 sends all 28 and says `"finish"`. The pager
stops after 3 `REQ`s with 100 events, complete. The hint saved page 4.

**C: 45 events, 40 of them at one second.** Events 0 to 4 have `created_at`
1999 and events 5 to 44 have 2000. The cap is 37.

| Page | `until` | Events the relay sends | Sent | New | Rule |
| --- | --- | --- | --- | --- | --- |
| 1 | none | 44 down to 8 | 37 | 37 | 2: page again with `until` 2000 |
| 2 | 2000 | 44 down to 8 again | 37 | 0 | 4: full and nothing new; gap at 2000, `until` 1999 |
| 3 | 1999 | 4 down to 0 | 5 | 5 | 2: page again with `until` 1999 |
| 4 | 1999 | 4 down to 0 again | 5 | 0 | 3: smaller than 37; stop |

The pager gets 42 of 45 events. Events 5, 6 and 7 were never sent, and the
result says so: a gap at `created_at` 2000.

## The example

The example is
[nostr/examples/subscriptions.coax](../examples/subscriptions.coax). It
loads `relay_read.coax`, which loads `relay.coax` and `message.coax`, and
the captured relay transcript. It feeds relay frames as text straight to
`rl_read`; chapter 29 already showed the WebSocket framing under them.

<!-- source: nostr/examples/subscriptions.coax setup -->
```jai
#load "../../coax/lib/nostr/relay_read.coax";
#load "../../coax/lib/nostr/fixtures/coax_relay.coax";
#import "Basic";

sx_input_name :: (d: int) -> string {
  names := [12] string.["RL_DELIVER", "RL_DUPLICATE", "RL_STALE", "RL_UNKNOWN", "RL_FORGED", "RL_EOSE",
                        "RL_ACK", "RL_CLOSED", "RL_NOTICE", "RL_AUTH", "RL_NOT_OPEN", "RL_BAD"];
  return names[d];
}
sx_sub_state_name :: (s: int) -> string {
  names := [7] string.["FREE", "WANT", "LIVE", "CAUGHT", "CLOSING", "REFUSED", "AUTH"];
  return names[s];
}
// The kinds each subscription slot asks for in the session: repository
// announcements, repository state, and comments (NIP-34, NIP-22).
SX_KINDS :: [3] int.[30617, 30621, 1111];
```

`sx_session` is the session worked out above. `sx_recv` reads one frame,
notes the slot's state before `rl_input`, and prints the answer with what
it means: stored or live for a delivery, the hint for an `EOSE`, the new
state for a `CLOSED`.

<!-- source: nostr/examples/subscriptions.coax session -->
```jai
// What the session decided, for the checks: 'a' + the rl_input answer for
// each relay frame, and 's' (stored) or 'l' (live) for each delivery.
SxTrace :: struct { decisions: [32] u8; nds: int; phases: [8] u8; np: int; }

// One captured relay.openagents.com EVENT (line `line` of COAX_RELAY),
// rewrapped for the subscription id `sub`.
sx_event :: (line: int, sub: string, out: [] u8) -> string {
  span := nm_line(COAX_RELAY, line);
  text := COAX_RELAY[span.from..<span.to];
  m := nostr_message_read(text);
  return rlr_event_frame(sub, text[m.event.from..<m.event.to], out);
}

// Writes what the open connection asks for, until rl_next has nothing.
sx_flush :: (r: [] Relay, now: int, loud: bool) {
  while true {
    o := rl_next(r, now);
    if o.kind == RL_OUT_NONE { return; }
    sb: [24] u8; out: [256] u8; n := 0;
    id := rl_sub_text(o.slot, r[0].subs[o.slot].serial, sb[..]);
    if o.kind == RL_OUT_REQ {
      kinds := [1] int.[SX_KINDS[o.slot]];
      f: NostrFilter; f.has_kinds = true; f.kinds = kinds[..]; f.has_since = o.has_since; f.since = o.since;
      filters := [1] NostrFilter.[f];
      n = nostr_message_write_req(id, filters[..], out[..], false);
    } else if o.kind == RL_OUT_CLOSE {
      n = nostr_message_write_close(id, out[..], false);
    }
    if loud { print("  send %\n", cast(string) out[0..<n]); }
  }
}

// One relay text frame: read, checked and admitted. A delivery is stored
// when its subscription had no EOSE yet, and live after.
sx_recv :: (t: [] SxTrace, r: [] Relay, gen: int, text: string, loud: bool) -> int {
  scratch: [NE_MAX_OUTPUT] u8;
  in := rl_read(text, scratch[..], false);
  slot := rl_sub_find(r, in.sub);
  before := ifx slot >= 0 then r[0].subs[slot].state else -1;
  d := rl_input(r, gen, in);
  t[0].decisions[t[0].nds] = cast(u8) ('a' + d); t[0].nds += 1;
  if loud {
    if text.count > 44 { print("  recv %... -> %", text[0..<44], sx_input_name(d)); }
    else { print("  recv % -> %", text, sx_input_name(d)); }
  }
  if d == RL_DELIVER {
    stored := before == RS_LIVE;
    t[0].phases[t[0].np] = ifx stored then 's' else 'l'; t[0].np += 1;
    if loud { print(" (%, created_at %)", ifx stored then "stored" else "live", in.created_at); }
  }
  if loud && d == RL_EOSE {
    h := sx_hint(text);
    print(" (hint %, stored set %)", sx_hint_text(h), ifx sx_complete(h) then "complete" else "unknown");
  }
  if loud && d == RL_CLOSED { print(" (now %)", sx_sub_state_name(r[0].subs[slot].state)); }
  if loud && d == RL_UNKNOWN && slot >= 0 { print(" (it is %)", sx_sub_state_name(before)); }
  if loud { print("\n"); }
  return d;
}

sx_show_subs :: (r: [] Relay) {
  for slot: 0..<3 {
    s := r[0].subs[slot]; sb: [24] u8;
    print("  slot %: % %, % delivered", slot, rl_sub_text(slot, s.serial, sb[..]), sx_sub_state_name(s.state), s.events);
    if s.has_newest { print(", newest created_at %", s.newest); }
    print("\n");
  }
}

// Three subscriptions on one relay: stored events, EOSE with and without a
// hint, live events, a CLOSED that waits for AUTH, an unsubscribe and the
// slot's next id, a relay refusal, and a reconnect.
sx_session :: (loud: bool) -> SxTrace {
  t: [1] SxTrace; r: [1] Relay; ev: [2048] u8;
  for k: 0..<3 { s := rl_subscribe(r[..]); }
  g := rl_start(r[..], 0); ok := rl_opened(r[..], g, 0);
  if loud { print("t=0 three subscriptions; connection gen % is open\n", g); }
  sx_flush(r[..], 0, loud);

  if loud { print("t=100 the relay sends:\n"); }
  d := sx_recv(t[..], r[..], g, sx_event(1, "s0.1", ev[..]), loud);
  d = sx_recv(t[..], r[..], g, "[\"EOSE\",\"s0.1\",[\"finish\"]]", loud);
  d = sx_recv(t[..], r[..], g, sx_event(6, "s2.1", ev[..]), loud);
  d = sx_recv(t[..], r[..], g, "[\"EOSE\",\"s2.1\"]", loud);
  d = sx_recv(t[..], r[..], g, sx_event(7, "s2.1", ev[..]), loud);
  d = sx_recv(t[..], r[..], g, sx_event(6, "s2.1", ev[..]), loud);
  d = sx_recv(t[..], r[..], g, "[\"CLOSED\",\"s1.1\",\"auth-required: sign in to read state\"]", loud);
  d = sx_recv(t[..], r[..], g, sx_event(4, "s1.1", ev[..]), loud);
  d = sx_recv(t[..], r[..], g, "[\"CLOSED\",\"s1.1\",\"error: closed again\"]", loud);

  // The caller ends s0.1; its id is unknown at once, before the CLOSE goes.
  ok = rl_unsubscribe(r[..], 0);
  if loud { print("t=200 rl_unsubscribe(0): slot 0 is %\n", sx_sub_state_name(r[0].subs[0].state)); }
  d = sx_recv(t[..], r[..], g, "[\"EOSE\",\"s0.1\"]", loud);
  sx_flush(r[..], 200, loud);
  s := rl_subscribe(r[..]);
  if loud { print("t=200 rl_subscribe: slot %, serial %\n", s, r[0].subs[s].serial); }
  sx_flush(r[..], 200, loud);
  if loud { print("t=250 the relay sends:\n"); }
  d = sx_recv(t[..], r[..], g, sx_event(1, "s0.1", ev[..]), loud);
  d = sx_recv(t[..], r[..], g, "[\"CLOSED\",\"s0.2\",\"restricted: subscription limit reached\"]", loud);
  if loud { sx_show_subs(r[..]); }

  // The connection drops; on the next one each open subscription is asked again.
  ok = rl_dropped(r[..], g, 300, 0);
  g = rl_start(r[..], 800); ok = rl_opened(r[..], g, 850);
  if loud { print("t=300 dropped; t=850 connection gen % is open\n", g); }
  sx_flush(r[..], 850, loud);
  if loud { print("t=900 the relay sends:\n"); }
  d = sx_recv(t[..], r[..], g, sx_event(6, "s2.1", ev[..]), loud);
  d = sx_recv(t[..], r[..], g, "[\"EOSE\",\"s2.1\",[\"finish\"]]", loud);
  d = sx_recv(t[..], r[..], g, "[\"CLOSED\",\"s1.1\",\"auth-required: sign in to read state\"]", loud);
  if loud { sx_show_subs(r[..]); }
  return t[0];
}
```

`main` runs the session, fills a fresh `Relay` to show the client's limit,
reads nine `EOSE` frames for their hints, and runs the three page
scenarios.

<!-- source: nostr/examples/subscriptions.coax main -->
```jai
main :: () {
  t := sx_session(true);
  print("decisions: %\n", cast(string) t.decisions[0..<t.nds]);
  print("phases: %\n", cast(string) t.phases[0..<t.np]);

  r: [1] Relay;
  print("rl_subscribe nine times:");
  for k: 0..<9 { print(" %", rl_subscribe(r[..])); }
  print("\n");

  frames := [9] string.["[\"EOSE\",\"s0.1\"]", "[\"EOSE\",\"s0.1\",[\"finish\"]]", "[\"EOSE\",\"s0.1\",[\"more\",\"auth\"]]",
                        "[\"EOSE\",\"s0.1\",[]]", "[\"EOSE\",\"s0.1\",\"finish\"]", "[\"EOSE\",\"s0.1\",[\"FINISH\",\"x-cursor\"]]",
                        "[\"EOSE\",\"s0.1\",[\"finish\",\"more\"]]", "[\"EOSE\",\"s0.1\",[\"auth\",\"finish\"]]",
                        "[\"EOSE\",\"s0.1\",[],2,3,4,5]"];
  for f: frames {
    h := sx_hint(f);
    print("% -> hint %, stored set %\n", f, sx_hint_text(h), ifx sx_complete(h) then "complete" else "unknown");
  }

  print("A: 100 events, the relay sends at most 37 per REQ, no hints\n");
  a := sx_store_pairs(37, false); ca := sx_catch_up(a, true); sx_report(a, ca);
  print("B: the same relay with NIP-67 hints\n");
  b := sx_store_pairs(37, true); cb := sx_catch_up(b, true); sx_report(b, cb);
  print("C: 40 events share created_at 2000, cap 37, no hints\n");
  c := sx_store_tie(37); cc := sx_catch_up(c, true); sx_report(c, cc);
}
```

To run it:

```shell
coax/bin/coax run nostr/examples/subscriptions.coax
```

The program prints:

```text
t=0 three subscriptions; connection gen 1 is open
  send ["REQ","s0.1",{"kinds":[30617]}]
  send ["REQ","s1.1",{"kinds":[30621]}]
  send ["REQ","s2.1",{"kinds":[1111]}]
t=100 the relay sends:
  recv ["EVENT","s0.1",{"id":"813d9fb02bd78ce24200b... -> RL_DELIVER (stored, created_at 1793214821)
  recv ["EOSE","s0.1",["finish"]] -> RL_EOSE (hint finish, stored set complete)
  recv ["EVENT","s2.1",{"id":"5f377761f58cae551065c... -> RL_DELIVER (stored, created_at 1793219505)
  recv ["EOSE","s2.1"] -> RL_EOSE (hint none, stored set unknown)
  recv ["EVENT","s2.1",{"id":"fd6757b7c673ad4ebe991... -> RL_DELIVER (live, created_at 1793219504)
  recv ["EVENT","s2.1",{"id":"5f377761f58cae551065c... -> RL_DUPLICATE
  recv ["CLOSED","s1.1","auth-required: sign in to ... -> RL_CLOSED (now AUTH)
  recv ["EVENT","s1.1",{"id":"375b0d296e3a16291582f... -> RL_UNKNOWN (it is AUTH)
  recv ["CLOSED","s1.1","error: closed again"] -> RL_UNKNOWN (it is AUTH)
t=200 rl_unsubscribe(0): slot 0 is CLOSING
  recv ["EOSE","s0.1"] -> RL_UNKNOWN (it is CLOSING)
  send ["CLOSE","s0.1"]
t=200 rl_subscribe: slot 0, serial 2
  send ["REQ","s0.2",{"kinds":[30617]}]
t=250 the relay sends:
  recv ["EVENT","s0.1",{"id":"813d9fb02bd78ce24200b... -> RL_UNKNOWN
  recv ["CLOSED","s0.2","restricted: subscription l... -> RL_CLOSED (now REFUSED)
  slot 0: s0.2 REFUSED, 0 delivered
  slot 1: s1.1 AUTH, 0 delivered
  slot 2: s2.1 CAUGHT, 2 delivered, newest created_at 1793219505
t=300 dropped; t=850 connection gen 2 is open
  send ["REQ","s1.1",{"kinds":[30621]}]
  send ["REQ","s2.1",{"kinds":[1111],"since":1793219505}]
t=900 the relay sends:
  recv ["EVENT","s2.1",{"id":"5f377761f58cae551065c... -> RL_DUPLICATE
  recv ["EOSE","s2.1",["finish"]] -> RL_EOSE (hint finish, stored set complete)
  recv ["CLOSED","s1.1","auth-required: sign in to ... -> RL_CLOSED (now AUTH)
  slot 0: s0.2 REFUSED, 0 delivered
  slot 1: s1.1 AUTH, 0 delivered
  slot 2: s2.1 CAUGHT, 2 delivered, newest created_at 1793219505
decisions: afafabhddddhbfh
phases: ssl
rl_subscribe nine times: 0 1 2 3 4 5 6 7 -1
["EOSE","s0.1"] -> hint none, stored set unknown
["EOSE","s0.1",["finish"]] -> hint finish, stored set complete
["EOSE","s0.1",["more","auth"]] -> hint more+auth, stored set unknown
["EOSE","s0.1",[]] -> hint none, stored set unknown
["EOSE","s0.1","finish"] -> hint none, stored set unknown
["EOSE","s0.1",["FINISH","x-cursor"]] -> hint none, stored set unknown
["EOSE","s0.1",["finish","more"]] -> hint finish+more, stored set unknown
["EOSE","s0.1",["auth","finish"]] -> hint finish+auth, stored set unknown
["EOSE","s0.1",[],2,3,4,5] -> hint frame refused, stored set unknown
A: 100 events, the relay sends at most 37 per REQ, no hints
  send ["REQ","s0.1",{"kinds":[1],"limit":500}]
  recv 37 events, 37 new, oldest created_at 1031; ["EOSE","s0.1"] -> RL_EOSE (hint none)
  send ["CLOSE","s0.1"]
  send ["REQ","s1.1",{"kinds":[1],"until":1031,"limit":500}]
  recv 37 events, 36 new, oldest created_at 1013; ["EOSE","s1.1"] -> RL_EOSE (hint none)
  send ["CLOSE","s1.1"]
  send ["REQ","s0.2",{"kinds":[1],"until":1013,"limit":500}]
  recv 28 events, 27 new, oldest created_at 1000; ["EOSE","s0.2"] -> RL_EOSE (hint none)
  send ["CLOSE","s0.2"]
  send ["REQ","s1.2",{"kinds":[1],"until":1000,"limit":500}]
  recv 2 events, 0 new, oldest created_at 1000; ["EOSE","s1.2"] -> RL_EOSE (hint none)
  send ["CLOSE","s1.2"]
  4 REQs, 100 of 100 events, stored set not known to be complete
B: the same relay with NIP-67 hints
  send ["REQ","s0.1",{"kinds":[1],"limit":500}]
  recv 37 events, 37 new, oldest created_at 1031; ["EOSE","s0.1",["more"]] -> RL_EOSE (hint more)
  send ["CLOSE","s0.1"]
  send ["REQ","s1.1",{"kinds":[1],"until":1031,"limit":500}]
  recv 37 events, 36 new, oldest created_at 1013; ["EOSE","s1.1",["more"]] -> RL_EOSE (hint more)
  send ["CLOSE","s1.1"]
  send ["REQ","s0.2",{"kinds":[1],"until":1013,"limit":500}]
  recv 28 events, 27 new, oldest created_at 1000; ["EOSE","s0.2",["finish"]] -> RL_EOSE (hint finish)
  send ["CLOSE","s0.2"]
  3 REQs, 100 of 100 events, stored set complete (the relay said finish)
C: 40 events share created_at 2000, cap 37, no hints
  send ["REQ","s0.1",{"kinds":[1],"limit":500}]
  recv 37 events, 37 new, oldest created_at 2000; ["EOSE","s0.1"] -> RL_EOSE (hint none)
  send ["CLOSE","s0.1"]
  send ["REQ","s1.1",{"kinds":[1],"until":2000,"limit":500}]
  recv 37 events, 0 new, oldest created_at 2000; ["EOSE","s1.1"] -> RL_EOSE (hint none)
  a full page with nothing new: stuck on created_at 2000; events there may be missing
  send ["CLOSE","s1.1"]
  send ["REQ","s0.2",{"kinds":[1],"until":1999,"limit":500}]
  recv 5 events, 5 new, oldest created_at 1999; ["EOSE","s0.2"] -> RL_EOSE (hint none)
  send ["CLOSE","s0.2"]
  send ["REQ","s1.2",{"kinds":[1],"until":1999,"limit":500}]
  recv 5 events, 0 new, oldest created_at 1999; ["EOSE","s1.2"] -> RL_EOSE (hint none)
  send ["CLOSE","s1.2"]
  4 REQs, 42 of 45 events, stored set not known to be complete, gap at created_at 2000
```

The session and the pages match the hand traces line for line. The
deliberate refusals in it are:

- frames for a subscription waiting for `AUTH`, one being closed, and an
  old id after its slot moved on: `RL_UNKNOWN`;
- a second `CLOSED` for a subscription the relay already closed:
  `RL_UNKNOWN`;
- the relay's subscription limit: `CLOSED` with `restricted:`, and the
  subscription stays refused across the reconnect;
- the ninth `rl_subscribe`: -1;
- hints that don't say complete: none, an empty array, a string instead of
  an array, `"FINISH"`, `"finish"` with `"more"` or with `"auth"`, and an
  `EOSE` with five trailing values, which `message.coax` refuses;
- in C, a page stuck on one second: the pager reports the gap instead of
  calling the result whole.

## How the code is checked

### Checks in the example

The checks are in
[nostr/examples/checks/subscriptions.coax](../examples/checks/subscriptions.coax),
which loads the example. They compare the code with values from outside it:

- the session's answers and phases against the hand trace above;
- `sx_hint` against NIP-67's three published `EOSE` examples, spaces as
  published, and against the two `EOSE` frames relay.openagents.com sent on
  2026-10-28 (lines 2 and 5 of `COAX_RELAY`);
- nine frames without a usable `"finish"`, none of which is complete;
- scenarios A, B and C against the page counts in the tables above, and C's
  missing events against the hand answer (events 5, 6 and 7);
- every cap from 3 to 60, with and without hints: every one of the 100
  events arrives, no gap is reported, and the set is complete exactly when
  the relay gives hints;
- cap 2, where each second's pair fills a page: 50 × 2 + 1 = 101 `REQ`s,
  all 100 events, and a reported gap, because the pager can't tell that
  nothing is missing.

<!-- source: nostr/examples/checks/subscriptions.coax check -->
```jai
// The session's answers, worked out by hand in the chapter before the
// program ran: 'a' + the RL_ code (a DELIVER, b DUPLICATE, d UNKNOWN,
// f EOSE, h CLOSED), and s (stored) or l (live) for each delivery.
HAND_DECISIONS :: "afafabhddddhbfh";
HAND_PHASES :: "ssl";
#test "the scripted session follows the hand trace" #steps 2000000000 {
  t := sx_session(false);
  #assert cast(string) t.decisions[0..<t.nds] == HAND_DECISIONS;
  #assert cast(string) t.phases[0..<t.np] == HAND_PHASES;
}
#test "NIP-67's published examples and relay.openagents.com's captured EOSE frames" {
  // NIP-67, Examples: the three EOSE frames, spaces as published.
  h := sx_hint("[\"EOSE\", \"sub2\", [\"finish\"]]");
  #assert h.read && h.finish && !h.more && !h.auth && sx_complete(h);
  h = sx_hint("[\"EOSE\", \"sub2b\", [\"more\"]]");
  #assert h.read && h.more && !h.finish && !sx_complete(h);
  h = sx_hint("[\"EOSE\", \"sub4\", [\"auth\", \"finish\"]]");
  #assert h.read && h.auth && h.finish && !sx_complete(h);
  // Captured 2026-10-28 from wss://relay.openagents.com (COAX_RELAY lines 2 and 5).
  lines := [2] int.[2, 5];
  for line: lines {
    span := nm_line(COAX_RELAY, line);
    h = sx_hint(COAX_RELAY[span.from..<span.to]);
    #assert h.read && h.finish && sx_complete(h);
  }
}
#test "a missing or unreadable hint is never complete" {
  // Every frame here lacks a usable "finish", or adds "more" or "auth".
  frames := [9] string.["[\"EOSE\",\"a\"]", "[\"EOSE\",\"a\",[]]", "[\"EOSE\",\"a\",\"finish\"]", "[\"EOSE\",\"a\",[\"FINISH\"]]",
                        "[\"EOSE\",\"a\",[[\"finish\"]]]", "[\"EOSE\",\"a\",{\"finish\":1}]", "[\"EOSE\",\"a\",[\"finish\",\"more\"]]",
                        "[\"EOSE\",\"a\",[\"finish\"],2,3,4,5]", "[\"EOSE\",[\"finish\"]]"];
  for f: frames { #assert !sx_complete(sx_hint(f)); }
  // Unknown values next to "finish" are ignored, and so are later elements.
  #assert sx_complete(sx_hint("[\"EOSE\",\"a\",[\"x-cursor\",{\"k\":[1]},\"finish\"],\"more\"]"));
  #assert !sx_hint("[\"EOSE\",\"a\",[\"finish\"],2,3,4,5]").read;
}
#test "the pages of scenarios A, B and C match the hand counts" #steps 2000000000 {
  // A: pages newest first, 37 each, until = the oldest created_at seen.
  a := sx_catch_up(sx_store_pairs(37, false), false);
  sentA := [4] int.[37, 37, 28, 2]; freshA := [4] int.[37, 36, 27, 0];
  #assert a.reqs == 4 && a.delivered == 100 && !a.complete && !a.gap;
  for k: 0..<4 { #assert a.sent[k] == sentA[k] && a.fresh[k] == freshA[k]; }
  // B: "finish" on the third page saves the fourth REQ.
  b := sx_catch_up(sx_store_pairs(37, true), false);
  #assert b.reqs == 3 && b.delivered == 100 && b.complete;
  // C: 40 events at 2000; the relay's first page is events 44 down to 8, so
  // 5, 6 and 7 are never sent.
  c := sx_catch_up(sx_store_tie(37), false);
  sentC := [4] int.[37, 37, 5, 5]; freshC := [4] int.[37, 0, 5, 0];
  #assert c.reqs == 4 && c.delivered == 42 && !c.complete && c.gap && c.gap_at == 2000;
  for k: 0..<4 { #assert c.sent[k] == sentC[k] && c.fresh[k] == freshC[k]; }
  for i: 0..<45 { #assert c.got[i] == (i < 5 || i > 7); }
}
#forall "pagination yields every event for any cap from 3 to 60" cap: 3..<61 #steps 2000000000 {
  // Ties come in pairs, so a page of 3 always gets past one; the id window
  // (64) holds a full page and its boundary events.
  both := [2] bool.[false, true];
  for hints: both {
    c := sx_catch_up(sx_store_pairs(cap, hints), false);
    #assert c.delivered == 100 && !c.gap && !c.stopped && c.complete == hints;
    for i: 0..<100 { #assert c.got[i]; }
  }
}
#test "a cap of 2 is stuck on every pair: the gap is reported, though nothing is lost" #steps 2000000000 {
  // Per timestamp, one REQ brings its pair and one comes back full of
  // duplicates (stuck); then until = 999 finds nothing: 50 * 2 + 1 REQs.
  c := sx_catch_up(sx_store_pairs(2, false), false);
  #assert c.reqs == 101 && c.delivered == 100 && c.gap && c.gap_at == 1000 && !c.complete && !c.stopped;
}
```

To run them:

```shell
coax/bin/coax test nostr/examples/checks/subscriptions.coax
```

On 2026-11-01 this reported 271 of 271 checks passed, including the checks
of every file the example loads. The session check verifies seven Schnorr
signatures at compile time and carries `#steps 2000000000`.

### The library's own checks

[relay.coax](../../coax/lib/nostr/relay.coax)'s checks cover the
subscription side used here: the connect-subscribe-`EOSE` path with
out-of-order `created_at`, unknown ids (`s0.2`, `s1.1`, `s9.1`, `x`), an
unsubscribed id that is unknown at once and its `CLOSE`, the slot's next
serial, the 8-slot limit, `CLOSED` with and without `auth-required:` and the
single retry, refused subscriptions staying refused across a reconnect,
the 64-id window forgetting its oldest id, and `rl_sub_text` and
`rl_sub_find` on edge ids. [message.coax](../../coax/lib/nostr/message.coax)
checks `EOSE` with zero and four trailing values and refuses five.

The [2026-10-29 relay client record](../../docs/verification/2026-10-29-nostr-relay-client.md)
reports 555 of 555 mutants of `relay.coax` killed, and the
[2026-10-30 relay_read record](../../docs/verification/2026-10-30-relay-read.md)
reports 5,371 of 5,371 killed across `relay_read.coax` and the `#authority`
units it loads. Neither file changed for this chapter, so those runs were
not repeated. The example and its checks are not `#authority` and were not
mutated.

## Summary

- A subscription id is `s<slot>.<serial>`. A new subscription in a slot
  gets a new serial, so a client never sends one id for two requests, and
  frames for an old id are `RL_UNKNOWN`.
- Only live and caught-up subscriptions accept frames. A delivery is stored
  before `EOSE` and live after; live doesn't mean newer.
- `EOSE` ends the stored events of one `REQ`. It is not a completeness
  claim. Only a NIP-67 `"finish"` without `"more"` or `"auth"` is, and a
  missing hint is "unknown".
- `rl_unsubscribe` sends `CLOSE` only when the relay has the `REQ`. A
  `CLOSED` is final unless it says `auth-required:`.
- The client holds 8 subscriptions; the relay may hold fewer and says so
  with `CLOSED`.
- One 64-id window drops duplicates across subscriptions and reconnects.
- Pages go backward with an inclusive `until`. Stop on `"finish"` or on a
  short page with nothing new; a full page with nothing new is a tie the
  client can't page past, and is reported as a gap.

## Dependencies

- [29. A relay connection](ch029-a-relay-connection.md)
- [23. Filters](ch023-filters.md)

## Sources

- [NIP-01](../nips/official/01.md), `REQ`, `CLOSE`, `EOSE`, `CLOSED`, and
  `since` and `until`.
- [NIP-67](../nips/official/67.md), the `EOSE` completeness hints and the
  `created_at` tie rule.
- [NIP-42](../nips/official/42.md), the `auth-required:` prefix.
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
