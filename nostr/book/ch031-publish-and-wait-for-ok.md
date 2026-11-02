# 31. Publish and wait for OK

Sending an event is not the same as storing it. A relay answers each `EVENT`
with an `OK` that names the event's id, says `true` or `false`, and gives a
reason. This chapter builds publishing that matches each `OK` to its publish,
keeps a deadline for the answer, and says what the client knows when no
answer comes.

Part IV, A client. Result: Publishing with acknowledgement deadlines, the
distinction between published, acknowledged, refused and unknown, and requeue
after AUTH.

## What this chapter teaches

- Pending publishes with a deadline per relay.
- `OK true`, `OK false` with a prefix, and timeout as unknown.
- `auth-required:` requeues once after successful AUTH.
- A duplicate (`duplicate:`) counts as stored.

Chapter 29 built the connection machine in `relay.coax` and showed one
publish surviving a reconnect. This chapter is about the publish itself:
what `rl_publish` keeps, what `rl_next` sends, how `rl_input` matches an
`OK`, what each `OK` prefix means and which ones the machine acts on, what a
timeout means, and why sending the same event again is safe. Subscriptions
are chapter 30's.

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [relay.coax](../../coax/lib/nostr/relay.coax) | `rl_publish(r, id)` | a publish slot for the event with this 32-byte id (`#authority`) |
| | `rl_next(r, now)` | `RL_OUT_EVENT` for the next queued publish; it sets the slot's deadline to `now + ack_ms` |
| | `rl_input(r, gen, m)` | an `OK`: matched by id to a publish, or to the `AUTH` event |
| | `rl_tick(r, now)` | marks a publish whose `OK` is overdue `RP_TIMEOUT` |
| | `rl_opened`, `rl_cancel_publish` | a new connection sends unanswered publishes again; the caller can drop one |
| [relay_read.coax](../../coax/lib/nostr/relay_read.coax) | `rl_read(text, scratch, overlap)` | one relay text frame into `RlIn`; for an `OK`, the id, the verdict and whether the message starts with `auth-required:` (`#authority`) |
| [message.coax](../../coax/lib/nostr/message.coax) | `nostr_message_read`, `nm_prefix`, `nostr_message_write_event` | chapter 24's reader, which refuses a malformed `OK`, and the `["EVENT",...]` writer |
| [nip42.coax](../../coax/lib/nostr/nip42.coax) | `nip42_input`, `nip42_template`, `nip42_send_fields` | chapter 38's `AUTH` challenge and answer, used here only to reach the requeue |
| [event_sign.coax](../../coax/lib/nostr/event_sign.coax) | `nostr_event_sign_step` | chapter 21's signer, which makes the example's events |

Like the rest of `relay.coax`, publishing is pure. The host keeps the
event's bytes, writes the frame and passes the time in milliseconds. The
machine keeps only the 32-byte id, the state and the deadline of each of its
8 publish slots.

## A publish and its states

A publish slot is in one of these states:

| State | Meaning |
| --- | --- |
| `RP_FREE` (0) | unused |
| `RP_QUEUED` (1) | waiting for an open connection; `rl_next` will send it |
| `RP_SENT` (2) | on the wire; the `OK` is due by `deadline` |
| `RP_ACCEPTED` (3) | the relay answered `OK true`: stored |
| `RP_REFUSED` (4) | the relay answered `OK false` |
| `RP_TIMEOUT` (5) | no `OK` by the deadline: the outcome is unknown |
| `RP_AUTH` (6) | the relay answered `OK false` with `auth-required:`; the publish waits for `AUTH` |

The chapter's summary line names four outcomes. Published is `RP_SENT`,
acknowledged is `RP_ACCEPTED`, refused is `RP_REFUSED`, and unknown is
`RP_TIMEOUT`. `RP_QUEUED` and `RP_AUTH` are on the way to one of them.

`rl_publish(r, id)` returns a slot or -1:

- An id that is not 32 bytes: -1.
- An id that is already pending (queued, sent or waiting for `AUTH`): that
  slot. The same event is one publish, so calling `rl_publish` twice doesn't
  send it twice.
- Otherwise the first slot that is not pending: free, or holding a settled
  publish (accepted, refused or timed out). The slot is cleared and becomes
  `RP_QUEUED`. When all 8 are pending: -1.

`rl_next` sends `CLOSE` and `REQ` frames first (chapter 29), then the queued
publishes in slot order. For each one it returns `RL_OUT_EVENT` with the
slot, sets the state to `RP_SENT` and sets `deadline = now + ack_ms`
(10000 ms by default). The host writes `["EVENT",<event>]` with
`nostr_message_write_event` from the bytes it kept for that slot.

## Matching an OK

NIP-01's `OK` is `["OK",<event id>,<true|false>,<message>]`. The id is the
only thing that ties an answer to a publish: a relay may answer in any order,
and nothing else in the frame names the event.

`rl_read` gives `rl_input` the id, the verdict (`accepted`) and
`auth_required`. `rl_input` checks, in order:

1. The generation and the open state, as in chapter 29 (`RL_STALE`,
   `RL_NOT_OPEN`).
2. Whether the id is the `AUTH` event's id while an `AUTH` is outstanding.
   Then the answer is about authentication: `RL_AUTH`.
3. Whether a slot in `RP_SENT` or `RP_TIMEOUT` has this id. Then it settles
   that slot and returns `RL_ACK`.
4. Otherwise `RL_UNKNOWN`. The machine changes nothing.

`RL_UNKNOWN` covers an id the client never published, and an id whose
publish is already settled. So a second `OK` for an accepted event can't
turn it into a refusal, and an `OK` for a publish that isn't on the wire yet
is not taken as its answer.

A frame that `message.coax` refuses never becomes an `OK`. `rl_read` returns
kind 0 for it, and `rl_input` returns `RL_BAD`. Two `OK`s are refused this
way in the example: one whose id is not 64 hex digits, and `OK false` with an
empty message. NIP-01 says a `false` message "MUST be a string formed by a
machine-readable single-word prefix followed by a `:`", so a refusal with no
reason is malformed (`NM_MESSAGE`).

## The verdict and the prefixes

