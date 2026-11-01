# 101. The Verse and the wallet

The Verse is where people meet: realms that players walk through together,
and a wallet that pays. This chapter shows how the Verse uses the Nostr
stack for two things. Players see each other through NIP-MV pose frames
that a Coax relay carries, and one player zaps another in the world, with
every payment decided by the Spark wallet's existing rules in
`verse/wallet.coax`. It runs both once, offline, with test keys, fixture
invoices and a fixed clock, by composing units that earlier chapters built.
It also says plainly which parts OpenAgents runs in production today and
which exist only in this book.

## What this chapter teaches

- Presence: NIP-MV pose frames published to a Coax relay, sent to every
  subscription that asks for the world, and drawn a fixed delay behind, as
  [Aethyr chapter 49](../../aethyr/book/ch049-rooms-and-presence.md) plans.
- The latency budget: when a move made on one screen is drawn on another,
  worked out on a fixed clock.
- An in-world zap from the zap request to the counted receipt, with the
  wallet's rules deciding whether it is paid and the balance it leaves.
- Wallet Connect (NIP-47) as a second wallet path, held to its own budget.
- Where each layer refuses: the relay, the client's ordering, the
  subscription, the wallet's rules, the receipt checks and the NWC budget.
- What runs in production today, and what is only in this book.

## Where the code is

No library code was written for this chapter. The example composes these
units, and adds only the glue between them (`publish`, `receive`, `draw_x`,
`zap_check` and `nwc_pay`, in the example):

| File | Procedure | What it decides | Chapter |
| --- | --- | --- | --- |
| [event_sign.coax](../../coax/lib/nostr/event_sign.coax), [event.coax](../../coax/lib/nostr/event.coax) | `nostr_event_sign_step`, `nostr_event_verify_vartime` | signing and verifying events | 13, 21 |
| [relay_server.coax](../../coax/lib/nostr/relay_server.coax), [store.coax](../../coax/lib/nostr/store.coax), [relay_route.coax](../../coax/lib/nostr/relay_route.coax) | `rs_admit_read`, `rs_admit_verified`, `rsd_admit`, `rr_fanout`, `rr_deliver` | whether the relay accepts an event, and who gets it | 34, 35, 38 |
| [filter.coax](../../coax/lib/nostr/filter.coax) | `nf_match`, `nf_write` | whether a subscription asks for an event | 23 |
| [nipmv.coax](../../coax/lib/nostr/nipmv.coax) | `mv_frame_read`, `mv_order`, `mv_online` | a pose frame for this world, its order, and presence | 85 |
| [nip57.coax](../../coax/lib/nostr/nip57.coax), [nip75.coax](../../coax/lib/nostr/nip75.coax) | `n57_pay`, `n57_request_tags`, `n57_invoice`, `n57_receipt`, `n75_tally` | the zap request, the receipt, and the goal's count | 71 |
| [nip47.coax](../../coax/lib/nostr/nip47.coax) | `n47_request`, `n47_spend`, `n47_code` | a Wallet Connect payment against its budget | 72 |
| [verse/wallet.coax](../../verse/wallet.coax) | `verse_wallet_msat_up`, `verse_wallet_msat_down`, `verse_wallet_amount_ok`, `bw_invoice_is`, `verse_wallet_may_send`, `verse_wallet_step` | whether the Spark wallet pays, and how the payment moves | this chapter |

`verse/wallet.coax`, `nip57.coax`, `nip47.coax`, `relay_server.coax` and
`relay_route.coax` decide money, admission or policy. Each is an
`#authority` file whose checks came first and whose mutation record has no
survivors; "How the code is checked" lists the records. Words and keys
never enter `verse/wallet.coax`: the host hands it amounts, fees and
times, and it answers with verdicts.

## What runs today and what is only in this book

In production today:

- **The Spark wallet.** Since 2026-10-28 the iOS app
  (`apps/openagents-ios`) sends and receives on Breez's Spark SDK on
  mainnet through `Wallet/SparkHost.swift`, and every payment rule is
  `verse/wallet.coax`: the keypad, LNURL-pay bounds through
  `verse_wallet_amount_ok`, the high-fee and expiry warnings,
  `verse_wallet_may_send` and the state machine `verse_wallet_step`
  ([docs/lightning/](../../docs/lightning/README.md)). It pays BOLT11
  invoices, Lightning addresses and LNURL-pay, and buys bitcoin through
  MoonPay and Cash App pages that `wallet.coax` checks. The 12 words that
  restore the wallet are the same words that derive the NIP-06 Nostr key,
  so one backup covers both.
- **Presence.** Realms on the web post their position over HTTP to
  [coax/web/api/room.c](../../coax/web/api/room.c), which answers with the
  others from SQL. Fairy Forest
  ([fairyroom.coax](../../apps/openagents-web/fairyroom.coax)) posts every
  0.25 seconds and draws the ten nearest others.
  [coax/web/presence-pair.coax](../../coax/web/presence-pair.coax) runs
  two headless Chromes and fails when either sees the other move later
  than its millisecond budget.
- **The Coax relay** runs as a tailnet preview (R1); `relay.openagents.com`
  is not yet the Coax relay (chapter 103).

Only in this book, today:

- NIP-MV pose frames: no realm publishes or reads them, and no relay
  carries them for a realm.
- Zaps: the app pays a Lightning address, but it sends no NIP-57 zap
  request with the payment and reads no receipts. Goals (NIP-75) are not
  shown in any realm.
- Wallet Connect: neither the app nor a realm opens a NIP-47 connection.
- `zap_check`, the glue that hands a zap to the wallet's rules, is in the
  example, not in `verse/wallet.coax`.

## The session

Two players meet in the plaza, chapter 85's world, and one zaps the other.
The keys are published test scalars:

| Key | Role | x-only public key |
| --- | --- | --- |
| 1 | owns the plaza (chapter 85) | `79be667e...16f81798` |
| 2 | Ana | `c6047f94...5c709ee5` |
| 3 | Mallory | `f9308a01...bce036f9` |
| 4 | Ben | `e493dbf1...e8c4cd13` |
| 7 | Ben's LNURL server, which signs his receipts | `5cbdf064...cac4f9bc` |

The clock starts at `T0 = 1790000000` seconds; pose frames count
milliseconds, written here as `+ms` after `T0`. Every network hop takes an
assumed 40 ms. Ana's Spark wallet reports 1,000 spendable sats and quotes
a 1-sat fee.

