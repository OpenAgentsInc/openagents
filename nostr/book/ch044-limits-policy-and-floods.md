# 44. Limits, policy and floods

A public relay is attacked. This chapter collects every limit and policy in
one place and drives each past its bound.

Part IX, A relay. Result: Per-connection and global limits, rate limits with
`rate-limited:` reasons, storage caps, slowloris timeouts and a write policy
that is declared, not implicit.

## What this chapter teaches

- Every limit the Coax relay enforces, in one table: the value, the code
  that enforces it, and the exact refusal a client receives.
- Rate limits in fixed windows: how many `EVENT`s and `REQ`s one connection
  may send in a minute, and how many `EVENT`s one key may publish across all
  connections, with `rate-limited:` reasons.
- How limits interact under a flood: which one a message hits first, and
  why a flood on one connection doesn't take another connection's budget.
- Slow clients: the handshake and frame deadlines that free connection
  slots, and why an idle WebSocket is kept.
- The write policy: who may write which events, stated in one procedure.
- Advertising the rate limits in the NIP-11 document from the same value the
  decisions use.

Chapter 37 already hit each NIP-11 `limitation` field at its value and one
past it. This chapter doesn't repeat those edges. It sends many messages at
once and shows what the relay does when each budget runs out.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [relay_rate.coax](../../coax/lib/nostr/relay_rate.coax) | `rt_take` | one message against one fixed window (`#authority`, new in this chapter) |
| | `rt_event`, `rt_req`, `rt_key_slot` | an `EVENT`'s connection and key windows, a `REQ`'s connection window, and the table of keys |
| | `RtConfig`, `rt_default`, `rt_config_ok` | the configurable limits: 120 `EVENT`s and 120 `REQ`s per connection and 60 `EVENT`s per key, a minute each |
| | `rt_prefix`, `rt_text`, `rt_nip11` | the `rate-limited:` reasons, and the limits written into the NIP-11 document |
| [relay_server.coax](../../coax/lib/nostr/relay_server.coax) | `RS_MAX_*`, `rs_req`, `rs_sub_open`, `rs_admit_policy` | the R1 limits: frames, subscriptions, filters, query cost, tags, `created_at` (`#authority`) |
| [relay_route.coax](../../coax/lib/nostr/relay_route.coax) | `rr_free_slot`, `rr_deadline`, `rr_frame_read`, `rr_message` | connection slots, deadlines, frame refusals and the `NOTICE` or `CLOSED` choice (`#authority`) |
| [store_core.coax](../../coax/lib/nostr/store_core.coax) | `RSD_MAX`, `RSD_ARENA`, `ns_store_admit` | the store's record and byte caps (`#authority`) |
| [relay_poll_budget.coax](../../coax/lib/net/relay_poll_budget.coax) | `rcp_step` | the client side: a bound on the work one poll may do (`#authority`) |
| [relay.coax](../../coax/relay/relay.coax) | `ry_serve`, `ry_req`, `ry_event` | the R1 relay program, which calls the procedures above |

`relay.coax` calls `relay_rate.coax`: each connection has an `RtConn`, the
relay one `RtKeys`, `ry_event` charges an EVENT once its signature holds
(`rt_event`), `ry_req` charges a REQ before `rs_req` (`rt_req`), and the
served document is `rt_nip11(RR_NIP11, ...)`. The limits are the relay's
`--rate EVENTS,REQS,KEY_EVENTS` flag, or `rt_default()` without it.

## Every limit in one table

The relay checks a message from the outside in: the connection, then the
frame, then the message, then the policy, then the store. Each row's
refusal is the exact text the relay sends.

| Limit | Value | Enforced by | What a client receives past it |
| --- | --- | --- | --- |
| Connections | 16 slots (`RY_CONNS`) | `rr_free_slot` | the socket is closed without a `101` |
| HTTP head | 10 s from accept (`RR_HEAD_S`) | `rr_deadline` | the socket is dropped |
| Part of a frame held | 30 s with no complete frame (`RR_FRAME_S`) | `rr_deadline` | close 1008 |
| Frame or message | 131,072 bytes (`RS_MAX_MESSAGE`) | `wss_frame_read`, `ws_take`, `rr_frame_read` | close 1009, before any payload byte |
| Subscription id | 1 to 64 characters (`MS_SUB_MAX`) | `ms_read`, `rr_message` | `NOTICE` `invalid: subscription id must contain 1 to 64 characters` |
| Filters per `REQ` | 16 (`RS_MAX_FILTERS`) | `ms_read`, `rr_message` | `CLOSED` `restricted: too many filters` |
| Query cost | 100,000 (`RS_MAX_COST`) | `rs_req`, `rs_cost` | `CLOSED` `restricted: query cost exceeds the configured limit` |
| A filter's decoded text | 8,192 bytes | `filter_read` in `rs_req` | `CLOSED` `invalid: malformed filter` |
| A subscription's filters | 8,192 bytes (`RS_SUB_TEXT`) | `rs_sub_open` | `CLOSED` `restricted: subscription filters exceed 8192 bytes` |
| Subscriptions | 32 per connection (`RS_MAX_SUBS`) | `rs_sub_open` | `CLOSED` `restricted: too many active subscriptions` |
| Events per filter | 127 (`RS_MAX_LIMIT`) | `rs_limit` | none: the `limit` is clamped |
| Tags | 256 (`RS_MAX_TAGS`) | `rs_admit_policy` | `OK false` `invalid: event has too many tags` |
| `created_at` ahead | 900 s (`RS_FUTURE`) | `rs_admit_policy` | `OK false` `invalid: the event's created_at timestamp is too far in the past or the future for this relay` |
| Stored events | 2,048 records, 4 MiB (`RSD_MAX`, `RSD_ARENA`) | `ns_store_admit` | `OK false` `error: relay storage is full` |
| `EVENT`s per connection | 120 a minute | `rt_event` | `OK false` `rate-limited: event rate exceeded` |
| `EVENT`s per key | 60 a minute | `rt_event` | `OK false` `rate-limited: event rate exceeded for this key` |
| Keys tracked | 64 with open windows (`RT_KEYS`) | `rt_key_slot` | `OK false` `rate-limited: too many keys are publishing; try again later` |
| `REQ`s per connection | 120 a minute | `rt_req` | `CLOSED` `rate-limited: REQ rate exceeded` |
| Output to a slow reader | 2 s to flush (`RY_IO_MS`) | `cxsrv_flush` in the C host | the connection is closed |