The verdict decides stored or not. The machine sets `RP_ACCEPTED` in exactly
one place: an `OK` whose verdict is `true`. A publish is never reported as
stored for any other reason.

The prefix says why. NIP-01 lists eight words, and NIP-42 adds
`auth-required`. `relay.coax` acts on one of them. The others are for the
caller, which can read the word with `nm_prefix` when it reads the frame:

| Prefix | Verdict | What `relay.coax` does | What the caller can do |
| --- | --- | --- | --- |
| none (`""`) | `true` | `RP_ACCEPTED` | nothing; it is stored |
| `duplicate:` | `true` | `RP_ACCEPTED` | nothing; the relay already had it, which is a success |
| `pow:` | `true` | `RP_ACCEPTED` | nothing; the relay reports the proof of work it saw |
| `pow:` | `false` | `RP_REFUSED` | sign a new event with more work (NIP-13); the same bytes get the same answer |
| `blocked:` | `false` | `RP_REFUSED` | don't send to this relay again; use another |
| `rate-limited:` | `false` | `RP_REFUSED` | publish the same event again later |
| `invalid:` | `false` | `RP_REFUSED` | fix the event; resending the same bytes is pointless |
| `restricted:` | `false` | `RP_REFUSED` | NIP-42: the authenticated key is not allowed; don't retry |
| `mute:` | `false` | `RP_REFUSED` | nothing; nobody was listening for this ephemeral event |
| `error:` | `false` | `RP_REFUSED` | publish again later; the relay failed |
| `auth-required:` | `false` | `RP_AUTH`, then sent again once after an accepted `AUTH` | sign the `AUTH` event (chapter 38) |
| any other word | `false` | `RP_REFUSED` | treat it as `error:` |

Two rules follow from going by the verdict:

- `duplicate:` counts as stored because NIP-01 sends it with `true`. A relay
  that sent `duplicate:` with `false` would be reported as refusing; the
  machine doesn't guess that the event is stored anyway.
- A prefix on an `OK true` changes nothing. `auth-required:` with `true` is
  accepted like any other `true`.

Retrying is the caller's decision, except for `auth-required:`. To send a
refused event again, the caller calls `rl_publish` with the same id. That
makes a new attempt in the first slot that is not pending; the refused slot
keeps its outcome until it is reused.

## auth-required: wait, authenticate, send once more

NIP-42 shows this exchange for a write:

```text
relay: ["AUTH", "<challenge>"]
client: ["EVENT", {"id": "012345...", ...}]
relay: ["OK", "012345...", false, "auth-required: we only accept events from registered users"]
client: ["AUTH", {"id": "abcdef...", ...}]
relay: ["OK", "abcdef...", true, ""]
client: ["EVENT", {"id": "012345...", ...}]
relay: ["OK", "012345...", true, ""]
```

`relay.coax` follows it. An `OK false` with `auth-required:` for a sent
publish makes it `RP_AUTH`. The publish is still pending: `rl_publish` with
the same id returns its slot, and `rl_tick` doesn't time it out, because no
answer is due. When the relay accepts the `AUTH` event (an `OK true` for the
id given to `rl_auth_sent`), every `RP_AUTH` publish goes back to
`RP_QUEUED`, and `rl_next` sends it again with a new deadline.

Each publish gets one such retry per connection. The slot's `auth_retried`
flag is set when it is queued again, and a second `auth-required:` after
that is `RP_REFUSED`: the relay asked for authentication, got it, and still
refused. A refused `AUTH` leaves the publish in `RP_AUTH`. A new connection
sends every `RP_AUTH` publish again and clears the flag, because a new
connection has a new challenge. Subscriptions closed with `auth-required:`
work the same way (chapter 29).

This requeue was added to `relay.coax` for this chapter. Before, an
`auth-required:` refusal of a publish was final, while the same refusal of a
subscription was retried. The check "publish: auth-required waits for AUTH
and is sent again once" was written first.

## A timeout means unknown

While the connection is open, `rl_tick(r, now)` marks every `RP_SENT`
publish with `now >= deadline` as `RP_TIMEOUT` and adds 1 to `timeouts`.
The deadline is inclusive: at `deadline - 1` nothing happens.

A timeout says nothing about the relay. The relay may have stored the event
and the `OK` may be late, or lost with the connection, or the relay may
never have read the frame. So `RP_TIMEOUT` is not a refusal, and the
machine doesn't resend on its own:

- A late `OK` still settles a timed-out publish. `rl_input` matches
  `RP_TIMEOUT` slots as well as `RP_SENT` ones.
- A reconnect doesn't send a timed-out publish again. Only `RP_SENT` and
  `RP_AUTH` publishes go back to `RP_QUEUED` on `rl_opened`.
- The caller can publish it again with `rl_publish`. A timed-out slot is
  not pending, so the same id starts a new attempt with a new deadline.

`rl_tick` checks deadlines only while the connection is open. A publish that
was on the wire when the connection dropped is not timed out: it is sent
again on the next connection.

## Sending again is safe

An event's id is the SHA-256 of its serialized content (chapter 21), so the
same event sent twice has the same id twice. A relay that already stored it
answers `OK true` with `duplicate:`, which the machine counts as stored.
That is why the machine can send unanswered publishes again after a
reconnect without asking the caller, and why the caller can publish again
after a timeout or `rate-limited:`: the worst case is a `duplicate:` answer.

What a client must not do is change the event between attempts. A new
`created_at` or new content makes a different id, and the relay would store
two events. The host keeps the signed bytes for each slot and sends exactly
those.

## Worked example: one session by hand

The script publishes three kind-1 notes to one relay. Chapter 21's signer
makes them with BIP-340 test vector 1's key, `created_at` 1793300000, no
tags, and contents `note A`, `note B` and `note C`. Their ids start
`aee488a2`, `c195de5e` and `0b31f609`; below they are A, B and C. The clock
is fixed, in milliseconds. The defaults hold: `ack_ms` 10000, `base_ms` 500.
The trace lists the three slots' states as digits, slot 0 first.

**Before the connection** (t=0). `rl_publish` puts A, B and C in slots 0, 1
and 2: `111`. Publishing A again returns slot 0, because A is pending. An id
of 31 bytes returns -1. `rl_next` returns `RL_OUT_NONE`: nothing is sent
before the connection opens.

