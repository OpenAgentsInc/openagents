# 113. Find a route and pay

Chapter 112 gave a node a map of the network's public channels.
Chapter 114 gave it an invoice: an amount, a payment hash, a payee and,
for a payee behind private channels, route hints. This chapter pays the
invoice. The payer searches the map for a route, builds chapter 111's
onion with a payload for every hop, and offers an HTLC on its own
channel. Each node on the way peels its layer of the onion, checks that
it is paid its fee, and offers an HTLC on its next channel. The payee
releases the preimage, and the preimage settles every HTLC back to the
payer.

A payment can fail on the way: a channel's balance is not public, so a
route that looks good may not carry the amount. The node that cannot
forward sends an error back in an error onion that only the payer can
read. The payer learns which channel failed, avoids it, and tries again.
An amount no single route carries is split into parts that travel apart
and meet at the payee.

The code runs every node in one process, over chapter 109's channels.
Five nodes pay each other in the example, and every commitment they sign
is a real one.

## What this chapter teaches

- Pathfinding: Dijkstra's algorithm from the payee back to the payer,
  adding each hop's fee and CLTV delta, and why the search runs
  backwards.
- What rules a channel out: the disable bit, `htlc_minimum_msat` and
  `htlc_maximum_msat`, and what the payer has learned of its liquidity.
- A cost that weighs the chance of failure: a uniform model of each
  channel's liquidity, and a penalty that grows with the share of it a
  payment needs.
- BOLT 4's hop payloads: `amt_to_forward`, `outgoing_cltv_value`,
  `short_channel_id`, and the payee's `payment_data` with
  `payment_secret` and `total_msat`.
- Forwarding: a node's checks of its fee and CLTV delta, and the
  failures it returns.
- Multi-part payments: splitting, and a payee that holds the parts until
  they add up to `total_msat`.
- Retries: reading the error onion, and what each kind of failure
  teaches the payer.

## Where Lightning uses this

| Where | What it uses | Chapter |
| --- | --- | --- |
| The map | channels and each end's policy, from verified gossip | 112 |
| The invoice | the amount, payment hash, payment secret, features and route hints | 114 |
| The onion | Sphinx packets and error onions | 111 |
| Each hop's HTLC | `update_add_htlc`, `update_fulfill_htlc` and `update_fail_htlc` over a channel | 109 |
| Each HTLC's transactions | commitment and HTLC transactions | 108 |
| A request paid with an invoice | a whole payment, end to end | 123 |

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [pay.coax](../../coax/lib/lightning/pay.coax) | `pay_map_graph`, `pay_map_hints`, `pay_map_local` | the payer's map: the graph, an invoice's hints, its own balances |
| | `pay_fee`, `pay_usable`, `pay_risk` | BOLT 7's fee, the limits, and the failure penalty |
| | `pay_find_route` | Dijkstra from the payee back to the payer |
| | `pay_payload`, `pay_onion`, `pay_read_payload` | BOLT 4's hop payloads and the onion |
| | `pay_on_htlc`, `pay_on_final`, `pay_failure` | a forwarding node and the payee |
| | `pay_step`, `pay_run` | moving messages between the nodes |
| | `pay_begin`, `pay_round`, `pay_invoice`, `pay_learn` | the payer: parts, retries and what errors teach |
| | `pay_open`, `pay_announce`, `pay_write_invoice` | building a network for the checks and the example |
| [channel.coax](../../coax/lib/lightning/channel.coax) | `lnh_add`, `lnh_fulfill`, `lnh_fail`, `lnh_receive`, `lnh_commit` | each channel's BOLT 2 state machine (chapter 109) |
| [onion.coax](../../coax/lib/lightning/onion.coax) | `onion_wrap`, `onion_peel`, `onion_error_create`, `onion_error_open` | Sphinx (chapter 111) |
| [gossip.coax](../../coax/lib/lightning/gossip.coax) | `gossip_graph_add_vartime` and the writers | the channel graph (chapter 112) |
| [invoice.coax](../../coax/lib/lightning/invoice.coax) | `invoice_decode_vartime`, `invoice_sign_vartime` | BOLT 11 invoices (chapter 114) |

Routes, onions and messages are public, so these procedures branch on
them. The secrets they touch, the nodes' keys and the session keys, go
only to chapter 111's onion code and to chapter 109's signer.

The library's codes and limits:

<!-- source: coax/lib/lightning/pay.coax codes -->
```jai
PAY_OK :: 0;
PAY_PENDING :: 1;        // parts are in flight or waiting: run another round
PAY_NO_ROUTE :: 2;       // no route carries the amount, even split into parts
PAY_REFUSED :: 3;        // the payee, or a permanent error at the payee, ended the payment
PAY_STUCK :: 4;          // a round changed nothing: the payee would time the parts out
PAY_TOO_MANY :: 5;       // more parts tried than PAY_MAX_PARTS
PAY_CAPACITY :: 6;       // a table here is full
PAY_BAD_INVOICE :: 7;    // an invoice with no amount, no payment_secret, or one that does not decode
PAY_CHANNEL :: 8;        // a channel refused a message: a broken node, not a failed payment

PAY_MAX_EDGES :: 32;     // directed channels the payer knows
PAY_MAX_NODES :: 24;
PAY_MAX_HOPS :: 20;      // BOLT 4's onion holds 27 small payloads; implementations stop near 20
PAY_MAX_CLTV :: 2016;    // the most blocks a payment may lock funds, as LND and CLN default
PAY_MIN_FINAL :: 18;     // BOLT 11's default min_final_cltv_expiry_delta
PAY_HOP_PENALTY :: 500;  // msat added for each forwarding hop, so shorter routes win ties
PAY_RISK :: 10000;       // msat added for a hop certain to fail, scaled by the chance it fails
PAY_MIN_PART :: 10000000;// the smallest part, in msat, a payment is split into
PAY_MAX_PARTS :: 16;     // parts sent for one payment, failures included
PAY_UNKNOWN_HI :: 1099511627776;   // 2^40 msat: no known limit on a route hint's channel
```

## Fees, limits and the chance of failure

Each end of a public channel says in its `channel_update` what it
charges to forward over the channel: a base fee in millisatoshis and a
proportional fee in millionths of the amount forwarded. BOLT 7 rounds
the proportional part down. The same update gives the CLTV delta, the
blocks the forwarding node wants between the HTLC it receives and the
one it offers, so that it has time to claim one after the other is
claimed from it.

The update does not say how much the channel can carry now. The
channel's capacity is split between its two ends, and the split moves
with every payment. BOLT 7 only promises that `htlc_maximum_msat` is no
more than the capacity. So the payer starts knowing that the amount it
can send one way is somewhere from 0 to `htlc_maximum_msat`, and narrows
that range as it learns.

This chapter uses the simplest model of what is in that range: every
value is equally likely, as in René Pickhardt and Stefan Richter's
"Optimally Reliable & Cheap Payment Flows on the Lightning Network".
Then an amount `a` between the known bounds `lo` and `hi` fails with
probability `(a - lo) / (hi - lo + 1)`, a straight line. Measurements of
the real network found liquidity is more often near one end of a
channel than in the middle, so LND's bimodal estimator and LDK's default
model put more weight there. A uniform model is still the one that is
easy to state, to compute in integers and to check against a second
implementation, and it ranks a single channel the same way.

The cost the search adds for a hop is its fee, a fixed penalty of 500
msat for each forwarding hop so that shorter routes win ties, and a risk
penalty of up to 10,000 msat in proportion to the chance of failure.
Strictly, the success probabilities of a route's hops multiply, so the
cost that adds up along a route is `-log P`. For a probability near 1,
`-log P` is close to `1 - P`, and `1 - P` is what the code charges: whole
millisatoshis that Coax and Python compute the same.

<!-- source: coax/lib/lightning/pay.coax risk -->
```jai
// BOLT 7's fee: fee_base_msat + amount * fee_proportional_millionths / 1,000,000.
pay_fee :: (base: int, ppm: int, amount: int) -> int { return base + amount * ppm / 1000000; }
// Whether the edge can carry amount on top of what this payment already
// has on it, as far as the payer knows.
pay_usable :: (e: PayEdge, amount: int) -> bool {
  return !e.disabled && !e.failed && amount >= e.htlc_min && amount <= e.htlc_max && amount + e.inflight <= e.hi;
}
// The uniform model: the liquidity left is equally likely to be any
// value from lo to hi, so an amount a above lo fails with probability
// (a - lo) / (hi - lo + 1). The penalty is PAY_RISK times that, in
// whole msat. -log of the success probability would add correctly along
// a route; this is its first-order term, exact enough near certainty and
// the same in every implementation.
pay_risk :: (e: PayEdge, amount: int) -> int {
  lo := e.lo - e.inflight; hi := e.hi - e.inflight;
  if lo < 0 { lo = 0; }
  if amount <= lo { return 0; }
  return PAY_RISK * (amount - lo) / (hi - lo + 1);
}
```

## The payer's map

The map holds one edge for each direction of each channel. Its policy
comes from the end that pays into the channel. `lo`, `hi` and
`inflight` are the payer's own knowledge: what it has learned of the
liquidity, and what this payment already has on the channel. A refresh
from the graph keeps that knowledge.

A route hint in an invoice is a private channel the payer could not
otherwise see. BOLT 11's `r` field lists, for each channel of the hint,
the node at its start, the short channel ID and that node's fees and
CLTV delta. The last channel ends at the payee. Nothing in the hint says
how much the channel carries, so its `hi` is left unbounded. The payer's
own channels are known exactly: their `lo` and `hi` are the balance it
can spend.