| Time | Step | Layer | Signer |
| --- | --- | --- | --- |
| +1000 ms | Ana's pose frame 1, kind `23300` | NIP-MV, NIP-01 | Ana |
| +1100 ms | Ben's pose frame 1 | NIP-MV | Ben |
| +1250 ms | Ana's pose frame 2 | NIP-MV | Ana |
| T0 | Ben's goal, kind `9041`, 50,000 msat | NIP-75 | Ben |
| T0 + 60 | Ana's zap request, kind `9734`, 21,000 msat | NIP-57 | Ana |
| T0 + 61 | the invoice, `lnbc210n...` | BOLT #11 | Ben's server |
| T0 + 62 | the wallet's rules, the payment | `verse/wallet.coax` | none |
| T0 + 70 | the zap receipt, kind `9735` | NIP-57 | Ben's server |

## The trace by hand

### The subscription

Each client asks the relay for pose frames in the plaza: kind `23300`
whose `w` tag is the plaza's address.

```text
["REQ","plaza",{"kinds":[23300],"#w":["33300:79be667e...16f81798:plaza"]}]
```

Kind `23300` is ephemeral (20000 to 29999), so the relay sends a frame to
the matching subscriptions and stores nothing. NIP-01 sends an event to
every matching subscription, including its author's, so each client skips
its own frames.

### When a move is drawn

Each client posts a frame every `Δ = 250` ms and draws others `D = 2Δ =
500` ms behind the time `t` written in their frame. A frame is drawn at
`t + D`, or when it arrives if that is later.

1. Ana's frame 1 has `t = +1000`. It reaches the relay at +1040 and Ben at
   +1080. Ben draws it at `max(1000 + 500, 1080) = +1500`.
2. Ana moves at +1001, just after frame 1 left. Frame 2 carries the move
   with `t = +1250`; Ben gets it at +1330 and draws it at +1750.
3. The move took `1750 - 1001 = 749` ms to appear on Ben's screen. The
   bound from Aethyr chapter 49 is `Δ + 2 · 40 + D = 250 + 80 + 500 =
   830` ms, inside the one-second budget that `presence-pair.coax`
   enforces.

At +1600 Ben draws Ana at `t = 1600 - 500 = +1100`, between frame 1
(x 3.2 at +1000) and frame 2 (x 4.2 at +1250):

```text
x = 3.2 + (4.2 - 3.2) · (1100 - 1000) / (1250 - 1000) = 3.2 + 0.4 = 3.6
```

Ana last heard from Ben at +1180. NIP-MV marks a player offline after
10,000 ms without a frame: at +11179 the gap is 9,999 ms (online), and at
+11180 it is 10,000 ms (offline).

### The zap's amounts

Ben's LNURL server accepts 1,000 to 100,000,000 msat. Spark pays whole
sats, so the wallet rounds the bounds inward:
`verse_wallet_msat_up(1000) = 1` and
`verse_wallet_msat_down(100000000) = 100000` sats.

1. `verse_wallet_amount_ok(21, 1, 100000, 1000)`: 21 is at least 1, at
   most 100,000, and at most the 1,000 spendable. It passes.
2. The invoice's human-readable part is `lnbc210n`: mainnet (`lnbc`), and
   210 units of `n`, 100 msat each, which is 21,000 msat. `bw_invoice_is`
   checks `21000 % 100 = 0` and `21000 / 100 = 210`. It passes.