**Connection 1.** `rl_start` at t=0 and `rl_opened` at t=50. `rl_next` sends
A, B and C, each due by 50 + 10000 = 10050: `222`. At t=120 the relay sends:

| Frame | Answer | Why |
| --- | --- | --- |
| `["OK",A,true,""]` | `RL_ACK` | slot 0 is `RP_SENT` with A's id; `true`, so `RP_ACCEPTED` |
| `["OK",A,true,"duplicate: already have this event"]` | `RL_UNKNOWN` | A is settled; no slot is waiting for it |
| `["OK",B,false,"rate-limited: slow down there chief"]` | `RL_ACK` | `false` without `auth-required:`, so slot 1 is `RP_REFUSED` |
| `["OK","b1a649eb...",true,""]` | `RL_UNKNOWN` | NIP-01's example id; this client never published it |
| `["OK",C,false,""]` | `RL_BAD` | a refusal with no prefix; `message.coax` refuses it |
| `["OK","b1a6",true,""]` | `RL_BAD` | the id is 2 bytes, not 32 |

The slots are `342`: A accepted, B refused, C still sent.

**C's deadline.** `rl_tick` at t=10049 changes nothing (10049 < 10050):
`342`. At t=10050, slot 2 is `RP_SENT` and 10050 >= 10050, so it becomes
`RP_TIMEOUT` and `timeouts` is 1: `345`. At t=10100 the relay sends
`["OK",C,true,"duplicate: already have this event"]`. Slot 2 is
`RP_TIMEOUT` with C's id, so the late answer settles it: `RL_ACK`,
`RP_ACCEPTED`, `343`.

**B again.** At t=10200 the caller publishes B again after the rate limit.
`rl_publish` scans the slots: slot 0 holds A, which is settled, so slot 0 is
the first one that is not pending; slot 1 holds B but is refused, so it is
not pending either and doesn't match. B goes into slot 0: `143`. `rl_next`
sends it, due by 10200 + 10000 = 20200: `243`. At t=10300 the connection
closes before the relay answers. `rl_dropped` with jitter 0 sets the
reconnect for 10300 + 500 = 10800. Slot 0 stays `RP_SENT`: `243`.

**Connection 2.** `rl_start` at t=10800 and `rl_opened` at t=10850. The open
moves slot 0 from `RP_SENT` back to `RP_QUEUED`: `143`. `rl_next` sends B
again, due by 10850 + 10000 = 20850. At t=10900 the relay sends:

| Frame | Answer | Why |
| --- | --- | --- |
| `["AUTH","c1"]` | `RL_AUTH` | `nip42_input` records challenge `c1` for connection 2; `rl_next` will ask for an `AUTH` |
| `["OK",B,false,"auth-required: we only accept events from registered users"]` | `RL_ACK` | `false` with `auth-required:` and no retry yet, so slot 0 is `RP_AUTH` |

The slots are `643`. `rl_next` now returns `RL_OUT_AUTH`. The host signs a
kind-22242 event with tags
`[["relay","wss://relay.example.com"],["challenge","c1"]]` and `created_at`
1793300000 + 10900 / 1000 = 1793300010, sends it, and reports its id with
`rl_auth_sent`. At t=10950 the relay answers that id with `OK true`:
`RL_AUTH`, and slot 0 goes back to `RP_QUEUED` with `auth_retried` set:
`143`. `rl_next` sends B a third time, due by 10950 + 10000 = 20950. At
t=11000 the relay sends `["OK",B,true,""]`: `RL_ACK`, `RP_ACCEPTED`, `343`.

So the decisions are ACK, UNKNOWN, ACK, UNKNOWN, BAD, BAD, ACK, AUTH, ACK,
AUTH, ACK, which as letters (`a` + the code) is `gdgdllgjgjg`. The slots
after each step are `111 222 342 342 345 343 243 243 143 643 143 343`, and
the deadlines are 10050, 10050, 10050, 20200, 20850 and 20950.

Every outcome appears: published (`RP_SENT`), acknowledged (`RP_ACCEPTED`,
three times, once through a late `duplicate:`), refused (`RP_REFUSED`,
slot 1) and unknown (`RP_TIMEOUT`, slot 2 at t=10050). B ends accepted in
slot 0, while slot 1 still records its first attempt's refusal.

## The example

The example is
[nostr/examples/publish-ok.coax](../examples/publish-ok.coax). It loads
`relay_read.coax` (which loads `relay.coax` and `message.coax`),
`nip42.coax` and `event_sign.coax`, and plays the host:

- `po_sign` signs an event with chapter 21's signer and keeps its bytes and
  id in a `PoEvent`.
- `po_flush` asks `rl_next` for frames until there are none. For each
  `RL_OUT_EVENT` it writes the slot's kept event with
  `nostr_message_write_event`. For `RL_OUT_AUTH` it builds chapter 38's
  template, signs it and hands it to `nip42_send_fields`, which calls
  `rl_auth_sent`.
- `po_recv` gives an `AUTH` challenge to `nip42_input` and every other text
  message to `rl_read` and `rl_input`.
- `po_ok` writes an `OK` frame for an id, a verdict and a message.
- `po_session` runs the session worked out above and records each decision,
  slot state and deadline in a `PoTrace` for the checks.

Chapter 29 showed the WebSocket frames around each message; this example
passes the text messages straight to `rl_read`.