<!-- source: coax/lib/lightning/pay.coax map -->
```jai
// One direction of a channel: from pays to. The policy is from's, out
// of its channel_update or a route hint. lo and hi bound what the payer
// believes from can send now; inflight is what this payment has on it.
PayEdge :: struct {
  scid: u64; from: [33] u8; to: [33] u8;
  fee_base: int; fee_ppm: int; cltv: int; htlc_min: int; htlc_max: int;
  disabled: bool;          // channel_flags' disable bit
  failed: bool;            // a permanent error named this channel or its node
  lo: int; hi: int;        // liquidity known, msat: at least lo, at most hi
  inflight: int;
}
PayMap :: struct { edges: [PAY_MAX_EDGES] PayEdge; n: int; }
pay_find_edge :: (m: [] PayMap, scid: u64, from: [] u8) -> int {
  for i: 0..<m[0].n { if m[0].edges[i].scid == scid && lnh_same(m[0].edges[i].from[..], from) { return i; } }
  return -1;
}
pay_add_edge :: (m: [] PayMap, scid: u64, from: [] u8, to: [] u8, hi: int) -> int {
  k := pay_find_edge(m, scid, from);
  if k >= 0 { return k; }
  if m[0].n == PAY_MAX_EDGES { return -1; }
  k = m[0].n; m[0].n += 1;
  m[0].edges[k] = PayEdge.{scid = scid, hi = hi};
  for i: 0..<33 { m[0].edges[k].from[i] = from[i]; m[0].edges[k].to[i] = to[i]; }
  return k;
}
// Every policy in chapter 112's graph. A new edge starts knowing nothing
// but the advertised limit: BOLT 7 says htlc_maximum_msat is at most the
// channel's capacity, so the liquidity is somewhere in [0, htlc_max].
// What the payer learned survives a refresh, cut to a lower htlc_max.
pay_map_graph :: (m: [] PayMap, g: [] GspGraph) -> int {
  for c: 0..<g[0].count {
    ch := g[0].channels[c];
    for d: 0..<2 {
      p := ch.policy[d];
      if !p.present { continue; }
      from := ifx d == 0 then ch.node1 else ch.node2; to := ifx d == 0 then ch.node2 else ch.node1;
      k := pay_add_edge(m, ch.scid, from[..], to[..], cast(int) p.htlc_max);
      if k < 0 { return PAY_CAPACITY; }
      e := m[0].edges[k];
      e.fee_base = cast(int) p.fee_base; e.fee_ppm = cast(int) p.fee_ppm; e.cltv = p.cltv;
      e.htlc_min = cast(int) p.htlc_min; e.htlc_max = cast(int) p.htlc_max;
      e.disabled = (p.channel_flags & 2) != 0;
      if e.hi > e.htlc_max { e.hi = e.htlc_max; }
      if e.lo > e.hi { e.lo = e.hi; }
      m[0].edges[k] = e;
    }
  }
  return PAY_OK;
}
// BOLT 11's r field: each hint is a private path to the payee, one hop
// per channel, each naming the node at the channel's start. The last
// channel of a hint goes to the payee. Nothing bounds its liquidity.
pay_map_hints :: (m: [] PayMap, inv: [] Invoice) -> int {
  for j: 0..<inv[0].hops_n {
    h := inv[0].hops[j];
    to := inv[0].payee;
    if j + 1 < inv[0].hops_n && inv[0].hops[j + 1].hint == h.hint { to = inv[0].hops[j + 1].pubkey; }
    k := pay_add_edge(m, h.scid, h.pubkey[..], to[..], PAY_UNKNOWN_HI);
    if k < 0 { return PAY_CAPACITY; }
    m[0].edges[k].fee_base = cast(int) h.fee_base; m[0].edges[k].fee_ppm = cast(int) h.fee_ppm;
    m[0].edges[k].cltv = h.cltv; m[0].edges[k].htlc_max = PAY_UNKNOWN_HI;
  }
  return PAY_OK;
}
// The payer's own channel: it knows its balance exactly.
pay_map_local :: (m: [] PayMap, k: int, spendable: int) { m[0].edges[k].lo = spendable; m[0].edges[k].hi = spendable; }
```

## Searching from the payee

A forwarding node's fee is charged on the amount it forwards, and that
amount includes the fees of every node after it. So the search starts at
the payee, where the amount is the invoice's, and works back. Each
node's label is the cost so far and the amount and expiry an HTLC must
carry to reach that node. Relaxing the channel from `u` to `v` adds
`u`'s fee on what `v` needs and `u`'s CLTV delta. The payer's own
channel adds no fee and no delta: the payer does not pay itself.

Dijkstra's algorithm settles the open node of least cost first. Every
cost is a sum of non-negative terms, so a settled node's label is final.
When the payer is settled, the route is the chain of edges each node's
label points to, from the payer to the payee. The search also stops a
route at 20 hops and at an expiry more than 2,016 blocks after the
current height, as LND and Core Lightning do by default.

<!-- source: coax/lib/lightning/pay.coax search -->
```jai
// A route from the payer: channel i ends at hop i, the last hop is the
// payee. amount[i] and cltv[i] are the HTLC's amount and expiry on
// channel i; the payer's own channel charges no fee.
PayRoute :: struct { code: int; hops: int; edge: [PAY_MAX_HOPS] int; amount: [PAY_MAX_HOPS] int; cltv: [PAY_MAX_HOPS] int; fee: int; cost: int; }
pay_node_index :: (ids: [] [33] u8, n: [] int, id: [] u8) -> int {
  for i: 0..<n[0] { if lnh_same(ids[i][..], id) { return i; } }
  if n[0] == PAY_MAX_NODES { return -1; }
  for i: 0..<33 { ids[n[0]][i] = id[i]; }
  n[0] += 1;
  return n[0] - 1;
}
// Dijkstra's algorithm from the payee back to the payer. Each node's
// label is the cost, amount and expiry an HTLC must arrive with there;
// relaxing the channel u -> v adds u's fee and CLTV delta to what v
// needs. Working backwards gives every fee its true amount: a node's fee
// is charged on what it forwards, which is known only from the payee's
// end. The first node of least cost is settled; ties go to the lower
// index, payee 0 and payer 1, then nodes in the order edges name them.
pay_find_route :: (m: [] PayMap, payer: [] u8, payee: [] u8, amount: int, final_cltv: int, height: int) -> PayRoute {
  ids: [PAY_MAX_NODES] [33] u8; n: [1] int;
  first_two := pay_node_index(ids[..], n[..], payee) + pay_node_index(ids[..], n[..], payer);
  if lnh_same(payer, payee) || amount <= 0 { return PayRoute.{code = PAY_NO_ROUTE}; }
  for i: 0..<m[0].n {
    if pay_node_index(ids[..], n[..], m[0].edges[i].from[..]) < 0 || pay_node_index(ids[..], n[..], m[0].edges[i].to[..]) < 0 {
      return PayRoute.{code = PAY_CAPACITY};
    }
  }
  cost: [PAY_MAX_NODES] int; need: [PAY_MAX_NODES] int; cltv: [PAY_MAX_NODES] int; hops: [PAY_MAX_NODES] int;
  next: [PAY_MAX_NODES] int; reached: [PAY_MAX_NODES] bool; done: [PAY_MAX_NODES] bool;
  reached[0] = true; need[0] = amount; cltv[0] = final_cltv; next[0] = -1;
  while true {
    u := -1;
    for i: 0..<n[0] { if reached[i] && !done[i] && (u < 0 || cost[i] < cost[u]) { u = i; } }
    if u < 0 { return PayRoute.{code = PAY_NO_ROUTE}; }
    done[u] = true;
    if u == 1 { break; }
    for k: 0..<m[0].n {
      e := m[0].edges[k];
      if !lnh_same(e.to[..], ids[u][..]) { continue; }
      v := pay_node_index(ids[..], n[..], e.from[..]);
      if done[v] || !pay_usable(e, need[u]) { continue; }
      first := v == 1;
      fee := ifx first then 0 else pay_fee(e.fee_base, e.fee_ppm, need[u]);
      c := cost[u] + fee + pay_risk(e, need[u]) + (ifx first then 0 else PAY_HOP_PENALTY);
      t := cltv[u] + (ifx first then 0 else e.cltv);
      if hops[u] + 1 > PAY_MAX_HOPS || t - height > PAY_MAX_CLTV { continue; }
      if !reached[v] || c < cost[v] {
        reached[v] = true; cost[v] = c; need[v] = need[u] + fee; cltv[v] = t; hops[v] = hops[u] + 1; next[v] = k;
      }
    }
  }
  r := PayRoute.{hops = hops[1], fee = need[1] - amount, cost = cost[1]};
  at := 1;
  for i: 0..<r.hops {
    k := next[at]; r.edge[i] = k;
    at = pay_node_index(ids[..], n[..], m[0].edges[k].to[..]);
    r.amount[i] = need[at]; r.cltv[i] = cltv[at];
  }
  return r;
}
```

## Hop payloads and the onion

Each hop's payload is a TLV stream (chapter 104). A forwarding hop gets
the amount and expiry of the HTLC it must offer and the short channel ID
of the channel to offer it on. The difference between what it receives
and what it forwards is its fee. The payee gets the amount and expiry
it should receive and `payment_data`: the invoice's `payment_secret`,
which only someone who read the invoice knows, and `total_msat`, the sum
of every part.

The onion wraps the payloads in hop order under the node keys of the
route, with the payment hash as associated data (chapter 111). A hop
that changes the hash breaks every HMAC after it.

<!-- source: coax/lib/lightning/pay.coax payloads -->
```jai
// BOLT 4's hop payloads. A forwarding hop gets amt_to_forward,
// outgoing_cltv_value and the short_channel_id of its next channel: what
// it must send on, so its fee is what it keeps. The payee gets the amount
// and expiry it should see, and payment_data: the invoice's
// payment_secret, which proves the payer read the invoice, and
// total_msat, which tells it how much all parts add up to.
pay_payload :: (m: [] PayMap, r: PayRoute, i: int, secret: [] u8, total: int, out: [] u8) -> int {
  v: [40] u8; at := 0;
  last := i == r.hops - 1;
  amt := ifx last then r.amount[i] else r.amount[i + 1];
  expiry := ifx last then r.cltv[i] else r.cltv[i + 1];
  at = tlv_write_vartime(out, at, 2, v[0..<tu_write_vartime(v[..], 0, cast(u64) amt)]);
  at = tlv_write_vartime(out, at, 4, v[0..<tu_write_vartime(v[..], 0, cast(u64) expiry)]);
  if !last {
    gsp_put64(v[..], 0, m[0].edges[r.edge[i + 1]].scid);
    at = tlv_write_vartime(out, at, 6, v[0..<8]);
  } else {
    for k: 0..<32 { v[k] = secret[k]; }
    at = tlv_write_vartime(out, at, 8, v[0..<tu_write_vartime(v[..], 32, cast(u64) total)]);
  }
  return at;
}
// The onion for a route: each hop's node key and payload, wrapped under
// the session key with the payment hash as associated data.
pay_onion :: (m: [] PayMap, r: PayRoute, session: [] u8, hash: [] u8, secret: [] u8, total: int, out: [] u8) -> int {
  h: [1] OnionHops; p: [64] u8;
  for i: 0..<r.hops {
    n := pay_payload(m, r, i, secret, total, p[..]);
    code := onion_add_hop_vartime(h[..], m[0].edges[r.edge[i]].to[..], p[0..<n]);
    if code != ONION_OK { return code; }
  }
  return onion_wrap(session, h[0].pubkeys[0..<h[0].keys_n], h[0].payloads[0..<h[0].payloads_n], hash, out);
}
// A hop's payload, read: the fields it has, and whether it is whole.
PayHop :: struct { code: int; amount: int; cltv: int; scid: u64; has_scid: bool; has_data: bool; secret: [32] u8; total: int; }
pay_read_payload :: (b: [] u8) -> PayHop {
  r: PayHop; known := [4] u64.[2, 4, 6, 8];
  if tlv_check_vartime(b, known[..]) != LN_OK { return PayHop.{code = PAY_INVALID_ONION_PAYLOAD}; }
  a := onion_record_vartime(b, 2); c := onion_record_vartime(b, 4);
  s := onion_record_vartime(b, 6); d := onion_record_vartime(b, 8);
  if !a.found || !c.found { return PayHop.{code = PAY_INVALID_ONION_PAYLOAD}; }
  x := tu_read_vartime(b[a.start..<a.start + a.length], 8); y := tu_read_vartime(b[c.start..<c.start + c.length], 4);
  if x.code != LN_OK || y.code != LN_OK { return PayHop.{code = PAY_INVALID_ONION_PAYLOAD}; }
  r.amount = cast(int) x.value; r.cltv = cast(int) y.value;
  if s.found {
    if s.length != 8 { return PayHop.{code = PAY_INVALID_ONION_PAYLOAD}; }
    r.has_scid = true; r.scid = gsp_u64(b, s.start);
  }
  if d.found {
    if d.length < 32 { return PayHop.{code = PAY_INVALID_ONION_PAYLOAD}; }
    t := tu_read_vartime(b[d.start + 32..<d.start + d.length], 8);
    if t.code != LN_OK { return PayHop.{code = PAY_INVALID_ONION_PAYLOAD}; }
    r.has_data = true; r.total = cast(int) t.value;
    for k: 0..<32 { r.secret[k] = b[d.start + k]; }
  }
  return r;
}
```

