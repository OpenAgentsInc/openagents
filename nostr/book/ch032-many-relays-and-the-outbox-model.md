# 32. Many relays and the outbox model

Nostr has no central server. Each user says in a relay list where they
publish and where they read. The outbox model uses those lists: read a
person's events from the relays they write to, and send an event that
mentions someone to the relays they read. This chapter reads NIP-65 relay
lists, chooses relays from them, and merges what several relays send for
one subscription.

Part IV, A client. Result: NIP-65 relay lists (kind 10002 with `r` tags
marked `read` or `write`), reading from an author's write relays, writing to
your write relays and the tagged users' read relays.

## What this chapter teaches

- Kind 10002, replaceable, with `["r", url, "read"|"write"]` tags; no marker
  means both.
- Reading: fetch an author's events from their write relays.
- Writing: send to your write relays and to each tagged person's read relays.
- Bounds: a maximum number of relays per author and per request, and a
  greedy cover that asks few relays for many follows.
- Merging results from many relays with deduplication by ID, and when a
  subscription on several relays is caught up.

Chapters 29 to 31 ran one relay connection. This chapter is the layer
above: which connections to open and what to ask each one. It opens no
socket. `relay.coax` runs each connection and checks each event's
signature, and the caller hands this layer what each connection delivered.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [nip65.coax](../../coax/lib/nostr/nip65.coax) | `nip65_read(e)` | a parsed kind 10002 event's `r` tags as a `Nip65List`: each relay's URL and its read and write flags (`#authority`) |
| [outbox.coax](../../coax/lib/nostr/outbox.coax) | `ob_relay(o, url)`, `ob_author(o, pubkey, l)` | a pool of up to 64 relays and a table of up to 64 authors with their read and write relays (`#authority`) |
| | `ob_publish(o, me, tagged, per_author, most)` | where to send an event: your write relays and each tagged user's read relays |
| | `ob_inbox(o, a, per_author)` | where to look for events that mention user `a`: `a`'s read relays |
| | `ob_cover(o, follows, per_author, most)` | which relays to ask for the follows' events, and which follows to ask each one for |
| | `ob_sub_start`, `ob_sub_event`, `ob_sub_eose`, `ob_sub_closed`, `ob_sub_tick`, `ob_caught_up` | one subscription sent to several relays: one copy per event id, and caught up when every relay has ended |
| [tags.coax](../../coax/lib/nostr/tags.coax) | `nt_start` | chapter 22's walk to an event's tag array |
| [relay_uri.coax](../../coax/lib/net/relay_uri.coax) | `relay_uri(url)` | chapter 28's relay URL decision, which every listed URL must pass |

Both new files carry `#authority;`. `nip65.coax` decides which lists are
admitted, and `outbox.coax` decides where events go and which delivered
events reach the application.

## A relay list

NIP-65 defines kind 10002. Its `r` tags name relays:

```jai
["r", "wss://alicerelay.example.com"]
["r", "wss://expensive-relay.example2.com", "write"]
["r", "wss://nostr-relay.example.com", "read"]
```

A relay with no marker is both a read relay and a write relay. A write relay
is one where the user publishes their own events. A read relay is one where
the user looks for events that mention them. Kind 10002 is replaceable
(chapter 22), so the newest list from a key is the whole list. The store
does that replacement; `nip65_read` reads one event.

`nip65_read(e)` takes an event that chapter 21's reader has parsed. It
checks `e.kind == 10002`, walks the tag array and reads every tag whose name
is `r`. It skips other tags and ignores the content. For each `r` tag:

1. The second value is the URL. It is JSON-unescaped into a 256-byte buffer
   and must pass `relay_uri`: `ws://` or `wss://`, a DNS host name (no IP
   literals, no credentials, no fragment) and an optional port and path.
2. An optional third value is the marker: exactly `read` or exactly
   `write`. `read` clears the write flag; `write` clears the read flag.
3. A URL already in the list is the same relay. Its flags are the union of
   both entries, so `["r", u, "read"]` and `["r", u, "write"]` together mean
   both.
4. A 17th distinct relay is refused. NIP-65 asks users to keep 2 to 4
   relays of each kind; 16 is this reader's bound.

URLs are compared byte for byte after unescaping. Nothing is normalized, so
`wss://a.example` and `wss://a.example/` are two relays. A client that wants
to merge such spellings must normalize before it builds the pool.

Each refusal names the tag it stopped at in `l.tag`:

| Code | Name | Cause |
| --- | --- | --- |
| 1 | `N65_EVENT` | the event or its tag array is not readable, or a tag is not an array |
| 2 | `N65_KIND` | the kind is not 10002 |
| 3 | `N65_TAG` | an `r` tag without a URL, with a value that is not a string, or with values after the marker |
| 4 | `N65_MARKER` | a third value other than `read` or `write`, including `""` and `Read` |
| 5 | `N65_URL` | `relay_uri` refuses the URL |
| 6 | `N65_TOO_MANY` | a 17th distinct relay |
| 7 | `N65_LIMIT` | a URL over 256 bytes, or one with a lone surrogate escape |

`nip65_read` does not check the signature. Events reach it through
`relay.coax`, which drops forged events (chapter 30), or through the store
(chapter 35), which keeps only verified ones.

