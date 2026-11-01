# 29. A relay connection

A client keeps a connection to each relay it uses. This chapter builds the
state machine that decides what to do when a connection opens, drops or
misbehaves. It does no I/O itself: the host moves bytes and passes the time.

Part VIII, A client. Result: The pure client state machine: OPEN, CONNECTING,
CLOSED and BACKOFF, connection generations that make stale reports harmless,
and capped jittered backoff.

## What this chapter teaches

- States and transitions.
- Generations: anything reported for an older connection is ignored.
- Backoff with a cap and host-supplied jitter.
- The NIP-42 hook: an `AUTH` challenge is passed to chapter 38's code.

Chapter 28 moved bytes: a socket, TLS, the WebSocket handshake and
`rc_scan`, which takes one frame at a time. This chapter sits on top of
that. It decides when to connect, when to give up on a connection, how long
to wait before the next one, what to send once the connection is open, and
what to do with each relay message. Chapters 30 and 31 go deeper into
subscriptions and publishes; this chapter uses one of each to show how they
survive a reconnect.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [relay.coax](../../coax/lib/nostr/relay.coax) | `rl_start`, `rl_opened`, `rl_dropped`, `rl_shutdown` | the connection's lifecycle, one generation per connection (`#authority`) |
| | `rl_tick(r, now)` | the deadlines: a connect that took too long, a backoff that ended, an `OK` that is overdue |
| | `rl_next(r, now)` | the next frame to send on the open connection: `CLOSE`, `REQ`, `EVENT`, then the `AUTH` request |
| | `rl_input(r, gen, m)` | one decoded relay message for generation `gen`: what it means and what changes |
| | `rl_backoff(r, attempt, jitter)` | the reconnect delay |
| | `rl_subscribe`, `rl_unsubscribe`, `rl_publish`, `rl_auth_sent` | what the caller asks for |
| [relay_read.coax](../../coax/lib/nostr/relay_read.coax) | `rl_read(text, scratch, overlap)` | one relay text frame into the machine's input, `RlIn`: chapter 24's `nostr_message_read` reads it, and an `EVENT`'s id and signature are checked before `verified` is set (`#authority`) |
| [relay_conn_rules.coax](../../coax/lib/net/relay_conn_rules.coax) | `rc_scan` | chapter 28's frame step, used here to take the scripted relay's frames |
| [ws.coax](../../coax/lib/net/ws.coax) | `ws_response` | chapter 26's check of the handshake answer |

`relay.coax` is pure. It reads no clock, draws no randomness and touches no
socket. Every procedure takes the time as `now`, in milliseconds, and the
backoff takes its jitter as an argument. A real host draws the jitter from
entropy; the example passes fixed numbers, so the same script always gives
the same transcript.

The machine's state lives in one `Relay` value, passed as a one-element
slice `r: [] Relay` so the procedures can change it. Its fields include the
connection `state`, the generation `gen`, the reconnect `attempt` and the
current `deadline`, plus 8 subscription slots, 8 publish slots and a window
of the last 64 event ids. The timing defaults are fields too: `connect_ms`
5000, `ack_ms` 10000, `base_ms` 500 and `cap_ms` 30000.

## States and transitions

A connection is in one of five states:

| State | Meaning | Leaves by |
| --- | --- | --- |
| `RL_IDLE` (0) | never started | `rl_start` |
| `RL_CONNECTING` (1) | the host is opening the socket and doing the handshake; the deadline is `now + connect_ms` | `rl_opened` (to OPEN), `rl_dropped` (to BACKOFF) |
| `RL_OPEN` (2) | frames flow both ways | `rl_dropped` (to BACKOFF) |
| `RL_BACKOFF` (3) | waiting before the next attempt; the deadline is `now + rl_backoff(...)` | `rl_start` (to CONNECTING) |
| `RL_SHUT` (4) | the caller stopped the connection; nothing restarts it | nothing |

The chapter's summary line names CLOSED and BACKOFF. The code has one state
for both: after a drop the machine is always backing off toward the next
attempt, and `RL_SHUT` is the only state that stays closed.

The host drives the machine in a loop. One turn is:

1. Call `rl_tick(r, now)`. It returns `RL_DO_CONNECT` when a backoff has
   ended, or `RL_DO_TIMEOUT` when a connect has taken longer than
   `connect_ms`. While the connection is open it also marks overdue
   publishes `RP_TIMEOUT`.
2. On `RL_DO_CONNECT`, or in `RL_IDLE`, call `rl_start(r, now)`, which
   returns the new generation, and open the socket. When the handshake
   succeeds, call `rl_opened(r, gen, now)`. When it fails, or on
   `RL_DO_TIMEOUT`, close the socket and call `rl_dropped(r, gen, now,
   jitter)`.
3. While the connection is open, call `rl_next(r, now)` until it returns
   `RL_OUT_NONE`, and write each frame it asks for.
4. Poll the socket. Pass each text message through `rl_read` to
   `rl_input(r, gen, in)`. When the socket closes or a frame is refused,
   call `rl_dropped`.

The Pylon's `py_step` in
[psionic/lib/pylon/run.coax](../../psionic/lib/pylon/run.coax) is this loop
over a real socket.

`rl_start` refuses with -1 unless the state is `RL_IDLE` or `RL_BACKOFF`, so
a second connect can't start while one is under way. `rl_opened` succeeds
only once per generation, from `RL_CONNECTING`. `rl_dropped` succeeds only
from `RL_CONNECTING` or `RL_OPEN`. When a call doesn't fit the state, it
returns `false` (or -1) and changes nothing.

## Generations

Every `rl_start` adds 1 to `gen` and returns it. The host keeps that number
with the socket and passes it back with every report: `rl_opened(r, gen,
now)`, `rl_dropped(r, gen, now, jitter)` and `rl_input(r, gen, m)`.