## The network

A network is a set of nodes and the channels between them. Each channel
holds chapter 109's two ends, each run by one node. A node keeps what
its channels do not: the HTLCs it was offered, with their onions and,
after peeling, the shared secret it needs to wrap a failure; the HTLCs
it offered, with what became of them; and the invoices it wrote.

<!-- source: coax/lib/lightning/pay.coax network -->
```jai
// A node forwards out of a channel at the policy it announced for it.
PayPolicy :: struct { fee_base: int; fee_ppm: int; cltv: int; htlc_min: int; htlc_max: int; }
// A channel: chapter 109's two ends, end[s] run by node[s].
PayLink :: struct { node: [2] int; scid: u64; announced: bool; policy: [2] PayPolicy; end: [2] LnhNode; }
PAY_IN_FREE :: 0; PAY_IN_ADDED :: 1; PAY_IN_FORWARDED :: 2; PAY_IN_HELD :: 3;
// An HTLC a node was offered, with its onion until the add is
// irrevocably committed, and the shared secret to wrap a failure.
PayIn :: struct {
  state: int; link: int; side: int; id: int; amount: int; expiry: int;
  hash: [32] u8; onion: [ONION_PACKET] u8; ss: [32] u8; total: int;
}
PAY_OUT_FREE :: 0; PAY_OUT_OFFERED :: 1; PAY_OUT_FULFILLED :: 2; PAY_OUT_FAILING :: 3; PAY_OUT_FAILED :: 4;
// An HTLC a node offered: forwarded for one of its ins, or the payer's
// own part. A failure's reason waits here until the removal is final.
PayOut :: struct {
  state: int; link: int; side: int; id: int; in_slot: int; part: int;
  preimage: [32] u8; reason: [300] u8; reason_n: int;
}
// What a payee keeps for an invoice it wrote.
PayInvoice :: struct { hash: [32] u8; preimage: [32] u8; secret: [32] u8; amount: int; }
PAY_SLOTS :: 8;
PayNode :: struct {
  secret: [32] u8; id: [33] u8;
  ins: [PAY_SLOTS] PayIn; outs: [PAY_SLOTS] PayOut;
  invoices: [4] PayInvoice; invoices_n: int;
}
PAY_NET_NODES :: 5; PAY_NET_LINKS :: 6;
PayNet :: struct { nodes: [PAY_NET_NODES] PayNode; n: int; links: [PAY_NET_LINKS] PayLink; links_n: int; height: int; }
pay_end :: (net: [] PayNet, l: int, s: int) -> [] LnhNode { return net[0].links[l].end[s..<s + 1]; }
```

Chapter 109's channel writes the onion field of `update_add_htlc` as
zeros and the reason of `update_fail_htlc` as empty, because the channel
never reads them. A node fills them into the message its channel just
queued. When a message arrives, the node reads the same fields back: an
onion, a preimage or a reason. A preimage is passed back at once: it
already lets the node claim the HTLC it was offered.

<!-- source: coax/lib/lightning/pay.coax wire -->
```jai
// Chapter 109's channel writes update_add_htlc's onion_routing_packet as
// zeros and update_fail_htlc's reason empty: they mean nothing to the
// channel. A node fills them into the message the channel just queued.
pay_put_onion :: (n: [] LnhNode, onion: [] u8) {
  k := n[0].out_n - 1;
  for i: 0..<ONION_PACKET { n[0].out[k].b[86 + i] = onion[i]; }
}
pay_put_reason :: (n: [] LnhNode, reason: [] u8) {
  k := n[0].out_n - 1;
  ln_put16(n[0].out[k].b[..], 42, reason.count);
  for i: 0..<reason.count { n[0].out[k].b[44 + i] = reason[i]; }
  n[0].out[k].n = 44 + reason.count;
}
pay_free_out :: (node: [] PayNode) -> int {
  for i: 0..<PAY_SLOTS { if node[0].outs[i].state == PAY_OUT_FREE { return i; } }
  return -1;
}
pay_find_out :: (node: [] PayNode, l: int, s: int, id: int) -> int {
  for i: 0..<PAY_SLOTS {
    o := node[0].outs[i];
    if o.state != PAY_OUT_FREE && o.link == l && o.side == s && o.id == id { return i; }
  }
  return -1;
}
// Offers an HTLC on the node's end s of link l with its onion, and keeps
// a record of it.
pay_offer :: (net: [] PayNet, l: int, s: int, amount: int, hash: [] u8, expiry: int, onion: [] u8, in_slot: int, part: int) -> int {
  k := net[0].links[l].node[s]; node := net[0].nodes[k..<k + 1];
  slot := pay_free_out(node);
  if slot < 0 { return LNH_FULL; }
  end := pay_end(net, l, s);
  code := lnh_add(end, amount, hash, expiry);
  if code != LNH_OK { return code; }
  pay_put_onion(end, onion);
  node[0].outs[slot] = PayOut.{state = PAY_OUT_OFFERED, link = l, side = s, id = end[0].next_id - 1, in_slot = in_slot, part = part};
  return LNH_OK;
}
// What a received message means to the node beyond its channel: an add
// brings an onion, a fulfill a preimage, a fail a reason.
pay_observe :: (net: [] PayNet, l: int, s: int, m: [] u8) -> int {
  k := net[0].links[l].node[s]; node := net[0].nodes[k..<k + 1];
  type := ln_u16(m, 0);
  if type == LNH_ADD {
    j := -1;
    for i: 0..<PAY_SLOTS { if j < 0 && node[0].ins[i].state == PAY_IN_FREE { j = i; } }
    if j < 0 { return PAY_CAPACITY; }
    x := PayIn.{state = PAY_IN_ADDED, link = l, side = s, id = lnh_get(m, 34, 8), amount = lnh_get(m, 42, 8), expiry = lnh_get(m, 82, 4)};
    for i: 0..<32 { x.hash[i] = m[50 + i]; }
    for i: 0..<ONION_PACKET { x.onion[i] = m[86 + i]; }
    node[0].ins[j] = x;
    return PAY_OK;
  }
  if type != LNH_FULFILL && type != LNH_FAIL { return PAY_OK; }
  o := pay_find_out(node, l, s, lnh_get(m, 34, 8));
  if o < 0 { return PAY_OK; }
  if type == LNH_FULFILL {
    for i: 0..<32 { node[0].outs[o].preimage[i] = m[42 + i]; }
    node[0].outs[o].state = PAY_OUT_FULFILLED;
    // The preimage is enough to claim the incoming HTLC: pass it back now.
    j := node[0].outs[o].in_slot;
    if j >= 0 {
      x := node[0].ins[j];
      code := lnh_fulfill(pay_end(net, x.link, x.side), x.id, node[0].outs[o].preimage[..]);
      if code != LNH_OK { return PAY_CHANNEL; }
      node[0].ins[j].state = PAY_IN_FREE; node[0].outs[o].state = PAY_OUT_FREE;
    }
    return PAY_OK;
  }
  n := ln_u16(m, 42);
  if n > 300 || 44 + n > m.count { n = 0; }
  for i: 0..<n { node[0].outs[o].reason[i] = m[44 + i]; }
  node[0].outs[o].reason_n = n; node[0].outs[o].state = PAY_OUT_FAILING;
  return PAY_OK;
}
```

## Forwarding

A node acts on an HTLC only once it is irrevocably committed in both
commitments. Before that, the node that offered it could still go to
the chain with a commitment that does not hold it. Then the node peels
the onion. If it is not the last hop, it finds the channel the payload
names and checks the payment against the policy it announced for that
channel: the fee, BOLT 7's base plus proportional fee on the amount to
forward, and the CLTV delta. If both hold and the channel takes the
HTLC, the node offers it with the next onion. Otherwise it fails the
HTLC it was offered.

BOLT 4 lists the failures and their fields. The `channel_update` that
the UPDATE failures carry is left out, with a length of zero, as BOLT 4
now recommends: a payer that applied updates from errors to its map
could be told a different map from anyone else.

<!-- source: coax/lib/lightning/pay.coax failures -->
```jai
// BOLT 4's failure codes: the top bits are flags.
PAY_BADONION :: 0x8000; PAY_PERM :: 0x4000; PAY_NODE :: 0x2000; PAY_UPDATE :: 0x1000;
PAY_TEMPORARY_CHANNEL_FAILURE :: 0x1007;   // UPDATE|7: the next channel could not take the HTLC
PAY_UNKNOWN_NEXT_PEER :: 0x400a;           // PERM|10: no channel with that short_channel_id
PAY_FEE_INSUFFICIENT :: 0x100c;            // UPDATE|12, with htlc_msat
PAY_INCORRECT_CLTV_EXPIRY :: 0x100d;       // UPDATE|13, with cltv_expiry
PAY_INCORRECT_OR_UNKNOWN :: 0x400f;        // PERM|15, with htlc_msat and height
PAY_FINAL_INCORRECT_CLTV :: 18;            // with cltv_expiry
PAY_FINAL_INCORRECT_AMOUNT :: 19;          // with incoming_htlc_amt
PAY_INVALID_ONION_PAYLOAD :: 0x4016;       // PERM|22, with type and offset
PAY_MPP_TIMEOUT :: 23;
// A failure message: the code, then its fields. The channel_update that
// UPDATE errors may carry is left out, with a zero length, as BOLT 4 now
// recommends: applying updates from errors fingerprints the payer.
pay_put :: (out: [] u8, at: int, v: int, width: int) { for i: 0..<width { out[at + i] = cast(u8) ((v >> (8 * (width - 1 - i))) & 255); } }
pay_failure :: (code: int, value: int, height: int, out: [] u8) -> int {
  ln_put16(out, 0, code);
  if code == PAY_TEMPORARY_CHANNEL_FAILURE { ln_put16(out, 2, 0); return 4; }
  if code == PAY_FEE_INSUFFICIENT { pay_put(out, 2, value, 8); ln_put16(out, 10, 0); return 12; }
  if code == PAY_INCORRECT_CLTV_EXPIRY { pay_put(out, 2, value, 4); ln_put16(out, 6, 0); return 8; }
  if code == PAY_INCORRECT_OR_UNKNOWN { pay_put(out, 2, value, 8); pay_put(out, 10, height, 4); return 14; }
  if code == PAY_FINAL_INCORRECT_CLTV { pay_put(out, 2, value, 4); return 6; }
  if code == PAY_FINAL_INCORRECT_AMOUNT { pay_put(out, 2, value, 8); return 10; }
  if code == PAY_INVALID_ONION_PAYLOAD { out[2] = 0; ln_put16(out, 3, 0); return 5; }
  return 2;
}
```