<!-- source: coax/lib/nostr/nip65.coax read -->
```jai
// The r tags of a parsed kind 10002 event. The signature is not checked
// here: relay.coax refuses forged events before they reach a reader.
nip65_read :: (e: NostrEvent) -> Nip65List {
  l: Nip65List; j: [1] Json;
  if !nt_start(j[..], e) { return l; }
  if e.kind != N65_KIND_RELAYS { return n65_fail(l, N65_KIND, 0); }
  k := 0; t := json_next(j[..]);
  while t != JSON_ARRAY_END {
    if t != JSON_ARRAY { return n65_fail(l, N65_EVENT, k); }
    depth := j[0].depth; t = json_next(j[..]);
    if t != JSON_STRING || !json_is(json_text(j[..]), "r") {
      // Another tag: read on to the end of its array.
      while j[0].depth >= depth { if json_next(j[..]) == JSON_ERROR { return n65_fail(l, N65_EVENT, k); } }
      k += 1; t = json_next(j[..]); continue;
    }
    url: [N65_URL_MOST] u8; n: int; read := true; write := true; values := 1;
    t = json_next(j[..]);
    while t != JSON_ARRAY_END {
      if t != JSON_STRING || values > 2 { return n65_fail(l, N65_TAG, k); }
      raw := json_text(j[..]);
      if values == 1 {
        n = json_unescape(raw, url[..]);
        if n < 0 { return n65_fail(l, N65_LIMIT, k); }
        if !relay_uri(cast(string) url[0..<n]).ok { return n65_fail(l, N65_URL, k); }
      } else if json_is(raw, "read") { write = false; }
      else if json_is(raw, "write") { read = false; }
      else { return n65_fail(l, N65_MARKER, k); }
      values += 1; t = json_next(j[..]);
    }
    if values == 1 { return n65_fail(l, N65_TAG, k); }
    at := l.count;
    for i: 0..<l.count { if n65_is(l, i, cast(string) url[0..<n]) { at = i; } }
    if at == N65_MOST { return n65_fail(l, N65_TOO_MANY, k); }
    if at == l.count {
      for i: 0..<n { l.url[at * N65_URL_MOST + i] = url[i]; }
      l.url_n[at] = n; l.count += 1;
    }
    l.read[at] = l.read[at] || read; l.write[at] = l.write[at] || write;
    k += 1; t = json_next(j[..]);
  }
  return n65_fail(l, N65_OK, 0);
}
```

## The pool and the author table

`outbox.coax` keeps two tables in one `Outbox`:

- The pool: up to 64 distinct relay URLs. `ob_relay(o, url)` returns a URL's
  index and adds it when new. It refuses a URL that `relay_uri` refuses, one
  over 256 bytes, and a 65th relay (-1). Relays are numbered in the order
  they first appear.
- The author table: up to 64 authors. `ob_author(o, pubkey, l)` adds each
  relay of the list to the pool and stores the author's read set and write
  set. A key already in the table gets the new list in place of the old.
  It returns the author's index, `-OB_LIST` for a refused list or
  `-OB_FULL` when the pool or the table is full.

A set of relays is a `u64` with bit `r` set for relay `r`. A set of authors
is a `u64` the same way. Both bounds are 64 so that one machine word holds
each set; a client that follows more people runs the cover once per group of
64.

## Where to send and where to look

NIP-65 says that when you publish an event, you send it to your write
relays and to all read relays of each user the event tags.
`ob_publish(o, me, tagged, per_author, most)` returns that set, with two
bounds:

- `per_author`: at most this many relays from each author's set, the
  lowest-numbered first.
- `most`: if the union has more than this many relays, the result is
  `OB_TOO_MANY`. The set is still reported, so the caller can choose what to
  drop. The code never drops a relay silently.

NIP-65 also asks the publisher to send its own kind 10002 event to every
relay it published to, so that readers there can find the rest of its
relays. That is a second publish by the caller with the same relay set.

`ob_inbox(o, a, per_author)` is the other half: to find events that mention
user `a`, ask `a`'s read relays.

## Asking few relays for many follows

To read the follows' events, a client asks each follow's write relays. Doing
that naively opens every write relay of every follow. `ob_cover(o, follows,
per_author, most)` chooses fewer relays with a greedy cover:

1. For level 1 to `per_author`:
   1. Every relay already in the plan takes, in relay order, each follow
      that writes to it, is not yet asked of it, and has fewer than `level`
      relays.
   2. While the plan has fewer than `most` relays: find the relay that
      would serve the most follows still needing a relay at this level. On a
      tie, take the lowest relay index. If no relay serves anyone, stop.
      Otherwise add it to the plan and assign those follows to it.
2. The result lists the picks in order, the follows to ask each pick for
   (`asks`), and how many relays each follow got (`got`).

Each level finishes before the next starts, so every follow gets one relay
before any follow gets two. A follow with no write relays gets none
(`got` 0). The caller decides the fallback, for example a relay hint from
the follow list (chapter 45) or from an `nprofile` (chapter 16).

Step 1.1 matters because a relay picked late in a level serves only the
follows still uncovered at that moment. At the next level it can serve the
others for free, since it is already in the plan.

Greedy cover is not always optimal: finding the smallest set cover is
NP-hard. The checks compare the greedy level-1 result with the smallest
cover found by trying every subset of the pool, for both fixtures below,
and they agree.