A report whose generation is not the current one is stale. `rl_input`
returns `RL_STALE`, and `rl_opened` and `rl_dropped` return `false`. A host
is never fully in step with its sockets: a read that was already in flight
can finish after the host gave up on that socket, and a close from an old
connection can arrive after a new one opened. Without generations, an old
socket's close would end the new connection, and an old subscription's
events would be taken as answers to the new `REQ`.

The machine also refuses messages while it is not open: `rl_input` returns
`RL_NOT_OPEN` for the current generation in any state but `RL_OPEN`. Nothing
counts as a relay message before the handshake has finished.

## Backoff

When a connection drops, `rl_dropped` sets the deadline to `now +
rl_backoff(r, attempt, jitter)` and adds 1 to `attempt`, up to 16.
`rl_opened` sets `attempt` back to 0. The delay is:

```text
d     = base_ms << attempt   when attempt is 0..16 and that is at most cap_ms - 1
      = cap_ms               otherwise
delay = d + (jitter & 0x7fffffff) % (d / 2 + 1)
```

With the defaults, attempts 0 to 5 wait 500, 1000, 2000, 4000, 8000 and
16000 ms, and attempt 6 and later wait 30000 ms. The jitter adds between 0
and `d / 2` ms. Masking with `0x7fffffff` makes a negative jitter
non-negative, so the host can pass any 32-bit number it drew.

The jitter spreads reconnects. When a relay restarts, every client sees the
drop at the same moment; without jitter they would all reconnect at the
same moments too, and the relay would get each wave at once. The machine
doesn't draw the jitter itself because it is pure: the host supplies the
random number, and a test supplies a fixed one.

## What each relay message does

`rl_read` turns one text frame into an `RlIn`: a kind (`EVENT`, `OK`,
`EOSE`, `CLOSED`, `NOTICE`), the subscription id, the event id and
`created_at`, the `OK` verdict, whether the message starts with
`auth-required:`, and `verified`. For an `EVENT`, `verified` is true only
when the id recomputed from the event's bytes matches and the Schnorr
signature checks. A frame that `nostr_message_read` refuses comes back as
kind 0.

`rl_input` checks in this order and returns the first answer that applies:

| Answer | When |
| --- | --- |
| `RL_STALE` | `gen` is not the current generation |
| `RL_NOT_OPEN` | the connection is not open |
| `RL_NOTICE` | a `NOTICE`; the machine counts it |
| `RL_AUTH` | an `AUTH` challenge, or the `OK` for the `AUTH` event the host sent |
| `RL_ACK` | an `OK` for a sent publish (or one that timed out) |
| `RL_UNKNOWN` | an `OK` for no pending publish; or an `EVENT`, `EOSE` or `CLOSED` whose subscription id names no live subscription |
| `RL_BAD` | any other kind, including kind 0, a refused frame |
| `RL_EOSE` | the relay finished its stored events; the subscription is `RS_CAUGHT` |
| `RL_CLOSED` | the relay ended the subscription; it is `RS_REFUSED`, or `RS_AUTH` after `auth-required:` |
| `RL_FORGED` | an `EVENT` that is not `verified`; the machine counts it and doesn't remember its id |
| `RL_DUPLICATE` | an event id already in the 64-id window |
| `RL_DELIVER` | a new, verified event for a live subscription |

A forged event's id is not remembered, so a genuine copy that arrives later
is still delivered. An `EOSE` means only that the relay sent what it had
stored; chapter 30 covers why it never means the set is complete.

## Reconnecting keeps the caller's work

A drop doesn't lose what the caller asked for. On `rl_opened`, every
subscription that was live, caught up or waiting for `AUTH` goes back to
`RS_WANT`, and every publish that was sent but not acknowledged goes back to
`RP_QUEUED`. `rl_next` then sends them again:

- The `REQ` carries `since` set to the newest `created_at` the subscription
  delivered. NIP-01's `since` is inclusive, so the relay sends the boundary
  event again, and the id window drops it as `RL_DUPLICATE`.
- The `EVENT` goes again with a new deadline, `now + ack_ms`.

A subscription the relay refused with `CLOSED` stays refused: asking again
would get the same answer.

## The NIP-42 hook

A relay can send `["AUTH", <challenge>]` at any time. `rl_read` returns kind
0 for it, because `message.coax` doesn't read `AUTH`; the host gives such a
frame to chapter 38's `nip42_input`, which records the challenge in the
machine (`auth` becomes `RA_CHALLENGED`). From then on `rl_next` returns
`RL_OUT_AUTH` until the host signs an `AUTH` event (kind 22242) and reports
its id with `rl_auth_sent(r, id)`. When the relay accepts that event with
`OK true`, each subscription the relay closed with `auth-required:` goes
back to `RS_WANT`, once per generation. A reconnect clears the challenge: a
new connection needs a new one. This chapter's example sends no challenge;
chapter 38 does.

## Worked example: one session by hand

The script runs one subscription (slot 0, wire id `s0.1`, kinds `[30617]`)
and one publish (slot 0, the captured repository announcement
`813d9fb0...9d21`, `created_at` 1793214821) against one relay. The clock is
fixed: each step happens at a stated time in milliseconds. The defaults
hold: `connect_ms` 5000, `base_ms` 500, `cap_ms` 30000, `ack_ms` 10000.

**Before any connection** (t=0). The state is `RL_IDLE`, generation 0. A
`NOTICE` reported for generation 0 names the current generation, but the
connection is not open: `RL_NOT_OPEN`. `rl_next` has nothing to send.