<!-- source: nostr/examples/publish-ok.coax example -->
```jai
#load "../../coax/lib/nostr/relay_read.coax";
#load "../../coax/lib/nostr/nip42.coax";
#load "../../coax/lib/nostr/event_sign.coax";
#import "Basic";

// BIP-340 test vector 1's published key and aux, as in chapter 21. Never
// sign with a real key in an example.
PO_KEY :: "b7e151628aed2a6abf7158809cf4f3c762e7160f38b4da56a784d9045190cfef";
PO_AUX :: "0000000000000000000000000000000000000000000000000000000000000001";
// The script's clock starts at this Unix time, in seconds; t is milliseconds
// after it.
PO_AT :: 1793300000;
PO_RELAY :: "wss://relay.example.com";
// The OK id that NIP-01's examples abbreviate as "b1a649ebe8...", in full.
PO_STRANGER :: "b1a649ebe8b435ec71d3784793f3bbf4b93e64e17568a741aecd4c7ddeafce30";

po_input_name :: (d: int) -> string {
  names := [12] string.["RL_DELIVER", "RL_DUPLICATE", "RL_STALE", "RL_UNKNOWN", "RL_FORGED", "RL_EOSE",
                        "RL_ACK", "RL_CLOSED", "RL_NOTICE", "RL_AUTH", "RL_NOT_OPEN", "RL_BAD"];
  return names[d];
}
po_tick_name :: (t: int) -> string {
  names := [3] string.["RL_DO_NONE", "RL_DO_CONNECT", "RL_DO_TIMEOUT"];
  return names[t];
}
po_pub_name :: (s: int) -> string {
  names := [7] string.["FREE", "QUEUED", "SENT", "ACCEPTED", "REFUSED", "TIMEOUT", "AUTH"];
  return names[s];
}

// A signed event the host keeps for its publish: its bytes and its id.
PoEvent :: struct { bytes: [1024] u8; count: int; id: [32] u8; }
po_text :: (e: [] PoEvent, k: int) -> string { return cast(string) e[k].bytes[0..<e[k].count]; }

po_sign :: (created_at: int, kind: int, tags: string, content: string) -> PoEvent {
  key: [32] u8; aux: [32] u8; e: PoEvent;
  k := hex_decode_checked(PO_KEY, key[..], false); a := hex_decode_checked(PO_AUX, aux[..], false);
  work: [1] NostrSignWork; scratch: [1024] u8; result := EVENT_MORE;
  while result == EVENT_MORE {
    result = nostr_event_sign_step(key[..], aux[..], created_at, kind, tags, content, work[..], scratch[..], e.bytes[..], false);
  }
  if result < 0 { return e; }
  e.count = result;
  r := nostr_event_read(cast(string) e.bytes[0..<result], scratch[..], false);
  e.id = r.id;
  return e;
}
// The first 8 hex digits of an id, for printing.
po_short :: (id: [] u8, out: [] u8) -> string {
  r := hex_encode_checked(cast(string) id[0..<4], out, false);
  return cast(string) out[0..<r.written];
}
// ["OK",<id>,<verdict>,<message>] with the id as 64 hex digits.
po_ok :: (id: [] u8, accepted: bool, message: string, out: [] u8) -> string {
  hex: [64] u8; r := hex_encode_checked(cast(string) id[0..<32], hex[..], false);
  n := json_raw(out, 0, "[\"OK\",\""); n = json_raw(out, n, cast(string) hex[..]);
  n = json_raw(out, n, ifx accepted then "\",true," else "\",false,");
  n = json_write_string(out, n, message); n = json_raw(out, n, "]");
  return cast(string) out[0..<n];
}

// What one session went through, for the checks: each rl_input decision
// (and nip42_input's), the three publish slots' states after each step, and
// each OK deadline the machine set.
PoTrace :: struct {
  decisions: [32] u8; nds: int;
  states: [64] u8; ns: int;
  deadlines: [16] int; nd: int;
}
po_decided :: (t: [] PoTrace, d: int) { t[0].decisions[t[0].nds] = cast(u8) ('a' + d); t[0].nds += 1; }
// Records (and, when loud, prints) the publish slots after one step.
po_slots :: (t: [] PoTrace, r: [] Relay, what: string, loud: bool) {
  for k: 0..<3 { t[0].states[t[0].ns] = cast(u8) ('0' + r[0].pubs[k].state); t[0].ns += 1; }
  t[0].states[t[0].ns] = ' '; t[0].ns += 1;
  if loud {
    print("  %: slot 0 %, slot 1 %, slot 2 %\n", what,
          po_pub_name(r[0].pubs[0].state), po_pub_name(r[0].pubs[1].state), po_pub_name(r[0].pubs[2].state));
  }
}

// Sends what the open connection asks for until rl_next has nothing: each
// queued EVENT from the bytes the host kept for that slot, and an AUTH
// event answering the relay's challenge (chapter 38).
po_flush :: (t: [] PoTrace, r: [] Relay, n: [] Nip42, gen: int, now: int, ev: [] PoEvent, held: [] int, loud: bool) {
  while true {
    o := rl_next(r, now);
    if o.kind == RL_OUT_NONE { return; }
    out: [1200] u8; sb: [8] u8;
    if o.kind == RL_OUT_EVENT {
      k := held[o.slot];
      w := nostr_message_write_event(po_text(ev, k), out[..], false);
      t[0].deadlines[t[0].nd] = r[0].pubs[o.slot].deadline; t[0].nd += 1;
      if loud {
        print("  send [\"EVENT\",{\"id\":\"%...\"}] (% bytes): event %, publish slot %, OK due by t=%\n",
              po_short(ev[k].id[..], sb[..]), w, cast(string) "ABC"[k..<k + 1], o.slot, r[0].pubs[o.slot].deadline);
      }
      continue;
    }
    if o.kind != RL_OUT_AUTH { return; }
    s := PO_AT + now / 1000;
    tags: [512] u8; tp := nip42_template(n, gen, s, tags[..]);
    a: [1] PoEvent; a[0] = po_sign(tp.created_at, tp.kind, tp.tags, tp.content);
    scratch: [1024] u8; e := nostr_event_read(po_text(a[..], 0), scratch[..], false);
    f := nip42_send_fields(r, n, gen, s, e.error == NE_OK, tp.kind, tp.created_at, tp.tags, a[0].id[..], po_text(a[..], 0), out[..]);
    if f.error != N42_OK { if loud { print("  AUTH not sent (%)\n", f.error); } return; }
    if loud { print("  send [\"AUTH\",{\"id\":\"%...\",\"kind\":22242,...}] (% bytes), tags %\n", po_short(a[0].id[..], sb[..]), f.written, tp.tags); }
  }
}

// One relay text message: an AUTH challenge goes to chapter 38's
// nip42_input, everything else through rl_read to rl_input. shown is what
// to print in place of a long message.
po_recv :: (t: [] PoTrace, r: [] Relay, n: [] Nip42, gen: int, text: string, shown: string, loud: bool) -> int {
  d := nip42_input(r, n, gen, text);
  if d < 0 {
    scratch: [64] u8;
    d = rl_input(r, gen, rl_read(text, scratch[..], false));
  }
  po_decided(t, d);
  if loud { print("  recv % -> %\n", shown, po_input_name(d)); }
  return d;
}

// One scripted session against one relay with a fixed clock: three
// publishes, every OK outcome, a timeout, a reconnect and an AUTH.
po_session :: (loud: bool) -> PoTrace {
  t: [1] PoTrace; r: [1] Relay; n: [1] Nip42; held: [RL_PUBS] int;
  e := nip42_init(n[..], PO_RELAY);
  ev: [3] PoEvent;
  ev[0] = po_sign(PO_AT, 1, "[]", "note A");
  ev[1] = po_sign(PO_AT, 1, "[]", "note B");
  ev[2] = po_sign(PO_AT, 1, "[]", "note C");
  sb: [8] u8; ok: [400] u8; d := 0;

  // Before any connection: three publishes wait in slots 0, 1 and 2.
  for k: 0..<3 {
    s := rl_publish(r[..], ev[k].id[..]); held[s] = k;
    if loud { print("t=0 publish event % (id %...): slot %\n", cast(string) "ABC"[k..<k + 1], po_short(ev[k].id[..], sb[..]), s); }
  }
  if loud {
    print("  publish A again: slot %; an id of 31 bytes: %\n", rl_publish(r[..], ev[0].id[..]), rl_publish(r[..], ev[0].id[0..<31]));
    print("  rl_next before the connection opens: %\n", ifx rl_next(r[..], 0).kind == RL_OUT_NONE then "RL_OUT_NONE" else "something");
  }
  po_slots(t[..], r[..], "queued", loud);

  // Connection 1: open at t=50 and send all three.
  g := rl_start(r[..], 0);
  if !rl_opened(r[..], g, 50) { return t[0]; }
  if loud { print("t=50 connection % open\n", g); }
  po_flush(t[..], r[..], n[..], g, 50, ev[..], held[..], loud);
  po_slots(t[..], r[..], "after sending", loud);

  if loud { print("t=120 the relay answers:\n"); }
  d = po_recv(t[..], r[..], n[..], g, po_ok(ev[0].id[..], true, "", ok[..]), "[\"OK\",A,true,\"\"]", loud);
  d = po_recv(t[..], r[..], n[..], g, po_ok(ev[0].id[..], true, "duplicate: already have this event", ok[..]), "[\"OK\",A,true,\"duplicate: already have this event\"]", loud);
  d = po_recv(t[..], r[..], n[..], g, po_ok(ev[1].id[..], false, "rate-limited: slow down there chief", ok[..]), "[\"OK\",B,false,\"rate-limited: slow down there chief\"]", loud);
  stranger: [32] u8; x := hex_decode_checked(PO_STRANGER, stranger[..], false);
  d = po_recv(t[..], r[..], n[..], g, po_ok(stranger[..], true, "", ok[..]), "[\"OK\",\"b1a649eb...\",true,\"\"] (no publish has this id)", loud);
  d = po_recv(t[..], r[..], n[..], g, po_ok(ev[2].id[..], false, "", ok[..]), "[\"OK\",C,false,\"\"] (a refusal must carry a prefix)", loud);
  d = po_recv(t[..], r[..], n[..], g, "[\"OK\",\"b1a6\",true,\"\"]", "[\"OK\",\"b1a6\",true,\"\"] (the id is 2 bytes)", loud);
  po_slots(t[..], r[..], "t=120", loud);

  // C's OK is overdue at 50 + 10000.
  d = rl_tick(r[..], 10049);
  if loud { print("t=10049 rl_tick: %, timeouts %\n", po_tick_name(d), r[0].timeouts); }
  po_slots(t[..], r[..], "t=10049", loud);
  d = rl_tick(r[..], 10050);
  if loud { print("t=10050 rl_tick: %, timeouts %\n", po_tick_name(d), r[0].timeouts); }
  po_slots(t[..], r[..], "t=10050", loud);
  if loud { print("t=10100 a late answer:\n"); }
  d = po_recv(t[..], r[..], n[..], g, po_ok(ev[2].id[..], true, "duplicate: already have this event", ok[..]), "[\"OK\",C,true,\"duplicate: already have this event\"]", loud);
  po_slots(t[..], r[..], "t=10100", loud);

  // The caller publishes B again; slot 0's publish is resolved, so B takes it.
  s := rl_publish(r[..], ev[1].id[..]); held[s] = 1;
  if loud { print("t=10200 publish event B again: slot %\n", s); }
  po_flush(t[..], r[..], n[..], g, 10200, ev[..], held[..], loud);
  po_slots(t[..], r[..], "t=10200", loud);

  // The relay closes before it answers.
  if !rl_dropped(r[..], g, 10300, 0) { return t[0]; }
  if loud { print("t=10300 the connection closes; reconnect at t=%\n", r[0].deadline); }
  po_slots(t[..], r[..], "t=10300", loud);

  // Connection 2: B goes again. The relay wants AUTH first.
  g = rl_start(r[..], r[0].deadline);
  if !rl_opened(r[..], g, 10850) { return t[0]; }
  if loud { print("t=10850 connection % open\n", g); }
  po_slots(t[..], r[..], "opened", loud);
  po_flush(t[..], r[..], n[..], g, 10850, ev[..], held[..], loud);
  if loud { print("t=10900 the relay answers:\n"); }
  d = po_recv(t[..], r[..], n[..], g, "[\"AUTH\",\"c1\"]", "[\"AUTH\",\"c1\"]", loud);
  d = po_recv(t[..], r[..], n[..], g, po_ok(ev[1].id[..], false, "auth-required: we only accept events from registered users", ok[..]),
              "[\"OK\",B,false,\"auth-required: we only accept events from registered users\"]", loud);
  po_slots(t[..], r[..], "t=10900", loud);
  po_flush(t[..], r[..], n[..], g, 10900, ev[..], held[..], loud);
  if loud { print("t=10950 the relay answers:\n"); }
  d = po_recv(t[..], r[..], n[..], g, po_ok(r[0].auth_id[..], true, "", ok[..]), "[\"OK\",<the AUTH event's id>,true,\"\"]", loud);
  po_slots(t[..], r[..], "t=10950", loud);
  po_flush(t[..], r[..], n[..], g, 10950, ev[..], held[..], loud);
  if loud { print("t=11000 the relay answers:\n"); }
  d = po_recv(t[..], r[..], n[..], g, po_ok(ev[1].id[..], true, "", ok[..]), "[\"OK\",B,true,\"\"]", loud);
  po_slots(t[..], r[..], "t=11000", loud);
  return t[0];
}

main :: () {
  t := po_session(true);
  print("decisions: %\n", cast(string) t.decisions[0..<t.nds]);
  print("slots: %\n", cast(string) t.states[0..<t.ns - 1]);
  print("deadlines:");
  for k: 0..<t.nd { print(" %", t.deadlines[k]); }
  print("\n");
}
```