<!-- source: coax/lib/nostr/outbox.coax cover -->
```jai
// The follows in `want` that still need a relay at this level, write to
// relay r, and are not yet asked of r.
ob_needing :: (o: [] Outbox, p: ObPlan, taken: [] u64, want: u64, r: int, level: int) -> u64 {
  m: u64 = 0; bit := cast(u64) 1 << r;
  for a: 0..<o[0].authors {
    if (want & (cast(u64) 1 << a)) != 0 && p.got[a] < level && (o[0].author[a].write & bit) != 0 && (taken[a] & bit) == 0 {
      m |= cast(u64) 1 << a;
    }
  }
  return m;
}
ob_assign :: (p: ObPlan, taken: [] u64, k: int, m: u64) -> ObPlan {
  q := p; r := q.relay[k];
  q.asks[k] |= m;
  for a: 0..<OB_AUTHORS {
    if (m & (cast(u64) 1 << a)) != 0 { q.got[a] += 1; taken[a] |= cast(u64) 1 << r; }
  }
  return q;
}
// Which relays to ask for the follows' events. For level 1 to per_author:
// first every relay already in the plan takes the follows it can serve at
// this level, in relay order; then, while the plan has fewer than `most`
// relays, the relay not yet in the plan that serves the most follows still
// needing one at this level joins it (ties: the lowest index). A relay
// picked later in a level serves only the follows still uncovered then; the
// next level's first step lets it serve the others. A follow
// with no write relays gets none (got 0); the caller decides the fallback.
ob_cover :: (o: [] Outbox, follows: [] int, per_author: int, most: int) -> ObPlan {
  // taken[a]: the relays asked for author a. slot[r]: 1 + relay r's pick,
  // or 0 while r is not in the plan.
  p: ObPlan; want: u64; taken: [OB_AUTHORS] u64; slot: [OB_RELAYS] int;
  for f: follows {
    if !ob_known(o, f) { p.error = OB_INDEX; return p; }
    want |= cast(u64) 1 << f;
  }
  for level: 1..<per_author + 1 {
    for r: 0..<o[0].relays {
      if slot[r] != 0 { p = ob_assign(p, taken[..], slot[r] - 1, ob_needing(o, p, taken[..], want, r, level)); }
    }
    // A relay already in the plan serves nobody more at this level: the
    // step above, or its own pick, gave it every follow it could take.
    while p.count < most {
      best := -1; best_n: int; best_m: u64;
      for r: 0..<o[0].relays {
        m := ob_needing(o, p, taken[..], want, r, level); n := ob_count(m);
        if n > best_n { best = r; best_n = n; best_m = m; }
      }
      if best < 0 { break; }
      p.relay[p.count] = best; p.count += 1; slot[best] = p.count;
      p = ob_assign(p, taken[..], p.count - 1, best_m);
    }
  }
  return p;
}
```

## One subscription on several relays

The plan becomes one `REQ` per chosen relay, each with an `authors` filter
from `asks`. Each relay answers with stored events, then `EOSE`, then live
events (chapter 30). The same event often arrives from several relays.

`ob_sub_start(s, relays, now, wait_ms)` starts the merge for a set of relays
with one deadline, `now + wait_ms`. Then:

- `ob_sub_event(s, relay, id)` returns `OB_NEW` the first time any relay
  delivers the id, `OB_DUP` after that, and `OB_NOT_ASKED` for a relay that
  the subscription was not sent to. The seen set holds the last 256 ids,
  like `relay.coax`'s per-connection set; an id older than that can be
  delivered again.
- `ob_sub_eose(s, relay)` and `ob_sub_closed(s, relay)` end a waiting relay.
  They return false for a relay that already ended, so a late `EOSE` after
  a timeout changes nothing.
- `ob_sub_tick(s, now)` times out every waiting relay once
  `now >= deadline`.
- `ob_caught_up(s)` is true when no relay of the subscription is still
  waiting: each sent `EOSE`, was closed, or timed out.

Caught up means the client has heard from every relay or stopped waiting
for it. It does not mean the client has every stored event: a timed-out or
closed relay may hold events the others don't.

<!-- source: coax/lib/nostr/outbox.coax merge -->
```jai
// An event (by id) that relay `relay` delivered for this subscription: OB_NEW
// the first time any relay delivers the id, OB_DUP after, OB_NOT_ASKED from
// a relay the subscription was not sent to. relay.coax has already checked
// the signature and dropped repeats from the same connection.
ob_sub_event :: (s: [] ObSub, relay: int, id: [32] u8) -> int {
  if !ob_asked(s, relay) { return OB_NOT_ASKED; }
  s[0].events[relay] += 1;
  if ob_seen(s, id) { s[0].dups += 1; return OB_DUP; }
  at := s[0].seen_next * 32;
  for i: 0..<32 { s[0].seen[at + i] = id[i]; }
  s[0].seen_next = (s[0].seen_next + 1) % OB_SEEN;
  if s[0].seen_count < OB_SEEN { s[0].seen_count += 1; }
  s[0].fresh += 1;
  return OB_NEW;
}
ob_sub_end :: (s: [] ObSub, relay: int, state: int) -> bool {
  if !ob_asked(s, relay) || s[0].state[relay] != OB_WAIT { return false; }
  s[0].state[relay] = state;
  return true;
}
// The relay sent EOSE: true when it was still waiting.
ob_sub_eose :: (s: [] ObSub, relay: int) -> bool { return ob_sub_end(s, relay, OB_EOSE); }
// The relay sent CLOSED or the connection failed: true when it was still waiting.
ob_sub_closed :: (s: [] ObSub, relay: int) -> bool { return ob_sub_end(s, relay, OB_CLOSED); }
// At or after the deadline, every relay still waiting times out. Returns how many did.
ob_sub_tick :: (s: [] ObSub, now: int) -> int {
  if now < s[0].deadline { return 0; }
  n := 0; rest := s[0].relays; r := 0;
  while rest != 0 {
    if (rest & 1) != 0 && s[0].state[r] == OB_WAIT { s[0].state[r] = OB_TIMEOUT; n += 1; }
    rest = rest >> 1; r += 1;
  }
  return n;
}
// Caught up: no relay of the subscription is still waiting.
ob_caught_up :: (s: [] ObSub) -> bool {
  rest := s[0].relays; r := 0;
  while rest != 0 {
    if (rest & 1) != 0 && s[0].state[r] == OB_WAIT { return false; }
    rest = rest >> 1; r += 1;
  }
  return true;
}
```

## Worked example: four follows and five relays by hand