<!-- source: coax/lib/lightning/pay.coax forward -->
```jai
// Fails an incoming HTLC with an error packet as it goes back.
pay_fail_in :: (net: [] PayNet, k: int, j: int, packet: [] u8) -> int {
  x := net[0].nodes[k].ins[j];
  end := pay_end(net, x.link, x.side);
  if lnh_fail(end, x.id) != LNH_OK { return PAY_CHANNEL; }
  pay_put_reason(end, packet);
  net[0].nodes[k].ins[j].state = PAY_IN_FREE;
  return PAY_OK;
}
// The erring node's packet: the failure under its um key, padded to 256
// bytes, then its ammag layer (chapter 111).
pay_fail_code :: (net: [] PayNet, k: int, j: int, code: int, value: int) -> int {
  f: [16] u8; packet: [300] u8; ss := net[0].nodes[k].ins[j].ss;
  n := onion_error_create(ss[..], f[0..<pay_failure(code, value, net[0].height, f[..])], ONION_ERROR_PAD, packet[..]);
  return pay_fail_in(net, k, j, packet[0..<n]);
}
// An HTLC now irrevocably committed: peel the onion. A forwarding node
// checks its fee and CLTV delta against the policy it announced for the
// next channel and offers the HTLC there; the payee checks the invoice.
pay_on_htlc :: (net: [] PayNet, k: int, j: int) -> int {
  node := net[0].nodes[k..<k + 1]; x := node[0].ins[j];
  payload: [ONION_PAYLOADS] u8; next: [ONION_PACKET] u8;
  p := onion_peel(node[0].secret[..], x.onion[..], x.hash[..], payload[..], next[..]);
  for i: 0..<32 { node[0].ins[j].ss[i] = p.ss[i]; }
  hop := ifx p.code == ONION_OK then pay_read_payload(payload[0..<p.payload_n]) else PayHop.{code = PAY_INVALID_ONION_PAYLOAD};
  if hop.code != PAY_OK { return pay_fail_code(net, k, j, hop.code, 0); }
  if !p.last {
    if !hop.has_scid { return pay_fail_code(net, k, j, PAY_INVALID_ONION_PAYLOAD, 0); }
    l := -1; s := 0;
    for i: 0..<net[0].links_n {
      for e: 0..<2 { if net[0].links[i].scid == hop.scid && net[0].links[i].node[e] == k { l = i; s = e; } }
    }
    if l < 0 { return pay_fail_code(net, k, j, PAY_UNKNOWN_NEXT_PEER, 0); }
    policy := net[0].links[l].policy[s];
    if x.amount - hop.amount < pay_fee(policy.fee_base, policy.fee_ppm, hop.amount) { return pay_fail_code(net, k, j, PAY_FEE_INSUFFICIENT, x.amount); }
    if x.expiry - policy.cltv < hop.cltv { return pay_fail_code(net, k, j, PAY_INCORRECT_CLTV_EXPIRY, x.expiry); }
    if pay_offer(net, l, s, hop.amount, x.hash[..], hop.cltv, next[..], j, -1) != LNH_OK {
      return pay_fail_code(net, k, j, PAY_TEMPORARY_CHANNEL_FAILURE, 0);
    }
    node[0].ins[j].state = PAY_IN_FORWARDED;
    return PAY_OK;
  }
  return pay_on_final(net, k, j, hop);
}
```

## The payee and multi-part payments

The payee checks what BOLT 4 lists for a final node. Most failures are
the same one, `incorrect_or_unknown_payment_details`, so that a node
probing for an invoice cannot tell an unknown hash from a wrong amount
or secret. The two exceptions report a hop that cheated: an HTLC whose
expiry or amount is below what the onion said it should be.

Each HTLC of a multi-part payment carries the same payment hash and
`total_msat`. The payee holds the parts, and fulfills none until they
add up to `total_msat`; then it fulfills all of them. BOLT 4 says a
payee that fulfills any part must fulfill every part, and fails held
parts with `mpp_timeout` after at least 60 seconds without the rest.

<!-- source: coax/lib/lightning/pay.coax final -->
```jai
// The payee. Every failure here is incorrect_or_unknown_payment_details,
// so a prober cannot tell an unknown hash from a wrong amount, except the
// two BOLT 4 keeps for a hop that cheated: an expiry or amount below what
// the onion says. Parts are held until those with this hash reach
// total_msat; then all are fulfilled together.
pay_on_final :: (net: [] PayNet, k: int, j: int, hop: PayHop) -> int {
  node := net[0].nodes[k..<k + 1]; x := node[0].ins[j];
  unknown := x.amount;
  if x.expiry < hop.cltv { return pay_fail_code(net, k, j, PAY_FINAL_INCORRECT_CLTV, x.expiry); }
  if x.amount < hop.amount { return pay_fail_code(net, k, j, PAY_FINAL_INCORRECT_AMOUNT, x.amount); }
  v := -1;
  for i: 0..<node[0].invoices_n { if lnh_same(node[0].invoices[i].hash[..], x.hash[..]) { v = i; } }
  secret := hop.secret;
  if v < 0 || !hop.has_data || !lnh_same(secret[..], node[0].invoices[v].secret[..]) { return pay_fail_code(net, k, j, PAY_INCORRECT_OR_UNKNOWN, unknown); }
  inv := node[0].invoices[v];
  if x.expiry < net[0].height + PAY_MIN_FINAL || hop.total < inv.amount || hop.total > 2 * inv.amount {
    return pay_fail_code(net, k, j, PAY_INCORRECT_OR_UNKNOWN, unknown);
  }
  node[0].ins[j].state = PAY_IN_HELD; node[0].ins[j].total = hop.total;
  sum := 0;
  for i: 0..<PAY_SLOTS {
    y := node[0].ins[i];
    if y.state == PAY_IN_HELD && lnh_same(y.hash[..], x.hash[..]) {
      if y.total != hop.total { return pay_fail_code(net, k, j, PAY_INCORRECT_OR_UNKNOWN, unknown); }
      sum += y.amount;
    }
  }
  if sum < hop.total { return PAY_OK; }
  for i: 0..<PAY_SLOTS {
    y := node[0].ins[i];
    if y.state != PAY_IN_HELD || !lnh_same(y.hash[..], x.hash[..]) { continue; }
    if lnh_fulfill(pay_end(net, y.link, y.side), y.id, inv.preimage[..]) != LNH_OK { return PAY_CHANNEL; }
    node[0].ins[i].state = PAY_IN_FREE;
  }
  return PAY_OK;
}
```

## Moving messages

`pay_step` is one pass over the network. Each outbox delivers one
message. Each node acts on the HTLCs that became irrevocable and on the
failures whose removal did. A failure is passed back only when its
removal is irrevocably committed, so the next node can no longer claim
the HTLC with the preimage. Passing it back, the node adds its layer to
the error onion. Then every channel end with updates to sign signs them.
`pay_run` repeats the pass until nothing moves.

<!-- source: coax/lib/lightning/pay.coax run -->
```jai
// One pass over the network: every outbox delivers one message; every
// node acts on HTLCs that became irrevocable and on failures whose
// removal did; every end with changes to sign and no revocation awaited
// signs. A failure is passed back only once its removal is final, so the
// next node cannot still claim the HTLC it failed.
pay_step :: (net: [] PayNet) -> int {
  moved := 0; buffer: [LNH_MSG] u8;
  for l: 0..<net[0].links_n {
    for s: 0..<2 {
      from := pay_end(net, l, s);
      if from[0].out_n == 0 { continue; }
      n := lnh_take(from, buffer[..]);
      if lnh_receive(pay_end(net, l, 1 - s), buffer[0..<n]) != LNH_OK { return -1; }
      if pay_observe(net, l, 1 - s, buffer[0..<n]) != PAY_OK { return -1; }
      moved += 1;
    }
  }
  for k: 0..<net[0].n {
    for j: 0..<PAY_SLOTS {
      x := net[0].nodes[k].ins[j];
      if x.state != PAY_IN_ADDED { continue; }
      end := pay_end(net, x.link, x.side);
      h := lnh_find(end, x.id, false);
      if h < 0 || end[0].htlcs[h].add != LNH_DONE { continue; }
      if pay_on_htlc(net, k, j) != PAY_OK { return -1; }
      moved += 1;
    }
    for o: 0..<PAY_SLOTS {
      y := net[0].nodes[k].outs[o];
      if y.state != PAY_OUT_FAILING { continue; }
      end := pay_end(net, y.link, y.side);
      h := lnh_find(end, y.id, true);
      if h >= 0 && end[0].htlcs[h].removing != LNH_DONE { continue; }
      moved += 1;
      if y.in_slot < 0 { net[0].nodes[k].outs[o].state = PAY_OUT_FAILED; continue; }
      x := net[0].nodes[k].ins[y.in_slot];
      onion_error_forward(x.ss[..], y.reason[0..<y.reason_n]);
      if pay_fail_in(net, k, y.in_slot, y.reason[0..<y.reason_n]) != PAY_OK { return -1; }
      net[0].nodes[k].outs[o].state = PAY_OUT_FREE;
    }
  }
  for l: 0..<net[0].links_n {
    for s: 0..<2 {
      end := pay_end(net, l, s);
      if end[0].waiting || !lnh_remote_changes(end) || end[0].out_n == LNH_QUEUE { continue; }
      if lnh_commit(end) != LNH_OK { return -1; }
      moved += 1;
    }
  }
  return moved;
}
// Steps until nothing moves.
pay_run :: (net: [] PayNet) -> int {
  while true {
    moved := pay_step(net);
    if moved < 0 { return PAY_CHANNEL; }
    if moved == 0 { return PAY_OK; }
  }
  return PAY_OK;
}
```