The R1 values are the Rust relay's (`crates/nostr-relay` at `0b401561a0^`),
as `relay_server.coax`'s header records. The rate limits are the Rust
relay's defaults too: `NOSTR_RELAY_RATE_EVENTS_PER_MIN_IP` 120,
`NOSTR_RELAY_RATE_EVENTS_PER_MIN_PUBKEY` 60 and
`NOSTR_RELAY_RATE_REQ_PER_MIN_IP` 120 in its `gateway/config.rs`.

NIP-01 names the machine-readable prefixes. `rate-limited:` tells a client
to slow down and try again. `restricted:` and `invalid:` tell it that the
same message will fail again. `error:` reports a relay-side problem.

## Rate limits in fixed windows

The rate rule is the Rust relay's `allow_counter` (`gateway/rate.rs`). A
counter has a start time and a count:

1. If the counter has no window open, or its window started `RT_WINDOW`
   (60) or more seconds ago, a new window starts now with a count of 0.
2. If the count has reached the limit, the message is refused and isn't
   counted.
3. Otherwise the count goes up by one and the message passes.

`rt_take` is that rule. A clock that went backwards also starts a new
window, so a corrected clock never locks a client out for longer than a
minute.

<!-- source: coax/lib/nostr/relay_rate.coax take -->
```jai
// Whether w no longer counts at now: never opened, RT_WINDOW seconds old,
// or opened after now (the clock went back).
rt_window_ended :: (w: RtWindow, now: int) -> bool {
  return w.count == 0 || now - w.started >= RT_WINDOW || now < w.started;
}
// One message at now against limit: true lets it through and counts it.
rt_take :: (w: [] RtWindow, limit: int, now: int) -> bool {
  if rt_window_ended(w[0], now) { w[0].started = now; w[0].count = 0; }
  if w[0].count >= limit { return false; }
  w[0].count += 1;
  return true;
}
```

A connection has two windows, one for `EVENT`s and one for `REQ`s. A key has
one window, shared by every connection that sends its events. `rt_event`
charges the connection first. Only when the connection's window passes the
event does it charge the key, so a connection that is over its own budget
can't use up a key's. The relay checks an `EVENT`'s rate before its
signature: verification costs about 145 ms per event in release on the R1
host (the flood record below), so a flood of bad signatures must spend the
same budget as good events.

The Rust relay counts per IP address. The C host gives the relay no peer
address, so a connection stands in for an address here. With 16 connection
slots, one client can hold every slot and send 16 × 120 events a minute;
per-address limits need the host to report the peer, which it doesn't yet.

The key table is bounded at `RT_KEYS` (64) entries, as the Rust relay
bounds its maps at `MAX_RATE_KEYS`. A new key takes the first entry whose
window has ended. When all 64 windows are open, a new key is refused. A
flood of new keys therefore can't grow the relay's memory; it can only make
new keys wait up to a minute.

A fixed window allows a burst at its edge. A connection whose window opened
at 1000 may send 120 events at 1059 and 120 more at 1060, 240 in two
seconds. A sliding window would prevent that at the cost of remembering
every message's time. The Rust relay accepted the edge burst, and so does
this one.

### Worked example: a budget over a burst, by hand

Take a small configuration: 3 `EVENT`s and 2 `REQ`s per connection, and 2
`EVENT`s per key, a minute each. Alice and Bob send twelve messages on one
connection. Work out each verdict from the three rules before the program
runs:

| # | t | Message | Connection window | Key window | Verdict |
| --- | --- | --- | --- | --- | --- |
| 1 | 1000 | `EVENT` Alice | none open: opens at 1000, count 1 | Alice: opens at 1000, count 1 | `OK true` |
| 2 | 1000 | `EVENT` Alice | count 2 | Alice: count 2 | `OK true` |
| 3 | 1001 | `EVENT` Alice | count 3 | Alice: 2 is her limit | `OK false` `rate-limited: event rate exceeded for this key` |
| 4 | 1002 | `EVENT` Bob | 3 is the limit | Bob isn't charged | `OK false` `rate-limited: event rate exceeded` |
| 5 | 1002 | `REQ` | `REQ` window opens at 1002, count 1 | | passes to the `REQ` rules |
| 6 | 1003 | `REQ` | count 2 | | passes |
| 7 | 1004 | `REQ` | 2 is the limit | | `CLOSED` `rate-limited: REQ rate exceeded` |
| 8 | 1059 | `EVENT` Bob | 1059 − 1000 = 59 < 60: still full | Bob isn't charged | `OK false` `rate-limited: event rate exceeded` |
| 9 | 1060 | `EVENT` Bob | 60 ≥ 60: opens at 1060, count 1 | Bob: opens at 1060, count 1 | `OK true` |
| 10 | 1060 | `EVENT` Alice | count 2 | Alice: 1060 − 1000 = 60: opens at 1060, count 1 | `OK true` |
| 11 | 1061 | `REQ` | 1061 − 1002 = 59: still full | | `CLOSED` `rate-limited: REQ rate exceeded` |
| 12 | 1062 | `REQ` | 60: opens at 1062, count 1 | | passes |

Three details follow from the rules:

- Message 3 is refused by Alice's window, but the connection still counts
  it, because the connection is charged first. That is why Bob's message 4
  finds the connection full.
- The `EVENT` and `REQ` windows are separate. The `REQ` window opened at
  1002, so it ends at 1062, not 1060.
- A window ends exactly 60 seconds after it opened: second 1059 is in it,
  second 1060 isn't.

The example prints the same twelve verdicts:

```text
  1  t=1000  EVENT Alice: OK true   (connection 1 of 3 since 1000, Alice 1 of 2)
  2  t=1000  EVENT Alice: OK true   (connection 2 of 3 since 1000, Alice 2 of 2)
  3  t=1001  EVENT Alice: OK false "rate-limited: event rate exceeded for this key"   (connection 3 of 3 since 1000, Alice 2 of 2)
  4  t=1002  EVENT Bob: OK false "rate-limited: event rate exceeded"   (connection 3 of 3 since 1000, Bob 0 of 2)
  5  t=1002  REQ: passes to the REQ rules   (REQs 1 of 2 since 1002)
  6  t=1003  REQ: passes to the REQ rules   (REQs 2 of 2 since 1002)
  7  t=1004  REQ: CLOSED "rate-limited: REQ rate exceeded"   (REQs 2 of 2 since 1002)
  8  t=1059  EVENT Bob: OK false "rate-limited: event rate exceeded"   (connection 3 of 3 since 1000, Bob 0 of 2)
  9  t=1060  EVENT Bob: OK true   (connection 1 of 3 since 1060, Bob 1 of 2)
  10  t=1060  EVENT Alice: OK true   (connection 2 of 3 since 1060, Alice 1 of 2)
  11  t=1061  REQ: CLOSED "rate-limited: REQ rate exceeded"   (REQs 2 of 2 since 1002)
  12  t=1062  REQ: passes to the REQ rules   (REQs 1 of 2 since 1062)
```

## Floods

Each flood below runs offline with a fixed clock, through the same
procedures `relay.coax` calls. The counts are worked out from the table of
limits.

### Many `EVENT`s

At the defaults, one connection that sends 130 events in one second from 13
keys (10 each) gets 120 `OK true` and 10 refusals. Each key is far below its
60, so the connection's window refuses: the first refusal is event 121,
`rate-limited: event rate exceeded`. One second before the window ends
(t = 1059) the connection is still refused; at t = 1060 it passes.

One key that publishes through five connections, 14 events each, gets 60
`OK true` and 10 refusals. No connection reaches 120, so the key's window
refuses: the first refusal is event 61, `rate-limited: event rate exceeded
for this key`.

Sixty-four new keys in one second fill the key table. The 65th key is
refused with `rate-limited: too many keys are publishing; try again later`
until the first windows end at t = 1060.

### Fairness between connections

Each connection's windows are its own. While connection A is refused, a
fresh connection B's first event passes. The one budget two connections
share is a key's: Alice can't publish more than 60 events a minute by
spreading them over connections.

The R1 relay serves connections in turn: each pass of `ry_serve` reads at
most one transport read (65,536 bytes) from each connection, then moves on.
A connection that pipelines many messages is answered one read at a time,
so it can't hold the loop while others wait for more than one read's work.

### Many `REQ`s

One connection sends 125 `REQ`s in one second, each with a new subscription
id. The relay applies three limits in order: the `REQ` rate, then the
filter rules, then the subscription table:

- `REQ`s 1 to 32 open a subscription each.
- `REQ`s 33 to 120 pass the rate but find all 32 slots taken: 88 `CLOSED`
  `restricted: too many active subscriptions`.
- `REQ`s 121 to 125 are over the rate: 5 `CLOSED` `rate-limited: REQ rate
  exceeded`. The relay doesn't read their filters.

A `REQ` that reuses a live subscription id replaces that subscription and
takes no new slot, but it is still a `REQ`, so it counts against the rate.
`CLOSE` frees a slot and isn't rate-limited.

### A huge filter

A single `REQ` can try to cost the relay as much work as possible. Each of
these is one message on a connection with free slots:

| `REQ` | Size | Refusal |
| --- | --- | --- |
| 17 empty filters | 50 bytes of filters | `CLOSED` `restricted: too many filters` |
| 8 empty filters | 8 × (127 + 0) × 100 = 101,600 > 100,000 | `CLOSED` `restricted: query cost exceeds the configured limit` |
| 7 empty filters | 7 × 12,700 = 88,900 | opens |
| 2 filters of 64 ids | 8,595 bytes > 8,192 | `CLOSED` `restricted: subscription filters exceed 8192 bytes` |
| 1 filter with an 8,200-byte `#t` value | 8,200 > 8,192 decoded bytes | `CLOSED` `invalid: malformed filter` |
| a 65-character sub id | | `NOTICE` `invalid: subscription id must contain 1 to 64 characters` |

The query cost is `relay_server.coax`'s `rs_cost`: an empty filter has
factor 100 and the clamped limit 127. The last two size rows are different limits.
`filter_read` decodes each filter into 8,192 bytes of space, so a single
value larger than that doesn't read at all. Filters that read but together
take more than the 8,192 bytes a subscription keeps are refused by
`rs_sub_open`.

### Frames

A frame's header states its length before any payload arrives. A masked
text frame whose header claims 131,073 bytes gets `WS_FRAME_TOO_BIG` from
`wss_frame_read`, and `rr_frame_read` closes with 1009 at once. A claim of
2^40 bytes is refused the same way, from its first length bytes. A claim of
exactly 131,072 bytes waits for the payload. So a client can't make the
relay buffer more than one frame and one read: `RY_IN` is `RS_MAX_MESSAGE +
14 + 65536` bytes per connection.

### Connections and slow clients

Sixteen connections take all 16 slots. `rr_free_slot` gives the 17th −1,
and the relay closes it without a `101`. Slots come back through
`rr_deadline`, which the relay calls for every connection at most a second
apart:

| Connection | At t = 1000 + 9 or 29 | At t = 1000 + 10 or 30 |
| --- | --- | --- |
| HTTP head not finished (accepted at 1000) | keep at 1009 | drop at 1010 |
| part of a frame held (last frame at 1000) | keep at 1029 | close 1008 at 1030 |
| idle WebSocket, nothing buffered | keep, however long | keep |

An idle WebSocket is kept because a Pylon waits for jobs that way. So
sixteen idle but valid connections still hold every slot; no deadline can
tell them from honest clients. Per-address connection limits would, and
they need the peer address from the host.