3. `verse_wallet_may_send(quoted, 21, 1, 1, 1000, T0 + 62, T0 + 3661)`:
   the state is `quoted`, 21 and 1 are valid, `T0 + 62` is before the
   invoice expires (`T0 + 61` plus BOLT #11's default 3,600 s), the fee 1
   is not above the shown fee 1, and `21 <= 1000 - 1`. The verdict is
   `send`.
4. The flow moves `idle → quoted → sending` with one send. A second
   confirm changes nothing. When the SDK reports the payment sent, the
   state is `succeeded`, and the balance the SDK reports is
   `1000 - 21 - 1 = 978` sats.

### The receipt

Ana's zap request is 669 bytes of JSON. The invoice's `h` field is the
SHA-256 of those bytes, `b008866d...17ef469e`, and its `p` field is the
SHA-256 of the preimage `01 02 ... 20`, `ae216c2e...b7a612c9`. Ben's
server publishes the receipt, and `n57_receipt` checks four things: the
signer is the `nostrPubkey` of Ben's pay response (key 7); the invoice's
amount equals the request's `amount` tag (21,000 msat); `h` is the hash of
the request in the `description` tag; and the preimage hashes to `p`. The
goal's tally counts each payment hash once.

### The event ids

| Event | id |
| --- | --- |
| Ana's frame 1 | `4dee2ad7f7fd4a23c17f9b6debe1b9e254d456f42dea11ddff41a3b164ab1312` |
| Ben's frame 1 | `5e0e48c558433980dd5b84a84d04784ddef1aefaeda2d8bfec44dc4bf2d5f56c` |
| Ana's frame 2 | `985ab81e495f312c04589db5f349d7a114dedfd5726cac93e9a215abd33f75ea` |
| Ben's goal | `138afa67d5c7f0c0a17eec7203cde3e7c234a6fb6be5e3b0e71b25df54736ff8` |
| Ana's zap request | `f72c3d9c908806583cdd040a441709bc4ec3085ef705024d25db1ed875b109b9` |
| the receipt | `a462e95d81fccb0c3d26d80f86bf54ac011ecfe64822f66eaea5f80653f7aaa3` |

Each is the SHA-256 of `[0,pubkey,created_at,kind,tags,content]`
(chapter 21), and the checks compare every one with an independent Python
computation.

## Refusals

Each layer refuses what it owns:

| Input | Layer | Refusal |
| --- | --- | --- |
| Ana's frame 1 sent again | the client's ordering (`mv_order`) | `dropped: n not newer` (the relay accepts it: it stores no ephemeral events, so it can't see the repeat) |
| Ana's frame 1 with `-10.5` changed to `-90.5` after signing | the relay | `invalid: event id does not match its content` |
| a frame for the garden, another world | the subscription (`nf_match`) | accepted by the relay, delivered to nobody |
| a receipt signed by Mallory | `n57_receipt` | not signed by the server's `nostrPubkey` |
| a receipt with another payment's preimage | `n57_receipt` | the preimage does not hash to the invoice's payment hash |
| the same receipt published twice | `n75_tally` | counted once, one duplicate |
| 2,000 sats with 978 left | the wallet's rules | `amount_ok` false and `over_limit` |
| a fee of 3 sats when the review screen showed 1 | the wallet's rules | `over_cap` |
| paying at the invoice's expiry | the wallet's rules | `expired` |
| an invoice for 20 sats on a 21-sat zap | the wallet's rules (`bw_invoice_is`) | invoice refused |
| a testnet invoice (`lntb`) | the wallet's rules | invoice refused |
| 21,500 msat, not whole sats | the wallet's rules | amount and invoice refused |
| the 21,000 msat zap over NWC with 12,000 msat a day | `n47_spend` | over the budget, answered `QUOTA_EXCEEDED` |

The 2,000-sat zap is the refusal "Done means" asks for: the zap request is
over the wallet's limits, and `verse/wallet.coax` refuses it.

## Wallet Connect as a second path

The Spark wallet in the app is the first wallet. NIP-47 lets a realm or an
agent ask another wallet service to pay, with a connection secret and a
budget the wallet service holds (chapter 72). The example charges the
same invoice to a connection: 12,000 msat a day refuses it, and 50,000
msat a day takes it twice (21,000, then 42,000 spent). Sealing the request
with NIP-44 and signing it are left out here; chapter 72 runs them. A
Spark-backed NWC service would apply both its own budget and
`verse/wallet.coax`'s rules; that service is not built.

## The example

The example is
[nostr/examples/verse-and-wallet.coax](../examples/verse-and-wallet.coax).
The constants come first:

<!-- source: nostr/examples/verse-and-wallet.coax setup -->
```jai
// Chapter 101: two players' presence through an in-memory Coax relay, then
// one in-world zap from Ana to Ben, from the request through the wallet's
// rules to the counted receipt, and the same payment over Wallet Connect.
// Offline: test keys, fixture invoices and a fixed clock. The event ids and
// the invoice come out the same in coax/tests/nostr/verse_wallet_vectors.py.
#load "../../coax/lib/nostr/nipmv.coax";
#load "../../coax/lib/nostr/nip75.coax";
#load "../../coax/lib/nostr/nip47.coax";
#load "../../coax/lib/nostr/event_sign.coax";
#load "../../coax/lib/nostr/store.coax";
#load "../../coax/lib/nostr/relay_route.coax";
#load "../../coax/lib/nostr/filter.coax";
#load "../../verse/wallet.coax";

// Test keys only, the secp256k1 scalars 2 (Ana), 3 (Mallory), 4 (Ben) and
// 7 (Ben's LNURL server). The plaza is chapter 85's world, owned by key 1.
// Never sign with a real key in an example, and never print one.
PEOPLE :: [4] string.["Ana", "Mallory", "Ben", "Ben's server"];
SCALAR :: [4] u8.[2, 3, 4, 7];
PUB :: [4] string.[
  "c6047f9441ed7d6d3045406e95c07cd85c778e4b8cef3ca7abac09b95c709ee5",
  "f9308a019258c31049344f85f89d5229b531c845836f99b08601f113bce036f9",
  "e493dbf1c10d80f3581e4904930b1404cc6c13900ee0758474fa94abe8c4cd13",
  "5cbdf0646e5db4eaa398f365f2ea7a0e3d419b7e0330e39ce92bddedcac4f9bc"];
ANA :: 0; MALLORY :: 1; BEN :: 2; SERVER :: 3;
T0 :: 1790000000;                       // the fixed clock, in seconds
MS :: 1790000000000;                    // the same instant in milliseconds
RELAY :: "wss://relay.example.com";
GARDEN :: "33300:79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798:garden";

// The presence budget. Aethyr chapter 49: a move is drawn on the other
// screen within one second (coax/web/presence-pair.coax's budget).
POST_MS :: 250;     // a client posts a frame every 250 ms (Fairy Forest's FF_ROOM_EVERY)
NET_MS :: 40;       // assumed one-way time, client to relay and relay to client
DRAW_MS :: 500;     // remote players are drawn D = 2 x 250 ms behind
BUDGET_MS :: 1000;

// Fixture values only: the payment's preimage is the bytes 01 02 ... 20,
// and every invoice carries a zero signature, so no wallet would pay it.
PREIMAGE :: "0102030405060708090a0b0c0d0e0f101112131415161718191a1b1c1d1e1f20";
OTHER_PREIMAGE :: "2222222222222222222222222222222222222222222222222222222222222222";
// The LNURL-pay response for ben@zaps.example.com: 1 to 100,000 sats, and
// the server's key signs the receipts.
PAY :: "{\"tag\":\"payRequest\",\"callback\":\"https://zaps.example.com/lnurlp/ben/callback\",\"minSendable\":1000,\"maxSendable\":100000000,\"metadata\":\"[[\\\"text/plain\\\",\\\"Ben\\\"]]\",\"allowsNostr\":true,\"nostrPubkey\":\"5cbdf0646e5db4eaa398f365f2ea7a0e3d419b7e0330e39ce92bddedcac4f9bc\"}";
// What Ana's Spark wallet reports (the host's numbers, not decided here).
BALANCE :: 1000;    // spendable sats
FEE :: 1;           // the quote's fee in sats, shown on the review screen
EXPIRY :: 3600;     // BOLT #11's default invoice expiry, seconds
```

The relay is the Coax relay's EVENT path with the socket replaced: read
the event and recompute its id, apply the policy, verify the signature,
admit it to the store, and send it to each connection whose subscription
matches.

<!-- source: nostr/examples/verse-and-wallet.coax relay -->
```jai
// The Coax relay's EVENT path (chapters 34 and 35), with the socket and clock
// replaced, and two connections that each hold one subscription.
Conn :: struct { auth: [1] RrAuth; sub: string; filter: NostrFilter; inbox: [4][4096] u8; sizes: [4] int; n: int; }
Relay :: struct {
  arena: [16384] u8; recs: [5] RsdRec; store: [1] RsdStore;
  scratch: [65536] u8; out: [512] u8;
  conns: [2] Conn;
}
// One EVENT frame from connection `from`: the OK the relay answers, and,
// when the event is accepted, the event sent to every connection whose
// subscription matches it, the author's own included (NIP-01).
publish :: (r: [] Relay, from: int, event: string, now: int) -> string {
  frame: [4096] u8;
  n := json_raw(frame[..], 0, "[\"EVENT\","); n = json_raw(frame[..], n, event); n = json_raw(frame[..], n, "]");
  f := cast(string) frame[0..<n];
  m := ms_read(f);
  #assert rr_message(m.error, m.type, m.has_sub).action == RR_EVENT;
  text := f[m.event_from..<m.event_to];
  // Read and recompute the id, then the policy, then the signature, then
  // the store (an ephemeral kind is accepted and never stored).
  a := rs_admit_read(text, now, rr_writes_need_auth(true), rr_authed(r[0].conns[from].auth[..]), r[0].scratch[..]);
  if a.reason == RS_BAD_EVENT { return text_of(ms_write_notice(rs_prefix(RS_BAD_EVENT), rs_text(RS_BAD_EVENT), r[0].out[..]), r[0].out[..]); }
  if a.reason == RS_ACCEPT { a = rs_admit_verified(a, nostr_event_verify_vartime(text, r[0].scratch[..], false)); }
  reason := a.reason;
  if reason == RS_ACCEPT { reason = rsd_admit(r[0].store[..], a, text); }
  if rr_fanout(reason) {
    for c: 0..<2 {
      k := r[0].conns[c].n;
      if rr_deliver(true, true, nf_match(r[0].conns[c].filter, a.e)) && k < 4 {
        r[0].conns[c].sizes[k] = ms_write_event(r[0].conns[c].sub, text, r[0].conns[c].inbox[k][..]);
        r[0].conns[c].n = k + 1;
      }
    }
  }
  id := a.e.id;
  return text_of(ms_write_ok(id[..], rs_accepted(reason), rs_prefix(reason), rs_text(reason), r[0].out[..]), r[0].out[..]);
}
```

A client verifies what the relay sends, skips its own frames, reads the
frame for its world, orders it by session and sequence number, and draws
it `D` behind:

<!-- source: nostr/examples/verse-and-wallet.coax presence -->
```jai
// One player's client: what it has seen of the others, by key and session.
Client :: struct { me: int; seen: [4] MvSeen; count: int; bodies: [4][1024] u8; got: int; x: [4] f64; t: [4] int; }
// A frame the relay delivered at arrive_ms (inbox entry k): check the
// event, skip the client's own, read the pose frame for the plaza, order
// it, and say when it is drawn: DRAW_MS after its own time t, or when it
// arrives if that is later.
receive :: (c: [] Client, conn: [] Conn, k: int, w: MvWorld, arrive_ms: int) {
  wire := cast(string) conn[0].inbox[k][0..<conn[0].sizes[k]];
  event := wire[12 + conn[0].sub.count..<wire.count - 1];     // ["EVENT","plaza",<event>]
  scratch: [8192] u8;
  e := nostr_event_verify_vartime(event, scratch[..], false);
  who := name_of(e.public_key);
  print("  % gets %'s frame at +% ms: ", PEOPLE[c[0].me], PEOPLE[who], arrive_ms - MS);
  if !e.verified { print("refused, bad signature\n"); return; }
  if who == c[0].me { print("its own, skipped\n"); return; }
  body := c[0].bodies[c[0].got][..]; c[0].got += 1;
  nb := json_unescape(event[e.content_from..<e.content_to], body);
  poses: [MV_ENTITIES] MvPose;
  f := mv_frame_read(e.kind, event[e.tags_from..<e.tags_to], cast(string) body[0..<nb], MV_PLAZA, w, poses[..]);
  if f.code != MvCode.ok { print("refused, %\n", MV_WORDS[cast(int) f.code]); return; }
  o := mv_order(c[0].seen[..], c[0].count, PUB[who], f.s, f.n, arrive_ms);
  if o.code != MvCode.ok { print("s % n %, %\n", f.s, f.n, MV_WORDS[cast(int) o.code]); return; }
  c[0].count = o.count;
  if f.n <= 2 { c[0].x[f.n] = poses[0].p[0]; c[0].t[f.n] = f.t; }
  drawn := f.t + DRAW_MS;
  if arrive_ms > drawn { drawn = arrive_ms; }
  print("s % n %, % at (%, %, %), drawn at +% ms\n", f.s, f.n, poses[0].id, poses[0].p[0], poses[0].p[1], poses[0].p[2], drawn - MS);
}
// Where a remote avatar is drawn at now_ms: D behind, between the two
// samples around that time, holding the last one past it (Aethyr 49).
draw_x :: (x1: f64, t1: int, x2: f64, t2: int, now_ms: int) -> f64 {
  at := now_ms - DRAW_MS;
  if at <= t1 { return x1; }
  if at >= t2 { return x2; }
  return x1 + (x2 - x1) * cast(f64) (at - t1) / cast(f64) (t2 - t1);
}
```

The zap glue hands the wallet's rules plain numbers in whole sats, as
`SparkHost.swift` does, and charges the NWC path to its budget:

<!-- source: nostr/examples/verse-and-wallet.coax zap -->
```jai
// The wallet's side of a zap, as the host hands it to verse/wallet.coax in
// whole sats: whether the LNURL server takes the amount and the balance
// covers it, whether the invoice states exactly the zap's amount on
// mainnet, and whether the confirmed quote (fee, the fee shown as cap,
// and when the invoice expires) may be paid. All three must hold.
ZapCheck :: struct { amount_ok: bool; invoice_ok: bool; verdict: Verdict; pays: bool; }
zap_check :: (pay: N57Pay, msat: int, invoice: string, fee: int, cap: int, spendable: int, now: int, expires: int) -> ZapCheck {
  c: ZapCheck;
  sats := msat / 1000;                          // Spark pays whole sats
  low := verse_wallet_msat_up(pay.min); high := verse_wallet_msat_down(pay.max);
  c.amount_ok = msat % 1000 == 0 && verse_wallet_amount_ok(sats, low, high, spendable);
  c.invoice_ok = bw_invoice_is(invoice, msat);
  c.verdict = verse_wallet_may_send(Pay.quoted, sats, fee, cap, spendable, now, expires);
  c.pays = c.amount_ok && c.invoice_ok && c.verdict == Verdict.send;
  return c;
}
show_check :: (label: string, c: ZapCheck) {
  print("%: amount %, invoice %, may_send % -> %\n", label, ifx c.amount_ok then "ok" else "refused",
    ifx c.invoice_ok then "ok" else "refused", VERDICT[cast(int) c.verdict], ifx c.pays then "pay" else "refused");
}
// The same invoice as a Wallet Connect pay_invoice request, charged to a
// connection's budget (chapter 72; sealing and signing are left out here).
nwc_pay :: (b: N47Budget, invoice: string, now: int) -> N47Spend {
  j: [2048] u8;
  n := json_raw(j[..], 0, "{\"method\":\"pay_invoice\",\"params\":{\"invoice\":\"");
  n = json_raw(j[..], n, invoice); n = json_raw(j[..], n, "\"}}");
  return n47_spend(b, now, n47_request(cast(string) j[0..<n]));
}
```

The session:

<!-- source: nostr/examples/verse-and-wallet.coax main -->
```jai
main :: () {
  hx: [64] u8;
  w := mv_plaza();
  print("== 1. Presence in the plaza through a Coax relay\n");
  r: [1] Relay;
  r[0].store[0].arena = r[0].arena[..]; r[0].store[0].recs = r[0].recs[..];
  kinds: [1] int; values: [1] string; tags: [1] NostrTagFilter;
  f := plaza_filter(kinds[..], values[..], tags[..]);
  fb: [512] u8; nf := nf_write(f, fb[..], false);
  print("each client sends [\"REQ\",\"plaza\",%]\n", cast(string) fb[0..<nf]);
  for c: 0..<2 {
    r[0].conns[c].sub = "plaza"; r[0].conns[c].filter = f;
    pub := key_of(ifx c == 0 then ANA else BEN);
    #assert rr_auth(r[0].conns[c].auth[..], RR_N42_OK, pub[..]) == RR_A_OK;   // as if NIP-42 AUTH had succeeded
  }
  print("connection 0 is Ana's, 1 is Ben's; both authenticated with NIP-42 (chapter 38)\n");
  fe: [4][2048] u8;
  a1 := pose_frame(ANA, MV_PLAZA, "0,-1", "ana", 1, 1000, "3.2,0,-10.5", fe[0][..]);
  b1 := pose_frame(BEN, MV_PLAZA, "-2,0", "ben", 1, 1100, "-70,0,12", fe[1][..]);
  a2 := pose_frame(ANA, MV_PLAZA, "0,-1", "ana", 2, 1250, "4.2,0,-10.5", fe[2][..]);
  print("Ana's frame 1, id %\n", hex32(a1.id, hx[..]));
  print("  content: %\n", a1.original[a1.content_from..<a1.content_to]);
  ana: [1] Client; ana[0].me = ANA; ben: [1] Client; ben[0].me = BEN;
  print("+1000 ms Ana posts frame 1; relay at +1040: %\n", publish(r[..], 0, a1.original, T0 + 1));
  receive(ben[..], r[0].conns[1..<2], 0, w, MS + 1000 + 2 * NET_MS);
  receive(ana[..], r[0].conns[0..<1], 0, w, MS + 1000 + 2 * NET_MS);
  print("+1100 ms Ben posts frame 1; relay at +1140: %\n", publish(r[..], 1, b1.original, T0 + 1));
  receive(ana[..], r[0].conns[0..<1], 1, w, MS + 1100 + 2 * NET_MS);
  print("+1250 ms Ana posts frame 2 (she moved at +1001); relay at +1290: %\n", publish(r[..], 0, a2.original, T0 + 1));
  receive(ben[..], r[0].conns[1..<2], 2, w, MS + 1250 + 2 * NET_MS);
  drawn := 1250 + DRAW_MS;
  print("Ana's move at +1001 is on Ben's screen at +%: % ms (bound %, budget %)\n", drawn, drawn - 1001,
    POST_MS + 2 * NET_MS + DRAW_MS, BUDGET_MS);
  print("Ben draws Ana at +1600: x = %\n", draw_x(ben[0].x[1], ben[0].t[1] - MS, ben[0].x[2], ben[0].t[2] - MS, 1600));
  print("Ana sees Ben at +11179: %, at +11180: %\n", ifx mv_online(ana[0].seen[0].ms, MS + 11179) then "online" else "offline",
    ifx mv_online(ana[0].seen[0].ms, MS + 11180) then "online" else "offline");

  print("presence refusals\n");
  print("Ana's frame 1 sent again; relay: %\n", publish(r[..], 0, a1.original, T0 + 1));
  receive(ben[..], r[0].conns[1..<2], 3, w, MS + 1400);
  eb: [2048] u8;
  edited := cast(string) eb[0..<a1.original.count];
  for i: 0..<a1.original.count { eb[i] = a1.original[i]; }
  at := 0; for i: 0..<edited.count - 4 { if at == 0 && edited[i..<i + 4] == "-10." { at = i; } }
  eb[at + 1] = '9';                                    // -10.5 becomes -90.5 after signing
  print("Ana's frame 1 with p changed after signing; relay: %\n", publish(r[..], 0, edited, T0 + 1));
  gb: [2048] u8;
  garden := pose_frame(ANA, GARDEN, "0,-1", "ana", 3, 1500, "3.2,0,-10.5", gb[..]);
  before := r[0].conns[1].n;
  print("Ana's frame for the garden; relay: %\n", publish(r[..], 0, garden.original, T0 + 1));
  print("  delivered to Ben: % (his filter asks for the plaza only)\n", r[0].conns[1].n - before);

  print("\n== 2. Ana zaps Ben 21 sats in the plaza\n");
  url: [256] u8; ln: [400] u8;
  nu := n57_lnurlp("ben@zaps.example.com", url[..]);
  nl := n57_lnurl(cast(string) url[0..<nu], ln[..]);
  lnurl := cast(string) ln[0..<nl];
  print("ben@zaps.example.com -> %\n", cast(string) url[0..<nu]);
  pay := n57_pay(PAY);
  print("pay response: %, % to % msat, receipts signed by %\n", N57P_WHY[pay.status], pay.min, pay.max, PEOPLE[name_of(pay.nostr_pubkey)]);
  gbuf: [1500] u8;
  goal := sign(BEN, T0, 9041, "[[\"relays\",\"wss://relay.example.com\"],[\"amount\",\"50000\"],[\"closed_at\",\"1790086400\"]]", "Busking in the plaza", gbuf[..]);
  gid := hex32(goal.id, hx[..]); gidb: [64] u8; for i: 0..<64 { gidb[i] = gid[i]; } goal_id := cast(string) gidb[..];
  print("Ben's goal, kind 9041 %: 50000 msat\n", goal_id);
  rb: [4][1500] u8;
  zr := zap_request(ANA, T0 + 60, 21000, goal_id, lnurl, "Nice song", rb[0][..]);
  print("zap request %, % bytes, sent to the callback\n", hex32(zr.id, hx[..]), zr.original.count);
  ib: [4][600] u8;
  inv := make_invoice(21000, T0 + 61, PREIMAGE, zr.original, "lnbc", ib[0][..]);
  print("invoice: %\n", inv);
  v := n57_invoice(inv);
  print("  % msat, h = %\n", v.msat, hex32(v.description_hash, hx[..]));
  print("  sha256(zap request)   %\n", hex32(sha_of(zr.original), hx[..]));

  print("the wallet's rules (verse/wallet.coax), Ana has % sats\n", BALANCE);
  ok := zap_check(pay, 21000, inv, FEE, FEE, BALANCE, T0 + 62, T0 + 61 + EXPIRY);
  show_check("21 sats, fee 1", ok);
  fl := verse_wallet_start();
  fl = verse_wallet_step(fl, Event.quote); fl = verse_wallet_step(fl, Event.confirm);
  print("flow: quote, confirm -> % (sends %)\n", PAY_STATE[cast(int) fl.state], fl.sends);
  fl = verse_wallet_step(fl, Event.confirm);
  print("a second confirm -> % (sends %)\n", PAY_STATE[cast(int) fl.state], fl.sends);
  fl = verse_wallet_step(fl, Event.sent);
  balance := BALANCE - 21 - FEE;
  print("the SDK reports sent -> %; balance % - 21 - % = % sats\n", PAY_STATE[cast(int) fl.state], BALANCE, FEE, balance);

  print("the receipt, and Ben's goal in the plaza\n");
  rcb: [4][3000] u8;
  receipt := zap_receipt(SERVER, T0 + 70, zr, goal_id, inv, PREIMAGE, rcb[0][..]);
  print("receipt %\n", hex32(receipt.id, hx[..]));
  show_zap("  n57_receipt", n57_receipt(receipt, pay.nostr_pubkey, lnurl));
  set: [4] NostrEvent;
  set[0] = receipt;
  set[1] = zap_receipt(SERVER, T0 + 80, zr, goal_id, inv, PREIMAGE, rcb[1][..]);        // published again
  set[2] = zap_receipt(MALLORY, T0 + 90, zr, goal_id, inv, PREIMAGE, rcb[2][..]);       // not Ben's server
  set[3] = zap_receipt(SERVER, T0 + 95, zr, goal_id, inv, OTHER_PREIMAGE, rcb[3][..]);  // another payment's preimage
  show_zap("  signed by Mallory", n57_receipt(set[2], pay.nostr_pubkey, lnurl));
  show_zap("  wrong preimage", n57_receipt(set[3], pay.nostr_pubkey, lnurl));
  t := n75_tally(goal, set[..], pay.nostr_pubkey, lnurl, T0 + 100);
  print("goal: % of 50000 msat, counted %, duplicates %, refused %\n", t.msat, t.counted, t.duplicates, t.refused);

  print("\n== 3. Refusals by the wallet's rules\n");
  show_check("2000 sats with 978 left", zap_check(pay, 2_000_000, inv, FEE, FEE, balance, T0 + 62, T0 + 61 + EXPIRY));
  show_check("21 sats, fee 3 over the shown 1", zap_check(pay, 21000, inv, 3, FEE, balance, T0 + 62, T0 + 61 + EXPIRY));
  show_check("21 sats at the invoice's expiry", zap_check(pay, 21000, inv, FEE, FEE, balance, T0 + 61 + EXPIRY, T0 + 61 + EXPIRY));
  short := make_invoice(20000, T0 + 61, PREIMAGE, zr.original, "lnbc", ib[1][..]);
  show_check("an invoice for 20 sats", zap_check(pay, 21000, short, FEE, FEE, balance, T0 + 62, T0 + 61 + EXPIRY));
  testnet := make_invoice(21000, T0 + 61, PREIMAGE, zr.original, "lntb", ib[2][..]);
  show_check("a testnet invoice", zap_check(pay, 21000, testnet, FEE, FEE, balance, T0 + 62, T0 + 61 + EXPIRY));
  show_check("21500 msat (not whole sats)", zap_check(pay, 21500, inv, FEE, FEE, balance, T0 + 62, T0 + 61 + EXPIRY));

  print("\n== 4. The same zap through Wallet Connect\n");
  day := N47Budget.{limit = 12000, period = 86400, start = T0};
  s := nwc_pay(day, inv, T0 + 62);
  print("budget 12000 msat a day: %, % msat\n", E47[s.error], s.msats);
  if s.error != N47_OK { print("  answered %\n", N47_CODES[n47_code(s.error)]); }
  s = nwc_pay(N47Budget.{limit = 50000, period = 86400, start = T0}, inv, T0 + 62);
  print("budget 50000 msat a day: %, spent % of %\n", E47[s.error], s.budget.spent, s.budget.limit);
  s = nwc_pay(s.budget, inv, T0 + 63);
  print("the same again: %, spent % of %\n", E47[s.error], s.budget.spent, s.budget.limit);
}
```

Its output:

```text
== 1. Presence in the plaza through a Coax relay
each client sends ["REQ","plaza",{"kinds":[23300],"#w":["33300:79be667ef9dcbbac55a06295ce870b07029bfcdb2dce28d959f2815b16f81798:plaza"]}]
connection 0 is Ana's, 1 is Ben's; both authenticated with NIP-42 (chapter 38)
Ana's frame 1, id 4dee2ad7f7fd4a23c17f9b6debe1b9e254d456f42dea11ddff41a3b164ab1312
  content: {\"v\":1,\"s\":\"ana\",\"n\":1,\"t\":1790000001000,\"e\":[{\"id\":\"avatar\",\"role\":\"avatar\",\"p\":[3.2,0,-10.5],\"q\":[0,0,0,1]}]}
+1000 ms Ana posts frame 1; relay at +1040: ["OK","4dee2ad7f7fd4a23c17f9b6debe1b9e254d456f42dea11ddff41a3b164ab1312",true,""]
  Ben gets Ana's frame at +1080 ms: s ana n 1, avatar at (3.2, 0.0, -10.5), drawn at +1500 ms
  Ana gets Ana's frame at +1080 ms: its own, skipped
+1100 ms Ben posts frame 1; relay at +1140: ["OK","5e0e48c558433980dd5b84a84d04784ddef1aefaeda2d8bfec44dc4bf2d5f56c",true,""]
  Ana gets Ben's frame at +1180 ms: s ben n 1, avatar at (-70.0, 0.0, 12.0), drawn at +1600 ms
+1250 ms Ana posts frame 2 (she moved at +1001); relay at +1290: ["OK","985ab81e495f312c04589db5f349d7a114dedfd5726cac93e9a215abd33f75ea",true,""]
  Ben gets Ana's frame at +1330 ms: s ana n 2, avatar at (4.2, 0.0, -10.5), drawn at +1750 ms
Ana's move at +1001 is on Ben's screen at +1750: 749 ms (bound 830, budget 1000)
Ben draws Ana at +1600: x = 3.6
Ana sees Ben at +11179: online, at +11180: offline
presence refusals
Ana's frame 1 sent again; relay: ["OK","4dee2ad7f7fd4a23c17f9b6debe1b9e254d456f42dea11ddff41a3b164ab1312",true,""]
  Ben gets Ana's frame at +1400 ms: s ana n 1, dropped: n not newer
Ana's frame 1 with p changed after signing; relay: ["OK","4dee2ad7f7fd4a23c17f9b6debe1b9e254d456f42dea11ddff41a3b164ab1312",false,"invalid: event id does not match its content"]
Ana's frame for the garden; relay: ["OK","e70ff5b21a9cb5d714507a40123db14d4181078a5b409631e50d4abd3cd32186",true,""]
  delivered to Ben: 0 (his filter asks for the plaza only)

== 2. Ana zaps Ben 21 sats in the plaza
ben@zaps.example.com -> https://zaps.example.com/.well-known/lnurlp/ben
pay response: ok, 1000 to 100000000 msat, receipts signed by Ben's server
Ben's goal, kind 9041 138afa67d5c7f0c0a17eec7203cde3e7c234a6fb6be5e3b0e71b25df54736ff8: 50000 msat
zap request f72c3d9c908806583cdd040a441709bc4ec3085ef705024d25db1ed875b109b9, 669 bytes, sent to the callback
invoice: lnbc210n1p4tzwaapp54cskcth4y3ar0qkpxhh6y7drunxuvyy5yu8462l9333qfdaxztyshp5kqygvm23fy0yndxkah2yc756yv2gaxuzjhtfvs7efrrvj9l0g60qqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqqkmm3wn
  21000 msat, h = b008866d51491e49b4d6edd44c7a9a23148e9b8295d69643d948c6c917ef469e
  sha256(zap request)   b008866d51491e49b4d6edd44c7a9a23148e9b8295d69643d948c6c917ef469e
the wallet's rules (verse/wallet.coax), Ana has 1000 sats
21 sats, fee 1: amount ok, invoice ok, may_send send -> pay
flow: quote, confirm -> sending (sends 1)
a second confirm -> sending (sends 1)
the SDK reports sent -> succeeded; balance 1000 - 21 - 1 = 978 sats
the receipt, and Ben's goal in the plaza
receipt a462e95d81fccb0c3d26d80f86bf54ac011ecfe64822f66eaea5f80653f7aaa3
  n57_receipt: valid, 21000 msat from Ana
  signed by Mallory: refused, not signed by the server's nostrPubkey
  wrong preimage: refused, the preimage does not hash to the invoice's payment hash
goal: 21000 of 50000 msat, counted 1, duplicates 1, refused 2

== 3. Refusals by the wallet's rules
2000 sats with 978 left: amount refused, invoice refused, may_send over_limit -> refused
21 sats, fee 3 over the shown 1: amount ok, invoice ok, may_send over_cap -> refused
21 sats at the invoice's expiry: amount ok, invoice ok, may_send expired -> refused
an invoice for 20 sats: amount ok, invoice refused, may_send send -> refused
a testnet invoice: amount ok, invoice refused, may_send send -> refused
21500 msat (not whole sats): amount refused, invoice refused, may_send send -> refused

== 4. The same zap through Wallet Connect
budget 12000 msat a day: over the budget, 21000 msat
  answered QUOTA_EXCEEDED
budget 50000 msat a day: ok, spent 21000 of 50000
the same again: ok, spent 42000 of 50000
```

The book's checks run this program in debug and release builds and compare
the output with
[nostr/fixtures/verse-and-wallet.out](../fixtures/verse-and-wallet.out).

## How the code is checked

### Checks in the example

The checks are in
[nostr/examples/checks/verse-and-wallet.coax](../examples/checks/verse-and-wallet.coax),
which loads the example, so running the example doesn't run them. The
values they compare with come from
[coax/tests/nostr/verse_wallet_vectors.py](../../coax/tests/nostr/verse_wallet_vectors.py),
which writes
[verse_wallet_vectors.coax](../../coax/tests/nostr/verse_wallet_vectors.coax).
It computes the events with Python integers and `hashlib`, BIP-340 and
NIP-01 from `nip59_reference.py`, and bech32 and the BOLT #11 fields
written from their specifications. It shares no code with Coax.

<!-- source: nostr/examples/checks/verse-and-wallet.coax check -->
```jai
// The event ids, the lnurl and the invoice come from
// coax/tests/nostr/verse_wallet_vectors.py, written from NIP-01, NIP-57,
// LUD-16, BIP-173 and BOLT #11 with hashlib and Python integers; it shares
// no code with Coax.
#test "the pose frames are Python's, byte for byte" #steps 2000000000 {
  hx: [64] u8; b: [3][2048] u8;
  #assert PUB[ANA] == PY_ANA && PUB[BEN] == PY_BEN && PUB[SERVER] == PY_SERVER;
  #assert hex32(pose_frame(ANA, MV_PLAZA, "0,-1", "ana", 1, 1000, "3.2,0,-10.5", b[0][..]).id, hx[..]) == PY_FRAME_ANA_1;
  #assert hex32(pose_frame(ANA, MV_PLAZA, "0,-1", "ana", 2, 1250, "4.2,0,-10.5", b[1][..]).id, hx[..]) == PY_FRAME_ANA_2;
  #assert hex32(pose_frame(BEN, MV_PLAZA, "-2,0", "ben", 1, 1100, "-70,0,12", b[2][..]).id, hx[..]) == PY_FRAME_BEN_1;
}

#test "the zap request, invoice and receipt are Python's" #steps 2000000000 {
  hx: [64] u8; url: [256] u8; ln: [400] u8; gb: [1500] u8; rb: [1500] u8; ib: [600] u8; cb: [3000] u8;
  nu := n57_lnurlp("ben@zaps.example.com", url[..]);
  nl := n57_lnurl(cast(string) url[0..<nu], ln[..]);
  #assert cast(string) ln[0..<nl] == PY_LNURL;
  goal := sign(BEN, T0, 9041, "[[\"relays\",\"wss://relay.example.com\"],[\"amount\",\"50000\"],[\"closed_at\",\"1790086400\"]]", "Busking in the plaza", gb[..]);
  #assert hex32(goal.id, hx[..]) == PY_GOAL_ID;
  zr := zap_request(ANA, T0 + 60, 21000, PY_GOAL_ID, PY_LNURL, "Nice song", rb[..]);
  #assert hex32(zr.id, hx[..]) == PY_REQUEST_ID;
  #assert hex32(sha_of(zr.original), hx[..]) == PY_REQUEST_SHA;
  inv := make_invoice(21000, T0 + 61, PREIMAGE, zr.original, "lnbc", ib[..]);
  #assert inv == PY_INVOICE;
  #assert hex32(n57_invoice(inv).payment_hash, hx[..]) == PY_PAYMENT_HASH;
  #assert hex32(zap_receipt(SERVER, T0 + 70, zr, PY_GOAL_ID, inv, PREIMAGE, cb[..]).id, hx[..]) == PY_RECEIPT_ID;
}

#test "the hand-worked numbers" {
  // 21 sats and a 1-sat fee from 1000; 21000 msat is 210n.
  #assert BALANCE - 21 - FEE == 978 && verse_wallet_sat_to_msat(21) == 21000;
  #assert n57_invoice(PY_INVOICE).msat == 21000;
  // Ana's move at +1001 rides frame 2 (t = +1250) and is drawn at +1750.
  #assert 1250 + DRAW_MS - 1001 == 749 && POST_MS + 2 * NET_MS + DRAW_MS == 830 && 830 <= BUDGET_MS;
  // Drawn at +1600, 500 ms behind: 100 ms into the 250 ms between samples.
  x := draw_x(3.2, 1000, 4.2, 1250, 1600);
  #assert x > 3.5999999 && x < 3.6000001;
  #assert draw_x(3.2, 1000, 4.2, 1250, 1400) == 3.2 && draw_x(3.2, 1000, 4.2, 1250, 1900) == 4.2;
}

// The wallet's verdict on a zap of `sats` with a 1-sat fee and 20 sats
// spendable: sent only when the balance covers amount and fee.
#forall "a zap over the balance is refused" sats: 0..<30 {
  pay := n57_pay(PAY);
  c := zap_check(pay, sats * 1000, PY_INVOICE, 1, 1, 20, T0 + 62, T0 + 3661);
  #assert !c.pays;   // only a 21-sat invoice is exact, and 21 + 1 is over 20
  if sats == 0 { #assert c.verdict == Verdict.bad_amount && !c.amount_ok; }
  else if sats + 1 > 20 { #assert c.verdict == Verdict.over_limit; }
  else { #assert c.verdict == Verdict.send; }
  #assert c.amount_ok == (sats >= 1 && sats <= 20);
  #assert c.invoice_ok == (sats == 21);
}

// The NWC budget takes the 21000 msat invoice only when it fits.
#forall "Wallet Connect's budget" limit: 20995..<21006 {
  s := nwc_pay(N47Budget.{limit = limit, period = 86400, start = T0}, PY_INVOICE, T0 + 62);
  if limit >= 21000 { #assert s.error == N47_OK && s.budget.spent == 21000; }
  else { #assert s.error == N47_QUOTA && s.budget.spent == 0; }
}
```

- The three pose frames' ids, Ben's goal, the zap request's id and the
  SHA-256 of its JSON, the lnurl, the whole invoice, the payment hash and
  the receipt's id are Python's.
- The hand-worked numbers: 978 sats, 749 ms against the 830 ms bound and
  the 1,000 ms budget, and x 3.6 at +1600.
- For every zap from 0 to 29 sats against 20 spendable and a 1-sat fee,
  the wallet says `send` exactly when `sats + 1 <= 20`, `over_limit` above
  that, and `bad_amount` at 0.
- For every NWC budget from 20,995 to 21,005 msat, the 21,000 msat invoice
  is paid exactly when it fits.

```shell
coax/bin/coax test nostr/examples/checks/verse-and-wallet.coax --run-steps 250000000
python3 coax/tests/nostr/verse_wallet_vectors.py | diff - coax/tests/nostr/verse_wallet_vectors.coax
```

On 2026-11-01, `coax test` ran 686 checks, the loaded units' own checks
included, and all passed; the two that sign the most events take about 6
seconds each. The Python program's output equals the committed constants.
The example's output was the same in debug and release. `coax mutate
--root-only` on the checks file found no mutation sites (the file holds
only checks), and the example's glue procedures were not mutated; that run
is not done.

### The units' records

No library file changed for this chapter, so the units' mutation runs were
not repeated. Their records, all with no survivors and none marked
equivalent:

- `verse/wallet.coax`: 471 of 471 mutants killed at `d7c266208a`
  (2026-10-31), 29 checks.
- `nip57.coax` 678 of 678 and `nip75.coax` 131 of 131, root-only, on
  2026-11-01 (chapter 71).
- `nip47.coax`, root-only, every mutant killed, on 2026-11-01 (chapter
  72).
- `nipmv.coax` 694 of 694, root-only, on 2026-11-01 (chapter 85).
- The relay:
  [2026-10-29-coax-relay-r1.md](../../docs/verification/2026-10-29-coax-relay-r1.md)
  and
  [2026-10-30-coax-relay-r1-rust-differential.md](../../docs/verification/2026-10-30-coax-relay-r1-rust-differential.md).

## What is not built yet

- Aethyr's rooms and presence (`aethyr/net/room.coax` and
  `aethyr/net/presence.coax`, Aethyr chapter 49), and any realm that
  publishes or reads NIP-MV frames.
- Presence measured over real sockets: the example's 40 ms hops are an
  assumption. `presence-pair.coax` measures the HTTP path today; it was
  not run for this chapter.
- Zaps in the app: `SparkHost.swift` sending the zap request with an
  LNURL payment, reading receipts, and `zap_check`'s rule moving into
  `verse/wallet.coax` with its own checks and mutation run.
- A Spark-backed Wallet Connect service, and NWC in a realm.
- Verse identities: players' profiles and follows (chapter 45) shown in a
  realm. The example uses bare test keys.

## Summary

- A Coax relay carries NIP-MV frames to every subscription that asks for
  the world; the client verifies, skips its own frames, orders by session
  and sequence number, and draws others 500 ms behind.
- On the fixed clock, a move reaches the other screen in 749 ms, inside
  the 830 ms bound and the one-second budget.
- An in-world zap is paid only when `verse/wallet.coax` says the amount,
  the invoice and the quote all pass; the 21-sat zap leaves 978 sats, and
  a zap over the balance is refused.
- The receipt counts only when Ben's server signed it, the amounts and
  description hash agree, and the preimage proves the payment.
- In production today the Spark wallet and its rules pay invoices and
  Lightning addresses, and presence runs over HTTP. NIP-MV presence, zaps
  in the Verse and Wallet Connect exist only in this book.

## Dependencies

- [85. Live media, worlds and
  experience](ch085-live-media-worlds-and-experience.md)
- [71. Lightning zaps and goals](ch071-lightning-zaps-and-goals.md)
- [72. Wallet Connect](ch072-wallet-connect.md)
- [Aethyr chapter 49, Rooms and
  presence](../../aethyr/book/ch049-rooms-and-presence.md)

## Sources

- [OpenAgents NIP-MV](../nips/openagents/NIP-MV.md)
- [NIP-57](../nips/official/57.md) and [NIP-75](../nips/official/75.md)
- [NIP-47](../nips/official/47.md)
- [NIP-01](../nips/official/01.md)
- The Breez Spark wallet plan: [docs/lightning/](../../docs/lightning/README.md).
- The Coax Nostr plan: [coax/nostr.md](../../coax/nostr.md).