## Paying

A payment holds the amounts still to place and every part it sent.
`pay_begin` reads the invoice: it needs an amount and a
`payment_secret`, and allows parts if the invoice sets `basic_mpp`. The
payer's balance on its own channel is chapter 109's `lnh_affords`
solved for the amount: what it holds less the reserve the peer asked
for and, as the channel's opener, the fee of a commitment with one more
HTLC.

Each part has its own session key, so every onion is new. The part's
amounts go into the map as `inflight`, so the next part's search sees
the liquidity the first one uses.

<!-- source: coax/lib/lightning/pay.coax payment -->
```jai
PAY_PART_FLYING :: 1; PAY_PART_SETTLED :: 2; PAY_PART_FAILED :: 3;
PayPart :: struct {
  state: int; amount: int; route: PayRoute; session: [32] u8; out: int;
  fail_hop: int; fail_code: int;        // where and why it failed
  onion_hash: [32] u8;
}
PayPayment :: struct {
  code: int; payer: int;
  hash: [32] u8; secret: [32] u8; payee: [33] u8;
  total: int; final_cltv: int; mpp: bool;
  parts: [PAY_MAX_PARTS] PayPart; parts_n: int;
  waiting: [PAY_MAX_PARTS] int; waiting_n: int;   // amounts not yet placed on a route
  preimage: [32] u8; settled: int; refused: bool;
}
// A payment of an invoice: the amount it asks, its payment_secret, and
// multi-part if it offers basic_mpp (feature bit 16 or 17).
pay_begin :: (p: [] PayPayment, payer: int, inv: [] Invoice, height: int) -> int {
  p[0] = PayPayment.{payer = payer};
  f := inv[0].features[0..<inv[0].features_n];
  if !inv[0].has_amount || inv[0].amount_msat == 0 || (!ln_feature_set(f, 14) && !ln_feature_set(f, 15)) {
    p[0].code = PAY_BAD_INVOICE; return p[0].code;
  }
  p[0].mpp = ln_feature_set(f, 16) || ln_feature_set(f, 17);
  p[0].hash = inv[0].payment_hash; p[0].secret = inv[0].secret; p[0].payee = inv[0].payee;
  p[0].total = cast(int) inv[0].amount_msat;
  p[0].final_cltv = height + cast(int) inv[0].min_final_cltv;
  p[0].waiting[0] = p[0].total; p[0].waiting_n = 1;
  p[0].code = PAY_PENDING;
  return p[0].code;
}
// The payer's spendable balance on its end of a channel: chapter 109's
// lnh_affords, solved for the amount.
pay_spendable :: (end: [] LnhNode) -> int {
  s := lnh_spec(end, true, LNH_ALL);
  fee := ifx end[0].funder then lnc_commit_fee(end[0].feerate, s.n + 1, false) else 0;
  v := s.to_local_msat - (end[0].their_cfg.reserve_sat + fee) * 1000;
  return ifx v < 0 then 0 else v;
}
// Each part's session key: SHA-256 of the payer's secret and the part's
// number, so every onion is new and the example repeats.
pay_session :: (node: [] PayNode, part: int, out: [] u8) {
  b: [33] u8;
  for i: 0..<32 { b[i] = node[0].secret[i]; }
  b[32] = cast(u8) part;
  lnh_sha(b[..], out);
}
pay_launch :: (net: [] PayNet, m: [] PayMap, p: [] PayPayment, r: PayRoute, amount: int) -> int {
  k := p[0].payer; node := net[0].nodes[k..<k + 1];
  if p[0].parts_n == PAY_MAX_PARTS { return PAY_TOO_MANY; }
  i := p[0].parts_n; p[0].parts_n += 1;
  part := PayPart.{state = PAY_PART_FLYING, amount = amount, route = r, out = -1};
  pay_session(node, i, part.session[..]);
  onion: [ONION_PACKET] u8;
  if pay_onion(m, r, part.session[..], p[0].hash[..], p[0].secret[..], p[0].total, onion[..]) != ONION_OK { return PAY_CAPACITY; }
  lnh_sha(onion[..], part.onion_hash[..]);
  for h: 0..<r.hops { m[0].edges[r.edge[h]].inflight += r.amount[h]; }
  l := -1; s := 0; first := m[0].edges[r.edge[0]].scid;
  for x: 0..<net[0].links_n { for e: 0..<2 { if net[0].links[x].scid == first && net[0].links[x].node[e] == k { l = x; s = e; } } }
  if l < 0 || pay_offer(net, l, s, r.amount[0], p[0].hash[..], r.cltv[0], onion[..], -1, i) != LNH_OK {
    // The payer's own channel refused: it knows that much now.
    part.state = PAY_PART_FAILED; part.fail_hop = -1;
    p[0].parts[i] = part;
    pay_settle_part(m, p, i);
    e := r.edge[0]; m[0].edges[e].hi = r.amount[0] - 1;
    p[0].waiting[p[0].waiting_n] = amount; p[0].waiting_n += 1;
    return PAY_OK;
  }
  end := pay_end(net, l, s);
  part.out = pay_find_out(node, l, s, end[0].next_id - 1);
  p[0].parts[i] = part;
  return PAY_OK;
}
pay_settle_part :: (m: [] PayMap, p: [] PayPayment, i: int) {
  r := p[0].parts[i].route;
  for h: 0..<r.hops { m[0].edges[r.edge[h]].inflight -= r.amount[h]; }
}
```

## Learning from errors

The payer opens the error onion with the part's shared secrets (chapter
111) and finds the hop that made it. BOLT 4's flags say what to do. A
NODE failure rules out every channel of that node. A PERM failure rules
out the channel out of the erring node. A `temporary_channel_failure`
says the channel could not carry what this payment had on it, so the
payer lowers `hi` below that. Any other UPDATE failure means the
payer's copy of the policy is out of date; the payer stops using the
channel, since it does not apply the update the error might carry. A
failure from the payee ends the payment, except `mpp_timeout`.

<!-- source: coax/lib/lightning/pay.coax learn -->
```jai
// What a failure teaches. The erring hop's index comes from the error
// onion; the channel at fault is the one out of it. A NODE error rules
// out the node's channels; a PERM error the channel; a
// temporary_channel_failure lowers what the channel is known to carry to
// less than what this payment had on it. An error from the payee ends
// the payment.
pay_learn :: (m: [] PayMap, p: [] PayPayment, i: int, reason: [] u8) {
  part := p[0].parts[i]; r := part.route;
  pubkeys: [PAY_MAX_HOPS * 33] u8;
  for h: 0..<r.hops { for b: 0..<33 { pubkeys[33 * h + b] = m[0].edges[r.edge[h]].to[b]; } }
  route: [1] OnionRoute; f: OnionFailure;
  f.code = onion_route(part.session[..], pubkeys[0..<33 * r.hops], route[..]);
  if f.code == ONION_OK { f = onion_error_open(route[..], reason); }
  if f.code != ONION_OK || f.failure_n < 2 { p[0].parts[i].fail_hop = -2; p[0].refused = true; return; }
  code := ln_u16(reason, f.failure);
  p[0].parts[i].fail_hop = f.hop; p[0].parts[i].fail_code = code;
  if f.hop == r.hops - 1 {
    if code != PAY_MPP_TIMEOUT { p[0].refused = true; }
    return;
  }
  e := r.edge[f.hop + 1];
  if (code & PAY_NODE) != 0 {
    bad := m[0].edges[r.edge[f.hop]].to;
    for k: 0..<m[0].n { if lnh_same(m[0].edges[k].from[..], bad[..]) || lnh_same(m[0].edges[k].to[..], bad[..]) { m[0].edges[k].failed = true; } }
  } else if code == PAY_TEMPORARY_CHANNEL_FAILURE {
    hi := m[0].edges[e].inflight - 1;
    if hi < m[0].edges[e].hi { m[0].edges[e].hi = hi; }
    if m[0].edges[e].lo > m[0].edges[e].hi { m[0].edges[e].lo = m[0].edges[e].hi; }
  } else {
    m[0].edges[e].failed = true;
  }
}
```

A round places every waiting amount. An amount with no route is split
in two, while the invoice allows parts and each half stays above 10,000
sat. Then the network runs, and each part that came back is read: a
settled part adds to what is paid, and a failed one teaches the map and
waits again, unless the payment was refused. The payment ends when every
part has settled, when it was refused and nothing is in flight, or when
a round changes nothing.

<!-- source: coax/lib/lightning/pay.coax round -->
```jai
// One round: place every waiting amount on a route, splitting what no
// route carries into halves when the invoice allows it; run the network;
// then read every part that came back. Returns PAY_PENDING while the
// payment goes on.
pay_round :: (net: [] PayNet, m: [] PayMap, p: [] PayPayment) -> int {
  if p[0].code != PAY_PENDING { return p[0].code; }
  k := p[0].payer; node := net[0].nodes[k..<k + 1];
  for l: 0..<net[0].links_n {
    for s: 0..<2 {
      if net[0].links[l].node[s] != k { continue; }
      e := pay_find_edge(m, net[0].links[l].scid, node[0].id[..]);
      if e >= 0 { pay_map_local(m, e, pay_spendable(pay_end(net, l, s)) + m[0].edges[e].inflight); }
    }
  }
  changed := false;
  while p[0].waiting_n > 0 {
    amount := p[0].waiting[0];
    for w: 1..<p[0].waiting_n { p[0].waiting[w - 1] = p[0].waiting[w]; }
    p[0].waiting_n -= 1;
    r := pay_find_route(m, node[0].id[..], p[0].payee[..], amount, p[0].final_cltv, net[0].height);
    if r.code == PAY_OK {
      code := pay_launch(net, m, p, r, amount);
      if code != PAY_OK { p[0].code = code; return code; }
      changed = true;
      continue;
    }
    if !p[0].mpp || amount < 2 * PAY_MIN_PART || p[0].waiting_n + 2 > PAY_MAX_PARTS { p[0].code = r.code; return r.code; }
    p[0].waiting[p[0].waiting_n] = amount / 2; p[0].waiting[p[0].waiting_n + 1] = amount - amount / 2;
    p[0].waiting_n += 2;
  }
  if pay_run(net) != PAY_OK { p[0].code = PAY_CHANNEL; return p[0].code; }
  flying := 0;
  for i: 0..<p[0].parts_n {
    part := p[0].parts[i];
    if part.state != PAY_PART_FLYING { continue; }
    o := node[0].outs[part.out];
    if o.state == PAY_OUT_FULFILLED {
      p[0].parts[i].state = PAY_PART_SETTLED; p[0].settled += part.amount; p[0].preimage = o.preimage;
      pay_settle_part(m, p, i); node[0].outs[part.out].state = PAY_OUT_FREE; changed = true;
    } else if o.state == PAY_OUT_FAILED {
      p[0].parts[i].state = PAY_PART_FAILED;
      pay_learn(m, p, i, o.reason[0..<o.reason_n]);
      pay_settle_part(m, p, i); node[0].outs[part.out].state = PAY_OUT_FREE; changed = true;
      if !p[0].refused { p[0].waiting[p[0].waiting_n] = part.amount; p[0].waiting_n += 1; }
    } else { flying += 1; }
  }
  if p[0].settled >= p[0].total && flying == 0 { p[0].code = PAY_OK; }
  else if p[0].refused && flying == 0 { p[0].code = PAY_REFUSED; }
  else if !changed { p[0].code = PAY_STUCK; }
  return p[0].code;
}
// Rounds until the payment ends.
pay_invoice :: (net: [] PayNet, m: [] PayMap, p: [] PayPayment) -> int {
  while pay_round(net, m, p) == PAY_PENDING { }
  return p[0].code;
}
```