Five lists. Alice, Bob, Carol and Dave are the follows; you publish. The
relays are `wss://r1.example` to `wss://r5.example`, written r1 to r5
below.

| Author | `r` tags | Writes to | Reads on |
| --- | --- | --- | --- |
| alice | r1 `write`, r2 `write`, r3 `read` | r1 r2 | r3 |
| bob | r2, r3 | r2 r3 | r2 r3 |
| carol | r2 `write`, r4 | r2 r4 | r4 |
| dave | r5 `write`, r1 `read`, r3 `read` | r5 | r1 r3 |
| you | r1, r4 `write`, r5 `read` | r1 r4 | r1 r5 |

The lists are added in the order alice, bob, carol, dave, you. Alice brings
r1, r2 and r3 (indexes 0, 1, 2), Carol brings r4 (3) and Dave brings r5 (4).
Bob and you bring no new relay. So relay rN has index N - 1.

**Publish.** You reply to a note and tag Alice and Dave. With
`per_author` 2: your write relays are r1 and r4, Alice's read relay is r3,
and Dave's read relays are r1 and r3. The union is r1, r3 and r4: 3 relays.
With `per_author` 1: your first write relay r1, Alice's r3 and Dave's first
read relay r1, so r1 and r3. With `most` 2 and `per_author` 2, the set of 3
is `OB_TOO_MANY`. To find events that mention you, ask your read relays r1
and r5.

**Cover with `per_author` 2 and `most` 8.** Level 1. No relay is in the
plan yet. Count the follows that write to each relay:

| Relay | Writers | Count |
| --- | --- | --- |
| r1 | alice | 1 |
| r2 | alice, bob, carol | 3 |
| r3 | bob | 1 |
| r4 | carol | 1 |
| r5 | dave | 1 |

r2 wins with 3 and takes Alice, Bob and Carol. Now only Dave needs a relay
at level 1, and only r5 serves him. r5 joins and takes Dave.

Level 2. Step 1.1: r2 and r5 are in the plan, but every follow that writes
to them is already asked of them. Step 1.2: Alice still needs a second
relay (r1), Bob needs one (r3), Carol needs one (r4). Dave writes only to
r5, which he already has, so no relay serves him. r1, r3 and r4 each serve
one follow. The tie goes to the lowest index: r1, then r3, then r4.

| Pick | Relay | Asked for |
| --- | --- | --- |
| 1 | r2 | alice, bob, carol |
| 2 | r5 | dave |
| 3 | r1 | alice |
| 4 | r3 | bob |
| 5 | r4 | carol |

Alice, Bob and Carol get 2 relays and Dave gets 1. With `per_author` 1 the
plan stops after pick 2. With `most` 3 it stops after pick 3: Alice on 2
relays, the others on 1. Asking every write relay would open all 5 relays
for level 1 instead of 2.

**Merge.** The subscription goes to all five relays of the 2-per-author
plan, with a 3000 ms wait. The scripted relays send these steps; e1 to e6
are event ids (the merge reads only ids, so they are placeholders):

| t (ms) | Relay | Message | Result | Caught up |
| --- | --- | --- | --- | --- |
| 10 | r2 | e1 | new | no |
| 10 | r2 | e2 | new | no |
| 12 | r2 | e3 | new | no |
| 15 | r2 | `EOSE` | ended | no |
| 20 | r1 | e1 | duplicate | no |
| 25 | r1 | `EOSE` | ended | no |
| 30 | r3 | e2 | duplicate | no |
| 31 | r3 | e5 | new | no |
| 35 | r3 | `EOSE` | ended | no |
| 40 | r5 | e4 | new | no |
| 60 | r4 | `CLOSED` | ended | no |
| 2999 | clock | | 0 timed out (2999 < 3000) | no: r5 waits |
| 3000 | clock | | r5 timed out | yes |
| 3100 | r5 | `EOSE` | ignored: r5 timed out | yes |
| 3200 | r5 | e4 | duplicate | yes |
| 3200 | r1 | e6 | new | yes |

Nine events arrive. Six ids are distinct (e1 to e6), so 6 are delivered and
3 are dropped as duplicates. As a trace, with `n` new, `d` duplicate, `e` an
accepted end, `-` an ignored end and a digit for a clock step, each followed
by `c` (caught up) or `.`, the session is
`n.n.n.e.d.e.d.n.e.n.e.0.1c-cdcnc`.

## Worked example: fifty authors by hand

The PLAN's fixture has 50 authors, numbered 0 to 49, and eight relays added
in this order: big, east, west, s0, s1, s2, s3 and s4. Author `i` writes to:

- big when `i < 30`
- east when `20 <= i < 40`
- west when `35 <= i < 48`
- `s(i mod 5)` when `i < 48`

Author 48 lists big as a read relay only, and author 49 has an empty list.
Neither has a write relay.

**Level 1** (`per_author` 2, `most` 8). 48 authors need a relay.

1. Counts: big 30 (0-29), east 20 (20-39), west 13 (35-47), s0 10, s1 10,
   s2 10, s3 9, s4 9. big wins and takes 0-29.
2. Still needing: 30-47 (18 authors). east serves 30-39 (10), west serves
   35-47 (13), s0 serves 30, 35, 40 and 45 (4), and so on. west wins and
   takes 35-47.
3. Still needing: 30-34 (5). east serves all 5; each s relay serves 1. east
   wins and takes 30-34.

Three relays cover all 48 authors once. No two relays can do it: big and
west miss 30-34, big and east miss 40-47, and every other pair misses more.
So level 1 is the smallest cover.