**Connection 1: the relay refuses the upgrade.** `rl_start` at t=0 makes
generation 1, `RL_CONNECTING`, deadline 0 + 5000 = 5000. At t=30 the relay
answers `HTTP/1.1 503`. `ws_response` returns `WS_HELLO_STATUS`, so the host
closes the socket and calls `rl_dropped(gen 1, t=30, jitter 0)`. Attempt 0:
d = 500 << 0 = 500, jitter 0 % 251 = 0, so the deadline is 30 + 500 = 530,
`RL_BACKOFF`, attempt 1. A late `rl_opened` for generation 1 returns
`false`: the state is no longer `RL_CONNECTING`.

**Connection 2: the relay stalls.** `rl_tick` at t=529 returns
`RL_DO_NONE` (529 < 530) and at t=530 `RL_DO_CONNECT`. `rl_start` makes
generation 2, deadline 530 + 5000 = 5530. A `NOTICE` for generation 2 before
the handshake is `RL_NOT_OPEN`; `rl_next` sends nothing; a second `rl_start`
returns -1. The relay never answers. `rl_tick` at t=5529 is `RL_DO_NONE`, at
t=5530 `RL_DO_TIMEOUT`. The host closes the socket and calls
`rl_dropped(gen 2, t=5530, jitter 7)`. Attempt 1: d = 500 << 1 = 1000, which
is at most 29999, so d stays 1000; the jitter adds 7 % (1000 / 2 + 1) =
7 % 501 = 7. The deadline is 5530 + 1007 = 6537, attempt 2.

**Connection 3: open, then a message too big.** `rl_tick` at t=6537 is
`RL_DO_CONNECT`; `rl_start` makes generation 3, deadline 11537. At t=6600 the
relay answers 101 with the accept value for the RFC 6455 sample key, and
`rl_opened` makes the state `RL_OPEN` and sets attempt to 0. A `NOTICE`
still reported for generation 2 is `RL_STALE`, and `rl_dropped(gen 2)` is
`false`. `rl_next` gives two frames: the `REQ` for `s0.1` with no `since`
(nothing delivered yet), then the publish, with its `OK` due by 6600 + 10000
= 16600. At t=6700 the relay sends eight frames:

| Frame | Answer | Why |
| --- | --- | --- |
| `EVENT s0.1`, the real event | `RL_DELIVER` | live subscription, verified, new id; the newest `created_at` is now 1793214821 |
| `EVENT s0.1`, one signature bit changed | `RL_FORGED` | the signature doesn't check; forged = 1 |
| `EVENT s0.1`, the real event again | `RL_DUPLICATE` | its id is in the window |
| `["EOSE","s0.1"]` | `RL_EOSE` | `s0.1` is now `RS_CAUGHT` |
| `["NOTICE","rate limits apply"]` | `RL_NOTICE` | notices = 1 |
| `EVENT s0.9`, the real event | `RL_UNKNOWN` | no subscription is named `s0.9` |
| `["EOSE"]` | `RL_BAD` | `message.coax` refuses an `EOSE` with no id, so `rl_read` returns kind 0 |
| a 2049-byte text message | refused, close 1009 | over the host's 2048-byte limit; `rc_scan` refuses it as too big |

The refusal ends the connection: the host sends close 1009 and calls
`rl_dropped(gen 3, t=6700, jitter 0)`. The open reset the attempt to 0, so
d = 500 and the deadline is 6700 + 500 = 7200, attempt 1. The publish was
sent, but its `OK` never came.

**Connection 4: everything is asked again.** `rl_tick` at t=7199 is
`RL_DO_NONE` and at t=7200 `RL_DO_CONNECT`. `rl_start` makes generation 4,
deadline 12200, and the 101 answer at t=7250 opens it. `rl_opened` moved
`s0.1` from `RS_CAUGHT` back to `RS_WANT` and the publish from `RP_SENT`
back to `RP_QUEUED`, so `rl_next` sends the `REQ` again with
`"since":1793214821`, then the publish again, due by 17250. At t=7300 the
relay sends:

| Frame | Answer | Why |
| --- | --- | --- |
| `EVENT s0.1`, the real event | `RL_DUPLICATE` | the boundary event that `since` asked for again; its id is still in the window |
| `["OK","813d9fb0...9d21",true,""]` | `RL_ACK` | the publish is `RP_ACCEPTED` |
| `["CLOSED","s0.1","error: shutting down idle subscription"]` | `RL_CLOSED` | no `auth-required:`, so `s0.1` is `RS_REFUSED` |
| `EVENT s0.1`, the real event | `RL_UNKNOWN` | a refused subscription is not live |
| close frame, code 1001 | the host echoes 1001 | `rc_scan` reports the close; the connection is over |

`rl_dropped(gen 4, t=7300, jitter 0)` gives deadline 7800, attempt 1. Then
`rl_shutdown` makes the state `RL_SHUT`: `rl_start` returns -1, and
`rl_tick` returns `RL_DO_NONE` at any time.

So the state after each step is IDLE, CONNECTING, BACKOFF, CONNECTING,
BACKOFF, CONNECTING, OPEN, BACKOFF, CONNECTING, OPEN, BACKOFF, SHUT, which
as digits is `013131231234`. The deadlines are 5000, 530, 5530, 6537, 11537,
7200, 12200 and 7800.

## The example

The example is
[nostr/examples/relay-connection.coax](../examples/relay-connection.coax).
It loads `relay_read.coax`, which loads `relay.coax` and `message.coax`, and
chapter 28's `relay_conn_rules.coax`. It plays the host:

- `cx_hello` checks a scripted handshake answer with `ws_response` and calls
  `rl_opened` only on `WS_HELLO_OK`.
- `cx_flush` asks `rl_next` for frames until there are none, and writes each
  `REQ` and `CLOSE` with chapter 24's writers.