## A network for the checks and the example

The channels open with chapter 109's messages. Each public channel's
gossip is written and signed by its own nodes and funding keys, and goes
through chapter 112's graph, which keeps it only if every signature
verifies. The payee's invoice is a real BOLT 11 invoice, signed with the
payee's key.

<!-- source: coax/lib/lightning/pay.coax setup -->
```jai
// A node's identity key, and an invoice it can be paid with.
pay_node :: (net: [] PayNet, secret: [] u8) -> int {
  k := net[0].n; net[0].n += 1;
  for i: 0..<32 { net[0].nodes[k].secret[i] = secret[i]; }
  net[0].nodes[k].id = gsp_public(secret);
  return k;
}
// A channel opened with chapter 109's messages: node a funds it with
// funding satoshis and pushes push msat to b.
pay_config :: (funder: bool, funding: int, push: int, k: int) -> LnhConfig {
  c := lnh_config(funder);
  c.funding_sat = funding; c.push_msat = push;
  for i: 2..<22 { c.spk[i] = cast(u8) (0x10 * k + i); }
  return c;
}
pay_open :: (net: [] PayNet, a: int, b: int, funding: int, push: int, scid: u64, announced: bool) -> int {
  l := net[0].links_n; net[0].links_n += 1;
  net[0].links[l].node[0] = a; net[0].links[l].node[1] = b;
  net[0].links[l].scid = scid; net[0].links[l].announced = announced;
  seed: [32] u8; txid: [32] u8;
  for i: 0..<32 { seed[i] = cast(u8) (0x40 + 2 * l); txid[i] = cast(u8) (0x80 + l + i); }
  lnh_init(pay_end(net, l, 0), true, seed[..], pay_config(true, funding, push, a));
  seed[0] += 1;
  lnh_init(pay_end(net, l, 1), false, seed[..], pay_config(false, funding, push, b));
  x := pay_end(net, l, 0); y := pay_end(net, l, 1);
  code := lnh_open(x);
  if code == LNH_OK { code = lnh_flush(x, y); }
  if code == LNH_OK { code = lnh_fund(x, txid[..], 0); }
  if code == LNH_OK { code = lnh_flush(x, y); }
  if code == LNH_OK { code = lnh_confirmed(x); }
  if code == LNH_OK { code = lnh_confirmed(y); }
  if code == LNH_OK { code = lnh_flush(x, y); }
  return code;
}
pay_policy :: (net: [] PayNet, l: int, s: int, base: int, ppm: int, cltv: int, htlc_max: int) {
  net[0].links[l].policy[s] = PayPolicy.{fee_base = base, fee_ppm = ppm, cltv = cltv, htlc_min = 1000, htlc_max = htlc_max};
}
// A public channel's gossip, written and signed by its nodes and added
// to a graph: the announcement with both funding keys, and each end's
// channel_update. disabled sets the disable bit of end s.
pay_announce :: (net: [] PayNet, l: int, g: [] GspGraph, timestamp: u64, disabled: int) -> int {
  x := net[0].links[l]; m: [512] u8; f: [1] u8;
  a := net[0].nodes[x.node[0]].secret; b := net[0].nodes[x.node[1]].secret;
  n := gossip_announcement_write_vartime(g[0].chain[..], x.scid, a[..], b[..], x.end[0].funding_secret[..], x.end[1].funding_secret[..], f[0..<0], m[..]);
  code := gossip_graph_add_vartime(g, m[0..<n]);
  for s: 0..<2 {
    key := ifx s == 0 then a[..] else b[..];
    other := net[0].nodes[x.node[1 - s]].id; me := net[0].nodes[x.node[s]].id;
    flags := (ifx gsp_less(me[..], other[..]) then 0 else 1) | (ifx disabled == s then 2 else 0);
    p := x.policy[s];
    n = gossip_update_write_vartime(key, g[0].chain[..], x.scid, timestamp, flags, p.cltv, cast(u64) p.htlc_min,
                                    cast(u64) p.fee_base, cast(u64) p.fee_ppm, cast(u64) p.htlc_max, m[..]);
    if code == GSP_OK { code = gossip_graph_add_vartime(g, m[0..<n]); }
  }
  return code;
}
// The payee's invoice: amount, payment_hash of a preimage made from k, a
// payment_secret, payment_secret and basic_mpp features, and any route
// hints. The payee keeps the preimage.
pay_write_invoice :: (net: [] PayNet, payee: int, amount: int, k: int, hints: [] InvHop, out: [] u8) -> int {
  node := net[0].nodes[payee..<payee + 1];
  v := node[0].invoices_n; node[0].invoices_n += 1;
  inv := PayInvoice.{amount = amount};
  lnh_preimage(k, inv.preimage[..], inv.hash[..]);
  for i: 0..<32 { inv.secret[i] = cast(u8) (k + 2 * i + 1); }
  node[0].invoices[v] = inv;
  w: [1] InvoiceWriter; features: [3] u8;
  ok := ln_feature_put(features[..], 8) && ln_feature_put(features[..], 14) && ln_feature_put(features[..], 17);
  code := invoice_begin_vartime(w[..], INV_REGTEST, true, cast(u64) amount, 1700000000 + cast(u64) k);
  code = invoice_put_bytes_vartime(w[..], INV_P, inv.hash[..]);
  code = invoice_put_bytes_vartime(w[..], INV_S, inv.secret[..]);
  d: [11] u8; t0 := inv_text("Chapter 113", d[..]);
  code = invoice_put_bytes_vartime(w[..], INV_D, d[..]);
  code = invoice_put_features_vartime(w[..], features[..]);
  if hints.count > 0 { code = invoice_put_route_vartime(w[..], hints); }
  t := invoice_sign_vartime(w[..], node[0].secret[..], out);
  return ifx t.code == INV_OK then t.length else -1;
}
// Each end's balance, msat, as its own latest commitment says.
pay_balance :: (net: [] PayNet, l: int, s: int) -> int { return lnh_spec(pay_end(net, l, s), true, LNH_NOW).to_local_msat; }
```

## The example

The example,
[bitcoin/examples/lightning-pay.coax](../examples/lightning-pay.coax),
opens five channels between Alice, Bob, Carol, Dave and Erin. The
channel from Dave to Erin is private. Alice builds her map from the
gossip of the other four and pays five invoices.