To run it:

```shell
coax/bin/coax run nostr/examples/publish-ok.coax --run-steps 250000000
```

Loading the signer needs a larger compile-time step budget than the default,
so the command carries `--run-steps 250000000`, as chapter 21's does. The
program prints:

```text
t=0 publish event A (id aee488a2...): slot 0
t=0 publish event B (id c195de5e...): slot 1
t=0 publish event C (id 0b31f609...): slot 2
  publish A again: slot 0; an id of 31 bytes: -1
  rl_next before the connection opens: RL_OUT_NONE
  queued: slot 0 QUEUED, slot 1 QUEUED, slot 2 QUEUED
t=50 connection 1 open
  send ["EVENT",{"id":"aee488a2..."}] (358 bytes): event A, publish slot 0, OK due by t=10050
  send ["EVENT",{"id":"c195de5e..."}] (358 bytes): event B, publish slot 1, OK due by t=10050
  send ["EVENT",{"id":"0b31f609..."}] (358 bytes): event C, publish slot 2, OK due by t=10050
  after sending: slot 0 SENT, slot 1 SENT, slot 2 SENT
t=120 the relay answers:
  recv ["OK",A,true,""] -> RL_ACK
  recv ["OK",A,true,"duplicate: already have this event"] -> RL_UNKNOWN
  recv ["OK",B,false,"rate-limited: slow down there chief"] -> RL_ACK
  recv ["OK","b1a649eb...",true,""] (no publish has this id) -> RL_UNKNOWN
  recv ["OK",C,false,""] (a refusal must carry a prefix) -> RL_BAD
  recv ["OK","b1a6",true,""] (the id is 2 bytes) -> RL_BAD
  t=120: slot 0 ACCEPTED, slot 1 REFUSED, slot 2 SENT
t=10049 rl_tick: RL_DO_NONE, timeouts 0
  t=10049: slot 0 ACCEPTED, slot 1 REFUSED, slot 2 SENT
t=10050 rl_tick: RL_DO_NONE, timeouts 1
  t=10050: slot 0 ACCEPTED, slot 1 REFUSED, slot 2 TIMEOUT
t=10100 a late answer:
  recv ["OK",C,true,"duplicate: already have this event"] -> RL_ACK
  t=10100: slot 0 ACCEPTED, slot 1 REFUSED, slot 2 ACCEPTED
t=10200 publish event B again: slot 0
  send ["EVENT",{"id":"c195de5e..."}] (358 bytes): event B, publish slot 0, OK due by t=20200
  t=10200: slot 0 SENT, slot 1 REFUSED, slot 2 ACCEPTED
t=10300 the connection closes; reconnect at t=10800
  t=10300: slot 0 SENT, slot 1 REFUSED, slot 2 ACCEPTED
t=10850 connection 2 open
  opened: slot 0 QUEUED, slot 1 REFUSED, slot 2 ACCEPTED
  send ["EVENT",{"id":"c195de5e..."}] (358 bytes): event B, publish slot 0, OK due by t=20850
t=10900 the relay answers:
  recv ["AUTH","c1"] -> RL_AUTH
  recv ["OK",B,false,"auth-required: we only accept events from registered users"] -> RL_ACK
  t=10900: slot 0 AUTH, slot 1 REFUSED, slot 2 ACCEPTED
  send ["AUTH",{"id":"637c0d50...","kind":22242,...}] (409 bytes), tags [["relay","wss://relay.example.com"],["challenge","c1"]]
t=10950 the relay answers:
  recv ["OK",<the AUTH event's id>,true,""] -> RL_AUTH
  t=10950: slot 0 QUEUED, slot 1 REFUSED, slot 2 ACCEPTED
  send ["EVENT",{"id":"c195de5e..."}] (358 bytes): event B, publish slot 0, OK due by t=20950
t=11000 the relay answers:
  recv ["OK",B,true,""] -> RL_ACK
  t=11000: slot 0 ACCEPTED, slot 1 REFUSED, slot 2 ACCEPTED
decisions: gdgdllgjgjg
slots: 111 222 342 342 345 343 243 243 143 643 143 343
deadlines: 10050 10050 10050 20200 20850 20950
```