- `cx_feed` takes the relay's scripted frames with `rc_scan`, turns each
  text message into an `RlIn` with `rl_read`, and passes it to `rl_input`.
  On a close or a refused frame it returns, and the session calls
  `rl_dropped`.
- `cx_session` runs the session worked out above and records each state,
  deadline and decision in a `CxTrace`, which the checks compare with the
  hand values.

The events are the captured relay.openagents.com repository announcement,
`RLR_REAL` in `coax/tests/nostr/relay_read_fixtures.coax`, and its copy with
one signature bit changed, `RLR_BAD_SIG`. Each `EVENT` frame is 698 bytes,
so `cx_frame` writes the 16-bit length form for it.

<!-- source: nostr/examples/relay-connection.coax example -->
```jai
#load "../../coax/lib/nostr/relay_read.coax";
#load "../../coax/lib/net/relay_conn_rules.coax";
#import "Basic";

// The example host's message limit. relay_conn.coax takes 128 KiB; a small
// limit lets the script carry a message that is too big.
CX_MSG :: 2048;
// The RFC 6455 sample key and its accept value (RFC 6455, section 1.3).
CX_KEY :: "dGhlIHNhbXBsZSBub25jZQ==";
CX_HELLO_101 :: "HTTP/1.1 101 Switching Protocols\r\nUpgrade: websocket\r\nConnection: Upgrade\r\nSec-WebSocket-Accept: s3pPLMBiTxaQ9kYGzzhZRbK+xOo=\r\n\r\n";
CX_HELLO_503 :: "HTTP/1.1 503 Service Unavailable\r\nRetry-After: 30\r\n\r\n";

cx_state_name :: (s: int) -> string {
  names := [5] string.["IDLE", "CONNECTING", "OPEN", "BACKOFF", "SHUT"];
  return names[s];
}
cx_input_name :: (d: int) -> string {
  names := [12] string.["RL_DELIVER", "RL_DUPLICATE", "RL_STALE", "RL_UNKNOWN", "RL_FORGED", "RL_EOSE",
                        "RL_ACK", "RL_CLOSED", "RL_NOTICE", "RL_AUTH", "RL_NOT_OPEN", "RL_BAD"];
  return names[d];
}
cx_tick_name :: (t: int) -> string {
  names := [3] string.["RL_DO_NONE", "RL_DO_CONNECT", "RL_DO_TIMEOUT"];
  return names[t];
}

cx_out_name :: (k: int) -> string {
  names := [5] string.["RL_OUT_NONE", "RL_OUT_REQ", "RL_OUT_CLOSE", "RL_OUT_EVENT", "RL_OUT_AUTH"];
  return names[k];
}
cx_hello_name :: (h: int) -> string {
  names := [9] string.["WS_HELLO_OK", "WS_HELLO_MORE", "WS_HELLO_TOO_BIG", "WS_HELLO_STATUS", "WS_HELLO_HEADER",
                       "WS_HELLO_UPGRADE", "WS_HELLO_CONNECTION", "WS_HELLO_ACCEPT", "WS_HELLO_EXTENSION"];
  return names[h];
}

// What one session went through: the connection state after each step, the
// deadlines the machine set and the rl_input decisions, for the checks.
CxTrace :: struct {
  states: [32] u8; steps: int;
  deadlines: [16] int; nd: int;
  decisions: [32] u8; nds: int;
}
// Records (and, when loud, prints) the connection after one step.
cx_step :: (t: [] CxTrace, r: [] Relay, now: int, what: string, loud: bool) {
  t[0].states[t[0].steps] = cast(u8) ('0' + r[0].state); t[0].steps += 1;
  if loud {
    print("t=% %: % gen % attempt %", now, what, cx_state_name(r[0].state), r[0].gen, r[0].attempt);
    if r[0].state == RL_CONNECTING || r[0].state == RL_BACKOFF { print(" deadline %", r[0].deadline); }
    print("\n");
  }
  if (r[0].state == RL_CONNECTING || r[0].state == RL_BACKOFF) && (t[0].nd == 0 || t[0].deadlines[t[0].nd - 1] != r[0].deadline) {
    t[0].deadlines[t[0].nd] = r[0].deadline; t[0].nd += 1;
  }
}
cx_decided :: (t: [] CxTrace, d: int) { t[0].decisions[t[0].nds] = cast(u8) ('a' + d); t[0].nds += 1; }

// One unmasked server frame appended at out[at..] (payloads up to 65535
// bytes). Returns the new end.
cx_frame :: (out: [] u8, at: int, op: int, payload: string) -> int {
  out[at] = cast(u8) (op + 128);
  h := 2;
  if payload.count < 126 { out[at + 1] = cast(u8) payload.count; }
  else { out[at + 1] = 126; out[at + 2] = cast(u8) (payload.count >> 8); out[at + 3] = cast(u8) (payload.count & 255); h = 4; }
  for b, i: payload { out[at + h + i] = b; }
  return at + h + payload.count;
}
cx_text :: (out: [] u8, at: int, payload: string) -> int { return cx_frame(out, at, WS_OP_TEXT, payload); }

// Sends what the open connection asks for, until rl_next has nothing.
cx_flush :: (r: [] Relay, now: int, loud: bool) {
  while true {
    o := rl_next(r, now);
    if o.kind == RL_OUT_NONE { return; }
    sb: [24] u8; out: [256] u8;
    if o.kind == RL_OUT_EVENT {
      if loud { print("  send [\"EVENT\",{\"id\":\"813d9fb0...}] (publish slot %, OK due by t=%)\n", o.slot, r[0].pubs[o.slot].deadline); }
      continue;
    }
    id := rl_sub_text(o.slot, r[0].subs[o.slot].serial, sb[..]);
    n := 0;
    if o.kind == RL_OUT_REQ {
      kinds := [1] int.[30617];
      f: NostrFilter; f.has_kinds = true; f.kinds = kinds[..]; f.has_since = o.has_since; f.since = o.since;
      filters := [1] NostrFilter.[f];
      n = nostr_message_write_req(id, filters[..], out[..], false);
    } else if o.kind == RL_OUT_CLOSE {
      n = nostr_message_write_close(id, out[..], false);
    }
    if loud { print("  send %\n", cast(string) out[0..<n]); }
  }
}

// The open connection's poll loop over a scripted wire: rc_scan takes each
// frame, rl_read turns a text message into the machine's input, and
// rl_input decides. Returns RC_CLOSED (the relay closed), RC_FAIL (a frame
// was refused) or RC_NONE (the script ended).
cx_feed :: (t: [] CxTrace, r: [] Relay, gen: int, wire: string, loud: bool) -> int {
  rd: [1] WsReader; msg: [CX_MSG] u8; at := 0;
  while true {
    v := rc_scan(rd[..], wire[at..<wire.count], msg[..]);
    if v.kind == RC_S_TEXT {
      text := cast(string) msg[0..<rd[0].used];
      scratch: [NE_MAX_OUTPUT] u8;
      in := rl_read(text, scratch[..], false);
      d := rl_input(r, gen, in);
      cx_decided(t, d);
      if loud {
        if text.count > 44 { print("  recv %... (% bytes) -> %\n", text[0..<44], text.count, cx_input_name(d)); }
        else { print("  recv % -> %\n", text, cx_input_name(d)); }
      }
      at += v.consumed; continue;
    }
    if v.kind == RC_S_CLOSE {
      if loud { print("  recv close % -> echo close %, RC_CLOSED\n", v.code, rc_close_reply(v.code)); }
      return RC_CLOSED;
    }
    if v.kind == RC_S_REFUSE {
      if loud { print("  frame refused -> send close %, RC_FAIL\n", v.code); }
      return RC_FAIL;
    }
    at += v.consumed;
    if v.kind == RC_S_MORE { return RC_NONE; }
  }
  return RC_NONE;
}

// The handshake answer: the host opens the connection only on WS_HELLO_OK.
cx_hello :: (r: [] Relay, gen: int, now: int, bytes: string, loud: bool) -> bool {
  h := ws_response(bytes, CX_KEY);
  first := bytes[0..<12];
  ok := h.status == WS_HELLO_OK && rl_opened(r, gen, now);
  if loud { print("t=% handshake answer \"%...\": % -> %\n", now, first, cx_hello_name(h.status), ifx ok then "rl_opened" else "close, rl_dropped"); }
  return ok;
}

// One scripted session against one relay with a fixed clock. The relay
// refuses the upgrade, then stalls, then serves a subscription until a
// message too big for the host, then closes after one more connection.
cx_session :: (loud: bool) -> CxTrace {
  t: [1] CxTrace; r: [1] Relay; wire: [6144] u8;
  // Before any connection: one subscription and one publish wait.
  s := rl_subscribe(r[..]);
  ev := rl_read(rlr_event_frame("s0.1", RLR_REAL, wire[..]), wire[2048..<6144], false);
  p := rl_publish(r[..], ev.id[..]);
  cx_step(t[..], r[..], 0, "subscribed slot 0, publish slot 0", loud);
  d := rl_input(r[..], 0, RlIn.{kind = RL_IN_NOTICE}); cx_decided(t[..], d);
  if loud { print("  a NOTICE before any connection -> %, rl_next %\n", cx_input_name(d), cx_out_name(rl_next(r[..], 0).kind)); }

  // Connection 1: the relay answers the upgrade with 503.
  g := rl_start(r[..], 0);
  cx_step(t[..], r[..], 0, "rl_start", loud);
  if !cx_hello(r[..], g, 30, CX_HELLO_503, loud) { ok := rl_dropped(r[..], g, 30, 0); }
  cx_step(t[..], r[..], 30, "dropped", loud);
  if loud { print("  rl_opened(gen 1) now: %\n", rl_opened(r[..], g, 40)); }

  // Connection 2: the relay accepts the socket and never answers.
  if loud { print("t=529 rl_tick: %\n", cx_tick_name(rl_tick(r[..], 529))); }
  if loud { print("t=530 rl_tick: %\n", cx_tick_name(rl_tick(r[..], 530))); }
  g = rl_start(r[..], 530);
  cx_step(t[..], r[..], 530, "rl_start", loud);
  d = rl_input(r[..], g, RlIn.{kind = RL_IN_NOTICE}); cx_decided(t[..], d);
  if loud {
    print("  a frame before the handshake -> %, rl_next %, rl_start again %\n", cx_input_name(d), cx_out_name(rl_next(r[..], 600).kind), rl_start(r[..], 600));
    print("t=5529 rl_tick: %\n", cx_tick_name(rl_tick(r[..], 5529)));
    print("t=5530 rl_tick: %\n", cx_tick_name(rl_tick(r[..], 5530)));
  }
  ok := rl_dropped(r[..], g, 5530, 7);
  cx_step(t[..], r[..], 5530, "connect timed out, jitter 7", loud);

  // Connection 3: open, subscribe, publish, read until a message is too big.
  if loud { print("t=6537 rl_tick: %\n", cx_tick_name(rl_tick(r[..], 6537))); }
  old := g;
  g = rl_start(r[..], 6537);
  cx_step(t[..], r[..], 6537, "rl_start", loud);
  ok = cx_hello(r[..], g, 6600, CX_HELLO_101, loud);
  cx_step(t[..], r[..], 6600, "opened", loud);
  d = rl_input(r[..], old, RlIn.{kind = RL_IN_NOTICE}); cx_decided(t[..], d);
  if loud { print("  a NOTICE reported for gen % -> %, rl_dropped(gen %) %\n", old, cx_input_name(d), old, rl_dropped(r[..], old, 6600, 0)); }
  cx_flush(r[..], 6600, loud);
  frame: [2048] u8; big: [CX_MSG + 1] u8;
  w := cx_text(wire[..], 0, rlr_event_frame("s0.1", RLR_REAL, frame[..]));
  w = cx_text(wire[..], w, rlr_event_frame("s0.1", RLR_BAD_SIG, frame[..]));
  w = cx_text(wire[..], w, rlr_event_frame("s0.1", RLR_REAL, frame[..]));
  w = cx_text(wire[..], w, "[\"EOSE\",\"s0.1\"]");
  w = cx_text(wire[..], w, "[\"NOTICE\",\"rate limits apply\"]");
  w = cx_text(wire[..], w, rlr_event_frame("s0.9", RLR_REAL, frame[..]));
  w = cx_text(wire[..], w, "[\"EOSE\"]");
  for i: 0..<big.count { big[i] = ' '; }
  w = cx_text(wire[..], w, cast(string) big[..]);
  if loud { print("t=6700 the relay sends % bytes:\n", w); }
  if cx_feed(t[..], r[..], g, cast(string) wire[0..<w], loud) != RC_NONE { ok = rl_dropped(r[..], g, 6700, 0); }
  cx_step(t[..], r[..], 6700, "dropped", loud);
  if loud { print("  subscription 0: % event, newest created_at %; forged %, notices %\n", r[0].subs[0].events, r[0].subs[0].newest, r[0].forged, r[0].notices); }

  // Connection 4: resubscribe with since, resend the publish, then the relay
  // ends the subscription and closes.
  if loud { print("t=7199 rl_tick: %\nt=7200 rl_tick: %\n", cx_tick_name(rl_tick(r[..], 7199)), cx_tick_name(rl_tick(r[..], 7200))); }
  g = rl_start(r[..], 7200);
  cx_step(t[..], r[..], 7200, "rl_start", loud);
  ok = cx_hello(r[..], g, 7250, CX_HELLO_101, loud);
  cx_step(t[..], r[..], 7250, "opened", loud);
  cx_flush(r[..], 7250, loud);
  w = cx_text(wire[..], 0, rlr_event_frame("s0.1", RLR_REAL, frame[..]));
  w = cx_text(wire[..], w, "[\"OK\",\"813d9fb02bd78ce24200ba986b62ef727f0c032f712cff82241bd465f4bc9d21\",true,\"\"]");
  w = cx_text(wire[..], w, "[\"CLOSED\",\"s0.1\",\"error: shutting down idle subscription\"]");
  w = cx_text(wire[..], w, rlr_event_frame("s0.1", RLR_REAL, frame[..]));
  cp: [2] u8; cp[0] = 0x03; cp[1] = 0xe9;    // close code 1001, going away
  w = cx_frame(wire[..], w, WS_OP_CLOSE, cast(string) cp[..]);
  if loud { print("t=7300 the relay sends % bytes:\n", w); }
  if cx_feed(t[..], r[..], g, cast(string) wire[0..<w], loud) != RC_NONE { ok = rl_dropped(r[..], g, 7300, 0); }
  cx_step(t[..], r[..], 7300, "dropped", loud);
  if loud { print("  publish slot %: state % (3 is ACCEPTED); subscription 0: state % (5 is REFUSED)\n", p, r[0].pubs[p].state, r[0].subs[s].state); }
  rl_shutdown(r[..]);
  cx_step(t[..], r[..], 7300, "rl_shutdown", loud);
  if loud { print("  rl_start %, rl_tick at t=99999 %\n", rl_start(r[..], 7400), cx_tick_name(rl_tick(r[..], 99999))); }
  return t[0];
}

main :: () {
  t := cx_session(true);
  print("states: %\n", cast(string) t.states[0..<t.steps]);
  print("decisions: %\n", cast(string) t.decisions[0..<t.nds]);
  print("deadlines:");
  for k: 0..<t.nd { print(" %", t.deadlines[k]); }
  print("\n");

  // The backoff schedule with the default base 500 ms and cap 30 s.
  r: [1] Relay;
  print("backoff, jitter 0:");
  for a: 0..<8 { print(" %", rl_backoff(r[..], a, 0)); }
  print("\n");
  print("attempt 0, jitter 0, 250, 251, 252: % % % %\n", rl_backoff(r[..], 0, 0), rl_backoff(r[..], 0, 250), rl_backoff(r[..], 0, 251), rl_backoff(r[..], 0, 252));
  print("attempt 6, jitter 15000 and 15001: % %\n", rl_backoff(r[..], 6, 15000), rl_backoff(r[..], 6, 15001));
}
```