**Level 2.** Step 1.1, in relay order: big has no writer left to take; east
takes 20-29 and 35-39, which write to it and have 1 relay; west has none
left. Now 20-29 and 35-39 have 2 relays. Still needing: 0-19, 30-34 and
40-47, 33 authors. Each s relay serves the needing authors with
`i mod 5` equal to its number:

| Relay | Needing authors it serves | Count |
| --- | --- | --- |
| s0 | 0, 5, 10, 15, 30, 40, 45 | 7 |
| s1 | 1, 6, 11, 16, 31, 41, 46 | 7 |
| s2 | 2, 7, 12, 17, 32, 42, 47 | 7 |
| s3 | 3, 8, 13, 18, 33, 43 | 6 |
| s4 | 4, 9, 14, 19, 34, 44 | 6 |

The sets don't overlap, so taking one doesn't change the others' counts.
s0 wins the tie with s1 and s2 by index, then s1, s2, s3 and s4.

| Pick | Relay | Asked for |
| --- | --- | --- |
| 1 | big | 0-29 |
| 2 | west | 35-47 |
| 3 | east | 20-39 (30-34 at level 1, 20-29 and 35-39 at level 2) |
| 4 | s0 | 0, 5, 10, 15, 30, 40, 45 |
| 5 | s1 | 1, 6, 11, 16, 31, 41, 46 |
| 6 | s2 | 2, 7, 12, 17, 32, 42, 47 |
| 7 | s3 | 3, 8, 13, 18, 33, 43 |
| 8 | s4 | 4, 9, 14, 19, 34, 44 |

Authors 0-47 get 2 relays each; 48 and 49 get none.

With `most` 4, the plan stops after s0. The 7 authors s0 serves have 2
relays, as do 20-29 and 35-39 (15): 22 in all. The other 26 of 0-47 have 1.

## The example

The example is [nostr/examples/outbox.coax](../examples/outbox.coax). It
loads `outbox.coax` (which loads `nip65.coax`) and chapter 21's signer:

- `read_lists` signs each of the five lists with secret key 1 to 5 and zero
  aux, verifies it with `nostr_event_verify_vartime`, reads it with
  `nip65_read` and records the signer with `ob_author`.
- `main` computes the publish sets, the inbox and three covers.
- `merge` plays the scripted relays above through `ob_sub_*` and records a
  trace for the checks.
- `fifty` builds the 50 lists from the rule above (unsigned: signatures are
  not what this part tests) and `main` prints the two plans.
- The refusals at the end are the deliberately broken inputs: a missing
  URL, the marker `both`, an `https://` URL, an IP literal, a value after the
  marker, an IPv6 literal, 17 relays, a kind 3 event, a refused list given
  to `ob_author` and a follow index past the table.

<!-- source: nostr/examples/outbox.coax example -->
```jai
main :: () {
  o: [1] Outbox;
  print("Relay lists (kind 10002, signed by keys 1-5 and verified)\n");
  read_lists(o[..], true);

  print("\nPublish a reply by you that tags alice and dave\n");
  tagged := [2] int.[0, 3];
  for cap: 1..<3 {
    s := ob_publish(o[..], 4, tagged[..], cap, 4);
    print("  % per author: ", cap); print_relays(o[..], s.relays);
    print(" (% relays, %)\n", s.count, OB[s.error]);
  }
  s := ob_publish(o[..], 4, tagged[..], 2, 2);
  print("  2 per author, at most 2 relays: "); print_relays(o[..], s.relays); print(" (% relays, %)\n", s.count, OB[s.error]);
  print("  mentions of you are read from: "); print_relays(o[..], ob_inbox(o[..], 4, 2).relays); print("\n");

  print("\nRead alice, bob, carol and dave from their write relays\n");
  follows := [4] int.[0, 1, 2, 3];
  pers := [3] int.[1, 2, 2]; mosts := [3] int.[8, 8, 3];
  for k: 0..<3 {
    per := pers[k]; most := mosts[k];
    p := ob_cover(o[..], follows[..], per, most);
    print("  % per author, at most % relays: % relays (%)\n", per, most, p.count, OB[p.error]);
    print_plan(o[..], p, true);
    print("    relays per author:"); for a: 0..<4 { print(" % %", NAMES[a], p.got[a]); } print("\n");
  }

  print("\nMerge one subscription across the 5 relays of the 2-per-author plan\n");
  p := ob_cover(o[..], follows[..], 2, 8);
  chosen: u64 = 0; for k: 0..<p.count { chosen |= cast(u64) 1 << p.relay[k]; }
  t := merge(o[..], chosen, true);
  print("  trace %\n", cast(string) t.steps[0..<t.n]);

  print("\nFifty authors over eight relays\n");
  f: [1] Outbox; fifty(f[..]); all := fifty_follows();
  limits := [2] int.[8, 4];
  for most: limits {
    q := ob_cover(f[..], all[..], 2, most);
    print("  2 per author, at most % relays: % relays\n", most, q.count);
    print_plan(f[..], q, false);
    counts: [3] u64; for a: 0..<50 { counts[q.got[a]] |= cast(u64) 1 << a; }
    for c: 0..<3 { print("    on % relay(s): ", c); print_authors(counts[c], false); print(" (% authors)\n", ob_count(counts[c])); }
  }

  print("\nRefusals\n");
  bad := [6] string.["[[\"r\",\"wss://r1.example\"],[\"r\"]]", "[[\"r\",\"wss://r1.example\",\"both\"]]",
                     "[[\"r\",\"https://r1.example\"]]", "[[\"r\",\"ws://127.0.0.1:7777\"]]",
                     "[[\"r\",\"wss://r1.example\",\"read\",\"extra\"]]", "[[\"r\",\"wss://[::1]/\"]]"];
  for k: 0..<6 {
    l := ob_list(bad[k]);
    print("  %: %, tag %\n", bad[k], N65[l.error], l.tag);
  }
  many: [1024] u8; n := json_raw(many[..], 0, "[");
  for i: 0..<17 {
    if i > 0 { n = json_raw(many[..], n, ","); }
    n = json_raw(many[..], n, "[\"r\",\"wss://m"); n = json_write_int(many[..], n, i); n = json_raw(many[..], n, ".example\"]");
  }
  n = json_raw(many[..], n, "]");
  l := ob_list(cast(string) many[0..<n]);
  print("  17 distinct r tags: %, tag %, % relays kept\n", N65[l.error], l.tag, l.count);
  l = nip65_read(n65_event(3, OB_BOB));
  print("  bob's tags as a kind 3 event: %\n", N65[l.error]);
  print("  an author whose list was refused: ob_author %\n", OB[-ob_author(o[..], ob_key(9), ob_list(bad[0]))]);
  past := [1] int.[50];
  print("  a follow index past the table: %\n", OB[ob_cover(f[..], past[..], 2, 8).error]);
}
```