A slow reader is a client that sends requests but doesn't read the
answers. The relay writes each answer with `cxsrv_flush(h, RY_IO_MS)`. When
the client's socket doesn't drain within 2 s, the flush fails and the relay
closes the connection. That path is in the C host, so the offline example
doesn't drive it; the flood record below computes its bound, 16 × 4 MiB of
pending output, from the code.

### Storage

The store has fixed caps: `RSD_MAX` (2,048) records and `RSD_ARENA` (4 MiB)
of event bytes. Past either, `ns_store_admit` answers `NSS_FULL`, which is
the relay's `RS_FULL`: `OK false` `error: relay storage is full`. The
example uses a store with room for 4 records, so the fifth event is refused.
The prefix is `error:`, not `rate-limited:`: waiting doesn't help until an
operator makes room.

## The write policy

The R1 relay's write policy is declared in two places, and nowhere else:

- `rs_admit_policy` in `relay_server.coax`, in this order: kind 22242 is
  never stored; more than 256 tags is refused; `created_at` more than 900 s
  ahead is refused; an expired event is refused; and when writes need
  authentication, an unauthenticated connection is refused with
  `auth-required:`.
- `RR_AUTH_WRITES` in `relay_route.coax`: `EVENT` needs at least one
  NIP-42 key on the connection; `REQ` and `CLOSE` never do. NIP-11 states
  this as `auth_required: false` and `restricted_writes: true` (chapter 37).

Any authenticated key may publish events signed by any author, as the Rust
relay allowed. There is no per-kind table of who may write what; the rules
above apply to every kind.

## Advertising the rate limits

NIP-11 has no standard rate-limit fields. Clients must ignore fields they
don't know, so `rt_nip11` adds three of this relay's own to the
`limitation` object: `events_per_minute`, `reqs_per_minute` and
`events_per_minute_per_key`. It writes them from the same `RtConfig` that
`rt_event` and `rt_req` take, so a change to the configuration changes the
document and the decisions together. A configuration with a limit below 1
or above 1,000,000 is refused, and no document is written:

<!-- source: coax/lib/nostr/relay_rate.coax nip11 -->
```jai
// The relay's document base with cfg's three limits added to its
// limitation object: the length written, or -1. The limitation object must
// be the document's last field and flat (numbers and booleans, as NIP-11's
// are): base ends in "}}" and holds "\"limitation\":{" with no brace after
// it but those two. An empty object gets no leading comma.
RT_LIMITATION :: "\"limitation\":{";
rt_nip11 :: (base: string, cfg: RtConfig, out: [] u8) -> int {
  if !rt_config_ok(cfg) || !rt_ends(base, "}}") { return -1; }
  body := base[0..<base.count - 2];
  // the last "limitation":{ in the body
  at := body.count - RT_LIMITATION.count;
  while at >= 0 && body[at..<at + RT_LIMITATION.count] != RT_LIMITATION { at -= 1; }
  if at < 0 { return -1; }
  at += RT_LIMITATION.count;
  for i: at..<body.count { if body[i] == '{' || body[i] == '}' { return -1; } }
  k := rt_raw(out, 0, body);
  if body.count > at { k = rt_raw(out, k, ","); }
  k = rt_raw(out, k, "\"events_per_minute\":"); k = rt_decimal(out, k, cfg.events);
  k = rt_raw(out, k, ",\"reqs_per_minute\":"); k = rt_decimal(out, k, cfg.reqs);
  k = rt_raw(out, k, ",\"events_per_minute_per_key\":"); k = rt_decimal(out, k, cfg.key_events);
  return rt_raw(out, k, "}}");
}
```

## The client's own budget

The client side has a work bound too. `relay_conn_poll_budget` lets a
caller cap one poll at `max_steps` frame scans and reads. `rcp_step(used,
max_steps)` admits a step only while `used < max_steps`, with `max_steps`
between 1 and 65,536. When the budget is spent, the client closes the
connection and returns `RC_FAIL`, never an empty result that looks like no
data. A relay that floods a client therefore can't keep one poll running.
The example runs a budget of 3 over a burst of 5 frames: steps 1 to 3 scan,
step 4 closes.

## The example

The example is
[nostr/examples/relay-limits.coax](../examples/relay-limits.coax). It loads
`relay_server.coax`, `relay_route.coax`, `relay_rate.coax`,
`store_core.coax` and `relay_poll_budget.coax`, with no sockets and no
signer. It:

1. runs the worked trace;
2. floods `EVENT`s and `REQ`s at the defaults and prints the counts and the
   first refusal of each kind;
3. sends each huge `REQ` and each oversized frame header;
4. fills the connection slots and walks the deadlines;
5. fills a four-record store;
6. runs the client's poll budget;
7. writes the NIP-11 document with the rate limits, then refuses two broken
   configurations.

The trace:

<!-- source: nostr/examples/relay-limits.coax trace -->
```jai
// The chapter's hand trace: a configuration of 3 EVENTs and 2 REQs per
// connection and 2 EVENTs per key, a minute each, and eleven messages on
// one connection from Alice and Bob.
Step :: struct { t: int; verb: string; who: int; }   // who: 0 Alice, 1 Bob (EVENT only)
st :: (t: int, verb: string, who: int) -> Step { return Step.{t = t, verb = verb, who = who}; }
// The count in the window of the key whose first byte is Alice's (0xa1) or
// Bob's (0xb0), without changing the table; 0 when it has no slot.
key_count :: (k: [] RtKeys, who: int) -> int {
  tag := ifx who == 0 then 0xa1 else 0xb0;
  for i: 0..<k[0].n { if k[0].key[32 * i] == cast(u8) tag { return k[0].w[i].count; } }
  return 0;
}
trace :: () {
  cfg := RtConfig.{events = 3, reqs = 2, key_events = 2};
  c: [1] RtConn; k: [1] RtKeys;
  keys: [2] [32] u8; keys[0] = key_of(0xa1, 0); keys[1] = key_of(0xb0, 0);
  names := [2] string.["Alice", "Bob"];
  steps := [12] Step.[
    st(1000, "EVENT", 0), st(1000, "EVENT", 0), st(1001, "EVENT", 0), st(1002, "EVENT", 1),
    st(1002, "REQ", 0), st(1003, "REQ", 0), st(1004, "REQ", 0), st(1059, "EVENT", 1),
    st(1060, "EVENT", 1), st(1060, "EVENT", 0), st(1061, "REQ", 0), st(1062, "REQ", 0)];
  for s, i: steps {
    if s.verb == "EVENT" {
      r := rt_event(c[..], k[..], cfg, keys[s.who][..], s.t);
      print("  %  t=%  EVENT %: %", i + 1, s.t, names[s.who], rt_name(r));
      if r != RT_OK { print(" \"%: %\"", rt_prefix(r), rt_text(r)); }
      print("   (connection % of 3 since %, % % of 2)\n", c[0].events.count, c[0].events.started, names[s.who], key_count(k[..], s.who));
    } else {
      r := rt_req(c[..], cfg, s.t);
      print("  %  t=%  REQ: %", i + 1, s.t, ifx r == RT_OK then "passes to the REQ rules" else "CLOSED");
      if r != RT_OK { print(" \"%: %\"", rt_prefix(r), rt_text(r)); }
      print("   (REQs % of 2 since %)\n", c[0].reqs.count, c[0].reqs.started);
    }
  }
}
```

The `EVENT` floods:

<!-- source: nostr/examples/relay-limits.coax event_flood -->
```jai
// EVENT floods at the relay's defaults (120 a minute per connection, 60 per
// key). Each EVENT is charged before its signature is checked, so a flood
// of bad signatures costs the same budget as good ones.
event_flood :: () {
  cfg := rt_default(); out: [256] u8; id: [32] u8;
  for i: 0..<32 { id[i] = 0x11; }

  // One connection, 130 EVENTs in one second from 13 keys, 10 each.
  a: [1] RtConn; k: [1] RtKeys; ok := 0; first := -1; last := RT_OK;
  for i: 0..<130 {
    r := send_event(a[..], k[..], cfg, 0xc0, i % 13, NOW);
    if r == RT_OK { ok += 1; } else if first < 0 { first = i + 1; last = r; }
  }
  print("one connection, 130 EVENTs at t=% from 13 keys: % OK true, % refused, the first at #%\n", NOW, ok, 130 - ok, first);
  print("  %\n", ok_line(id[..], last, out[..]));

  // The same connection at t=NOW+59 and t=NOW+60.
  print("  at t=%: %; at t=%: %\n", NOW + 59, rt_name(send_event(a[..], k[..], cfg, 0xc0, 0, NOW + 59)),
    NOW + 60, rt_name(send_event(a[..], k[..], cfg, 0xc0, 0, NOW + 60)));

  // One key on five connections, 14 EVENTs each: the key's window is shared.
  k2: [1] RtKeys; conns: [5] RtConn; ok = 0; first = -1;
  for c: 0..<5 {
    for e: 0..<14 {
      r := send_event(conns[c..<c + 1], k2[..], cfg, 0xd0, 0, NOW);
      if r == RT_OK { ok += 1; } else if first < 0 { first = c * 14 + e + 1; last = r; }
    }
  }
  print("one key on 5 connections, 70 EVENTs at t=%: % OK true, % refused, the first at #%\n", NOW, ok, 70 - ok, first);
  print("  %\n", ok_line(id[..], last, out[..]));

  // Fairness: while connection A is refused, connection B is not.
  b: [1] RtConn;
  print("connection A again: %; a fresh connection B, a new key: %\n",
    rt_name(send_event(a[..], k[..], cfg, 0xc0, 99, NOW + 60)), rt_name(send_event(b[..], k[..], cfg, 0xe0, 0, NOW + 60)));

  // The key table: RT_KEYS keys with open windows, then one more.
  k3: [1] RtKeys; c3: [1] RtConn; ok = 0;
  for i: 0..<RT_KEYS { if send_event(c3[..], k3[..], cfg, 0xf0, i, NOW) == RT_OK { ok += 1; } }
  r := send_event(c3[..], k3[..], cfg, 0xf1, 0, NOW);
  print("% new keys at t=% on one connection: % OK true; key % at t=%: %\n", RT_KEYS, NOW, ok, RT_KEYS + 1, NOW, rt_name(r));
  print("  %\n", ok_line(id[..], r, out[..]));
  print("  key % at t=%, when the first windows ended: %\n", RT_KEYS + 1, NOW + 60, rt_name(send_event(c3[..], k3[..], cfg, 0xf1, 0, NOW + 60)));
}
```

The `REQ` flood, in the relay's order:

<!-- source: nostr/examples/relay-limits.coax req_flood -->
```jai
// 125 REQs in one second on one connection, each with a new subscription
// id, through the relay's order: the REQ rate first, then rs_req's filter
// rules, then rs_sub_open's 32 subscriptions.
req_flood :: (subs: [] RsSub) {
  cfg := rt_default(); c: [1] RtConn;
  q: [256] u8; d: [8] u8; out: [256] u8;
  open := 0; full := 0; rate := 0; first_full := -1; first_rate := -1;
  last_full: string; last_rate: string;
  for i: 1..<126 {
    sid: [8] u8; sid[0] = 'q'; s := num(i, d[..]); for ch, j: s { sid[1 + j] = ch; }
    sub := cast(string) sid[0..<1 + s.count];
    t := req_text(sub, "{\"kinds\":[1]}", q[..]);
    m := ms_read(t);
    if rt_req(c[..], cfg, NOW) != RT_OK {
      rate += 1; if first_rate < 0 { first_rate = i; }
      n := ms_write_closed(sub, rt_prefix(RT_REQ_RATE), rt_text(RT_REQ_RATE), out[..]);
      if i == 121 { print("  #121: %\n", cast(string) out[0..<n]); }
      continue;
    }
    if rs_req(m, t) != RS_ACCEPT { continue; }
    slot := rs_sub_open(subs, m, t, RS_MAX_SUBS);
    if slot >= 0 { open += 1; continue; }
    full += 1; if first_full < 0 { first_full = i; }
    if i == 33 {
      n := ms_write_closed(sub, rs_prefix(-slot), rs_text(-slot), out[..]);
      print("  #33: %\n", cast(string) out[0..<n]);
    }
  }
  print("one connection, 125 REQs at t=%: % opened (EOSE), % CLOSED from #% (32 subscriptions), % CLOSED from #% (REQ rate)\n",
    NOW, open, full, first_full, rate, first_rate);
}
```