To run it:

```shell
coax/bin/coax run nostr/examples/relay-connection.coax
```

The program prints:

```text
t=0 subscribed slot 0, publish slot 0: IDLE gen 0 attempt 0
  a NOTICE before any connection -> RL_NOT_OPEN, rl_next RL_OUT_NONE
t=0 rl_start: CONNECTING gen 1 attempt 0 deadline 5000
t=30 handshake answer "HTTP/1.1 503...": WS_HELLO_STATUS -> close, rl_dropped
t=30 dropped: BACKOFF gen 1 attempt 1 deadline 530
  rl_opened(gen 1) now: false
t=529 rl_tick: RL_DO_NONE
t=530 rl_tick: RL_DO_CONNECT
t=530 rl_start: CONNECTING gen 2 attempt 1 deadline 5530
  a frame before the handshake -> RL_NOT_OPEN, rl_next RL_OUT_NONE, rl_start again -1
t=5529 rl_tick: RL_DO_NONE
t=5530 rl_tick: RL_DO_TIMEOUT
t=5530 connect timed out, jitter 7: BACKOFF gen 2 attempt 2 deadline 6537
t=6537 rl_tick: RL_DO_CONNECT
t=6537 rl_start: CONNECTING gen 3 attempt 2 deadline 11537
t=6600 handshake answer "HTTP/1.1 101...": WS_HELLO_OK -> rl_opened
t=6600 opened: OPEN gen 3 attempt 0
  a NOTICE reported for gen 2 -> RL_STALE, rl_dropped(gen 2) false
  send ["REQ","s0.1",{"kinds":[30617]}]
  send ["EVENT",{"id":"813d9fb0...}] (publish slot 0, OK due by t=16600)
t=6700 the relay sends 4920 bytes:
  recv ["EVENT","s0.1",{"id":"813d9fb02bd78ce24200b... (698 bytes) -> RL_DELIVER
  recv ["EVENT","s0.1",{"id":"813d9fb02bd78ce24200b... (698 bytes) -> RL_FORGED
  recv ["EVENT","s0.1",{"id":"813d9fb02bd78ce24200b... (698 bytes) -> RL_DUPLICATE
  recv ["EOSE","s0.1"] -> RL_EOSE
  recv ["NOTICE","rate limits apply"] -> RL_NOTICE
  recv ["EVENT","s0.9",{"id":"813d9fb02bd78ce24200b... (698 bytes) -> RL_UNKNOWN
  recv ["EOSE"] -> RL_BAD
  frame refused -> send close 1009, RC_FAIL
t=6700 dropped: BACKOFF gen 3 attempt 1 deadline 7200
  subscription 0: 1 event, newest created_at 1793214821; forged 1, notices 1
t=7199 rl_tick: RL_DO_NONE
t=7200 rl_tick: RL_DO_CONNECT
t=7200 rl_start: CONNECTING gen 4 attempt 1 deadline 12200
t=7250 handshake answer "HTTP/1.1 101...": WS_HELLO_OK -> rl_opened
t=7250 opened: OPEN gen 4 attempt 0
  send ["REQ","s0.1",{"kinds":[30617],"since":1793214821}]
  send ["EVENT",{"id":"813d9fb0...}] (publish slot 0, OK due by t=17250)
t=7300 the relay sends 1551 bytes:
  recv ["EVENT","s0.1",{"id":"813d9fb02bd78ce24200b... (698 bytes) -> RL_DUPLICATE
  recv ["OK","813d9fb02bd78ce24200ba986b62ef727f0c0... (81 bytes) -> RL_ACK
  recv ["CLOSED","s0.1","error: shutting down idle ... (58 bytes) -> RL_CLOSED
  recv ["EVENT","s0.1",{"id":"813d9fb02bd78ce24200b... (698 bytes) -> RL_UNKNOWN
  recv close 1001 -> echo close 1001, RC_CLOSED
t=7300 dropped: BACKOFF gen 4 attempt 1 deadline 7800
  publish slot 0: state 3 (3 is ACCEPTED); subscription 0: state 5 (5 is REFUSED)
t=7300 rl_shutdown: SHUT gen 4 attempt 1
  rl_start -1, rl_tick at t=99999 RL_DO_NONE
states: 013131231234
decisions: kkcaebfidlbghd
deadlines: 5000 530 5530 6537 11537 7200 12200 7800
backoff, jitter 0: 500 1000 2000 4000 8000 16000 30000 30000
attempt 0, jitter 0, 250, 251, 252: 500 750 500 501
attempt 6, jitter 15000 and 15001: 45000 30000
```