Run it from the repository root:

```shell
coax/bin/coax run nostr/examples/outbox.coax --run-steps 250000000
```

The output, identical in debug and release:

```text
Relay lists (kind 10002, signed by keys 1-5 and verified)
  alice: list ok, 3 relays: r1 write r2 write r3 read
    writes to r1 r2; reads mentions on r3
  bob: list ok, 2 relays: r2 both r3 both
    writes to r2 r3; reads mentions on r2 r3
  carol: list ok, 2 relays: r2 write r4 both
    writes to r2 r4; reads mentions on r4
  dave: list ok, 3 relays: r5 write r1 read r3 read
    writes to r5; reads mentions on r1 r3
  you: list ok, 3 relays: r1 both r4 write r5 read
    writes to r1 r4; reads mentions on r1 r5

Publish a reply by you that tags alice and dave
  1 per author: r1 r3 (2 relays, ok)
  2 per author: r1 r3 r4 (3 relays, ok)
  2 per author, at most 2 relays: r1 r3 r4 (3 relays, too many)
  mentions of you are read from: r1 r5

Read alice, bob, carol and dave from their write relays
  1 per author, at most 8 relays: 2 relays (ok)
    pick 1: r2 asked for alice, bob, carol
    pick 2: r5 asked for dave
    relays per author: alice 1 bob 1 carol 1 dave 1
  2 per author, at most 8 relays: 5 relays (ok)
    pick 1: r2 asked for alice, bob, carol
    pick 2: r5 asked for dave
    pick 3: r1 asked for alice
    pick 4: r3 asked for bob
    pick 5: r4 asked for carol
    relays per author: alice 2 bob 2 carol 2 dave 1
  2 per author, at most 3 relays: 3 relays (ok)
    pick 1: r2 asked for alice, bob, carol
    pick 2: r5 asked for dave
    pick 3: r1 asked for alice
    relays per author: alice 2 bob 1 carol 1 dave 1

Merge one subscription across the 5 relays of the 2-per-author plan
  t=0 REQ to r1 r2 r3 r4 r5, wait 3000 ms for EOSE
  t=10 r2 EVENT e1: new
  t=10 r2 EVENT e2: new
  t=12 r2 EVENT e3: new
  t=15 r2 EOSE (ended)
  t=20 r1 EVENT e1: duplicate
  t=25 r1 EOSE (ended)
  t=30 r3 EVENT e2: duplicate
  t=31 r3 EVENT e5: new
  t=35 r3 EOSE (ended)
  t=40 r5 EVENT e4: new
  t=60 r4 CLOSED (ended)
  t=2999 clock: 0 relay(s) timed out
  t=3000 clock: 1 relay(s) timed out, caught up
  t=3100 r5 EOSE (ignored: timed out), caught up
  t=3200 r5 EVENT e4: duplicate, caught up
  t=3200 r1 EVENT e6: new, caught up
  6 events delivered once, 3 duplicates dropped; per relay: r1 2 (EOSE) r2 3 (EOSE) r3 2 (EOSE) r4 0 (closed) r5 2 (timed out)
  trace n.n.n.e.d.e.d.n.e.n.e.0.1c-cdcnc

Fifty authors over eight relays
  2 per author, at most 8 relays: 8 relays
    pick 1: big asked for 0-29
    pick 2: west asked for 35-47
    pick 3: east asked for 20-39
    pick 4: s0 asked for 0, 5, 10, 15, 30, 40, 45
    pick 5: s1 asked for 1, 6, 11, 16, 31, 41, 46
    pick 6: s2 asked for 2, 7, 12, 17, 32, 42, 47
    pick 7: s3 asked for 3, 8, 13, 18, 33, 43
    pick 8: s4 asked for 4, 9, 14, 19, 34, 44
    on 0 relay(s): 48-49 (2 authors)
    on 1 relay(s): none (0 authors)
    on 2 relay(s): 0-47 (48 authors)
  2 per author, at most 4 relays: 4 relays
    pick 1: big asked for 0-29
    pick 2: west asked for 35-47
    pick 3: east asked for 20-39
    pick 4: s0 asked for 0, 5, 10, 15, 30, 40, 45
    on 0 relay(s): 48-49 (2 authors)
    on 1 relay(s): 1-4, 6-9, 11-14, 16-19, 31-34, 41-44, 46-47 (26 authors)
    on 2 relay(s): 0, 5, 10, 15, 20-30, 35-40, 45 (22 authors)

Refusals
  [["r","wss://r1.example"],["r"]]: tag, tag 1
  [["r","wss://r1.example","both"]]: marker, tag 0
  [["r","https://r1.example"]]: url, tag 0
  [["r","ws://127.0.0.1:7777"]]: url, tag 0
  [["r","wss://r1.example","read","extra"]]: tag, tag 0
  [["r","wss://[::1]/"]]: url, tag 0
  17 distinct r tags: too many, tag 16, 16 relays kept
  bob's tags as a kind 3 event: kind
  an author whose list was refused: ob_author list
  a follow index past the table: index
```