The document, and the deliberately broken configurations:

<!-- source: nostr/examples/relay-limits.coax nip11 -->
```jai
// The relay's document with the rate limits added from the same RtConfig
// the decisions use, and a configuration that is refused.
advertise :: () {
  doc: [1024] u8;
  n := rt_nip11(RR_NIP11, rt_default(), doc[..]);
  at := 0;
  while cast(string) doc[at..<at + RT_LIMITATION.count] != RT_LIMITATION { at += 1; }
  print("% bytes (RR_NIP11 is %); its limitation object:\n  %\n", n, RR_NIP11.count, cast(string) doc[at..<n - 1]);
  bad := rt_default(); bad.reqs = 0;
  print("reqs_per_minute 0: rt_config_ok %, rt_nip11 %\n", rt_config_ok(bad), rt_nip11(RR_NIP11, bad, doc[..]));
  bad = rt_default(); bad.events = RT_LIMIT_MAX + 1;
  print("events_per_minute %: rt_config_ok %, rt_nip11 %\n", bad.events, rt_config_ok(bad), rt_nip11(RR_NIP11, bad, doc[..]));
}
```

Run it:

```shell
coax/bin/coax run nostr/examples/relay-limits.coax
```

Its output after the trace, in full (the book's checks compare it in debug
and release with
[nostr/fixtures/relay-limits.out](../fixtures/relay-limits.out)):

```text
== EVENT floods at the defaults: 120 per connection, 60 per key, 120 REQs, a minute each
one connection, 130 EVENTs at t=1000 from 13 keys: 120 OK true, 10 refused, the first at #121
  ["OK","1111111111111111111111111111111111111111111111111111111111111111",false,"rate-limited: event rate exceeded"]
  at t=1059: OK false; at t=1060: OK true
one key on 5 connections, 70 EVENTs at t=1000: 60 OK true, 10 refused, the first at #61
  ["OK","1111111111111111111111111111111111111111111111111111111111111111",false,"rate-limited: event rate exceeded for this key"]
connection A again: OK true; a fresh connection B, a new key: OK true
64 new keys at t=1000 on one connection: 64 OK true; key 65 at t=1000: OK false
  ["OK","1111111111111111111111111111111111111111111111111111111111111111",false,"rate-limited: too many keys are publishing; try again later"]
  key 65 at t=1060, when the first windows ended: OK true

== a REQ flood
  #33: ["CLOSED","q33","restricted: too many active subscriptions"]
  #121: ["CLOSED","q121","rate-limited: REQ rate exceeded"]
one connection, 125 REQs at t=1000: 32 opened (EOSE), 88 CLOSED from #33 (32 subscriptions), 5 CLOSED from #121 (REQ rate)

== huge REQs (on a connection with room)
17 filters: ["CLOSED","h1","restricted: too many filters"]
8 open filters: ["CLOSED","h2","restricted: query cost exceeds the configured limit"]
7 open filters: opened
two filters of 64 ids, 8595 bytes: ["CLOSED","h4","restricted: subscription filters exceed 8192 bytes"]
one filter with an 8200-byte value: ["CLOSED","h5","invalid: malformed filter"]
65-character sub id: ["NOTICE","invalid: subscription id must contain 1 to 64 characters"]

== frames
a text frame header claiming 131073 bytes: WS_FRAME_TOO_BIG, close 1009
one claiming 131072 bytes: WS_FRAME_MORE: waits for the payload
one claiming 2^40 bytes: WS_FRAME_TOO_BIG, close 1009

== connections and slow clients
16 connections hold every slot; the 17th: slot -1 (the relay closes it without a 101)
deadlines, accepted or last frame at t=1000:
  HTTP head unfinished:  t=1009: keep, t=1010: drop
  part of a frame held:  t=1029: keep, t=1030: close 1008
  idle WebSocket:        t=87400: keep

== storage
event 4: ["OK","0400000000000000000000000000000000000000000000000000000000000000",true,""]
event 5: ["OK","0500000000000000000000000000000000000000000000000000000000000000",false,"error: relay storage is full"]
the relay's store: 2048 records, 4194304 bytes

== the client's poll budget
  step 1: scan one frame
  step 2: scan one frame
  step 3: scan one frame
  step 4: budget spent: close, RC_FAIL
  step 5: budget spent: close, RC_FAIL

== NIP-11
594 bytes (RR_NIP11 is 517); its limitation object:
  "limitation":{"max_message_length":131072,"max_subscriptions":32,"max_limit":127,"max_subid_length":64,"max_event_tags":256,"auth_required":false,"restricted_writes":true,"created_at_upper_limit":900,"default_limit":127,"events_per_minute":120,"reqs_per_minute":120,"events_per_minute_per_key":60}
reqs_per_minute 0: rt_config_ok false, rt_nip11 -1
events_per_minute 1000001: rt_config_ok false, rt_nip11 -1
```

The two broken configurations are the deliberate refusals: `reqs_per_minute`
0 and `events_per_minute` 1,000,001 each make `rt_config_ok` false and
`rt_nip11` return −1, so no document advertises a limit the relay can't
enforce. The program exits with status 0.

## How the code is checked

### Checks in the example

The checks are in
[nostr/examples/checks/relay-limits.coax](../examples/checks/relay-limits.coax),
which loads the example:

- `rt_take` gives the same verdict as a transliteration of the Rust
  relay's `allow_counter` on 4,000 messages at pseudo-random times 0 to 8
  seconds apart, for limits 1 to 5, starting before second 60 so that the
  counter's creation time matters;