The transcript matches the hand trace line for line. The deliberate
refusals in it are:

- a message before any connection and a message before the handshake:
  `RL_NOT_OPEN`;
- a message and a drop reported for an old generation: `RL_STALE` and
  `false`;
- a second `rl_start` while connecting, and `rl_start` after shutdown: -1;
- a forged event: `RL_FORGED`, counted and not remembered;
- an event for a subscription that doesn't exist, and for one the relay
  closed: `RL_UNKNOWN`;
- a malformed relay message, `["EOSE"]`: `RL_BAD`;
- a 2049-byte message against a 2048-byte limit: close 1009, and the
  connection drops. The library's own limit is 128 KiB (`RC_MSG_MAX` in
  `relay_conn.coax`); the example uses a small one so the script can carry
  a message over it.

The last three lines are the backoff schedule with the defaults: attempt 6
reaches the 30000 ms cap, jitter 250 is the largest that attempt 0 adds
(250 % 251), and 251 wraps to 0.

## How the code is checked

### Checks in the example

The checks are in
[nostr/examples/checks/relay-connection.coax](../examples/checks/relay-connection.coax),
which loads the example. They compare the code with values from outside it:

- the session's states, decisions and deadlines against the hand trace
  above, each deadline written as its own sum;
- the scripted 101 answer against RFC 6455's published accept value for the
  sample key (section 1.3), so the handshake in the script is a real one;