Every value matches the hand tables: the publish set r1 r3 r4, the five
picks r2, r5, r1, r3, r4, the merge trace, and both fifty-author plans with
26 authors on one relay when `most` is 4. Each refusal prints its code and
the tag it stopped at.

## What is not built

- Discovery: NIP-65 asks clients to spread kind 10002 events to well-known
  indexer relays. Choosing those relays is the caller's.
- Fallbacks for follows without a relay list (`got` 0). A client can use a
  relay hint from the follow's `p` tag or an `nprofile`; this layer reports
  who is uncovered and does not guess.
- Relay health: the cover does not know which relays are slow or down. A
  client can leave a failing relay out of the pool and run the cover again.
- URL normalization, as said above.
- More than 64 relays or 64 authors in one cover. A client splits its
  follows into groups.
- NIP-17's kind 10050 inbox relays for private messages are chapter 60's.

## How the code is checked

### Checks in the example

The checks are in
[nostr/examples/checks/outbox.coax](../examples/checks/outbox.coax), which
loads the example. They compare the code with values from outside it:

- each signed list is recorded under the x coordinate of `k·G` for k = 1
  to 5, the published small multiples of the secp256k1 generator
- the publish sets and the four-follow plan against the hand table
- the greedy level-1 count against the smallest cover found by trying every
  subset of the pool: 2 relays for the five-relay pool (9 of its 32 subsets
  cover, counted by hand), 3 for the eight-relay pool
- the fifty-author plan, author by author, against the rule that built the
  lists, and the 26 authors on one relay when `most` is 4
- the merge trace against the hand trace, and the new and duplicate counts
  against a recount of distinct ids in the script

<!-- source: nostr/examples/checks/outbox.coax check -->
```jai
// The x coordinates of 1G to 5G on secp256k1, as published (SEC 2 gives G;
// 2G to 5G are the well-known small multiples). Keys 1 to 5 sign the lists,
// so these are alice, bob, carol, dave and you.
MULTIPLES :: [5] string.[
  "79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798",
  "c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5",
  "f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9",
  "e493dbf1c10d80f3581e4904930b1404cc6c13900ee0758474fa94abe8c4cd13",
  "2f8bde4d1a07209355b4a7250a5c5128e88b84bddc619ab7cba8d569b240efe4"];
#test "each signed list is recorded under its signer, k*G" #steps 2000000000 {
  o: [1] Outbox; read_lists(o[..], false);
  for a: 0..<5 {
    key: [32] u8; r := hex_decode_checked(MULTIPLES[a], key[..], false);
    for i: 0..<32 { #assert o[0].author[a].pubkey[i] == key[i]; }
  }
}

// The chapter's hand tables. Relays r1-r5 are 0-4; alice-dave are 0-3.
#test "publish and cover match the hand tables" #steps 2000000000 {
  o: [1] Outbox; read_lists(o[..], false); tagged := [2] int.[0, 3]; follows := [4] int.[0, 1, 2, 3];
  #assert ob_publish(o[..], 4, tagged[..], 2, 4).relays == ob_mask("0 2 3");
  #assert ob_publish(o[..], 4, tagged[..], 1, 4).relays == ob_mask("0 2");
  #assert ob_publish(o[..], 4, tagged[..], 2, 2).error == OB_TOO_MANY;
  p := ob_cover(o[..], follows[..], 2, 8);
  relays := [5] int.[1, 4, 0, 2, 3]; asks := [5] string.["0 1 2", "3", "0", "1", "2"];
  #assert p.count == 5;
  for k: 0..<5 { #assert p.relay[k] == relays[k] && p.asks[k] == ob_mask(asks[k]); }
}

// The smallest number of relays that puts every author with a write relay
// on at least one, and how many subsets of the pool do so, by trying every
// subset. The whole pool always covers, so the search for k ends.
Cover :: struct { size: int; covers: int; }
smallest_cover :: (o: [] Outbox, authors: int) -> Cover {
  fits: [65] bool;   // fits[k]: some subset of k relays covers everyone
  c: Cover;
  for subset: 0..<(1 << o[0].relays) {
    ok := true;
    for a: 0..<authors {
      w := o[0].author[a].write;
      if w != 0 && (w & cast(u64) subset) == 0 { ok = false; }
    }
    if ok { fits[ob_count(cast(u64) subset)] = true; c.covers += 1; }
  }
  while !fits[c.size] { c.size += 1; }
  return c;
}
#test "one relay per author: greedy is as small as the best subset" #steps 2000000000 {
  o: [1] Outbox; read_lists(o[..], false); follows := [4] int.[0, 1, 2, 3];
  // Covers of the five relays: r5 for dave, then r2 with any of r1, r3, r4
  // (8), or r1, r3 and r4 without r2 (1): 9.
  c := smallest_cover(o[..], 4); #assert c.size == 2 && c.covers == 9 && ob_cover(o[..], follows[..], 1, 8).count == 2;
  f: [1] Outbox; fifty(f[..]); all := fifty_follows();
  #assert smallest_cover(f[..], 50).size == 3 && ob_cover(f[..], all[..], 1, 8).count == 3;
  // The search itself: nobody to cover needs no relay; two authors on two
  // different relays need both.
  e: [1] Outbox; #assert ob_author(e[..], ob_key(1), ob_list("[[\"r\",\"wss://a.example\",\"read\"]]")) == 0 && smallest_cover(e[..], 1).size == 0;
  #assert ob_author(e[..], ob_key(2), ob_list("[[\"r\",\"wss://b.example\",\"write\"]]")) == 1 && smallest_cover(e[..], 2).size == 1;
  #assert ob_author(e[..], ob_key(1), ob_list("[[\"r\",\"wss://a.example\",\"write\"]]")) == 0 && smallest_cover(e[..], 2).size == 2;
}

// The fifty-author table, from the rule that built the lists: author i
// writes to big (i < 30), east (20 <= i < 40), west (35 <= i < 48) and
// s(i mod 5) (i < 48). Pick k's authors are worked out in the chapter.
in_s :: (i: int, k: int) -> bool { return i < 48 && i % 5 == k && (i < 20 || (i >= 30 && i < 35) || i >= 40); }
#test "fifty authors: the plan follows the hand table" #steps 2000000000 {
  f: [1] Outbox; fifty(f[..]); all := fifty_follows();
  p := ob_cover(f[..], all[..], 2, 8);
  #assert p.count == 8;
  order := [8] int.[0, 2, 1, 3, 4, 5, 6, 7];   // big, west, east, s0-s4
  for k: 0..<8 {
    #assert p.relay[k] == order[k];
    for i: 0..<50 {
      want := false;
      if k == 0 { want = i < 30; } else if k == 1 { want = i >= 35 && i < 48; } else if k == 2 { want = i >= 20 && i < 40; } else { want = in_s(i, k - 3); }
      #assert ((p.asks[k] & (cast(u64) 1 << i)) != 0) == want;
    }
  }
  for i: 0..<50 { #assert p.got[i] == ifx i < 48 then 2 else 0; }
  q := ob_cover(f[..], all[..], 2, 4);
  #assert q.count == 4;
  ones := 0; for i: 0..<50 { if q.got[i] == 1 { ones += 1; } }
  #assert ones == 26 && q.got[48] == 0 && q.got[49] == 0 && q.got[0] == 2 && q.got[1] == 1 && q.got[20] == 2 && q.got[30] == 2 && q.got[31] == 1;
}

// The merge's trace, worked out by hand in the chapter. Fresh and duplicate
// counts are recounted from the script: distinct ids among the events, and
// the rest.
HAND_TRACE :: "n.n.n.e.d.e.d.n.e.n.e.0.1c-cdcnc";
#test "the scripted relays merge to one copy per id" #steps 2000000000 {
  o: [1] Outbox; read_lists(o[..], false); follows := [4] int.[0, 1, 2, 3];
  p := ob_cover(o[..], follows[..], 2, 8);
  chosen: u64 = 0; for k: 0..<p.count { chosen |= cast(u64) 1 << p.relay[k]; }
  t := merge(o[..], chosen, false);
  #assert cast(string) t.steps[0..<t.n] == HAND_TRACE;
  events := 0; distinct := 0; seen: [256] bool;
  for step: SCRIPT {
    if step.what != STEP_EVENT { continue; }
    events += 1;
    if !seen[step.id] { seen[step.id] = true; distinct += 1; }
  }
  #assert t.fresh == distinct && t.dups == events - distinct && distinct == 6 && events == 9;
}
```