The transcript matches the hand trace line for line. The deliberate
refusals in it are:

- an id of 31 bytes: `rl_publish` returns -1;
- an `OK` for an event this client never published, and a second `OK` for
  one already settled: `RL_UNKNOWN`, and nothing changes;
- `OK false` with an empty message, which NIP-01 forbids: `RL_BAD`;
- an `OK` whose id is 2 bytes: `RL_BAD`;
- `rate-limited:`: `RP_REFUSED`, with no retry by the machine;
- no `OK` by the deadline: `RP_TIMEOUT`, the outcome unknown until the late
  answer comes.

## How the code is checked

### Checks in the example

The checks are in
[nostr/examples/checks/publish-ok.coax](../examples/checks/publish-ok.coax),
which loads the example. They compare the code with values from outside it:

- the session's decisions, slot states and deadlines against the hand trace
  above, each deadline written as its own sum;
- NIP-01's eleven `OK` examples, copied from the NIP with the abbreviated
  id written in full: each reads with the prefix word the NIP lists, the
  three with `true` make the publish `RP_ACCEPTED`, and the eight with
  `false` make it `RP_REFUSED`, whatever the word;
- NIP-42's "`auth-required` in response to an `EVENT` message" exchange,
  line by line: `rl_next` sends `EVENT`, then `AUTH`, then the same `EVENT`,
  and the publish ends accepted;