<!-- source: bitcoin/examples/lightning-pay.coax example -->
```jai
#load "../../coax/lib/lightning/pay.coax";
#load "../../coax/lib/nostr/hex.coax";
#import "Basic";

// Bytes as lowercase hex.
show :: (b: [] u8) {
  text: [200] u8;
  r := hex_encode_checked(cast(string) b, text[..], false);
  print("%", cast(string) text[0..<r.written]);
}
NAMES :: [5] string.["Alice", "Bob", "Carol", "Dave", "Erin"];
CODES :: [9] string.["OK", "PENDING", "NO_ROUTE", "REFUSED", "STUCK", "TOO_MANY", "CAPACITY", "BAD_INVOICE", "CHANNEL"];
// A secret of 32 bytes made from two small numbers, for the example.
secret :: (k: int, c: int) -> [32] u8 {
  out: [32] u8;
  for i: 0..<32 { out[i] = cast(u8) ((k * i + c) & 255); }
  return out;
}
failure_name :: (code: int) -> string {
  if code == PAY_TEMPORARY_CHANNEL_FAILURE { return "temporary_channel_failure"; }
  if code == PAY_INCORRECT_OR_UNKNOWN { return "incorrect_or_unknown_payment_details"; }
  if code == PAY_UNKNOWN_NEXT_PEER { return "unknown_next_peer"; }
  if code == PAY_FEE_INSUFFICIENT { return "fee_insufficient"; }
  return "another failure";
}
name_of :: (net: [] PayNet, id: [] u8) -> string {
  for k: 0..<net[0].n { if lnh_same(net[0].nodes[k].id[..], id) { return NAMES[k]; } }
  return "?";
}
scid_show :: (v: u64) { t: [23] u8; print("%", cast(string) t[0..<gossip_scid_text(v, t[..])]); }
// Each part: its amount and route, the fees and expiries, the onion's
// hash, and what came of it.
parts :: (net: [] PayNet, m: [] PayMap, p: [] PayPayment) {
  for i: 0..<p[0].parts_n {
    part := p[0].parts[i]; r := part.route;
    print("  part %: % msat via Alice", i + 1, part.amount);
    for h: 0..<r.hops { print(" > %", name_of(net, m[0].edges[r.edge[h]].to[..])); }
    print(", fee % msat\n    channels", r.fee);
    for h: 0..<r.hops { print(" "); scid_show(m[0].edges[r.edge[h]].scid); print(" % msat to %", r.amount[h], r.cltv[h]); }
    print("\n    onion "); show(part.onion_hash[0..<8]);
    if part.state == PAY_PART_SETTLED { print(": settled\n"); continue; }
    if part.fail_hop < 0 { print(": refused by Alice's own channel\n"); continue; }
    print(": failed at %, % (0x", name_of(net, m[0].edges[r.edge[part.fail_hop]].to[..]), failure_name(part.fail_code));
    b := [2] u8.[cast(u8) (part.fail_code >> 8), cast(u8) (part.fail_code & 255)];
    show(b[..]); print(")\n");
  }
}
// Pays an invoice from Alice and prints what happened.
pay :: (net: [] PayNet, m: [] PayMap, text: [] u8) {
  inv: [1] Invoice;
  code := invoice_decode_vartime(cast(string) text, inv[..]);
  print("  invoice: % characters, %, % msat to %, % route hint hops\n", text.count,
        ifx code == INV_OK then "signature verifies" else "refused", inv[0].amount_msat, name_of(net, inv[0].payee[..]), inv[0].hops_n);
  code = pay_map_hints(m, inv[..]);
  p: [1] PayPayment;
  code = pay_begin(p[..], 0, inv[..], net[0].height);
  print("  multi-part allowed: %\n", p[0].mpp);
  rounds := 0;
  while pay_round(net, m, p[..]) == PAY_PENDING { rounds += 1; }
  parts(net, m, p[..]);
  h: [32] u8; want := p[0].hash; lnh_sha(p[0].preimage[..], h[..]);
  print("  result: %, rounds: %, settled: % msat, preimage matches the hash: %\n", CODES[p[0].code], rounds + 1,
        p[0].settled, lnh_same(h[..], want[..]));
}

main :: () {
  net: [1] PayNet; net[0].height = 1000;
  for k: 0..<5 { s := secret(k + 3, 2 * k + 1); n := pay_node(net[..], s[..]); }
  print("nodes\n");
  for k: 0..<5 { print("  % ", NAMES[k]); show(net[0].nodes[k].id[..]); print("\n"); }

  print("channels, opened with chapter 109's messages\n");
  // Each channel's opener, the other end, and its funding in satoshis.
  from := [5] int.[0, 1, 0, 2, 3]; to := [5] int.[1, 3, 2, 3, 4];
  funding := [5] int.[2000000, 1000000, 2000000, 1000000, 1000000];
  for l: 0..<5 {
    code := pay_open(net[..], from[l], to[l], funding[l], 0, gossip_scid(cast(u64) (101 + l), 1, 0), l != 4);
    print("  % - % ", NAMES[from[l]], NAMES[to[l]]); scid_show(net[0].links[l].scid);
    print(": % sat from %, %, %\n", funding[l], NAMES[from[l]], ifx code == LNH_OK then "open" else "refused",
          ifx l == 4 then "private" else "announced");
  }
  // Bob forwards at 1,000 msat + 100 ppm and 40 blocks, Carol at 500 msat
  // + 50 ppm and 80 blocks, Dave at 1,000 msat + 1,000 ppm and 144 blocks.
  for l: 0..<5 {
    for s: 0..<2 {
      k := net[0].links[l].node[s];
      base := ifx k == 2 then 500 else 1000; ppm := ifx k == 2 then 50 else ifx k == 3 then 1000 else 100;
      cltv := ifx k == 2 then 80 else ifx k == 3 then 144 else 40;
      pay_policy(net[..], l, s, base, ppm, cltv, (funding[l] - 10000) * 1000);
    }
  }

  print("gossip\n");
  g: [1] GspGraph; chain: [32] u8; t := tx_unhex(LNH_CHAIN, chain[..]);
  gossip_graph_begin(g[..], chain[..]);
  for l: 0..<4 { print("  "); scid_show(net[0].links[l].scid); print(": %\n", ifx pay_announce(net[..], l, g[..], 1762156800, -1) == GSP_OK then "announced, both updates added" else "refused"); }
  m: [1] PayMap;
  code := pay_map_graph(m[..], g[..]);
  print("  Alice's map: % channels, % directions\n", g[0].count, m[0].n);

  text: [1000] u8; hint: [1] InvHop;
  print("1. Alice pays Dave 50,000 sat\n");
  n := pay_write_invoice(net[..], 3, 50000000, 1, hint[0..<0], text[..]);
  pay(net[..], m[..], text[0..<n]);
  print("2. Alice pays Dave 1,400,000 sat: more than any one route carries\n");
  n = pay_write_invoice(net[..], 3, 1400000000, 2, hint[0..<0], text[..]);
  pay(net[..], m[..], text[0..<n]);
  print("3. Alice pays Dave 280,000 sat: Carol's side has only 250,000\n");
  n = pay_write_invoice(net[..], 3, 280000000, 3, hint[0..<0], text[..]);
  pay(net[..], m[..], text[0..<n]);
  print("4. Alice pays Erin 100,000 sat over Dave's private channel\n");
  hint[0] = InvHop.{pubkey = net[0].nodes[3].id, scid = net[0].links[4].scid, fee_base = 1000, fee_ppm = 1000, cltv = 144};
  n = pay_write_invoice(net[..], 4, 100000000, 4, hint[..], text[..]);
  pay(net[..], m[..], text[0..<n]);
  print("5. Alice pays an invoice Dave has forgotten\n");
  n = pay_write_invoice(net[..], 3, 10000000, 5, hint[0..<0], text[..]);
  net[0].nodes[3].invoices_n -= 1;
  pay(net[..], m[..], text[0..<n]);

  print("balances, msat, as each side's commitment says\n");
  for l: 0..<5 {
    a := net[0].links[l].node[0]; b := net[0].links[l].node[1];
    print("  "); scid_show(net[0].links[l].scid);
    print(": % %, % %; agree: %\n", NAMES[a], pay_balance(net[..], l, 0), NAMES[b], pay_balance(net[..], l, 1),
          lnh_spec(pay_end(net[..], l, 1), true, LNH_NOW).to_remote_msat == pay_balance(net[..], l, 0));
  }
}
```

It prints:

```text
nodes
  Alice 021379cc96aa0aa0f16d54bacc78dca79c4ef9cbb1d898f0f1a2eea3521c7ec38a
  Bob 030f222aee3aed39ebed8cd0c93601ed5e1385c45f359569e8327bc6d4a6b988fa
  Carol 02522e9a5fa836149555d96e2cbbf397be69628e906ef6c72e80aba0877ba899d9
  Dave 03cdff513b8c55d6710751128eeb8a09a57c388fa18d08978871e987396c2fd9e6
  Erin 03780e2df933bf0e2d7cb6ccad772254036563488a314f579f4d93e40228709897
channels, opened with chapter 109's messages
  Alice - Bob 101x1x0: 2000000 sat from Alice, open, announced
  Bob - Dave 102x1x0: 1000000 sat from Bob, open, announced
  Alice - Carol 103x1x0: 2000000 sat from Alice, open, announced
  Carol - Dave 104x1x0: 1000000 sat from Carol, open, announced
  Dave - Erin 105x1x0: 1000000 sat from Dave, open, private
gossip
  101x1x0: announced, both updates added
  102x1x0: announced, both updates added
  103x1x0: announced, both updates added
  104x1x0: announced, both updates added
  Alice's map: 4 channels, 8 directions
1. Alice pays Dave 50,000 sat
  invoice: 266 characters, signature verifies, 50000000 msat to Dave, 0 route hint hops
  multi-part allowed: true
  part 1: 50000000 msat via Alice > Carol > Dave, fee 3000 msat
    channels 103x1x0 50003000 msat to 1098 104x1x0 50000000 msat to 1018
    onion e5f37ce9a85db772: settled
  result: OK, rounds: 1, settled: 50000000 msat, preimage matches the hash: true
2. Alice pays Dave 1,400,000 sat: more than any one route carries
  invoice: 265 characters, signature verifies, 1400000000 msat to Dave, 0 route hint hops
  multi-part allowed: true
  part 1: 700000000 msat via Alice > Carol > Dave, fee 35500 msat
    channels 103x1x0 700035500 msat to 1098 104x1x0 700000000 msat to 1018
    onion 92405c6f54b08650: settled
  part 2: 700000000 msat via Alice > Bob > Dave, fee 71000 msat
    channels 101x1x0 700071000 msat to 1058 102x1x0 700000000 msat to 1018
    onion 03bb27c197758bcd: settled
  result: OK, rounds: 1, settled: 1400000000 msat, preimage matches the hash: true
3. Alice pays Dave 280,000 sat: Carol's side has only 250,000
  invoice: 267 characters, signature verifies, 280000000 msat to Dave, 0 route hint hops
  multi-part allowed: true
  part 1: 280000000 msat via Alice > Carol > Dave, fee 14500 msat
    channels 103x1x0 280014500 msat to 1098 104x1x0 280000000 msat to 1018
    onion 7e3b080ee369e8e3: failed at Carol, temporary_channel_failure (0x1007)
  part 2: 280000000 msat via Alice > Bob > Dave, fee 29000 msat
    channels 101x1x0 280029000 msat to 1058 102x1x0 280000000 msat to 1018
    onion 280715720c32bcf6: settled
  result: OK, rounds: 2, settled: 280000000 msat, preimage matches the hash: true
4. Alice pays Erin 100,000 sat over Dave's private channel
  invoice: 349 characters, signature verifies, 100000000 msat to Erin, 1 route hint hops
  multi-part allowed: true
  part 1: 100000000 msat via Alice > Carol > Dave > Erin, fee 106505 msat
    channels 103x1x0 100106505 msat to 1242 104x1x0 100101000 msat to 1162 105x1x0 100000000 msat to 1018
    onion ea3324dd3998aaed: settled
  result: OK, rounds: 1, settled: 100000000 msat, preimage matches the hash: true
5. Alice pays an invoice Dave has forgotten
  invoice: 266 characters, signature verifies, 10000000 msat to Dave, 0 route hint hops
  multi-part allowed: true
  part 1: 10000000 msat via Alice > Carol > Dave, fee 1000 msat
    channels 103x1x0 10001000 msat to 1098 104x1x0 10000000 msat to 1018
    onion 78d6f0449b452cec: failed at Dave, incorrect_or_unknown_payment_details (0x400f)
  result: REFUSED, rounds: 1, settled: 0 msat, preimage matches the hash: false
balances, msat, as each side's commitment says
  101x1x0: Alice 1019900000, Bob 980100000; agree: true
  102x1x0: Bob 20000000, Dave 980000000; agree: true
  103x1x0: Alice 1149854995, Carol 850145005; agree: true
  104x1x0: Carol 149899000, Dave 850101000; agree: true
  105x1x0: Dave 900000000, Erin 100000000; agree: true
```

- Carol forwards for 500 msat + 50 ppm and Bob for 1,000 msat + 100
  ppm, so the 50,000-sat payment goes through Carol. Its HTLC on Carol's
  channel to Dave expires at block 1,018, the height plus BOLT 11's
  default 18 blocks; the one before it is 80 blocks earlier, Carol's
  delta.
- 1,400,000 sat is more than either channel to Dave advertises, so
  Alice splits it. The first half goes through Carol. With 700,000 sat
  of the second already on Carol's channel in Alice's map, the second
  half goes through Bob. Dave holds the first part until the second
  arrives, then fulfills both.
- Carol has 250,000 sat left on her side, so she cannot forward 280,000.
  She returns `temporary_channel_failure`; Alice learns that the
  channel carries less than 280,000 sat and pays through Bob.
- Erin's invoice carries a route hint for Dave's private channel. Alice
  adds it to her map and the route ends with it: Dave's fee of 1,000
  msat + 1,000 ppm and his delta of 144 blocks come from the hint.
- Dave does not know the last invoice's hash. He returns
  `incorrect_or_unknown_payment_details`, Alice does not retry, and the
  failed HTLCs leave every balance as it was.
- The balances add up: Bob and Carol each gained exactly the fees they
  charged, and both ends of every channel agree.