- `rl_backoff` for attempts 0 to 19 and seven jitters against the schedule
  computed in the check by doubling 500 up to 30000;
- the host's message limit: 2048 bytes are taken, 2049 are refused with
  1009.

<!-- source: nostr/examples/checks/relay-connection.coax check -->
```jai
// The session's trace, worked out by hand in the chapter before the program
// ran. States: 0 IDLE, 1 CONNECTING, 2 OPEN, 3 BACKOFF, 4 SHUT. Decisions:
// 'a' + the RL_ code (a DELIVER, b DUPLICATE, c STALE, d UNKNOWN, e FORGED,
// f EOSE, g ACK, h CLOSED, i NOTICE, k NOT_OPEN, l BAD).
HAND_STATES :: "013131231234";
HAND_DECISIONS :: "kkcaebfidlbghd";
#test "the scripted session follows the hand trace" #steps 2000000000 {
  t := cx_session(false);
  #assert cast(string) t.states[0..<t.steps] == HAND_STATES;
  #assert cast(string) t.decisions[0..<t.nds] == HAND_DECISIONS;
  // Each deadline from its own arithmetic: connect 5000 ms, backoff 500 ms
  // doubled per attempt, jitter mod (half the delay + 1).
  hand := [8] int.[0 + 5000, 30 + 500, 530 + 5000, 5530 + 1000 + 7 % 501, 6537 + 5000, 6700 + 500, 7200 + 5000, 7300 + 500];
  #assert t.nd == 8;
  for k: 0..<8 { #assert t.deadlines[k] == hand[k]; }
}
#test "the scripted 101 answer carries RFC 6455's published accept value" {
  // RFC 6455, section 1.3: key dGhlIHNhbXBsZSBub25jZQ== gives
  // s3pPLMBiTxaQ9kYGzzhZRbK+xOo=.
  accept: [28] u8;
  #assert ws_accept_of(CX_KEY, accept[..]) && cast(string) accept[..] == "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=";
  #assert ws_response(CX_HELLO_101, CX_KEY).status == WS_HELLO_OK;
  #assert ws_response(CX_HELLO_503, CX_KEY).status == WS_HELLO_STATUS;
}
#test "rl_backoff against the schedule computed here" {
  r: [1] Relay;
  jitters := [7] int.[0, 1, 7, 250, 251, 15000, 15001];
  for a: 0..<20 {
    d := 500; k := 0;
    while k < a && d < 30000 { d *= 2; k += 1; }
    if d > 30000 { d = 30000; }
    for j: jitters { #assert rl_backoff(r[..], a, j) == d + j % (d / 2 + 1); }
  }
}
#test "the host's message limit is exact" {
  wire: [CX_MSG + 8] u8; text: [CX_MSG + 1] u8; rd: [1] WsReader; msg: [CX_MSG] u8;
  for i: 0..<text.count { text[i] = ' '; }
  w := cx_text(wire[..], 0, cast(string) text[0..<CX_MSG]);
  #assert rc_scan(rd[..], cast(string) wire[0..<w], msg[..]).kind == RC_S_TEXT;
  rd2: [1] WsReader;
  w = cx_text(wire[..], 0, cast(string) text[..]);
  v := rc_scan(rd2[..], cast(string) wire[0..<w], msg[..]);
  #assert v.kind == RC_S_REFUSE && v.code == 1009;
}
```