- a timed-out publish is not sent again after a reconnect, a late `OK` on
  the new connection still settles it, and publishing a timed-out event
  again sends it with a new deadline.

<!-- source: nostr/examples/checks/publish-ok.coax check -->
```jai
// The session's trace, worked out by hand in the chapter before the program
// ran. Decisions: 'a' + the RL_ code (d UNKNOWN, g ACK, j AUTH, l BAD).
// Slots: publish slots 0, 1 and 2 after each step (1 QUEUED, 2 SENT,
// 3 ACCEPTED, 4 REFUSED, 5 TIMEOUT, 6 AUTH).
HAND_DECISIONS :: "gdgdllgjgjg";
HAND_SLOTS :: "111 222 342 342 345 343 243 243 143 643 143 343";
#test "the scripted session follows the hand trace" #steps 2000000000 {
  t := po_session(false);
  #assert cast(string) t.decisions[0..<t.nds] == HAND_DECISIONS;
  #assert cast(string) t.states[0..<t.ns - 1] == HAND_SLOTS;
  // Each OK deadline is the send time plus ack_ms, 10000.
  hand := [6] int.[50 + 10000, 50 + 10000, 50 + 10000, 10200 + 10000, 10850 + 10000, 10950 + 10000];
  #assert t.nd == 6;
  for k: 0..<6 { #assert t.deadlines[k] == hand[k]; }
}

// NIP-01's eleven OK examples, byte for byte, except that the id the NIP
// abbreviates as "b1a649ebe8..." is written in full.
NIP01_OK :: [11] string.[
  "[\"OK\", \"b1a649ebe8b435ec71d3784793f3bbf4b93e64e17568a741aecd4c7ddeafce30\", true, \"\"]",
  "[\"OK\", \"b1a649ebe8b435ec71d3784793f3bbf4b93e64e17568a741aecd4c7ddeafce30\", true, \"pow: difficulty 25>=24\"]",
  "[\"OK\", \"b1a649ebe8b435ec71d3784793f3bbf4b93e64e17568a741aecd4c7ddeafce30\", true, \"duplicate: already have this event\"]",
  "[\"OK\", \"b1a649ebe8b435ec71d3784793f3bbf4b93e64e17568a741aecd4c7ddeafce30\", false, \"blocked: you are banned from posting here\"]",
  "[\"OK\", \"b1a649ebe8b435ec71d3784793f3bbf4b93e64e17568a741aecd4c7ddeafce30\", false, \"blocked: please register your pubkey at https://my-expensive-relay.example.com\"]",
  "[\"OK\", \"b1a649ebe8b435ec71d3784793f3bbf4b93e64e17568a741aecd4c7ddeafce30\", false, \"rate-limited: slow down there chief\"]",
  "[\"OK\", \"b1a649ebe8b435ec71d3784793f3bbf4b93e64e17568a741aecd4c7ddeafce30\", false, \"invalid: event creation date is too far off from the current time\"]",
  "[\"OK\", \"b1a649ebe8b435ec71d3784793f3bbf4b93e64e17568a741aecd4c7ddeafce30\", false, \"pow: difficulty 26 is less than 30\"]",
  "[\"OK\", \"b1a649ebe8b435ec71d3784793f3bbf4b93e64e17568a741aecd4c7ddeafce30\", false, \"restricted: not allowed to write.\"]",
  "[\"OK\", \"b1a649ebe8b435ec71d3784793f3bbf4b93e64e17568a741aecd4c7ddeafce30\", false, \"error: could not connect to the database\"]",
  "[\"OK\", \"b1a649ebe8b435ec71d3784793f3bbf4b93e64e17568a741aecd4c7ddeafce30\", false, \"mute: no one was listening to your ephemeral event and it wasn't handled in any way, it was ignored\"]"];
// NIP-01: "true when an event has been accepted by the relay, false
// otherwise". The prefix words as the NIP lists them (1 duplicate, 2 pow,
// 3 blocked, 4 rate-limited, 5 invalid, 6 restricted, 7 mute, 8 error).
NIP01_STORED :: [11] bool.[true, true, true, false, false, false, false, false, false, false, false];
NIP01_PREFIX :: [11] int.[0, 2, 1, 3, 3, 4, 5, 2, 6, 8, 7];
#test "NIP-01's OK examples: the verdict alone decides stored or refused" {
  id: [32] u8; x := hex_decode_checked(PO_STRANGER, id[..], false);
  for k: 0..<11 {
    r: [1] Relay; g := rl_open_at(r[..], 0);
    #assert rl_publish(r[..], id[..]) == 0 && rl_next(r[..], 0).kind == RL_OUT_EVENT;
    m := nostr_message_read(NIP01_OK[k]);
    #assert m.error == NM_OK && m.prefix == NIP01_PREFIX[k];
    scratch: [64] u8;
    #assert rl_input(r[..], g, rl_read(NIP01_OK[k], scratch[..], false)) == RL_ACK;
    #assert r[0].pubs[0].state == ifx NIP01_STORED[k] then RP_ACCEPTED else RP_REFUSED;
  }
}

// NIP-42's "auth-required in response to an EVENT message" flow, with the
// NIP's abbreviated ids "012345..." and "abcdef..." padded with zeros.
#test "NIP-42's EVENT flow: refused, AUTH, the same EVENT again, accepted" {
  r: [1] Relay; n: [1] Nip42; g := rl_open_at(r[..], 0);
  e := nip42_init(n[..], PO_RELAY); #assert e == N42_OK;
  ev: [32] u8; x := hex_decode_checked("0123450000000000000000000000000000000000000000000000000000000000", ev[..], false);
  au: [32] u8; y := hex_decode_checked("abcdef0000000000000000000000000000000000000000000000000000000000", au[..], false);
  scratch: [64] u8;
  #assert nip42_input(r[..], n[..], g, "[\"AUTH\", \"<challenge>\"]") == RL_AUTH;      // relay: ["AUTH", "<challenge>"]
  #assert rl_publish(r[..], ev[..]) == 0;
  o := rl_next(r[..], 0); #assert o.kind == RL_OUT_EVENT;                            // client: ["EVENT", {"id": "012345...", ...}]
  #assert rl_input(r[..], g, rl_read("[\"OK\", \"0123450000000000000000000000000000000000000000000000000000000000\", false, \"auth-required: we only accept events from registered users\"]", scratch[..], false)) == RL_ACK;
  #assert r[0].pubs[0].state == RP_AUTH;
  o = rl_next(r[..], 0); #assert o.kind == RL_OUT_AUTH && rl_auth_sent(r[..], au[..]);   // client: ["AUTH", {"id": "abcdef...", ...}]
  #assert rl_input(r[..], g, rl_read("[\"OK\", \"abcdef0000000000000000000000000000000000000000000000000000000000\", true, \"\"]", scratch[..], false)) == RL_AUTH;
  o = rl_next(r[..], 0); #assert o.kind == RL_OUT_EVENT && o.slot == 0;              // client: ["EVENT", {"id": "012345...", ...}]
  #assert rl_input(r[..], g, rl_read("[\"OK\", \"0123450000000000000000000000000000000000000000000000000000000000\", true, \"\"]", scratch[..], false)) == RL_ACK;
  #assert r[0].pubs[0].state == RP_ACCEPTED && rl_next(r[..], 0).kind == RL_OUT_NONE;
}

#test "a timed-out publish is not sent again on reconnect; a late OK still settles it" {
  r: [1] Relay; g := rl_open_at(r[..], 0); a := rl_id_of(40);
  #assert rl_publish(r[..], a[..]) == 0 && rl_next(r[..], 0).kind == RL_OUT_EVENT;
  #assert rl_tick(r[..], 10000) == RL_DO_NONE && r[0].pubs[0].state == RP_TIMEOUT;
  #assert rl_dropped(r[..], g, 10000, 0);
  g = rl_start(r[..], 10500); #assert rl_opened(r[..], g, 10500);
  #assert r[0].pubs[0].state == RP_TIMEOUT && rl_next(r[..], 10500).kind == RL_OUT_NONE;
  #assert rl_input(r[..], g, RlIn.{kind = RL_IN_OK, id = a, accepted = true}) == RL_ACK && r[0].pubs[0].state == RP_ACCEPTED;
  // or the caller publishes a timed-out event again: a new attempt, sent
  // with a new deadline (slot 0 is free again: its publish is settled)
  b := rl_id_of(41);
  #assert rl_publish(r[..], b[..]) == 0 && rl_next(r[..], 10500).kind == RL_OUT_EVENT && rl_tick(r[..], 20500) == RL_DO_NONE;
  #assert r[0].pubs[0].state == RP_TIMEOUT && rl_publish(r[..], b[..]) == 0 && r[0].pubs[0].state == RP_QUEUED;
  #assert rl_next(r[..], 21000).kind == RL_OUT_EVENT && r[0].pubs[0].deadline == 31000;
}
```