- the worked trace gives the twelve hand verdicts, and the windows end at
  the hand start times 1060 and 1062;
- the floods give the hand counts: 120 and 10, 60 and 10, 64 keys and a
  refused 65th, and 32, 88 and 5 for the `REQ` flood;
- the NIP-11 document equals chapter 37's hand-written document with the
  three fields written in by hand, 517 − 2 + 79 = 594 bytes, and is JSON;
- the defaults are the Rust relay's, and the R1 constants are the values
  in `relay_server.coax`'s header and the flood record.

<!-- source: nostr/examples/checks/relay-limits.coax check -->
```jai
// The Rust relay's counter (crates/nostr-relay at 0b401561a0^,
// gateway/rate.rs), transliterated: a counter is created at a key's first
// message (new_counter: started = now, count 0), and allow_counter resets it
// when started.elapsed() >= window, refuses at count >= limit, else counts.
RustCounter :: struct { exists: bool; started: int; count: int; }
rust_allow :: (c: [] RustCounter, limit: int, window: int, now: int) -> bool {
  if !c[0].exists { c[0] = RustCounter.{exists = true, started = now, count = 0}; }
  if now - c[0].started >= window { c[0].started = now; c[0].count = 0; }
  if c[0].count >= limit { return false; }
  c[0].count += 1;
  return true;
}
#test "rt_take gives the Rust relay's verdict on 4000 messages at irregular times" {
  seed := 12345;
  for limit: 1..<6 {
    w: [1] RtWindow; r: [1] RustCounter; now := 10;    // before second 60: the creation time matters
    for i: 0..<800 {
      seed = (seed * 1103515245 + 12345) % 2147483648;
      now += (seed / 65536) % 9;           // 0 to 8 seconds apart, often the same second
      #assert rt_take(w[..], limit, now) == rust_allow(r[..], limit, RT_WINDOW, now);
    }
  }
}
#test "the worked trace gives the hand verdicts" {
  // Worked out in the chapter before the program ran: 1 OK, 2 OK, 3 key,
  // 4 connection, 5 REQ ok, 6 REQ ok, 7 REQ refused, 8 connection, 9 OK,
  // 10 OK, 11 REQ refused, 12 REQ ok.
  cfg := RtConfig.{events = 3, reqs = 2, key_events = 2};
  c: [1] RtConn; k: [1] RtKeys; alice := key_of(0xa1, 0); bob := key_of(0xb0, 0);
  #assert rt_event(c[..], k[..], cfg, alice[..], 1000) == RT_OK && rt_event(c[..], k[..], cfg, alice[..], 1000) == RT_OK;
  #assert rt_event(c[..], k[..], cfg, alice[..], 1001) == RT_KEY_RATE && rt_event(c[..], k[..], cfg, bob[..], 1002) == RT_EVENT_RATE;
  #assert rt_req(c[..], cfg, 1002) == RT_OK && rt_req(c[..], cfg, 1003) == RT_OK && rt_req(c[..], cfg, 1004) == RT_REQ_RATE;
  #assert rt_event(c[..], k[..], cfg, bob[..], 1059) == RT_EVENT_RATE;
  #assert rt_event(c[..], k[..], cfg, bob[..], 1060) == RT_OK && rt_event(c[..], k[..], cfg, alice[..], 1060) == RT_OK;
  #assert rt_req(c[..], cfg, 1061) == RT_REQ_RATE && rt_req(c[..], cfg, 1062) == RT_OK;
  #assert c[0].events.started == 1060 && c[0].events.count == 2 && c[0].reqs.started == 1062;
}
#test "the floods give the hand counts" {
  cfg := rt_default();
  // 130 EVENTs from 13 keys: the connection's 120, then 10 refusals
  a: [1] RtConn; k: [1] RtKeys;
  for i: 0..<130 { r := send_event(a[..], k[..], cfg, 0xc0, i % 13, NOW); #assert (r == RT_OK) == (i < 120); #assert r == RT_OK || r == RT_EVENT_RATE; }
  // one key on 5 connections of 14: the key's 60, then 10 refusals
  k2: [1] RtKeys; conns: [5] RtConn;
  for i: 0..<70 { r := send_event(conns[i / 14..<i / 14 + 1], k2[..], cfg, 0xd0, 0, NOW); #assert (r == RT_OK) == (i < 60); #assert r == RT_OK || r == RT_KEY_RATE; }
  // RT_KEYS = 64 keys, then the 65th
  k3: [1] RtKeys; c3: [1] RtConn;
  for i: 0..<64 { #assert send_event(c3[..], k3[..], cfg, 0xf0, i, NOW) == RT_OK; }
  #assert send_event(c3[..], k3[..], cfg, 0xf1, 0, NOW) == RT_KEYS_FULL && send_event(c3[..], k3[..], cfg, 0xf1, 0, NOW + 60) == RT_OK;
  // 125 REQs: 32 open, 88 hit the subscription cap, 5 hit the REQ rate
  subs: [RS_MAX_SUBS] RsSub; c: [1] RtConn; q: [64] u8; d: [8] u8;
  open := 0; full := 0; rate := 0;
  for i: 1..<126 {
    sid: [8] u8; sid[0] = 'q'; s := num(i, d[..]); for ch, j: s { sid[1 + j] = ch; }
    t := req_text(cast(string) sid[0..<1 + s.count], "{\"kinds\":[1]}", q[..]);
    if rt_req(c[..], cfg, NOW) != RT_OK { rate += 1; continue; }
    if rs_sub_open(subs[..], ms_read(t), t, RS_MAX_SUBS) >= 0 { open += 1; } else { full += 1; }
  }
  #assert open == 32 && full == 88 && rate == 5;
}
// relay-info's HAND_DOC (chapter 37) with the three rate fields written in
// by hand before its closing "}}": 517 - 2 + 79 = 594 bytes.
HAND_DOC :: "{\"name\":\"OpenAgents Coax relay (R1)\",\"description\":\"A Nostr relay written in Coax. Tailnet preview; relay.openagents.com is unchanged. Publishing needs NIP-42 AUTH; reading is open.\",\"supported_nips\":[1,9,11,40,42,45,62,70],\"software\":\"https://github.com/OpenAgentsInc/openagents\",\"version\":\"r1\",\"limitation\":{\"max_message_length\":131072,\"max_subscriptions\":32,\"max_limit\":127,\"max_subid_length\":64,\"max_event_tags\":256,\"auth_required\":false,\"restricted_writes\":true,\"created_at_upper_limit\":900,\"default_limit\":127,\"events_per_minute\":120,\"reqs_per_minute\":120,\"events_per_minute_per_key\":60}}";
#test "the NIP-11 document is the hand-written one, and JSON" {
  doc: [1024] u8;
  n := rt_nip11(RR_NIP11, rt_default(), doc[..]);
  #assert HAND_DOC.count == 594 && n == 594 && cast(string) doc[0..<n] == HAND_DOC && json_valid(HAND_DOC);
  // the Rust relay's defaults (gateway/config.rs: RATE_EVENTS_PER_MIN_IP 120,
  // RATE_EVENTS_PER_MIN_PUBKEY 60, RATE_REQ_PER_MIN_IP 120)
  #assert rt_default().events == 120 && rt_default().key_events == 60 && rt_default().reqs == 120;
}
#test "the R1 limits are the Rust relay's, as relay_server.coax's header states" {
  #assert RS_MAX_MESSAGE == 131072 && RS_MAX_SUBS == 32 && RS_MAX_FILTERS == 16 && RS_MAX_LIMIT == 127;
  #assert RS_MAX_COST == 100000 && RS_MAX_TAGS == 256 && RS_FUTURE == 900 && RS_SUB_TEXT == 8192;
  #assert RR_HEAD_S == 10 && RR_FRAME_S == 30 && RR_CLOSE_SLOW == 1008 && RSD_MAX == 2048 && RSD_ARENA == 4194304;
  // 8 open filters cost 8 x (127 + 0) x 100 = 101600 > 100000; 7 cost 88900
  #assert 8 * 127 * 100 > RS_MAX_COST && 7 * 127 * 100 <= RS_MAX_COST;
}
```