To run them:

```shell
coax/bin/coax test nostr/examples/checks/outbox.coax --run-steps 250000000
```

On 2026-11-01 this reported 383 of 383 checks passed, including the
checks of every file the example loads.

### The library's own checks

[nip65.coax](../../coax/lib/nostr/nip65.coax) checks NIP-65's example list
byte for byte, skipped tags, duplicate URLs, escapes, the kind, every
refusal with its tag index, the 256-byte URL bound and the 16-relay bound.
[outbox.coax](../../coax/lib/nostr/outbox.coax) checks the pool and table
bounds, list replacement, publish and inbox sets, the cover on the
four-follow fixture and on small cases where a relay picked second gains
an author at level 2, and the merge: duplicates, unasked relays, late
`EOSE`, the deadline at its exact millisecond, restart, the all-zero id and
the 256-id window.

Both files were written test first. Root-only mutation on 2026-11-01:
`nip65.coax` 109 mutants, 109 killed; `outbox.coax` 377 mutants, 377
killed; none marked equivalent. Earlier runs found survivors that led to
code changes: a separate `N65_LIMIT` code (an over-long URL and an empty
URL were refused the same way), and a cover and merge loop shape in which
every comparison is observable. The `// book:` markers were added to both
files after those runs; they are comments and change no mutation site.
Root-only mutation of the checks file: 44 mutants, 44 killed, after the
subset search was changed to also count covering subsets.
Mutation of the loaded files (`tags.coax`, `relay_uri.coax`, `json.coax`)
was not rerun: none of them changed.

## Summary

- A kind 10002 event's `r` tags list relays; no marker means read and write.
  `nip65_read` refuses malformed tags, bad markers, URLs `relay_uri`
  refuses, and more than 16 relays.
- Publish to your write relays and every tagged user's read relays;
  `ob_publish` bounds both per author and in total and reports, never
  drops.
- Read an author from their write relays. `ob_cover` covers every follow
  once before any twice, with a bound per author and in total.
- `ob_sub_*` merges one subscription across relays: one copy per id, caught
  up when every relay has sent `EOSE`, closed or timed out.

## Dependencies

- [30. Subscriptions and EOSE](ch030-subscriptions-and-eose.md)
- [31. Publish and wait for OK](ch031-publish-and-wait-for-ok.md)

## Sources

- [NIP-65](../nips/official/65.md), relay list metadata: the kind, the
  markers, where to read and where to publish.
- [NIP-01](../nips/official/01.md), `REQ`, `EOSE`, `CLOSED` and replaceable
  events.
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