To run them:

```shell
coax/bin/coax test nostr/examples/checks/publish-ok.coax --run-steps 250000000
```

On 2026-11-01 this reported 399 of 399 checks passed, including the checks
of every file the example loads. The session check signs four events at
compile time and carries `#steps 2000000000`.

### The library's own checks

[relay.coax](../../coax/lib/nostr/relay.coax) has 14 checks. The publish
ones cover `OK true`, `OK false`, a timeout at its exact deadline, a late
`OK`, unknown ids, the same event published twice, a malformed id, slot
reuse when the queue is full, a resend after a reconnect, and the new
`auth-required:` check: waiting, no timeout while waiting, one retry after
an accepted `AUTH`, a second refusal being final, a fresh retry for a new
publish of the same event, and a resend on a new connection.
[relay_read.coax](../../coax/lib/nostr/relay_read.coax) checks that
`auth-required:` is matched exactly (case and colon included) on `OK` and
`CLOSED`, and never on `NOTICE`.

Because `relay.coax` changed, its mutation run was repeated on 2026-11-01:
578 mutants, 578 killed, none marked equivalent. The
[relay client record](../../docs/verification/2026-10-29-nostr-relay-client.md)
has the earlier run (555 of 555) and a part for this change. The checks of
`relay_read.coax` (266), `nip42.coax` (49) and chapter 29's example (299)
pass with the change. The
[2026-10-30 relay_read record](../../docs/verification/2026-10-30-relay-read.md)
(5,371 of 5,371) and `nip42.coax`'s mutation run were not repeated: neither
file changed. Root-only mutation of this chapter's checks file reports no
mutation sites.

## Summary

- A publish is keyed by its event id. The same id while pending is the same
  publish; a settled one can be published again as a new attempt.
- `rl_next` sends a queued publish and sets its deadline to `now + ack_ms`.
- An `OK` is matched by id to a sent or timed-out publish. Anything else is
  `RL_UNKNOWN`, and a malformed `OK` is `RL_BAD`.
- The verdict decides: `true` is stored, including `duplicate:`; `false` is
  refused. The machine acts on one prefix, `auth-required:`, which waits for
  `AUTH` and sends the publish once more. The caller reads the others.
- No `OK` by the deadline is `RP_TIMEOUT`: unknown, not refused. A late `OK`
  still settles it.
- Sending the same signed event again is safe: a relay that has it answers
  `duplicate:`. A reconnect resends publishes that were sent but unanswered.

## Dependencies

- [29. A relay connection](ch029-a-relay-connection.md)
- [21. Events, IDs and signatures](ch021-events-ids-and-signatures.md)

## Sources

- [NIP-01](../nips/official/01.md), the `OK` message, its examples and the
  standard prefixes.
- [NIP-42](../nips/official/42.md), the `auth-required:` and `restricted:`
  prefixes and the write flow with `AUTH`.
- [NIP-13](../nips/official/13.md), proof of work, for the `pow:` prefix.
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