To run them:

```shell
coax/bin/coax test nostr/examples/checks/relay-limits.coax
```

On 2026-11-01 `coax test` ran 323 checks, including the loaded units' own
checks, and all passed in 13 s. `coax mutate --root-only` on the checks
file killed 24 of 24 mutants (the first run left 2 alive in the Rust
transliteration's counter creation; starting the sequence at second 10
killed them).

### The library change

`coax/lib/nostr/relay_rate.coax` is new, `#authority`, and loads nothing.
Its seven checks were written first. On 2026-11-01:

- `coax test coax/lib/nostr/relay_rate.coax --library`: 7 of 7 pass.
- `coax mutate coax/lib/nostr/relay_rate.coax --library`: 215 mutants,
  215 killed, 0 survived, with no mutant marked equivalent. The first runs
  left 9 and then 4 alive; each was killed by a new check or by restating
  code whose mutants could not be observed (the document search became a
  backward scan, and the suffix test became `rt_ends` with its own checks).

No other library file changed.

### What isn't built

- An EVENT is charged after its signature check, not before it, so a
  client that sends forged events pays for their verification without
  using its window. The flood driver `relay_flood.py`, which stores 2,100
  events on one connection, passes `--rate` with the largest limits.
- A connection stands in for an IP address: the C host gives the relay no
  peer address.
- Per-address limits need the C host to report each connection's peer.
- There is no per-kind write table.

### Records not rerun

The R1 relay's limits over real sockets are in
[2026-10-30-coax-relay-r1-tailnet-flood.md](../../docs/verification/2026-10-30-coax-relay-r1-tailnet-flood.md).
`relay_flood.py` drove 216 connections, 512 subscriptions, oversized
frames, 2,100 events (2,048 stored, 52 refused with `relay storage is
full`) and the slowloris cases; 31 of 31 checks passed in release and under
ASan+UBSan, with resident memory flat at about 30 MiB. That record also
has `relay_route.coax`'s mutation run with `rr_deadline`: 270 of 270
mutants killed. None of it was rerun for
this chapter, because `relay.coax`, `relay_route.coax` and
`relay_server.coax` are unchanged.

## Summary

- Every limit has a value, one procedure that enforces it, and an exact
  refusal: a close code, `NOTICE`, `CLOSED` or `OK false` with a NIP-01
  prefix.
- Rate limits count in fixed 60-second windows, the Rust relay's rule: 120
  `EVENT`s and 120 `REQ`s per connection and 60 `EVENT`s per key by
  default, refused with `rate-limited:`.
- A connection is charged before a key, and before the signature check;
  one connection's flood doesn't spend another's budget.
- Deadlines free slots held by silent or trickling sockets; an idle
  WebSocket is kept.
- The rate limits are an `RtConfig` value, and `rt_nip11` writes the same
  value into the NIP-11 document.

## Dependencies

- [34. Admit an event](ch034-admit-an-event.md)
- [36. Match filters with indexes](ch036-match-filters-with-indexes.md)
- [37. NIP-11 relay information](ch037-nip-11-relay-information.md)

## Sources

- [NIP-01](../nips/official/01.md): `OK`, `CLOSED` and `NOTICE`, and the
  `rate-limited:`, `restricted:`, `invalid:` and `error:` prefixes.
- [NIP-11](../nips/official/11.md): the `limitation` object, and clients
  ignoring unknown fields.
- The Rust relay at `0b401561a0^`: `crates/nostr-relay/src/gateway/rate.rs`
  (`allow_counter`, `MAX_RATE_KEYS`) and `gateway/config.rs` (the default
  rates). Read with `git show 0b401561a0^:<path>`.
- [RFC 6455](https://www.rfc-editor.org/rfc/rfc6455) section 7.4.1: close
  codes 1008 and 1009.
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md), section 17.