The book's checks run this program in debug and release builds and
compare the output with
[bitcoin/fixtures/lightning-pay.out](../fixtures/lightning-pay.out).

## How the code is checked

### Checks in the example

The checks are in
[bitcoin/examples/checks/lightning-pay.coax](../examples/checks/lightning-pay.coax),
which loads the example:

- for 36 amounts from 1,000,000 msat to 1,226,000,000 msat, the search
  returns the cheaper of two routes costed by hand, or no route when
  neither carries the amount, and the route's amounts and expiries
  follow BOLT 7 hop by hop
- after a `temporary_channel_failure` for an amount, the payer's search
  sends that amount another way

<!-- source: bitcoin/examples/checks/lightning-pay.coax check -->
```jai
// Four nodes: the payer 0, the payee 3, and the paths
// 0 > 1 > 3 (1,000 msat + 100 ppm, 40 blocks) and 0 > 2 > 3 (500 msat +
// 50 ppm, 80 blocks). The payer believes 2 > 3 carries at most 400,000
// sat, so the risk penalty grows faster there. The search only compares
// node IDs, so real keys are made only when an onion needs them.
four :: (m: [] PayMap, real: bool) -> [4] [33] u8 {
  ids: [4] [33] u8; s: [32] u8;
  for k: 0..<4 {
    s[31] = cast(u8) (k + 1);
    if real { ids[k] = gsp_public(s[..]); } else { ids[k][0] = 2; ids[k][32] = cast(u8) (k + 1); }
  }
  m[0] = PayMap.{};
  from := [4] int.[0, 1, 0, 2]; to := [4] int.[1, 3, 2, 3];
  for e: 0..<4 {
    k := pay_add_edge(m, cast(u64) (e + 1), ids[from[e]][..], ids[to[e]][..], 900000000);
    m[0].edges[k].htlc_min = 1000; m[0].edges[k].htlc_max = 900000000;
    m[0].edges[k].fee_base = ifx e == 1 then 1000 else ifx e == 3 then 500 else 0;
    m[0].edges[k].fee_ppm = ifx e == 1 then 100 else ifx e == 3 then 50 else 0;
    m[0].edges[k].cltv = ifx e == 1 then 40 else ifx e == 3 then 80 else 0;
  }
  m[0].edges[3].hi = 400000000;
  return ids;
}
// The cost of the path through edge first, then edge second, by hand.
by_hand :: (m: [] PayMap, first: int, second: int, amount: int) -> int {
  e := m[0].edges[second];
  if !pay_usable(e, amount) { return -1; }
  fee := pay_fee(e.fee_base, e.fee_ppm, amount);
  if !pay_usable(m[0].edges[first], amount + fee) { return -1; }
  return fee + PAY_HOP_PENALTY + pay_risk(e, amount) + pay_risk(m[0].edges[first], amount + fee);
}
// For amounts from 1,000,000 to 1,225,000,000 msat, the search's route
// is the cheaper of the two by hand, or none when neither carries it, and
// its amounts and expiries follow BOLT 7 hop by hop.
cheapest :: (k: int) {
  m: [1] PayMap; ids := four(m[..], false);
  amount := 1000000 * (k * k + 1);
  r := pay_find_route(m[..], ids[0][..], ids[3][..], amount, 1018, 1000);
  a := by_hand(m[..], 0, 1, amount); b := by_hand(m[..], 2, 3, amount);
  if a < 0 && b < 0 { #assert r.code == PAY_NO_ROUTE; return; }
  best := ifx a < 0 then b else ifx b < 0 then a else ifx a < b then a else b;
  #assert r.code == PAY_OK && r.hops == 2 && r.cost == best && r.amount[1] == amount && r.cltv[1] == 1018;
  e := m[0].edges[r.edge[1]];
  #assert r.amount[0] == amount + pay_fee(e.fee_base, e.fee_ppm, amount) && r.cltv[0] == 1018 + e.cltv;
  #assert r.fee == r.amount[0] - amount;
}
#forall k: 0..<36 { cheapest(k); }
// After a temporary_channel_failure from node 2 with amount on 2 > 3,
// the payer knows that channel carries less than amount: no route for
// amount or more uses it, and the payment moves to 1 > 3.
learned :: (k: int) {
  m: [1] PayMap; ids := four(m[..], true);
  amount := 50000000 * (k + 1);
  r := pay_find_route(m[..], ids[0][..], ids[3][..], amount, 1018, 1000);
  p: [1] PayPayment; p[0].parts[0] = PayPart.{route = r}; p[0].parts[0].session[0] = 7;
  route: [1] OnionRoute; pubkeys: [66] u8; f: [4] u8; packet: [292] u8;
  for h: 0..<2 { for b: 0..<33 { pubkeys[33 * h + b] = m[0].edges[r.edge[h]].to[b]; } }
  session := p[0].parts[0].session;
  #assert onion_route(session[..], pubkeys[..], route[..]) == ONION_OK;
  n := onion_error_create(route[0].ss[0..<32], f[0..<pay_failure(PAY_TEMPORARY_CHANNEL_FAILURE, 0, 0, f[..])], 256, packet[..]);
  for h: 0..<2 { m[0].edges[r.edge[h]].inflight += r.amount[h]; }
  pay_learn(m[..], p[..], 0, packet[0..<n]);
  pay_settle_part(m[..], p[..], 0);
  #assert r.edge[1] == 3 && m[0].edges[3].hi == amount - 1 && m[0].edges[3].inflight == 0;
  again := pay_find_route(m[..], ids[0][..], ids[3][..], amount, 1018, 1000);
  #assert again.code == PAY_OK && again.edge[1] == 1;
}
#forall k: 0..<6 #steps 4000000000 { learned(k); }
```

```shell
coax/bin/coax test bitcoin/examples/checks/lightning-pay.coax --run-steps 250000000
```

`coax test` runs 594 checks, including the loaded files' own checks,
on 2026-11-03.

### The library's own checks

`pay.coax` is an `#authority` file. Its checks cover BOLT 7's fee, the
limits and the risk penalty; BOLT 4's failure messages; the search, with
disabled channels, `htlc_minimum_msat` and `htlc_maximum_msat`, learned
limits, failed channels, the payer's own balance, the CLTV and hop
limits and route hints; payloads written and read, with malformed ones
refused; an onion peeled hop by hop; gossip turned into the map; a
forwarding node's checks and each failure it returns, opened by the
payer; a payee holding parts until `total_msat`; what each failure
teaches; and payments over one channel opened with chapter 109's
messages, settled and refused.

Every check has a fixed limit on the work it may do, and each commitment
a channel signs uses much of it. A payment over two real channels does
not fit in one check. So the checks of a forwarding node and of the
payee give the node an HTLC its channel already holds irrevocably, and
the payments through several channels run in the example, natively.

```shell
coax/bin/coax test coax/lib/lightning/pay.coax --run-steps 250000000
```

`coax test` runs 590 checks, including those of the files it loads, on
2026-11-03. Mutation testing: not run (owner speed order).

### An independent model

[pay.py](../../coax/tests/lightning/pay.py) computes every line the
example prints. Its payer is written from BOLT 4, BOLT 7 and BOLT 11 and
this chapter's model; its graph is
[bolt7.py](../../coax/tests/lightning/bolt7.py)'s, fed with gossip that
bolt7.py writes and signs; its invoices come from
[bolt11.py](../../coax/tests/lightning/bolt11.py) and its onions from
[sphinx.py](../../coax/tests/lightning/sphinx.py). Its network is a
model of balances: each channel keeps both sides' balances and its
HTLCs, and refuses an HTLC the offerer could not pay while keeping its
reserve and, as opener, BOLT 3's commitment fee. None of it shares code
with Coax.

```shell
python3 -I coax/tests/lightning/pay.py --coax coax/bin/coax
```

On 2026-11-03 the BOLT 4, BOLT 7 and BOLT 11 pins check, all 65 lines
agree, and the example's output in native debug and release builds
equals them. Wasm was not run.

## What this chapter leaves out

- Waiting for the parts of a payment with a timer: the payee here holds
  incomplete parts until the payer stops, and never sends `mpp_timeout`.
- `update_fail_malformed_htlc` for an onion that does not decrypt, and
  the BADONION failures it carries.
- Learning from success, and letting what was learned fade with time.
- Shadow routes and random CLTV offsets, which hide how far the payee
  is; blinded paths in BOLT 12 invoices (chapter 115).
- Trampoline routing, and keysend payments without an invoice.
- The `channel_update` in an UPDATE failure, and `channel_disabled`.
- Retransmitting onions and failure reasons after `channel_reestablish`:
  chapter 109 resends updates with an empty onion.
- Splitting by the liquidity each route can carry, as minimum-cost-flow
  solvers do, instead of halving.

## Summary

- The search runs from the payee back to the payer, because each fee is
  charged on what is left to forward.
- A route's cost is its fees plus a penalty for each hop and a penalty
  for the chance of failure, here under a uniform model of each
  channel's liquidity.
- Each hop's payload says what to forward and where; the payee's says
  `payment_secret` and `total_msat`.
- A forwarding node acts only on an irrevocably committed HTLC, checks
  its fee and CLTV delta, and fails the HTLC back with an error only the
  payer can read.
- The payer learns from each failure which channel or node to avoid,
  splits what no route carries, and tries again.

## Dependencies

- [109. Open, update and close a channel](ch109-open-update-and-close-a-channel.md)
- [111. Onion routing and blinded paths](ch111-onion-routing-and-blinded-paths.md)
- [112. Gossip and the channel graph](ch112-gossip-and-the-channel-graph.md)
- [114. Invoices](ch114-invoices.md)

## Sources

- [BOLT 4](https://github.com/lightning/bolts/blob/master/04-onion-routing.md),
  "Packet Structure" (the payload TLVs and `payment_data`), "Basic
  Multi-Part Payments", "Failure Messages" and "Receiving Failure
  Codes", at lightning/bolts revision `19146282`, pinned in
  [coax/tests/vectors/upstream/bolts/](../../coax/tests/vectors/upstream/bolts/04-onion-routing.md).
- [BOLT 7](https://github.com/lightning/bolts/blob/master/07-routing-gossip.md),
  "HTLC Fees" and "Recommendations for Routing", at the same revision.
- [BOLT 2](https://github.com/lightning/bolts/blob/master/02-peer-protocol.md),
  "Adding an HTLC" and "Removing an HTLC", at the same revision.
- [BOLT 11](https://github.com/lightning/bolts/blob/master/11-payment-encoding.md),
  the `r`, `s`, `c` and `9` fields, at the same revision.
- René Pickhardt and Stefan Richter,
  [Optimally Reliable & Cheap Payment Flows on the Lightning Network](https://arxiv.org/abs/2107.05322),
  2021, for the uniform liquidity model.
- All pinned files are listed in [SOURCES.md](../../coax/tests/vectors/SOURCES.md).