To run them:

```shell
coax/bin/coax test nostr/examples/checks/relay-connection.coax
```

On 2026-11-01 this reported 298 of 298 checks passed, including the checks
of every file the example loads. The session check verifies four Schnorr
signatures at compile time and needs a larger step budget than the default,
so it carries `#steps 2000000000`.

### The library's own checks

[relay.coax](../../coax/lib/nostr/relay.coax) has 13 checks: the
connect-subscribe-EOSE path, stale generations, unknown and forged input,
publishes with `OK true`, `OK false`, a timeout and a late `OK`, queue
overflow, drop and reconnect with `since`, the backoff bounds (including a
small base that shows the 16-doubling limit), the `AUTH` hook and its
re-arming on reconnect, shutdown, and the duplicate window forgetting its
oldest id, the subscription ids, and the exported state reader.
[relay_read.coax](../../coax/lib/nostr/relay_read.coax) checks a real and a
forged event through `rl_input`, scratch too small or aliased, every relay
frame kind with and without `auth-required:`, and refused frames as kind 0.

The [2026-10-29 relay client record](../../docs/verification/2026-10-29-nostr-relay-client.md)
reports 555 of 555 mutants of `relay.coax` killed, with none marked
equivalent. The
[2026-10-30 relay_read record](../../docs/verification/2026-10-30-relay-read.md)
reports 5,371 of 5,371 killed for `relay_read.coax`, a count that includes
`relay.coax`'s and `message.coax`'s mutants because `coax mutate` mutates
every `#authority` unit a file loads. `relay.coax` has not changed since the
first record (`d7373bcc82`), so those runs were not repeated for this
chapter. Root-only mutation of the checks file reports no mutation sites:
it holds tests and constants only.

## Summary

- The connection is IDLE, CONNECTING, OPEN, BACKOFF or SHUT. Each call
  checks the state and refuses with `false` or -1 when it doesn't fit.
- Each connection has a generation. A report for any other generation is
  stale and changes nothing.
- Nothing is a relay message until the handshake has finished:
  `RL_NOT_OPEN`.
- The reconnect delay is 500 ms doubled per attempt, capped at 30 s, plus
  host-supplied jitter of up to half the delay. A successful open resets it.
- On reopen, subscriptions are asked again with `since` set to the newest
  event delivered, and unacknowledged publishes are sent again. A refused
  subscription stays refused.
- The machine is pure: the host passes the time, the jitter and decoded
  messages, so a fixed script gives a fixed transcript.

## Dependencies

- [24. Relay messages](ch024-relay-messages.md)
- [28. Sockets, TLS and relay URLs](ch028-sockets-tls-and-relay-urls.md)

## Sources

- [NIP-01](../nips/official/01.md), the relay messages, `since` and `CLOSED`.
- [NIP-42](../nips/official/42.md), the `AUTH` challenge and the
  `auth-required:` prefix.
- [RFC 6455](https://www.rfc-editor.org/rfc/rfc6455), section 1.3 (the
  sample key and accept value) and section 7.4 (close codes 1001 and 1009).
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
