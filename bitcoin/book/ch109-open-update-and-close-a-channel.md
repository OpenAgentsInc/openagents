# 109. Open, update and close a channel

Chapter 108 built the transactions a channel is made of. This chapter
builds the protocol that makes them: BOLT 2's messages, which two peers
exchange to open a channel, to add, settle and fail payments in it, to
recover from a lost connection, and to close it together.

The code is a pure state machine. A node is a value. It takes one
message at a time and either refuses it, changing nothing, or applies it
and queues its replies. Nothing touches a socket or a chain, so two nodes
in one process can run the whole protocol, and every commitment they
sign is built and checked with chapter 108's code.

## What this chapter teaches

- Opening a channel: `open_channel`, `accept_channel`, `funding_created`,
  `funding_signed` and `channel_ready`, and why the opener may broadcast
  the funding transaction only after it holds a signed commitment.
- Updates: `update_add_htlc`, `update_fulfill_htlc` and
  `update_fail_htlc`, and the four stages each update passes through
  before both commitments hold it.
- `commitment_signed` and `revoke_and_ack`: signing the peer's next
  commitment with all its HTLC transactions, and revoking the old one by
  handing over its per-commitment secret.
- `channel_reestablish`: what each side forgets on a disconnection, what
  it sends again, and how a node learns that it lost state.
- A mutual close: `shutdown`, then the `closing_signed` fee negotiation
  and the closing transaction.

## Where Lightning uses this protocol

| Where | What it uses | Chapter |
| --- | --- | --- |
| Messages on the wire | BOLT 1's framing and TLVs | 104 |
| The encrypted connection | the Noise handshake | 105 |
| Each commitment's keys and secrets | chapters 106 and 107 | 106, 107 |
| Each commitment and HTLC transaction | chapter 108's builder | 108 |
| Closing alone, and penalties | the latest or a revoked commitment on chain | 110 |
| Routing a payment over several channels | the onion in `update_add_htlc` | 111 |

## Where the code is

| File | Procedure | What it does |
| --- | --- | --- |
| [channel.coax](../../coax/lib/lightning/channel.coax) | `lnh_init(node, funder, seed, config)` | a node with its secrets and settings |
| | `lnh_receive(node, message)` | applies one message from the peer, or refuses it |
| | `lnh_open`, `lnh_fund`, `lnh_confirmed` | the opener's commands, and the chain's signal |
| | `lnh_add`, `lnh_fulfill`, `lnh_fail`, `lnh_commit` | updates and signing |
| | `lnh_shutdown` | starts a mutual close |
| | `lnh_disconnect`, `lnh_reestablish` | a lost connection, and the reconnection |
| | `lnh_take`, `lnh_deliver` | moves a message out of a node's outbox |
| [commitment.coax](../../coax/lib/lightning/commitment.coax) | `lnc_commitment_vartime`, `lnc_sighash_vartime`, `lnc_htlc_sighash_vartime` | each commitment and HTLC transaction (chapter 108) |
| [keys.coax](../../coax/lib/lightning/keys.coax) | `lnk_tweak`, `lnk_encode` | the commitment keys (chapter 106) |
| [shachain.coax](../../coax/lib/lightning/shachain.coax) | `shachain_generate`, `shachain_insert` | the per-commitment secrets (chapter 107) |
| [ecdsa_sign.coax](../../coax/lib/crypto/ecdsa_sign.coax) | `ecdsa_sign` | RFC 6979 signatures (chapter 20) |

Messages are public, and the procedures that read and write them branch
on their contents. The node's secrets go only to chapter 20's
constant-time signer and to chapter 106's key arithmetic.

The library's codes, limits and message types:

<!-- source: coax/lib/lightning/channel.coax codes -->
```jai
LNH_OK :: 0;
LNH_UNKNOWN :: 1;          // an even message type this node does not handle
LNH_TRUNCATED :: 2;        // the message ends inside a field
LNH_WRONG_STATE :: 3;      // a message or command the channel's state does not allow
LNH_WRONG_CHANNEL :: 4;    // another channel's id
LNH_BAD_PARAMS :: 5;       // an open or accept value out of range, or a bad point or script
LNH_BAD_SIGNATURE :: 6;    // a commitment, HTLC or closing signature that does not verify
LNH_BAD_SECRET :: 7;       // a per-commitment secret that is not the one expected
LNH_BAD_ID :: 8;           // an HTLC id that is not the next one, or names no committed HTLC
LNH_BAD_PREIMAGE :: 9;     // a preimage whose SHA-256 is not the payment hash
LNH_BAD_AMOUNT :: 10;      // below htlc_minimum_msat, or more than the offerer can pay
LNH_TOO_MANY :: 11;        // more HTLCs than max_accepted_htlcs
LNH_NO_CHANGES :: 12;      // a commitment_signed that commits no update
LNH_HTLC_COUNT :: 13;      // not one HTLC signature per HTLC output
LNH_SHUTTING :: 14;        // an update_add_htlc after shutdown
LNH_BAD_FEE :: 15;         // a closing fee not between the last two offers, or over the funder's balance
LNH_WAITING :: 16;         // a second commitment_signed before the first is revoked
LNH_PENDING :: 17;         // shutdown with uncommitted updates
LNH_BEHIND :: 18;          // channel_reestablish proves this node lost state: do not broadcast
LNH_FULL :: 19;            // the outbox or the HTLC table is full

LNH_MSG :: 1500;           // the longest message here: update_add_htlc, 1,452 bytes
LNH_QUEUE :: 8;            // messages waiting in one outbox
LNH_SLOTS :: LNC_MAX_HTLCS;// HTLCs in flight, both ways
LNH_ONION :: 1366;         // update_add_htlc's onion_routing_packet
LNH_CLOSE_TX :: 400;       // the longest signed closing transaction

// Message types (BOLT 2).
LNH_OPEN :: 32; LNH_ACCEPT :: 33; LNH_FUNDING_CREATED :: 34; LNH_FUNDING_SIGNED :: 35;
LNH_READY :: 36; LNH_SHUTDOWN :: 38; LNH_CLOSING_SIGNED :: 39;
LNH_ADD :: 128; LNH_FULFILL :: 130; LNH_FAIL :: 131;
LNH_COMMIT :: 132; LNH_REVOKE :: 133; LNH_REESTABLISH :: 136;
```

## The node

A node holds its own secrets and basepoints, the peer's basepoints and
settings, both commitment numbers, the HTLCs in flight and an outbox.
The channel moves through nine states, from `init` to `closed`.

Each update, an HTLC's add or its removal, also has a stage. BOLT 2 says
an update is "irrevocably committed" only once both commitments hold it
and both old commitments are revoked. Until then a peer may still
broadcast a commitment without it. For an update this node proposed,
the stages are:

1. Proposed: the update message is sent.
2. Signed: a `commitment_signed` this node sent covers it.
3. Acked: the peer's `revoke_and_ack` arrived, so the peer's commitment
   holds it for good.
4. Done: the peer's `commitment_signed` covers it for this node too, and
   this node revoked its old commitment.

An update the peer proposed goes the other way: acked first, when the
peer's `commitment_signed` arrives and this node revokes, then signed,
then done.

<!-- source: coax/lib/lightning/channel.coax state -->
```jai
// The channel's life: v1 establishment, normal operation, then closing.
LNH_INIT :: 0;             // nothing sent
LNH_OPEN_SENT :: 1;        // opener: open_channel sent, waiting for accept_channel
LNH_ACCEPTED :: 2;         // opener: accepted, its wallet makes the funding transaction
LNH_ACCEPT_SENT :: 3;      // accepter: waiting for funding_created
LNH_CREATED_SENT :: 4;     // opener: waiting for funding_signed
LNH_FUNDING :: 5;          // both hold commitment 0; waiting for depth and channel_ready
LNH_NORMAL :: 6;           // updates flow
LNH_SHUTDOWN_STATE :: 7;   // shutdown sent or received: no new HTLCs
LNH_CLOSED :: 8;           // closing_signed agreed: the closing transaction is signed
LNH_STATES :: [9] string.["init", "open sent", "accepted", "accept sent", "created sent", "funding",
                          "normal", "shutdown", "closed"];

// An update moves through four stages. One this node proposed is
// PROPOSED, SIGNED once a commitment_signed it sent covers it, ACKED when
// the peer revokes its old commitment, and DONE when the peer's
// commitment_signed covers it for this node too and this node revokes.
// One the peer proposed is PROPOSED, ACKED once its commitment_signed
// arrives and this node revokes, SIGNED once this node's commitment_signed
// covers it, and DONE at the peer's revocation.
LNH_NONE :: -1; LNH_PROPOSED :: 0; LNH_DONE :: 3;
LNH_MINE_SIGNED :: 1; LNH_MINE_ACKED :: 2;
LNH_THEIRS_ACKED :: 1; LNH_THEIRS_SIGNED :: 2;
// How an HTLC leaves: not yet, fulfilled with its preimage, or failed.
LNH_KEEP :: 0; LNH_FULFILLED :: 1; LNH_FAILED :: 2;

// An HTLC and the stages of its add and its removal. mine says this node
// offered it; the removal is the other node's update.
LnhHtlc :: struct {
  id: int; mine: bool; amount_msat: int; expiry: int; payment_hash: [32] u8;
  add: int; removal: int; removing: int; preimage: [32] u8;
}
// A node's five basepoints.
LnhPoints :: struct { funding: [33] u8; revocation: [33] u8; payment: [33] u8; delayed: [33] u8; htlc: [33] u8; }
// What a node asks of its peer and offers when it opens or accepts.
LnhConfig :: struct {
  funding_sat: int; push_msat: int;       // the opener's
  feerate: int;                           // the opener's feerate_per_kw
  dust_limit: int;                        // for this node's own commitments
  reserve_sat: int;                       // the balance the peer must keep
  htlc_minimum_msat: int;                 // the smallest HTLC this node accepts
  to_self_delay: int;                     // the delay on the peer's to_local
  max_accepted: int;                      // HTLCs this node accepts at once
  close_fee: int; close_fee_min: int; close_fee_max: int;   // closing fee wanted and accepted
  spk: [34] u8; spk_n: int;               // where this node's closing output goes
}
LnhMsg :: struct { n: int; b: [LNH_MSG] u8; }
LnhNode :: struct {
  funder: bool; state: int;
  funding_secret: [32] u8; revocation_secret: [32] u8; payment_secret: [32] u8;
  delayed_secret: [32] u8; htlc_secret: [32] u8; seed: [32] u8;
  my: LnhPoints; their: LnhPoints;
  cfg: LnhConfig;                         // this node's
  their_cfg: LnhConfig;                   // the peer's, from open_channel or accept_channel
  funding_sat: int; push_msat: int; feerate: int;
  temporary_id: [32] u8; channel_id: [32] u8;
  funding_txid: [32] u8; funding_index: int;
  their_point: [33] u8;                   // for the peer's oldest unrevoked commitment
  their_next_point: [33] u8;              // for the one after it
  local_number: int;                      // this node's latest commitment
  remote_number: int;                     // the peer's latest commitment, signed by this node
  waiting: bool;                          // for the peer's revoke_and_ack
  local_sig: [64] u8;                     // the peer's signature of local_number
  local_htlc_sigs: [LNC_MAX_HTLCS * 64] u8; local_htlc_n: int;
  htlcs: [LNH_SLOTS] LnhHtlc; n_htlcs: int;
  paid_msat: int;                         // net amount of settled HTLCs this node paid the peer
  next_id: int; their_next_id: int;
  ready_sent: bool; ready_received: bool;
  shutdown_sent: bool; shutdown_received: bool;
  close_sent: int; close_received: int;   // the last fees offered, -1 before any
  closing: [LNH_CLOSE_TX] u8; closing_n: int;
  reestablishing: bool;
  seq: int; commit_seq: int; revoke_seq: int;   // the order commitments and revocations went out
  store: [1] Shachain;                    // the peer's per-commitment secrets
  cached: [2] int; cached_point: [2] LnkKey; cached_secret: [64] u8;   // this node's last two points and secrets
  out: [LNH_QUEUE] LnhMsg; out_n: int;
}
```

A node derives all its secrets from one 32-byte seed: SHA256(seed || i)
for i from 1 to 7.

<!-- source: coax/lib/lightning/channel.coax init -->
```jai
// A node with its secrets from seed and its own configuration.
lnh_init :: (n: [] LnhNode, funder: bool, seed: [] u8, cfg: LnhConfig) {
  n[0] = LnhNode.{funder = funder, cfg = cfg, close_sent = -1, close_received = -1};
  n[0].cached[0] = -1; n[0].cached[1] = -1;
  lnh_secret(seed, 1, n[0].funding_secret[..]); lnh_secret(seed, 2, n[0].revocation_secret[..]);
  lnh_secret(seed, 3, n[0].payment_secret[..]); lnh_secret(seed, 4, n[0].delayed_secret[..]);
  lnh_secret(seed, 5, n[0].htlc_secret[..]); lnh_secret(seed, 6, n[0].seed[..]);
  lnh_secret(seed, 7, n[0].temporary_id[..]);
  n[0].my.funding = lnh_base_point(n[0].funding_secret[..]).bytes;
  n[0].my.revocation = lnh_base_point(n[0].revocation_secret[..]).bytes;
  n[0].my.payment = lnh_base_point(n[0].payment_secret[..]).bytes;
  n[0].my.delayed = lnh_base_point(n[0].delayed_secret[..]).bytes;
  n[0].my.htlc = lnh_base_point(n[0].htlc_secret[..]).bytes;
  n[0].store[0] = shachain_new();
  if funder { n[0].funding_sat = cfg.funding_sat; n[0].push_msat = cfg.push_msat; n[0].feerate = cfg.feerate; }
}
```

## Which updates a commitment holds

`lnh_in` decides from an update's stage whether a commitment holds it.
`now` is the latest commitment signed, `next` is the one about to be
signed, and `all` counts every update sent or received, which is what a
new HTLC must be checked against. `lnh_spec` turns the node's HTLC table
into chapter 108's balances and HTLC list, for this node's commitment or
the peer's.

<!-- source: coax/lib/lightning/channel.coax spec -->
```jai
// Whether an update at stage, proposed by this node (mine) or the peer,
// is in this node's commitment (local) or the peer's. now: the latest
// signed commitment. next: the one about to be signed. all: every update
// sent or received, for checking what a new HTLC may spend.
LNH_NOW :: 0; LNH_NEXT :: 1; LNH_ALL :: 2;
lnh_in :: (stage: int, mine: bool, local: bool, when: int) -> bool {
  if stage == LNH_NONE { return false; }
  if when == LNH_ALL { return true; }
  next := when == LNH_NEXT;
  need := 0;
  if local { need = ifx mine then LNH_MINE_ACKED else (ifx next then LNH_PROPOSED else LNH_THEIRS_ACKED); }
  else { need = ifx mine then (ifx next then LNH_PROPOSED else LNH_MINE_SIGNED) else (ifx next then LNH_THEIRS_ACKED else LNH_THEIRS_SIGNED); }
  return stage >= need;
}
// One side's view of the channel: the owner's balance, the other's, and
// the HTLCs still open, offered meaning offered by the owner. slot maps
// each HTLC back to the node's table.
LnhSpec :: struct {
  to_local_msat: int; to_remote_msat: int;
  n: int; htlcs: [LNC_MAX_HTLCS] LncHtlc; slot: [LNC_MAX_HTLCS] int;
}
lnh_spec :: (n: [] LnhNode, local: bool, when: int) -> LnhSpec {
  r: LnhSpec;
  opener := n[0].funding_sat * 1000 - n[0].push_msat;
  mine := (ifx n[0].funder then opener else n[0].push_msat) - n[0].paid_msat;
  theirs := n[0].funding_sat * 1000 - mine;
  for i: 0..<n[0].n_htlcs {
    h := n[0].htlcs[i];
    if !lnh_in(h.add, h.mine, local, when) { continue; }
    if h.mine { mine -= h.amount_msat; } else { theirs -= h.amount_msat; }
    if h.removal != LNH_KEEP && lnh_in(h.removing, !h.mine, local, when) {
      // Fulfilled pays the receiver; failed returns the amount.
      paid_to_me := (h.removal == LNH_FULFILLED) != h.mine;
      if paid_to_me { mine += h.amount_msat; } else { theirs += h.amount_msat; }
      continue;
    }
    k := r.n;
    r.htlcs[k] = LncHtlc.{offered = h.mine == local, amount_msat = h.amount_msat, expiry = h.expiry, payment_hash = h.payment_hash};
    r.slot[k] = i; r.n += 1;
  }
  r.to_local_msat = ifx local then mine else theirs;
  r.to_remote_msat = ifx local then theirs else mine;
  return r;
}
```

Settled HTLCs are dropped from the table, and what a fulfilled one paid
is kept in `paid_msat`.

## Keys for each commitment

Each commitment uses fresh keys, from the owner's per-commitment point
for that number (chapter 106). This node's points come from its own
shachain seed (chapter 107), which counts down from 2^48 - 1. For the
peer, a node only ever needs two points: the one for the peer's oldest
unrevoked commitment, and the next. `channel_ready` and each
`revoke_and_ack` bring the next one.

<!-- source: coax/lib/lightning/channel.coax points -->
```jai
// This node's per-commitment secret and point for commitment number:
// chapter 107's shachain counts down from 2^48 - 1.
lnh_my_secret :: (n: [] LnhNode, number: int, out: [] u8) {
  code := shachain_generate(n[0].seed[..], shachain_index(cast(u64) number), out);
}
// A node uses each secret and point several times (to send the point,
// then to build the commitment it belongs to), so it keeps the last two.
lnh_cache :: (n: [] LnhNode, number: int) -> int {
  slot := number % 2;
  if n[0].cached[slot] == number { return slot; }
  s: [32] u8; lnh_my_secret(n, number, s[..]);
  n[0].cached_point[slot] = lnh_base_point(s[..]);
  for i: 0..<32 { n[0].cached_secret[32 * slot + i] = s[i]; s[i] = 0; }
  n[0].cached[slot] = number;
  return slot;
}
lnh_my_point :: (n: [] LnhNode, number: int) -> LnkKey { return n[0].cached_point[lnh_cache(n, number)]; }
// The peer's point for one of its two commitments this node can sign.
lnh_their_point :: (n: [] LnhNode, number: int) -> LnkKey {
  oldest := n[0].remote_number - ifx n[0].waiting then 1 else 0;
  r: LnkKey; r.bytes = ifx number == oldest then n[0].their_point else n[0].their_next_point;
  return r;
}
```

The derivations are chapter 106's, computed with multiplications that
need fewer steps. A multiple of G uses the signer's precomputed table,
and the revocation key uses whichever of its two secrets the node holds.
A check compares every key with chapter 106's procedures.

<!-- source: coax/lib/lightning/channel.coax derive -->
```jai
// Chapter 106's derivations, computed another way. A multiple of G uses
// the signer's precomputed comb table, and any other point the
// fixed-window multiplication of lnk_point; both are constant time. The
// keys are the same (a check compares them with lnk_point,
// lnk_pubkey_vartime and lnk_revocation_pubkey_vartime), and Coax's
// compile-time interpreter, where every check runs, runs these several
// times faster.
lnh_base_point :: (secret: [] u8) -> LnkKey {
  if secret.count != LNK_SECRET { return LnkKey.{code = LNK_BAD_SECRET}; }
  k := sc_load(secret);
  if !k.ok || sc_is_zero(k.value) { k = ScLoad.{}; return LnkKey.{code = LNK_BAD_SECRET}; }
  r := lnk_encode(SgResult.{value = smb_multiply_web(k.value), ok = true});
  k = ScLoad.{};
  return r;
}
lnh_times :: (k: ScScalar, p: [] u8) -> SgResult {
  q := se_decode(p);
  if !q.ok { return SgResult.{}; }
  return smc_multiply(k, q.value);
}
lnh_pubkey :: (basepoint: [] u8, point: [] u8) -> LnkKey {
  if !lnh_point_ok(basepoint) || !lnh_point_ok(point) { return LnkKey.{code = LNK_BAD_POINT}; }
  t := lnk_tweak(point, basepoint);
  if t.code != LNK_OK { return LnkKey.{code = t.code}; }
  return lnk_encode(sg_add(se_decode(basepoint).value, smb_multiply_web(t.value)));
}
// R·SHA256(R || P) + P·SHA256(P || R). A node always holds one of the two
// secrets: its own revocation basepoint's when the commitment is the
// peer's (of_base), its own per-commitment secret when it is its own. That
// term is a multiple of G.
lnh_revocation_pubkey :: (revocation_basepoint: [] u8, point: [] u8, secret: [] u8, of_base: bool) -> LnkKey {
  if !lnh_point_ok(revocation_basepoint) || !lnh_point_ok(point) { return LnkKey.{code = LNK_BAD_POINT}; }
  tr := lnk_tweak(revocation_basepoint, point); tp := lnk_tweak(point, revocation_basepoint);
  if tr.code != LNK_OK || tp.code != LNK_OK { return LnkKey.{code = LNK_BAD_TWEAK}; }
  k := sc_load(secret);
  known := smb_multiply_web(sc_mul(k.value, ifx of_base then tr.value else tp.value));
  k = ScLoad.{};
  other := ifx of_base then lnh_times(tp.value, point) else lnh_times(tr.value, revocation_basepoint);
  if !other.ok { return LnkKey.{code = LNK_INFINITY}; }
  return lnk_encode(sg_add(known, other.value));
}
// Chapter 21's ECDSA verification with low S, R = u1·G + u2·Q taken from
// the comb and the fixed-window multiplication instead of
// ecmult_vartime.
lnh_verify :: (hash: [] u8, public_key: [] u8, signature: [] u8) -> bool {
  key := se_decode(public_key);
  if !key.ok { return false; }
  p := ev_prepare_vartime(hash, key.value, signature, true);
  if !p.ready { return false; }
  q := smc_multiply(p.u2, p.point);
  if !q.ok { return false; }
  sum := sg_add(smb_multiply_web(p.u1), q.value);
  if !sum.ok { return false; }
  affine := sg_to_affine(sum.value).value;
  if affine.infinity != 0 { return false; }
  x: [32] u8;
  #assert sf_store(affine.x, x[..]);
  return sc_equal(sc_reduce_bytes(x[..]).value, p.r);
}
```

`lnh_view` fills in chapter 108's channel for one side's commitment. The
owner's `to_local` waits the delay the *other* side asked for, and the
owner's own dust limit trims its outputs. Under `option_static_remotekey`
the other side's balance goes straight to its `payment_basepoint`.

<!-- source: coax/lib/lightning/channel.coax view -->
```jai
// Chapter 108's channel for one commitment: this node's (local) or the
// peer's, with that owner's per-commitment point. The owner's to_local
// waits the delay the other side asked for, and the owner's dust limit
// trims its HTLCs.
lnh_view :: (n: [] LnhNode, local: bool, number: int, point: [] u8) -> LncChannel {
  c: LncChannel;
  c.funding_txid = n[0].funding_txid; c.funding_index = n[0].funding_index; c.funding_sat = n[0].funding_sat;
  owner := ifx local then n[0].my else n[0].their;
  other := ifx local then n[0].their else n[0].my;
  c.local_funding = owner.funding; c.remote_funding = other.funding;
  secret: [32] u8;
  if local { slot := lnh_cache(n, number); for i: 0..<32 { secret[i] = n[0].cached_secret[32 * slot + i]; } }
  else { for i: 0..<32 { secret[i] = n[0].revocation_secret[i]; } }
  c.revocation = lnh_revocation_pubkey(other.revocation[..], point, secret[..], !local).bytes;
  for i: 0..<32 { secret[i] = 0; }
  c.local_delayed = lnh_pubkey(owner.delayed[..], point).bytes;
  c.remote = other.payment;                                    // option_static_remotekey
  c.local_htlc = lnh_pubkey(owner.htlc[..], point).bytes;
  c.remote_htlc = lnh_pubkey(other.htlc[..], point).bytes;
  c.to_self_delay = ifx local then n[0].their_cfg.to_self_delay else n[0].cfg.to_self_delay;
  c.dust_limit = ifx local then n[0].cfg.dust_limit else n[0].their_cfg.dust_limit;
  opener := ifx n[0].funder then n[0].my.payment else n[0].their.payment;
  accepter := ifx n[0].funder then n[0].their.payment else n[0].my.payment;
  c.obscured = lnc_obscured_vartime(number, opener[..], accepter[..]);
  c.local_funder = local == n[0].funder;
  return c;
}
```

A `commitment_signed` carries the signature of the commitment, then one
signature for each HTLC output's HTLC transaction, in output order. The
receiver rebuilds the commitment itself and checks every signature, so
a wrong amount, a missing HTLC or a stray one all show up as a bad
signature or a wrong count.

<!-- source: coax/lib/lightning/channel.coax sigs -->
```jai
// Signs one side's commitment and its HTLC transactions: out gets the
// commitment signature, then one per HTLC output in output order. The
// count of HTLC signatures, or -1.
lnh_sign_commitment :: (n: [] LnhNode, c: LncChannel, cm: LncCommit, spec: LnhSpec, point: [] u8, out: [] u8) -> int {
  ch := c; m := cm; s := spec; digest: [32] u8; key: [32] u8;
  if !lnc_sighash_vartime(ch, m, digest[..]) || !lnh_sign(n[0].funding_secret[..], digest[..], out[0..<64]) { return -1; }
  k := 0;
  if m.n > lnh_plain_outputs(m) && lnh_htlc_privkey(n, point, key[..]) != LNK_OK { return -1; }
  for i: 0..<m.n {
    if m.out[i].kind != LNC_OFFERED && m.out[i].kind != LNC_RECEIVED { continue; }
    if !lnc_htlc_sighash_vartime(ch, m, s.htlcs[0..<s.n], i, n[0].feerate, lnc_remote_hashtype(ch), digest[..]) { return -1; }
    k += 1;
    if !lnh_sign(key[..], digest[..], out[64 * k..<64 * k + 64]) { return -1; }
  }
  for i: 0..<32 { key[i] = 0; }
  return k;
}
// Checks the peer's signatures of this node's commitment: sigs holds the
// commitment signature, then htlc_n HTLC signatures. LNH_OK, or why not.
lnh_check_commitment :: (n: [] LnhNode, c: LncChannel, cm: LncCommit, spec: LnhSpec, sigs: [] u8, htlc_n: int) -> int {
  ch := c; m := cm; s := spec; digest: [32] u8;
  if m.code != LNC_OK { return LNH_BAD_AMOUNT; }
  if htlc_n != m.n - lnh_plain_outputs(m) { return LNH_HTLC_COUNT; }
  if !lnc_sighash_vartime(ch, m, digest[..]) || !lnh_verify(digest[..], ch.remote_funding[..], sigs[0..<64]) {
    return LNH_BAD_SIGNATURE;
  }
  k := 0;
  for i: 0..<m.n {
    if m.out[i].kind != LNC_OFFERED && m.out[i].kind != LNC_RECEIVED { continue; }
    k += 1;
    if !lnc_htlc_sighash_vartime(ch, m, s.htlcs[0..<s.n], i, n[0].feerate, lnc_remote_hashtype(ch), digest[..]) ||
       !lnh_verify(digest[..], ch.remote_htlc[..], sigs[64 * k..<64 * k + 64]) { return LNH_BAD_SIGNATURE; }
  }
  return LNH_OK;
}
lnh_plain_outputs :: (cm: LncCommit) -> int {
  m := cm; k := 0;
  for i: 0..<m.n { if m.out[i].kind != LNC_OFFERED && m.out[i].kind != LNC_RECEIVED { k += 1; } }
  return k;
}
```

## Opening a channel

The opener sends `open_channel`: the chain, a temporary channel id, the
funding amount, what it pushes to the peer, its limits, its five
basepoints and its first per-commitment point. A `channel_type` TLV
names the channel's features; here only `option_static_remotekey`
(feature bit 12) is supported.

<!-- source: coax/lib/lightning/channel.coax open -->
```jai
LNH_CHAIN :: "06226e46111a0b59caaf126043eb5bbf28c34f3a5e332a1fc7b2b73cf188910f";   // regtest's genesis hash
LNH_CHANNEL_TYPE :: cast(u64) 1;       // TLV: the channel type's feature bits
LNH_STATIC_REMOTEKEY :: 12;            // the only channel type here
// The fields open_channel and accept_channel share, from dust_limit to
// first_per_commitment_point.
lnh_write_params :: (n: [] LnhNode, out: [] u8, at0: int, accept: bool) -> int {
  c := n[0].cfg;
  at := lnh_put(out, at0, c.dust_limit, 8);
  at = lnh_put(out, at, n[0].funding_sat * 1000, 8);          // max_htlc_value_in_flight_msat: no limit
  at = lnh_put(out, at, c.reserve_sat, 8);
  at = lnh_put(out, at, c.htlc_minimum_msat, 8);
  if accept { at = lnh_put(out, at, 3, 4); }                    // minimum_depth
  else { at = lnh_put(out, at, n[0].feerate, 4); }
  at = lnh_put(out, at, c.to_self_delay, 2);
  at = lnh_put(out, at, c.max_accepted, 2);
  at = lnh_copy(out, at, n[0].my.funding[..]); at = lnh_copy(out, at, n[0].my.revocation[..]);
  at = lnh_copy(out, at, n[0].my.payment[..]); at = lnh_copy(out, at, n[0].my.delayed[..]);
  at = lnh_copy(out, at, n[0].my.htlc[..]);
  first := lnh_my_point(n, 0);
  return lnh_copy(out, at, first.bytes[..]);
}
lnh_write_type :: (out: [] u8, at: int) -> int {
  f: [2] u8; ok := ln_feature_put(f[..], LNH_STATIC_REMOTEKEY);
  return tlv_write_vartime(out, at, LNH_CHANNEL_TYPE, f[..]);
}
// The opener's first message.
lnh_open :: (n: [] LnhNode) -> int {
  if !n[0].funder || n[0].state != LNH_INIT { return LNH_WRONG_STATE; }
  m: [LNH_MSG] u8; chain: [32] u8; b := tx_unhex(LNH_CHAIN, chain[..]);
  at := lnh_put(m[..], 0, LNH_OPEN, 2);
  at = lnh_copy(m[..], at, chain[..]); at = lnh_copy(m[..], at, n[0].temporary_id[..]);
  at = lnh_put(m[..], at, n[0].funding_sat, 8); at = lnh_put(m[..], at, n[0].push_msat, 8);
  at = lnh_write_params(n, m[..], at, false);
  m[at] = 0; at += 1;                                           // channel_flags: not announced
  at = lnh_write_type(m[..], at);
  n[0].state = LNH_OPEN_SENT;
  return lnh_queue(n, m[0..<at]);
}
// Reads the shared fields into a config and the peer's basepoints and
// first point; checks them against BOLT 2's limits.
LnhParams :: struct { code: int; cfg: LnhConfig; points: LnhPoints; first: [33] u8; feerate: int; }
lnh_read_params :: (m: [] u8, at0: int, accept: bool, funding_sat: int) -> LnhParams {
  r: LnhParams; at := at0;
  r.cfg.dust_limit = lnh_get(m, at, 8); at += 16;               // and max_htlc_value_in_flight_msat
  r.cfg.reserve_sat = lnh_get(m, at, 8); at += 8;
  r.cfg.htlc_minimum_msat = lnh_get(m, at, 8); at += 8;
  r.feerate = ifx accept then 0 else lnh_get(m, at, 4); at += 4;
  r.cfg.to_self_delay = lnh_get(m, at, 2); at += 2;
  r.cfg.max_accepted = lnh_get(m, at, 2); at += 2;
  for i: 0..<33 {
    r.points.funding[i] = m[at + i]; r.points.revocation[i] = m[at + 33 + i]; r.points.payment[i] = m[at + 66 + i];
    r.points.delayed[i] = m[at + 99 + i]; r.points.htlc[i] = m[at + 132 + i]; r.first[i] = m[at + 165 + i];
  }
  p := r.points;
  ok := lnh_point_ok(p.funding[..]) && lnh_point_ok(p.revocation[..]) && lnh_point_ok(p.payment[..]) &&
        lnh_point_ok(p.delayed[..]) && lnh_point_ok(p.htlc[..]) && lnh_point_ok(r.first[..]);
  // BOLT 2: dust_limit_satoshis at least 354, the reserve at least the
  // dust limit, max_accepted_htlcs at most 483, and an unreasonable delay
  // refused (here, more than 2,016 blocks).
  ok = ok && r.cfg.dust_limit >= 354 && r.cfg.reserve_sat >= r.cfg.dust_limit && r.cfg.reserve_sat <= funding_sat;
  ok = ok && r.cfg.max_accepted >= 1 && r.cfg.max_accepted <= 483 && r.cfg.to_self_delay <= 2016;
  ok = ok && (accept || r.feerate >= 253);
  r.code = ifx ok then LNH_OK else LNH_BAD_PARAMS;
  return r;
}
LNH_PARAMS_SIZE :: 8 * 4 + 4 + 2 + 2 + 33 * 6;
// The TLVs after the fields: channel_type must be static_remotekey alone.
lnh_check_type :: (m: [] u8, at: int) -> int {
  known: [1] u64; known[0] = LNH_CHANNEL_TYPE;
  if tlv_check_vartime(m[at..<m.count], known[..]) != LN_OK { return LNH_BAD_PARAMS; }
  if at >= m.count { return LNH_BAD_PARAMS; }
  r := tlv_next_vartime(m, at, cast(u64) 0, true);
  want: [2] u8; ok := ln_feature_put(want[..], LNH_STATIC_REMOTEKEY);
  if r.type != LNH_CHANNEL_TYPE || !lnh_same(m[r.start..<r.start + r.length], want[..]) { return LNH_BAD_PARAMS; }
  return LNH_OK;
}
```

The accepter checks the values against BOLT 2's limits and answers with
`accept_channel`, its own limits and points, and how many confirmations
it wants. It also checks that the opener can pay for the first
commitment and still keep the reserve.

<!-- source: coax/lib/lightning/channel.coax accept -->
```jai
// The accepter checks open_channel and answers with accept_channel.
lnh_on_open :: (n: [] LnhNode, m: [] u8) -> int {
  if n[0].funder || n[0].state != LNH_INIT { return LNH_WRONG_STATE; }
  fixed := 2 + 64 + 16 + LNH_PARAMS_SIZE + 1;
  if m.count < fixed { return LNH_TRUNCATED; }
  chain: [32] u8; b := tx_unhex(LNH_CHAIN, chain[..]);
  funding := lnh_get(m, 66, 8); push := lnh_get(m, 74, 8);
  p := lnh_read_params(m, 82, false, funding);
  // Not wumbo: under 2^24 satoshis, and the push within the funding.
  if !lnh_same(m[2..<34], chain[..]) || funding < 1000 || funding >= 16777216 || push > funding * 1000 { return LNH_BAD_PARAMS; }
  if p.code != LNH_OK { return p.code; }
  if lnh_check_type(m, fixed) != LNH_OK { return LNH_BAD_PARAMS; }
  // The opener pays the first commitment's fee from its balance.
  if funding * 1000 - push < (lnc_commit_fee(p.feerate, 0, false) + n[0].cfg.reserve_sat) * 1000 { return LNH_BAD_AMOUNT; }
  lnh_accept_params(n, p);
  for i: 0..<32 { n[0].temporary_id[i] = m[34 + i]; }
  n[0].funding_sat = funding; n[0].push_msat = push; n[0].feerate = p.feerate;
  out: [LNH_MSG] u8;
  at := lnh_put(out[..], 0, LNH_ACCEPT, 2);
  at = lnh_copy(out[..], at, n[0].temporary_id[..]);
  at = lnh_write_params(n, out[..], at, true);
  at = lnh_write_type(out[..], at);
  n[0].state = LNH_ACCEPT_SENT;
  return lnh_queue(n, out[0..<at]);
}
// The opener checks accept_channel; its wallet then makes the funding
// transaction.
lnh_on_accept :: (n: [] LnhNode, m: [] u8) -> int {
  if !n[0].funder || n[0].state != LNH_OPEN_SENT { return LNH_WRONG_STATE; }
  fixed := 2 + 32 + LNH_PARAMS_SIZE;
  if m.count < fixed { return LNH_TRUNCATED; }
  if !lnh_same(m[2..<34], n[0].temporary_id[..]) { return LNH_WRONG_CHANNEL; }
  p := lnh_read_params(m, 34, true, n[0].funding_sat);
  if p.code != LNH_OK { return p.code; }
  if lnh_check_type(m, fixed) != LNH_OK { return LNH_BAD_PARAMS; }
  lnh_accept_params(n, p);
  n[0].state = LNH_ACCEPTED;
  return LNH_OK;
}
```

The opener's wallet now builds the funding transaction, but does not
broadcast it. `funding_created` names the funding outpoint and carries
the opener's signature of the accepter's first commitment. The accepter
answers with `funding_signed`, its signature of the opener's. Only then
does the opener broadcast. Before that, a peer that disappeared would
leave the funds locked in a 2-of-2 with no way out. From here the
channel id is the funding txid with the output index XORed into its
last two bytes.

<!-- source: coax/lib/lightning/channel.coax funding -->
```jai
// channel_id: the funding txid with the output index XORed into its last
// two bytes.
lnh_channel_id :: (n: [] LnhNode) {
  for i: 0..<32 { n[0].channel_id[i] = n[0].funding_txid[i]; }
  n[0].channel_id[30] ^= cast(u8) ((n[0].funding_index >> 8) & 255);
  n[0].channel_id[31] ^= cast(u8) (n[0].funding_index & 255);
}
// The opener names the funding outpoint its wallet made (txid in hash
// order) and signs the accepter's commitment 0.
lnh_fund :: (n: [] LnhNode, txid: [] u8, index: int) -> int {
  if n[0].state != LNH_ACCEPTED { return LNH_WRONG_STATE; }
  for i: 0..<32 { n[0].funding_txid[i] = txid[i]; }
  n[0].funding_index = index;
  lnh_channel_id(n);
  b := lnh_build(n, false, 0, LNH_NOW);
  sigs: [64] u8;
  if b.cm.code != LNC_OK || lnh_sign_commitment(n, b.c, b.cm, b.spec, b.point[..], sigs[..]) != 0 { return LNH_BAD_AMOUNT; }
  m: [LNH_MSG] u8;
  at := lnh_put(m[..], 0, LNH_FUNDING_CREATED, 2);
  at = lnh_copy(m[..], at, n[0].temporary_id[..]); at = lnh_copy(m[..], at, txid);
  at = lnh_put(m[..], at, index, 2); at = lnh_copy(m[..], at, sigs[..]);
  n[0].state = LNH_CREATED_SENT;
  return lnh_queue(n, m[0..<at]);
}
// The accepter checks the opener's signature of its commitment 0, then
// signs the opener's.
lnh_on_funding_created :: (n: [] LnhNode, m: [] u8) -> int {
  if n[0].funder || n[0].state != LNH_ACCEPT_SENT { return LNH_WRONG_STATE; }
  if m.count < 2 + 32 + 32 + 2 + 64 { return LNH_TRUNCATED; }
  if !lnh_same(m[2..<34], n[0].temporary_id[..]) { return LNH_WRONG_CHANNEL; }
  keep := n[0];
  for i: 0..<32 { n[0].funding_txid[i] = m[34 + i]; }
  n[0].funding_index = lnh_get(m, 66, 2);
  lnh_channel_id(n);
  mine := lnh_build(n, true, 0, LNH_NOW);
  code := lnh_check_commitment(n, mine.c, mine.cm, mine.spec, m[68..<132], 0);
  if code != LNH_OK { n[0] = keep; return code; }
  for i: 0..<64 { n[0].local_sig[i] = m[68 + i]; }
  theirs := lnh_build(n, false, 0, LNH_NOW);
  sigs: [64] u8;
  if lnh_sign_commitment(n, theirs.c, theirs.cm, theirs.spec, theirs.point[..], sigs[..]) != 0 { n[0] = keep; return LNH_BAD_AMOUNT; }
  out: [LNH_MSG] u8;
  at := lnh_put(out[..], 0, LNH_FUNDING_SIGNED, 2);
  at = lnh_copy(out[..], at, n[0].channel_id[..]); at = lnh_copy(out[..], at, sigs[..]);
  n[0].state = LNH_FUNDING;
  return lnh_queue(n, out[0..<at]);
}
// The opener checks the accepter's signature of its commitment 0. Only
// now may it broadcast the funding transaction.
lnh_on_funding_signed :: (n: [] LnhNode, m: [] u8) -> int {
  if !n[0].funder || n[0].state != LNH_CREATED_SENT { return LNH_WRONG_STATE; }
  if m.count < 2 + 32 + 64 { return LNH_TRUNCATED; }
  if !lnh_same(m[2..<34], n[0].channel_id[..]) { return LNH_WRONG_CHANNEL; }
  mine := lnh_build(n, true, 0, LNH_NOW);
  code := lnh_check_commitment(n, mine.c, mine.cm, mine.spec, m[34..<98], 0);
  if code != LNH_OK { return code; }
  for i: 0..<64 { n[0].local_sig[i] = m[34 + i]; }
  n[0].state = LNH_FUNDING;
  return LNH_OK;
}
```

When the funding transaction is deep enough, each side sends
`channel_ready` with its point for commitment 1, and the channel is
open.

<!-- source: coax/lib/lightning/channel.coax ready -->
```jai
// The chain says the funding transaction is deep enough: channel_ready
// carries the point for commitment 1.
lnh_write_ready :: (n: [] LnhNode) -> int {
  m: [LNH_MSG] u8;
  at := lnh_put(m[..], 0, LNH_READY, 2);
  at = lnh_copy(m[..], at, n[0].channel_id[..]);
  second := lnh_my_point(n, 1);
  at = lnh_copy(m[..], at, second.bytes[..]);
  return lnh_queue(n, m[0..<at]);
}
lnh_confirmed :: (n: [] LnhNode) -> int {
  if n[0].state != LNH_FUNDING || n[0].ready_sent { return LNH_WRONG_STATE; }
  code := lnh_write_ready(n);
  if code != LNH_OK { return code; }
  n[0].ready_sent = true;
  if n[0].ready_received { n[0].state = LNH_NORMAL; }
  return LNH_OK;
}
lnh_on_ready :: (n: [] LnhNode, m: [] u8) -> int {
  if m.count < 2 + 32 + 33 { return LNH_TRUNCATED; }
  if !lnh_same(m[2..<34], n[0].channel_id[..]) { return LNH_WRONG_CHANNEL; }
  if n[0].ready_received { return LNH_OK; }                     // a repeat after reconnecting: ignored
  if n[0].state != LNH_FUNDING { return LNH_WRONG_STATE; }
  if !lnh_point_ok(m[34..<67]) { return LNH_BAD_PARAMS; }
  for i: 0..<33 { n[0].their_next_point[i] = m[34 + i]; }
  n[0].ready_received = true;
  if n[0].ready_sent { n[0].state = LNH_NORMAL; }
  return LNH_OK;
}
```

## Adding an HTLC

`update_add_htlc` offers an HTLC: its id, which counts up from 0 for
each sender, the amount, the payment hash, the expiry, and chapter 111's
1,366-byte onion. The receiver checks that the id is the next one, that
it accepts this many HTLCs and this amount, and that the offerer can
afford it. The offerer's balance, less the amount and, for the funder,
the fee of a commitment with one more HTLC, must keep the reserve the
other side asked for.

<!-- source: coax/lib/lightning/channel.coax add -->
```jai
lnh_find :: (n: [] LnhNode, id: int, mine: bool) -> int {
  for i: 0..<n[0].n_htlcs { if n[0].htlcs[i].id == id && n[0].htlcs[i].mine == mine { return i; } }
  return -1;
}
lnh_count :: (n: [] LnhNode, mine: bool) -> int {
  k := 0; for i: 0..<n[0].n_htlcs { if n[0].htlcs[i].mine == mine { k += 1; } }
  return k;
}
// Whether the offerer (this node when mine) can pay amount more: its
// balance over every update so far, less the amount, less the fee of a
// commitment with one more HTLC when it is the funder, must keep the
// reserve the other side asked for.
lnh_affords :: (n: [] LnhNode, mine: bool, amount: int) -> bool {
  s := lnh_spec(n, true, LNH_ALL);
  balance := ifx mine then s.to_local_msat else s.to_remote_msat;
  reserve := ifx mine then n[0].their_cfg.reserve_sat else n[0].cfg.reserve_sat;
  fee := ifx mine == n[0].funder then lnc_commit_fee(n[0].feerate, s.n + 1, false) else 0;
  return balance - amount >= (reserve + fee) * 1000;
}
lnh_write_add :: (n: [] LnhNode, h: LnhHtlc) -> int {
  x := h; m: [LNH_MSG] u8;
  at := lnh_put(m[..], 0, LNH_ADD, 2);
  at = lnh_copy(m[..], at, n[0].channel_id[..]);
  at = lnh_put(m[..], at, x.id, 8); at = lnh_put(m[..], at, x.amount_msat, 8);
  at = lnh_copy(m[..], at, x.payment_hash[..]); at = lnh_put(m[..], at, x.expiry, 4);
  for i: 0..<LNH_ONION { m[at + i] = 0; }                       // chapter 111's onion; not routed here
  return lnh_queue(n, m[0..<at + LNH_ONION]);
}
// Offers an HTLC to the peer.
lnh_add :: (n: [] LnhNode, amount_msat: int, payment_hash: [] u8, expiry: int) -> int {
  if n[0].state != LNH_NORMAL || n[0].reestablishing { return ifx n[0].state == LNH_SHUTDOWN_STATE then LNH_SHUTTING else LNH_WRONG_STATE; }
  if n[0].n_htlcs == LNH_SLOTS || n[0].out_n == LNH_QUEUE { return LNH_FULL; }
  if lnh_count(n, true) >= n[0].their_cfg.max_accepted { return LNH_TOO_MANY; }
  if amount_msat < n[0].their_cfg.htlc_minimum_msat || amount_msat <= 0 || !lnh_affords(n, true, amount_msat) { return LNH_BAD_AMOUNT; }
  if expiry <= 0 || expiry >= 500000000 { return LNH_BAD_PARAMS; }
  h := LnhHtlc.{id = n[0].next_id, mine = true, amount_msat = amount_msat, expiry = expiry, add = LNH_PROPOSED, removal = LNH_KEEP, removing = LNH_NONE};
  for i: 0..<32 { h.payment_hash[i] = payment_hash[i]; }
  code := lnh_write_add(n, h);
  if code != LNH_OK { return code; }
  n[0].htlcs[n[0].n_htlcs] = h; n[0].n_htlcs += 1; n[0].next_id += 1;
  return LNH_OK;
}
lnh_on_add :: (n: [] LnhNode, m: [] u8) -> int {
  if m.count < 2 + 32 + 8 + 8 + 32 + 4 + LNH_ONION { return LNH_TRUNCATED; }
  if !lnh_same(m[2..<34], n[0].channel_id[..]) { return LNH_WRONG_CHANNEL; }
  if n[0].shutdown_received { return LNH_SHUTTING; }
  if n[0].state != LNH_NORMAL && n[0].state != LNH_SHUTDOWN_STATE { return LNH_WRONG_STATE; }
  id := lnh_get(m, 34, 8); amount := lnh_get(m, 42, 8); expiry := lnh_get(m, 82, 4);
  if id != n[0].their_next_id { return LNH_BAD_ID; }
  if n[0].n_htlcs == LNH_SLOTS || lnh_count(n, false) >= n[0].cfg.max_accepted { return LNH_TOO_MANY; }
  if amount < n[0].cfg.htlc_minimum_msat || amount <= 0 || !lnh_affords(n, false, amount) { return LNH_BAD_AMOUNT; }
  if expiry >= 500000000 { return LNH_BAD_PARAMS; }
  h := LnhHtlc.{id = id, mine = false, amount_msat = amount, expiry = expiry, add = LNH_PROPOSED, removal = LNH_KEEP, removing = LNH_NONE};
  for i: 0..<32 { h.payment_hash[i] = m[50 + i]; }
  n[0].htlcs[n[0].n_htlcs] = h; n[0].n_htlcs += 1; n[0].their_next_id += 1;
  return LNH_OK;
}
```

## Fulfilling or failing an HTLC

Only the receiver of an HTLC removes it, and only once the add is
irrevocably committed. `update_fulfill_htlc` carries the preimage, and
the amount goes to the receiver. `update_fail_htlc` carries an
encrypted reason for the payer (chapter 111; empty here), and the
amount goes back.

<!-- source: coax/lib/lightning/channel.coax remove -->
```jai
// The receiver of an HTLC removes it, once both commitments hold it:
// fulfilled with the preimage, or failed.
lnh_write_remove :: (n: [] LnhNode, h: LnhHtlc) -> int {
  x := h; m: [LNH_MSG] u8;
  at := lnh_put(m[..], 0, ifx x.removal == LNH_FULFILLED then LNH_FULFILL else LNH_FAIL, 2);
  at = lnh_copy(m[..], at, n[0].channel_id[..]);
  at = lnh_put(m[..], at, x.id, 8);
  if x.removal == LNH_FULFILLED { at = lnh_copy(m[..], at, x.preimage[..]); }
  else { at = lnh_put(m[..], at, 0, 2); }                       // reason: chapter 111's error onion; none here
  return lnh_queue(n, m[0..<at]);
}
lnh_remove :: (n: [] LnhNode, id: int, removal: int, preimage: [] u8) -> int {
  if (n[0].state != LNH_NORMAL && n[0].state != LNH_SHUTDOWN_STATE) || n[0].reestablishing { return LNH_WRONG_STATE; }
  i := lnh_find(n, id, false);
  if i < 0 || n[0].htlcs[i].add != LNH_DONE || n[0].htlcs[i].removal != LNH_KEEP { return LNH_BAD_ID; }
  if removal == LNH_FULFILLED {
    hash: [32] u8; lnh_sha(preimage, hash[..]);
    if !lnh_same(hash[..], n[0].htlcs[i].payment_hash[..]) { return LNH_BAD_PREIMAGE; }
  }
  h := n[0].htlcs[i];
  h.removal = removal; h.removing = LNH_PROPOSED;
  if removal == LNH_FULFILLED { for k: 0..<32 { h.preimage[k] = preimage[k]; } }
  code := lnh_write_remove(n, h);
  if code == LNH_OK { n[0].htlcs[i] = h; }
  return code;
}
lnh_fulfill :: (n: [] LnhNode, id: int, preimage: [] u8) -> int { return lnh_remove(n, id, LNH_FULFILLED, preimage); }
lnh_fail :: (n: [] LnhNode, id: int) -> int { none: [32] u8; return lnh_remove(n, id, LNH_FAILED, none[..]); }
lnh_on_remove :: (n: [] LnhNode, m: [] u8, removal: int) -> int {
  tail := ifx removal == LNH_FULFILLED then 32 else 2;
  if m.count < 2 + 32 + 8 + tail { return LNH_TRUNCATED; }
  if !lnh_same(m[2..<34], n[0].channel_id[..]) { return LNH_WRONG_CHANNEL; }
  if n[0].state != LNH_NORMAL && n[0].state != LNH_SHUTDOWN_STATE { return LNH_WRONG_STATE; }
  i := lnh_find(n, lnh_get(m, 34, 8), true);
  if i < 0 || n[0].htlcs[i].add != LNH_DONE || n[0].htlcs[i].removal != LNH_KEEP { return LNH_BAD_ID; }
  if removal == LNH_FULFILLED {
    hash: [32] u8; lnh_sha(m[42..<74], hash[..]);
    if !lnh_same(hash[..], n[0].htlcs[i].payment_hash[..]) { return LNH_BAD_PREIMAGE; }
    for k: 0..<32 { n[0].htlcs[i].preimage[k] = m[42 + k]; }
  }
  n[0].htlcs[i].removal = removal; n[0].htlcs[i].removing = LNH_PROPOSED;
  return LNH_OK;
}
```

## Signing and revoking

`commitment_signed` signs the peer's next commitment: every update this
node proposed, and every update of the peer's this node has acked. A
node signs again only after the peer revoked the commitment before, so
at most two of the peer's commitments are ever unrevoked.

<!-- source: coax/lib/lightning/channel.coax commit -->
```jai
// Whether the peer's next commitment would differ from its last: an
// update of this node's not yet signed, or one of the peer's this node
// has acked but not yet signed.
lnh_remote_changes :: (n: [] LnhNode) -> bool {
  for i: 0..<n[0].n_htlcs {
    h := n[0].htlcs[i];
    if (h.mine && h.add == LNH_PROPOSED) || (!h.mine && h.add == LNH_THEIRS_ACKED) { return true; }
    if (!h.mine && h.removing == LNH_PROPOSED) || (h.mine && h.removing == LNH_THEIRS_ACKED) { return true; }
  }
  return false;
}
// The same for this node's next commitment.
lnh_local_changes :: (n: [] LnhNode) -> bool {
  for i: 0..<n[0].n_htlcs {
    h := n[0].htlcs[i];
    if (!h.mine && h.add == LNH_PROPOSED) || (h.mine && h.add == LNH_MINE_ACKED) { return true; }
    if (h.mine && h.removing == LNH_PROPOSED) || (!h.mine && h.removing == LNH_MINE_ACKED) { return true; }
  }
  return false;
}
// Moves every update of one proposer (mine) from stage from to stage to.
lnh_advance :: (n: [] LnhNode, mine: bool, from: int, to: int) {
  for i: 0..<n[0].n_htlcs {
    if n[0].htlcs[i].mine == mine && n[0].htlcs[i].add == from { n[0].htlcs[i].add = to; }
    if n[0].htlcs[i].mine != mine && n[0].htlcs[i].removing == from { n[0].htlcs[i].removing = to; }
  }
}
// Forgets HTLCs whose add and removal are both done, keeping what a
// fulfilled one paid.
lnh_compact :: (n: [] LnhNode) {
  k := 0;
  for i: 0..<n[0].n_htlcs {
    h := n[0].htlcs[i];
    if h.add == LNH_DONE && h.removal != LNH_KEEP && h.removing == LNH_DONE {
      if h.removal == LNH_FULFILLED { n[0].paid_msat += ifx h.mine then h.amount_msat else -h.amount_msat; }
      continue;
    }
    n[0].htlcs[k] = h; k += 1;
  }
  n[0].n_htlcs = k;
}
lnh_write_commit :: (n: [] LnhNode) -> int {
  b := lnh_build(n, false, n[0].remote_number, LNH_NOW);
  sigs: [64 * (LNC_MAX_HTLCS + 1)] u8;
  k := lnh_sign_commitment(n, b.c, b.cm, b.spec, b.point[..], sigs[..]);
  if b.cm.code != LNC_OK || k < 0 { return LNH_BAD_AMOUNT; }
  m: [LNH_MSG] u8;
  at := lnh_put(m[..], 0, LNH_COMMIT, 2);
  at = lnh_copy(m[..], at, n[0].channel_id[..]);
  at = lnh_copy(m[..], at, sigs[0..<64]);
  at = lnh_put(m[..], at, k, 2);
  at = lnh_copy(m[..], at, sigs[64..<64 * (k + 1)]);
  return lnh_queue(n, m[0..<at]);
}
// Signs the peer's next commitment: every update of this node's, and
// every update of the peer's this node has acked.
lnh_commit :: (n: [] LnhNode) -> int {
  if (n[0].state != LNH_NORMAL && n[0].state != LNH_SHUTDOWN_STATE) || n[0].reestablishing { return LNH_WRONG_STATE; }
  if n[0].waiting { return LNH_WAITING; }
  if !lnh_remote_changes(n) { return LNH_NO_CHANGES; }
  if n[0].out_n == LNH_QUEUE { return LNH_FULL; }
  keep := n[0];
  lnh_advance(n, true, LNH_PROPOSED, LNH_MINE_SIGNED);
  lnh_advance(n, false, LNH_THEIRS_ACKED, LNH_THEIRS_SIGNED);
  n[0].remote_number += 1; n[0].waiting = true;
  code := lnh_write_commit(n);
  if code != LNH_OK { n[0] = keep; return code; }
  n[0].seq += 1; n[0].commit_seq = n[0].seq;
  return LNH_OK;
}
```

The receiver checks the signatures of its next commitment, keeps them so
that it can broadcast it, and revokes its previous commitment:
`revoke_and_ack` carries that commitment's per-commitment secret and the
point for the commitment after next.

<!-- source: coax/lib/lightning/channel.coax receive_commit -->
```jai
lnh_write_revoke :: (n: [] LnhNode) -> int {
  m: [LNH_MSG] u8; s: [32] u8;
  lnh_my_secret(n, n[0].local_number - 1, s[..]);
  at := lnh_put(m[..], 0, LNH_REVOKE, 2);
  at = lnh_copy(m[..], at, n[0].channel_id[..]);
  at = lnh_copy(m[..], at, s[..]);
  next := lnh_my_point(n, n[0].local_number + 1);
  at = lnh_copy(m[..], at, next.bytes[..]);
  for i: 0..<32 { s[i] = 0; }
  return lnh_queue(n, m[0..<at]);
}
// The peer signed this node's next commitment. Check every signature,
// keep them, then revoke the previous commitment: its secret, and the
// point for the commitment after next.
lnh_on_commit :: (n: [] LnhNode, m: [] u8) -> int {
  if m.count < 2 + 32 + 64 + 2 { return LNH_TRUNCATED; }
  if !lnh_same(m[2..<34], n[0].channel_id[..]) { return LNH_WRONG_CHANNEL; }
  if n[0].state != LNH_NORMAL && n[0].state != LNH_SHUTDOWN_STATE { return LNH_WRONG_STATE; }
  htlc_n := lnh_get(m, 98, 2);
  if m.count < 100 + 64 * htlc_n { return LNH_TRUNCATED; }
  if !lnh_local_changes(n) { return LNH_NO_CHANGES; }
  if n[0].out_n == LNH_QUEUE { return LNH_FULL; }
  b := lnh_build(n, true, n[0].local_number + 1, LNH_NEXT);
  sigs: [64 * (LNC_MAX_HTLCS + 1)] u8;
  if htlc_n > LNC_MAX_HTLCS { return LNH_HTLC_COUNT; }
  for i: 0..<64 { sigs[i] = m[34 + i]; }
  for i: 0..<64 * htlc_n { sigs[64 + i] = m[100 + i]; }
  code := lnh_check_commitment(n, b.c, b.cm, b.spec, sigs[..], htlc_n);
  if code != LNH_OK { return code; }
  for i: 0..<64 { n[0].local_sig[i] = sigs[i]; }
  for i: 0..<64 * htlc_n { n[0].local_htlc_sigs[i] = sigs[64 + i]; }
  n[0].local_htlc_n = htlc_n;
  n[0].local_number += 1;
  lnh_advance(n, false, LNH_PROPOSED, LNH_THEIRS_ACKED);
  lnh_advance(n, true, LNH_MINE_ACKED, LNH_DONE);
  lnh_compact(n);
  code = lnh_write_revoke(n);
  n[0].seq += 1; n[0].revoke_seq = n[0].seq;
  if code == LNH_OK { code = lnh_maybe_close(n); }
  return code;
}
```

The secret must give the point the node used for that commitment, and it
must fit chapter 107's store, which can then hand back any old secret.
That is what lets chapter 110 take the whole balance of a peer that
broadcasts a revoked commitment.

<!-- source: coax/lib/lightning/channel.coax receive_revoke -->
```jai
// The peer revoked its previous commitment. Its secret must make the
// point this node used for it, and must fit chapter 107's store: keeping
// it is what lets chapter 110 punish a broadcast of that commitment.
lnh_on_revoke :: (n: [] LnhNode, m: [] u8) -> int {
  if m.count < 2 + 32 + 32 + 33 { return LNH_TRUNCATED; }
  if !lnh_same(m[2..<34], n[0].channel_id[..]) { return LNH_WRONG_CHANNEL; }
  if !n[0].waiting || n[0].reestablishing { return LNH_WRONG_STATE; }
  p := lnh_base_point(m[34..<66]);
  if p.code != LNK_OK || !lnh_same(p.bytes[..], n[0].their_point[..]) { return LNH_BAD_SECRET; }
  if !lnh_point_ok(m[66..<99]) { return LNH_BAD_PARAMS; }
  store := n[0].store;
  if shachain_insert(store[..], shachain_index(cast(u64) (n[0].remote_number - 1)), m[34..<66]) != SHACHAIN_OK { return LNH_BAD_SECRET; }
  n[0].store = store;
  n[0].their_point = n[0].their_next_point;
  for i: 0..<33 { n[0].their_next_point[i] = m[66 + i]; }
  n[0].waiting = false;
  lnh_advance(n, true, LNH_MINE_SIGNED, LNH_MINE_ACKED);
  lnh_advance(n, false, LNH_THEIRS_SIGNED, LNH_DONE);
  lnh_compact(n);
  return lnh_maybe_close(n);
}
```

## Closing together

`shutdown` names the script the sender's balance goes to, and from then
on neither side adds an HTLC. A node sends it only when none of its
own updates is still unsigned.

<!-- source: coax/lib/lightning/channel.coax shutdown -->
```jai
// A closing script must be one BOLT 2 allows; here P2WPKH or P2WSH.
lnh_spk_ok :: (s: [] u8) -> bool {
  return (s.count == 22 && s[0] == 0 && s[1] == 20) || (s.count == 34 && s[0] == 0 && s[1] == 32);
}
lnh_pending :: (n: [] LnhNode, mine: bool) -> bool {
  for i: 0..<n[0].n_htlcs {
    h := n[0].htlcs[i];
    if (h.mine == mine && h.add == LNH_PROPOSED) || (h.mine != mine && h.removing == LNH_PROPOSED) { return true; }
  }
  return false;
}
lnh_write_shutdown :: (n: [] LnhNode) -> int {
  m: [LNH_MSG] u8;
  at := lnh_put(m[..], 0, LNH_SHUTDOWN, 2);
  at = lnh_copy(m[..], at, n[0].channel_id[..]);
  at = lnh_put(m[..], at, n[0].cfg.spk_n, 2);
  at = lnh_copy(m[..], at, n[0].cfg.spk[0..<n[0].cfg.spk_n]);
  code := lnh_queue(n, m[0..<at]);
  if code == LNH_OK { n[0].shutdown_sent = true; n[0].state = LNH_SHUTDOWN_STATE; }
  return code;
}
// Starts a mutual close: no new HTLCs either way from here.
lnh_shutdown :: (n: [] LnhNode) -> int {
  if n[0].state != LNH_NORMAL || n[0].shutdown_sent || n[0].reestablishing { return LNH_WRONG_STATE; }
  if lnh_pending(n, true) { return LNH_PENDING; }
  return lnh_write_shutdown(n);
}
lnh_on_shutdown :: (n: [] LnhNode, m: [] u8) -> int {
  if m.count < 2 + 32 + 2 { return LNH_TRUNCATED; }
  if !lnh_same(m[2..<34], n[0].channel_id[..]) { return LNH_WRONG_CHANNEL; }
  if (n[0].state != LNH_NORMAL && n[0].state != LNH_SHUTDOWN_STATE) || n[0].shutdown_received { return LNH_WRONG_STATE; }
  k := lnh_get(m, 34, 2);
  if m.count < 36 + k { return LNH_TRUNCATED; }
  if !lnh_spk_ok(m[36..<36 + k]) { return LNH_BAD_PARAMS; }
  n[0].their_cfg.spk_n = k;
  for i: 0..<k { n[0].their_cfg.spk[i] = m[36 + i]; }
  n[0].shutdown_received = true; n[0].state = LNH_SHUTDOWN_STATE;
  code := LNH_OK;
  if !n[0].shutdown_sent { code = lnh_write_shutdown(n); }
  if code == LNH_OK { code = lnh_maybe_close(n); }
  return code;
}
```

Once both sides sent `shutdown` and no HTLC or unsigned update is left,
the funder offers a fee in `closing_signed`, with its signature of the
closing transaction at that fee. The closing transaction spends the
funding output to the two scripts, in BIP-69 order, with the funder
paying the fee and outputs below the larger dust limit left out. A side
that accepts the fee signs it back and the channel is closed. Otherwise
it counters with a fee strictly between the last two offers, so the
offers always converge.

<!-- source: coax/lib/lightning/channel.coax closing -->
```jai
// The closing transaction for a fee: version 2, the funding outpoint with
// sequence 0xFFFFFFFF, each side's balance to its shutdown script with
// the funder paying the fee, outputs under the larger dust limit left
// out, in BIP-69 order, and locktime 0. Its length.
lnh_closing_unsigned :: (n: [] LnhNode, fee: int, out: [] u8) -> int {
  s := lnh_spec(n, true, LNH_NOW);
  mine := s.to_local_msat / 1000; theirs := s.to_remote_msat / 1000;
  if n[0].funder { mine -= fee; } else { theirs -= fee; }
  dust := ifx n[0].cfg.dust_limit > n[0].their_cfg.dust_limit then n[0].cfg.dust_limit else n[0].their_cfg.dust_limit;
  a := n[0].cfg; b := n[0].their_cfg;
  // Ours first unless theirs sorts before it.
  first_theirs := theirs < mine || (theirs == mine && lnh_less(b.spk[0..<b.spk_n], a.spk[0..<a.spk_n]));
  count := (ifx mine >= dust then 1 else 0) + (ifx theirs >= dust then 1 else 0);
  at := bt_le(out, 0, 2, 4);
  at = bt_var_put(out, at, 1);
  at = bt_put(out, at, n[0].funding_txid[..]); at = bt_le(out, at, n[0].funding_index, 4);
  at = bt_var_put(out, at, 0); at = bt_le(out, at, 0xffffffff, 4);
  at = bt_var_put(out, at, count);
  for k: 0..<2 {
    theirs_now := (k == 0) == first_theirs;
    sat := ifx theirs_now then theirs else mine;
    if sat < dust { continue; }
    c := ifx theirs_now then b else a;
    at = bt_le(out, at, sat, 8);
    at = bt_var_put(out, at, c.spk_n); at = bt_put(out, at, c.spk[0..<c.spk_n]);
  }
  return bt_le(out, at, 0, 4);
}
lnh_less :: (a: [] u8, b: [] u8) -> bool {
  for i: 0..<a.count { if i >= b.count { return false; } if a[i] != b[i] { return a[i] < b[i]; } }
  return a.count < b.count;
}
lnh_closing_sighash :: (n: [] LnhNode, fee: int, out: [] u8) -> bool {
  u: [LNH_CLOSE_TX] u8; ws: [73] u8;
  k := lnh_closing_unsigned(n, fee, u[..]);
  wn := lnc_funding_script_vartime(n[0].my.funding[..], n[0].their.funding[..], ws[..]);
  return bt_sighash_v0(u[0..<k], bt_tx_read(u[0..<k]), 0, ws[0..<wn], n[0].funding_sat, BT_ALL, out);
}
// The signed closing transaction, both signatures in the script's order.
lnh_closing_signed :: (n: [] LnhNode, fee: int, mine: [] u8, theirs: [] u8, out: [] u8) -> int {
  u: [LNH_CLOSE_TX] u8; w: [256] u8; ws: [73] u8; da: [72] u8; db: [72] u8;
  k := lnh_closing_unsigned(n, fee, u[..]);
  wn := lnc_funding_script_vartime(n[0].my.funding[..], n[0].their.funding[..], ws[..]);
  mine_first := lnh_same(n[0].my.funding[..], ws[2..<35]);
  na := ecdsa_der(ifx mine_first then mine else theirs, da[..]);
  nb := ecdsa_der(ifx mine_first then theirs else mine, db[..]);
  at := lnc_item(w[..], 0, w[0..<0], -1);
  at = lnc_item(w[..], at, da[0..<na], BT_ALL); at = lnc_item(w[..], at, db[0..<nb], BT_ALL);
  at = lnc_item(w[..], at, ws[0..<wn], -1);
  return lnc_wrap(u[0..<k], 4, w[0..<at], out);
}
lnh_write_closing :: (n: [] LnhNode, fee: int, sig: [] u8) -> int {
  m: [LNH_MSG] u8;
  at := lnh_put(m[..], 0, LNH_CLOSING_SIGNED, 2);
  at = lnh_copy(m[..], at, n[0].channel_id[..]);
  at = lnh_put(m[..], at, fee, 8); at = lnh_copy(m[..], at, sig);
  return lnh_queue(n, m[0..<at]);
}
lnh_offer :: (n: [] LnhNode, fee: int) -> int {
  digest: [32] u8; sig: [64] u8;
  if !lnh_closing_sighash(n, fee, digest[..]) || !lnh_sign(n[0].funding_secret[..], digest[..], sig[..]) { return LNH_BAD_FEE; }
  code := lnh_write_closing(n, fee, sig[..]);
  if code == LNH_OK { n[0].close_sent = fee; }
  return code;
}
// Negotiation starts once both shutdowns are sent and no HTLC or update
// is left: the funder offers first.
lnh_quiet :: (n: [] LnhNode) -> bool {
  return n[0].shutdown_sent && n[0].shutdown_received && n[0].n_htlcs == 0 && !n[0].waiting;
}
lnh_maybe_close :: (n: [] LnhNode) -> int {
  if !n[0].funder || n[0].close_sent >= 0 || !lnh_quiet(n) { return LNH_OK; }
  return lnh_offer(n, n[0].cfg.close_fee);
}
// The peer's offer. The same fee as this node's last offer closes the
// channel; a fee this node accepts is signed back and closes it; any
// other gets a counter-offer strictly between the two.
lnh_on_closing :: (n: [] LnhNode, m: [] u8) -> int {
  if m.count < 2 + 32 + 8 + 64 { return LNH_TRUNCATED; }
  if !lnh_same(m[2..<34], n[0].channel_id[..]) { return LNH_WRONG_CHANNEL; }
  if n[0].state != LNH_SHUTDOWN_STATE || !lnh_quiet(n) { return LNH_WRONG_STATE; }
  fee := lnh_get(m, 34, 8);
  s := lnh_spec(n, true, LNH_NOW);
  funder_sat := (ifx n[0].funder then s.to_local_msat else s.to_remote_msat) / 1000;
  if fee > funder_sat { return LNH_BAD_FEE; }
  if n[0].close_sent >= 0 && n[0].close_received >= 0 {
    lo := ifx n[0].close_sent < n[0].close_received then n[0].close_sent else n[0].close_received;
    hi := ifx n[0].close_sent < n[0].close_received then n[0].close_received else n[0].close_sent;
    if fee != n[0].close_sent && (fee <= lo || fee >= hi) { return LNH_BAD_FEE; }
  }
  digest: [32] u8;
  if !lnh_closing_sighash(n, fee, digest[..]) || !lnh_verify(digest[..], n[0].their.funding[..], m[42..<106]) {
    return LNH_BAD_SIGNATURE;
  }
  n[0].close_received = fee;
  agree := fee == n[0].close_sent || (fee >= n[0].cfg.close_fee_min && fee <= n[0].cfg.close_fee_max);
  if !agree {
    counter := ifx n[0].close_sent < 0 then n[0].cfg.close_fee else (fee + n[0].close_sent) / 2;
    if counter == fee || (n[0].close_sent >= 0 && counter == n[0].close_sent) { agree = true; }
    else { return lnh_offer(n, counter); }
  }
  mine: [64] u8;
  if !lnh_sign(n[0].funding_secret[..], digest[..], mine[..]) { return LNH_BAD_SIGNATURE; }
  if fee != n[0].close_sent {
    code := lnh_write_closing(n, fee, mine[..]);
    if code != LNH_OK { return code; }
    n[0].close_sent = fee;
  }
  n[0].closing_n = lnh_closing_signed(n, fee, mine[..], m[42..<106], n[0].closing[..]);
  n[0].state = LNH_CLOSED;
  return LNH_OK;
}
```

## Reconnecting

When the connection drops, a node forgets every update no
`commitment_signed` has covered, its own and the peer's: the peer
forgets them too. HTLC ids that were forgotten are used again.

On reconnecting, each side sends `channel_reestablish`: the number of
the next commitment it expects to receive, the number of the next
revocation it expects, the last per-commitment secret it received from
the peer, and its current point. From these the peer works out what was
lost: a `revoke_and_ack`, a `commitment_signed` with the updates it
covered, or both, which it sends again in their first order. If the
peer asks for a revocation this node never sent, and proves it with a
correct secret, this node has lost state. It must not broadcast its own
commitment, which is revoked, and it refuses with `LNH_BEHIND`.

<!-- source: coax/lib/lightning/channel.coax reestablish -->
```jai
// On disconnection a node forgets every update no commitment_signed has
// covered: the peer's, which BOLT 2 says to reverse, and its own, which
// the peer reverses.
lnh_disconnect :: (n: [] LnhNode) {
  if n[0].state < LNH_FUNDING || n[0].state == LNH_CLOSED { return; }
  k := 0;
  for i: 0..<n[0].n_htlcs {
    h := n[0].htlcs[i];
    if h.add == LNH_PROPOSED {
      if h.mine { n[0].next_id -= 1; } else { n[0].their_next_id -= 1; }
      continue;
    }
    if h.removing == LNH_PROPOSED { h.removal = LNH_KEEP; h.removing = LNH_NONE; }
    n[0].htlcs[k] = h; k += 1;
  }
  n[0].n_htlcs = k;
  n[0].out_n = 0;
  n[0].close_sent = -1; n[0].close_received = -1;
  n[0].reestablishing = true;
}
// channel_reestablish: the next commitment number this node expects to
// be signed, the next revocation it expects (the count of secrets it
// holds), the last secret it received, and its current point.
lnh_reestablish :: (n: [] LnhNode) -> int {
  if !n[0].reestablishing { return LNH_WRONG_STATE; }
  m: [LNH_MSG] u8; s: [32] u8;
  next_revocation := n[0].remote_number - ifx n[0].waiting then 1 else 0;
  if next_revocation > 0 { code := shachain_lookup(n[0].store[..], shachain_index(cast(u64) (next_revocation - 1)), s[..]); }
  at := lnh_put(m[..], 0, LNH_REESTABLISH, 2);
  at = lnh_copy(m[..], at, n[0].channel_id[..]);
  at = lnh_put(m[..], at, n[0].local_number + 1, 8);
  at = lnh_put(m[..], at, next_revocation, 8);
  at = lnh_copy(m[..], at, s[..]);
  current := lnh_my_point(n, n[0].local_number);
  at = lnh_copy(m[..], at, current.bytes[..]);
  return lnh_queue(n, m[0..<at]);
}
// What the peer's channel_reestablish asks for, and what it proves.
LnhResync :: struct { code: int; ready: bool; revoke: bool; commit: bool; }
lnh_resync :: (n: [] LnhNode, m: [] u8) -> LnhResync {
  r: LnhResync;
  next_commit := lnh_get(m, 34, 8); next_revocation := lnh_get(m, 42, 8);
  if next_commit == 0 { r.code = LNH_BAD_PARAMS; return r; }
  // The secret must be this node's for next_revocation - 1. If the peer
  // asks for a revocation past the last one this node sent and still
  // shows a correct secret, this node has lost state.
  s: [32] u8;
  if next_revocation > 0 { lnh_my_secret(n, next_revocation - 1, s[..]); }
  if !lnh_same(m[50..<82], s[..]) { r.code = LNH_BAD_SECRET; return r; }
  if next_revocation > n[0].local_number { r.code = LNH_BEHIND; return r; }
  if next_revocation == n[0].local_number - 1 { r.revoke = true; }
  else if next_revocation != n[0].local_number { r.code = LNH_BAD_PARAMS; return r; }
  if n[0].waiting && next_commit == n[0].remote_number { r.commit = true; }
  else if next_commit != n[0].remote_number + 1 { r.code = LNH_BAD_PARAMS; return r; }
  r.ready = next_commit == 1 && n[0].local_number == 0;
  return r;
}
// Sends again what the peer missed: channel_ready, then the
// revoke_and_ack and the commitment_signed (with the updates it covers)
// in the order they first went out.
lnh_on_reestablish :: (n: [] LnhNode, m: [] u8) -> int {
  if m.count < 2 + 32 + 16 + 32 + 33 { return LNH_TRUNCATED; }
  if !lnh_same(m[2..<34], n[0].channel_id[..]) { return LNH_WRONG_CHANNEL; }
  if !n[0].reestablishing { return LNH_WRONG_STATE; }
  r := lnh_resync(n, m);
  if r.code != LNH_OK { return r.code; }
  n[0].reestablishing = false;
  code := LNH_OK;
  if r.ready { code = lnh_write_ready(n); }
  if r.revoke && (!r.commit || n[0].revoke_seq < n[0].commit_seq) && code == LNH_OK { code = lnh_write_revoke(n); }
  if r.commit && code == LNH_OK {
    for i: 0..<n[0].n_htlcs {
      h := n[0].htlcs[i];
      if h.mine && h.add == LNH_MINE_SIGNED && code == LNH_OK { code = lnh_write_add(n, h); }
      if !h.mine && h.removing == LNH_MINE_SIGNED && code == LNH_OK { code = lnh_write_remove(n, h); }
    }
    if code == LNH_OK { code = lnh_write_commit(n); }
  }
  if r.revoke && r.commit && n[0].revoke_seq > n[0].commit_seq && code == LNH_OK { code = lnh_write_revoke(n); }
  if code == LNH_OK { code = lnh_maybe_close(n); }
  return code;
}
```

## Receiving a message

`lnh_receive` sends each message type to its handler. While
reconnecting, only `channel_reestablish` is allowed. An unknown odd
type is ignored and an unknown even one is refused, as BOLT 1 says.

<!-- source: coax/lib/lightning/channel.coax dispatch -->
```jai
// One message from the peer. A refusal changes nothing: the caller sends
// an error and fails the channel, or drops the connection.
lnh_receive :: (n: [] LnhNode, m: [] u8) -> int {
  if m.count < 2 { return LNH_TRUNCATED; }
  type := ln_u16(m, 0);
  if n[0].reestablishing && type != LNH_REESTABLISH { return LNH_WRONG_STATE; }
  if type == LNH_OPEN { return lnh_on_open(n, m); }
  if type == LNH_ACCEPT { return lnh_on_accept(n, m); }
  if type == LNH_FUNDING_CREATED { return lnh_on_funding_created(n, m); }
  if type == LNH_FUNDING_SIGNED { return lnh_on_funding_signed(n, m); }
  if type == LNH_READY { return lnh_on_ready(n, m); }
  if type == LNH_ADD { return lnh_on_add(n, m); }
  if type == LNH_FULFILL { return lnh_on_remove(n, m, LNH_FULFILLED); }
  if type == LNH_FAIL { return lnh_on_remove(n, m, LNH_FAILED); }
  if type == LNH_COMMIT { return lnh_on_commit(n, m); }
  if type == LNH_REVOKE { return lnh_on_revoke(n, m); }
  if type == LNH_SHUTDOWN { return lnh_on_shutdown(n, m); }
  if type == LNH_CLOSING_SIGNED { return lnh_on_closing(n, m); }
  if type == LNH_REESTABLISH { return lnh_on_reestablish(n, m); }
  return ifx type % 2 == 1 then LNH_OK else LNH_UNKNOWN;           // it's OK to be odd
}
// Moves the oldest message in from's outbox to to. The message's type,
// 0 when the outbox is empty; code gets to's answer.
LnhDelivery :: struct { type: int; code: int; n: int; }
lnh_deliver :: (from: [] LnhNode, to: [] LnhNode, buffer: [] u8) -> LnhDelivery {
  k := lnh_take(from, buffer);
  if k == 0 { return LnhDelivery.{}; }
  return LnhDelivery.{type = ln_u16(buffer, 0), code = lnh_receive(to, buffer[0..<k]), n = k};
}
```

The two test nodes, Alice and Bob, and the funding outpoint Alice's
wallet makes:

<!-- source: coax/lib/lightning/channel.coax pair -->
```jai
// Alice opens 1,000,000 satoshis and pushes 200,000,000 msat to Bob.
lnh_config :: (alice: bool) -> LnhConfig {
  c := LnhConfig.{funding_sat = 1000000, push_msat = 200000000, feerate = 2500, dust_limit = 546, reserve_sat = 10000,
                  htlc_minimum_msat = 1000, to_self_delay = ifx alice then 144 else 720, max_accepted = 4,
                  close_fee = ifx alice then 700 else 500, close_fee_min = ifx alice then 600 else 300,
                  close_fee_max = ifx alice then 900 else 600, spk_n = 22};
  c.spk[1] = 20;                                                // P2WPKH: OP_0, a 20-byte push
  for i: 2..<22 { c.spk[i] = ifx alice then 0xaa else 0xbb; }
  return c;
}
lnh_pair :: (a: [] LnhNode, b: [] LnhNode) {
  sa: [32] u8; sb: [32] u8;
  for i: 0..<32 { sa[i] = 0x11; sb[i] = 0x22; }
  lnh_init(a, true, sa[..], lnh_config(true));
  lnh_init(b, false, sb[..], lnh_config(false));
}
// Every message both ways until both outboxes are empty: the first
// refusal, or LNH_OK.
lnh_flush :: (a: [] LnhNode, b: [] LnhNode) -> int {
  buffer: [LNH_MSG] u8;
  while a[0].out_n > 0 || b[0].out_n > 0 {
    if a[0].out_n > 0 { d := lnh_deliver(a, b, buffer[..]); if d.code != LNH_OK { return d.code; } }
    if b[0].out_n > 0 { d := lnh_deliver(b, a, buffer[..]); if d.code != LNH_OK { return d.code; } }
  }
  return LNH_OK;
}
// The funding outpoint Alice's wallet makes; chapter 108 built a whole
// funding transaction.
LNH_FUNDING_TXID :: "8984484a580b825b9972d7adb15050b3ab624ccd731946b3eeddb92f4e7ef6be";
lnh_establish :: (a: [] LnhNode, b: [] LnhNode) -> int {
  txid: [32] u8; t := tx_unhex(LNH_FUNDING_TXID, txid[..]);
  code := lnh_open(a);
  if code == LNH_OK { code = lnh_flush(a, b); }
  if code == LNH_OK { code = lnh_fund(a, txid[..], 0); }
  if code == LNH_OK { code = lnh_flush(a, b); }
  if code == LNH_OK { code = lnh_confirmed(a); }
  if code == LNH_OK { code = lnh_confirmed(b); }
  if code == LNH_OK { code = lnh_flush(a, b); }
  return code;
}
lnh_preimage :: (k: int, preimage: [] u8, hash: [] u8) {
  for i: 0..<32 { preimage[i] = cast(u8) (k + i); }
  lnh_sha(preimage, hash);
}
```

## The example

The example,
[bitcoin/examples/lightning-channel.coax](../examples/lightning-channel.coax),
runs Alice and Bob through a channel's life. It prints each message
with its length and the start of its SHA-256, and each side's
commitment after each round.

<!-- source: bitcoin/examples/lightning-channel.coax example -->
```jai
#load "../../coax/lib/lightning/channel.coax";
#load "../../coax/lib/nostr/hex.coax";
#import "Basic";

// Bytes as lowercase hex, and a txid in its usual reversed order.
show :: (b: [] u8) {
  text: [2048] u8;
  r := hex_encode_checked(cast(string) b, text[..], false);
  print("%", cast(string) text[0..<r.written]);
}
show_txid :: (h: [] u8) { r: [32] u8; for i: 0..<32 { r[i] = h[31 - i]; } show(r[..]); }
// Chapter 36's interpreter with the standard flags: input 0 of tx spends
// amount satoshis locked by spk.
FLAGS :: SCRIPT_VERIFY_P2SH | SCRIPT_VERIFY_STRICTENC | SCRIPT_VERIFY_DERSIG | SCRIPT_VERIFY_LOW_S
  | SCRIPT_VERIFY_NULLDUMMY | SCRIPT_VERIFY_MINIMALDATA | SCRIPT_VERIFY_CLEANSTACK | SCRIPT_VERIFY_CHECKLOCKTIMEVERIFY
  | SCRIPT_VERIFY_CHECKSEQUENCEVERIFY | SCRIPT_VERIFY_WITNESS | SCRIPT_VERIFY_MINIMALIF | SCRIPT_VERIFY_NULLFAIL
  | SCRIPT_VERIFY_WITNESS_PUBKEYTYPE;
runs :: (tx: [] u8, spk: [] u8, amount: int) -> string {
  ins: [1] TxIn; outs: [LNC_MAX_OUTPUTS] TxOut; items: [8] TxItem; w: [1] ScriptWork;
  t := tx_read_vartime(tx, true, ins[..], outs[..], items[..]);
  if t.code != TX_OK { return "unreadable"; }
  sp := ScriptSpend.{b=tx, t=t, ins=ins[0..<t.nin], outs=outs[0..<t.nout], items=items[0..<t.nitems], index=0, amount=amount, spk=spk};
  return SCRIPT_ERR_NAMES[script_verify_vartime(sp, FLAGS, w[..])];
}
CODES :: [20] string.["OK", "UNKNOWN", "TRUNCATED", "WRONG_STATE", "WRONG_CHANNEL", "BAD_PARAMS", "BAD_SIGNATURE",
                      "BAD_SECRET", "BAD_ID", "BAD_PREIMAGE", "BAD_AMOUNT", "TOO_MANY", "NO_CHANGES", "HTLC_COUNT",
                      "SHUTTING", "BAD_FEE", "WAITING", "PENDING", "BEHIND", "FULL"];
message_name :: (type: int) -> string {
  if type == LNH_OPEN { return "open_channel"; }
  if type == LNH_ACCEPT { return "accept_channel"; }
  if type == LNH_FUNDING_CREATED { return "funding_created"; }
  if type == LNH_FUNDING_SIGNED { return "funding_signed"; }
  if type == LNH_READY { return "channel_ready"; }
  if type == LNH_SHUTDOWN { return "shutdown"; }
  if type == LNH_CLOSING_SIGNED { return "closing_signed"; }
  if type == LNH_ADD { return "update_add_htlc"; }
  if type == LNH_FULFILL { return "update_fulfill_htlc"; }
  if type == LNH_FAIL { return "update_fail_htlc"; }
  if type == LNH_COMMIT { return "commitment_signed"; }
  if type == LNH_REVOKE { return "revoke_and_ack"; }
  if type == LNH_REESTABLISH { return "channel_reestablish"; }
  return "unknown";
}
// One message from one node to the other: its type, length and the start
// of its SHA-256, and the receiver's answer if it refuses.
step :: (from: [] LnhNode, to: [] LnhNode, names: string) {
  m: [LNH_MSG] u8; h: [32] u8;
  d := lnh_deliver(from, to, m[..]);
  lnh_sha(m[0..<d.n], h[..]);
  print("  % %, % bytes, sha256 ", names, message_name(d.type), d.n); show(h[0..<4]);
  if d.code != LNH_OK { print(": refused, %", CODES[d.code]); }
  print("\n");
}
// Every message both ways, Alice's first.
pump :: (a: [] LnhNode, b: [] LnhNode) {
  while a[0].out_n > 0 || b[0].out_n > 0 {
    while a[0].out_n > 0 { step(a, b, "Alice -> Bob:"); }
    while b[0].out_n > 0 { step(b, a, "Bob -> Alice:"); }
  }
}
// A node's latest commitment: its txid and outputs.
commitment :: (n: [] LnhNode, name: string) {
  b := lnh_build(n, true, n[0].local_number, LNH_NOW);
  u: [LNC_TX] u8; txid: [32] u8;
  k := lnc_unsigned_vartime(b.c, b.cm, u[..]); lnc_txid_vartime(u[0..<k], txid[..]);
  print("  %'s commitment %: ", name, n[0].local_number); show_txid(txid[..]); print("\n   ");
  for i: 0..<b.cm.n {
    o := b.cm.out[i];
    print(" % %", LNC_KINDS[o.kind], o.sat);
    if o.kind == LNC_OFFERED || o.kind == LNC_RECEIVED { print(" (expiry %)", o.expiry); }
  }
  print(", fee %\n", b.cm.fee);
}
both :: (a: [] LnhNode, b: [] LnhNode) { commitment(a, "Alice"); commitment(b, "Bob"); }
refused :: (what: string, code: int) { print("  %: %\n", what, CODES[code]); }

main :: () {
  a: [1] LnhNode; b: [1] LnhNode; txid: [32] u8; m: [LNH_MSG] u8;
  p0: [32] u8; h0: [32] u8; p1: [32] u8; h1: [32] u8; p2: [32] u8; h2: [32] u8;
  lnh_preimage(1, p0[..], h0[..]); lnh_preimage(2, p1[..], h1[..]); lnh_preimage(3, p2[..], h2[..]);
  lnh_pair(a[..], b[..]);

  print("Open: Alice funds 1,000,000 satoshis and pushes 200,000 to Bob\n");
  #assert lnh_open(a[..]) == LNH_OK;
  pump(a[..], b[..]);
  t := tx_unhex(LNH_FUNDING_TXID, txid[..]);
  print("  Alice's wallet funds "); show_txid(txid[..]); print(":0\n");
  #assert lnh_fund(a[..], txid[..], 0) == LNH_OK;
  pump(a[..], b[..]);
  print("  channel_id "); show(a[0].channel_id[..]); print("\n");
  print("  the funding transaction is 3 blocks deep\n");
  #assert lnh_confirmed(a[..]) == LNH_OK && lnh_confirmed(b[..]) == LNH_OK;
  pump(a[..], b[..]);
  print("  state: Alice %, Bob %\n", LNH_STATES[a[0].state], LNH_STATES[b[0].state]);
  both(a[..], b[..]);

  print("Alice offers two HTLCs: 50,000 and 20,000 satoshis\n");
  #assert lnh_add(a[..], 50000000, h0[..], 500) == LNH_OK && lnh_add(a[..], 20000000, h1[..], 510) == LNH_OK;
  #assert lnh_commit(a[..]) == LNH_OK;
  pump(a[..], b[..]);
  #assert lnh_commit(b[..]) == LNH_OK;
  pump(a[..], b[..]);
  both(a[..], b[..]);

  print("Bob fulfills the first and fails the second\n");
  #assert lnh_fulfill(b[..], 0, p0[..]) == LNH_OK && lnh_fail(b[..], 1) == LNH_OK;
  #assert lnh_commit(b[..]) == LNH_OK;
  pump(a[..], b[..]);
  #assert lnh_commit(a[..]) == LNH_OK;
  pump(a[..], b[..]);
  both(a[..], b[..]);

  print("Bob offers 30,000 satoshis; his commitment_signed is lost\n");
  #assert lnh_add(b[..], 30000000, h2[..], 600) == LNH_OK && lnh_commit(b[..]) == LNH_OK;
  step(b[..], a[..], "Bob -> Alice:");
  k := lnh_take(b[..], m[..]);
  print("  lost: %\n", message_name(ln_u16(m[..], 0)));
  lnh_disconnect(a[..]); lnh_disconnect(b[..]);
  print("  reconnected: Alice forgot the add; HTLCs held: Alice %, Bob %\n", a[0].n_htlcs, b[0].n_htlcs);
  #assert lnh_reestablish(a[..]) == LNH_OK && lnh_reestablish(b[..]) == LNH_OK;
  pump(a[..], b[..]);
  #assert lnh_commit(a[..]) == LNH_OK;
  pump(a[..], b[..]);
  print("Alice fulfills it\n");
  #assert lnh_fulfill(a[..], 0, p2[..]) == LNH_OK && lnh_commit(a[..]) == LNH_OK;
  pump(a[..], b[..]);
  #assert lnh_commit(b[..]) == LNH_OK;
  pump(a[..], b[..]);
  both(a[..], b[..]);

  print("Messages and commands out of order\n");
  x: [1] LnhNode; y: [1] LnhNode; x[0] = a[0]; y[0] = b[0];
  #assert lnh_add(x[..], 10000000, h0[..], 700) == LNH_OK;
  k = lnh_take(x[..], m[..]); m[41] = 9;
  refused("update_add_htlc with id 9, not 2", lnh_receive(y[..], m[0..<k]));
  m[41] = 2;
  refused("Alice signs twice", ifx lnh_receive(y[..], m[0..<k]) == LNH_OK && lnh_commit(x[..]) == LNH_OK then lnh_commit(x[..]) else -1);
  refused("Alice signs with nothing to sign", lnh_commit(a[..]));
  refused("Bob fulfills an HTLC both have not yet signed", lnh_fulfill(y[..], 1, p1[..]));
  refused("Alice offers 775,000 satoshis, into her reserve", lnh_add(a[..], 775000000, h0[..], 700));
  for i: 0..<k { m[i] = 0; }
  m[1] = LNH_REVOKE; for i: 0..<32 { m[2 + i] = a[0].channel_id[i]; }
  q := lnh_my_point(b[..], 9); for i: 0..<33 { m[66 + i] = q.bytes[i]; }
  refused("revoke_and_ack with no commitment outstanding", lnh_receive(a[..], m[0..<99]));
  m[1] = LNH_CLOSING_SIGNED;
  refused("closing_signed before shutdown", lnh_receive(a[..], m[0..<106]));
  m[1] = LNH_OPEN;
  refused("open_channel on an open channel", lnh_receive(b[..], m[0..<321]));

  print("Bob could close alone with his latest commitment\n");
  c := lnh_build(b[..], true, b[0].local_number, LNH_NOW);
  d: [32] u8; mine: [LNC_DER] u8; theirs: [LNC_DER] u8; tx: [LNC_TX] u8; ws: [73] u8; fund: [34] u8;
  #assert lnc_sighash_vartime(c.c, c.cm, d[..]);
  mn := lnc_sign(b[0].funding_secret[..], d[..], mine[..]);
  tn := ecdsa_der(b[0].local_sig[..], theirs[..]);
  n := lnc_signed_vartime(c.c, c.cm, mine[0..<mn], theirs[0..<tn], tx[..]);
  wn := lnc_funding_script_vartime(c.c.local_funding[..], c.c.remote_funding[..], ws[..]);
  fn := p2wsh_script_vartime(ws[0..<wn], fund[..]);
  print("  it spends the funding output: %, % bytes\n", runs(tx[0..<n], fund[0..<fn], c.c.funding_sat), n);

  print("Close: Alice wants a 700-satoshi fee, Bob 300 to 600\n");
  #assert lnh_shutdown(a[..]) == LNH_OK;
  pump(a[..], b[..]);
  print("  closing fee %, state: Alice %, Bob %\n", a[0].close_sent, LNH_STATES[a[0].state], LNH_STATES[b[0].state]);
  #assert lnh_same(a[0].closing[0..<a[0].closing_n], b[0].closing[0..<b[0].closing_n]);
  print("  closing transaction "); show(a[0].closing[0..<a[0].closing_n]); print("\n");
  print("  it spends the funding output: %\n", runs(a[0].closing[0..<a[0].closing_n], fund[0..<fn], c.c.funding_sat));
}
```

The output:

```text
Open: Alice funds 1,000,000 satoshis and pushes 200,000 to Bob
  Alice -> Bob: open_channel, 325 bytes, sha256 7acc816c
  Bob -> Alice: accept_channel, 276 bytes, sha256 f03850b5
  Alice's wallet funds bef67e4e2fb9ddeeb3461973cd4c62abb35050b1add772995b820b584a488489:0
  Alice -> Bob: funding_created, 132 bytes, sha256 424bcee1
  Bob -> Alice: funding_signed, 98 bytes, sha256 354824d9
  channel_id 8984484a580b825b9972d7adb15050b3ab624ccd731946b3eeddb92f4e7ef6be
  the funding transaction is 3 blocks deep
  Alice -> Bob: channel_ready, 67 bytes, sha256 1e6833ff
  Bob -> Alice: channel_ready, 67 bytes, sha256 06811499
  state: Alice normal, Bob normal
  Alice's commitment 0: 33b9376d6af5312c0b0390e47331a73bd705741cc0ed776108a28415350088b4
    to_remote 200000 to_local 798190, fee 1810
  Bob's commitment 0: 70264411b144fbb1b513f70803cf2226153c0ea541f9554af298849fac1f088a
    to_local 200000 to_remote 798190, fee 1810
Alice offers two HTLCs: 50,000 and 20,000 satoshis
  Alice -> Bob: update_add_htlc, 1452 bytes, sha256 7dea08cb
  Alice -> Bob: update_add_htlc, 1452 bytes, sha256 2be2bfac
  Alice -> Bob: commitment_signed, 228 bytes, sha256 56ef7cfc
  Bob -> Alice: revoke_and_ack, 99 bytes, sha256 8c421e99
  Bob -> Alice: commitment_signed, 228 bytes, sha256 27572f4a
  Alice -> Bob: revoke_and_ack, 99 bytes, sha256 f9b96998
  Alice's commitment 1: 10ffae2e0894f5558b0058197bf1fc72b411c2ebfb90d4a65df633288e10ca18
    offered 20000 (expiry 510) offered 50000 (expiry 500) to_remote 200000 to_local 727330, fee 2670
  Bob's commitment 1: 054fa46f1f27e352a40602bedd13a44faf41f387f2794524676e8cad64562c31
    received 20000 (expiry 510) received 50000 (expiry 500) to_local 200000 to_remote 727330, fee 2670
Bob fulfills the first and fails the second
  Bob -> Alice: update_fulfill_htlc, 74 bytes, sha256 9b1f88bb
  Bob -> Alice: update_fail_htlc, 44 bytes, sha256 a675d799
  Bob -> Alice: commitment_signed, 100 bytes, sha256 7fb37c62
  Alice -> Bob: revoke_and_ack, 99 bytes, sha256 fc075b32
  Alice -> Bob: commitment_signed, 100 bytes, sha256 bba99bce
  Bob -> Alice: revoke_and_ack, 99 bytes, sha256 bdaa1f57
  Alice's commitment 2: fea492cdef40c353fb89aa9256ee883f95d3383608d217559ca47214edea49aa
    to_remote 250000 to_local 748190, fee 1810
  Bob's commitment 2: 1845ccdd5ca22d9b7299d6cda252a9fd18e16af056efe1563553d24fb2437c7c
    to_local 250000 to_remote 748190, fee 1810
Bob offers 30,000 satoshis; his commitment_signed is lost
  Bob -> Alice: update_add_htlc, 1452 bytes, sha256 bff94c38
  lost: commitment_signed
  reconnected: Alice forgot the add; HTLCs held: Alice 0, Bob 1
  Alice -> Bob: channel_reestablish, 115 bytes, sha256 517a7719
  Bob -> Alice: channel_reestablish, 115 bytes, sha256 cfc93ce4
  Bob -> Alice: update_add_htlc, 1452 bytes, sha256 bff94c38
  Bob -> Alice: commitment_signed, 164 bytes, sha256 d9a7cdbc
  Alice -> Bob: revoke_and_ack, 99 bytes, sha256 2f206c0f
  Alice -> Bob: commitment_signed, 164 bytes, sha256 e51faebf
  Bob -> Alice: revoke_and_ack, 99 bytes, sha256 74ce7e18
Alice fulfills it
  Alice -> Bob: update_fulfill_htlc, 74 bytes, sha256 805100e4
  Alice -> Bob: commitment_signed, 100 bytes, sha256 80815bf2
  Bob -> Alice: revoke_and_ack, 99 bytes, sha256 5b796fc5
  Bob -> Alice: commitment_signed, 100 bytes, sha256 b312e5ca
  Alice -> Bob: revoke_and_ack, 99 bytes, sha256 a0884783
  Alice's commitment 4: 693a70e5e694de25e299d729742b087edb6fbc09c30cfcb85b07b8437854a48d
    to_remote 220000 to_local 778190, fee 1810
  Bob's commitment 4: 043d6749c521b0455c62a3e4e39fae27c6faa356ea05ea5425255839e3ac2636
    to_local 220000 to_remote 778190, fee 1810
Messages and commands out of order
  update_add_htlc with id 9, not 2: BAD_ID
  Alice signs twice: WAITING
  Alice signs with nothing to sign: NO_CHANGES
  Bob fulfills an HTLC both have not yet signed: BAD_ID
  Alice offers 775,000 satoshis, into her reserve: BAD_AMOUNT
  revoke_and_ack with no commitment outstanding: WRONG_STATE
  closing_signed before shutdown: WRONG_STATE
  open_channel on an open channel: WRONG_STATE
Bob could close alone with his latest commitment
  it spends the funding output: OK, 347 bytes
Close: Alice wants a 700-satoshi fee, Bob 300 to 600
  Alice -> Bob: shutdown, 58 bytes, sha256 172a9570
  Bob -> Alice: shutdown, 58 bytes, sha256 c18262e4
  Alice -> Bob: closing_signed, 106 bytes, sha256 8bf23739
  Bob -> Alice: closing_signed, 106 bytes, sha256 58dcd132
  Alice -> Bob: closing_signed, 106 bytes, sha256 c4542295
  Bob -> Alice: closing_signed, 106 bytes, sha256 377a1f17
  closing fee 600, state: Alice closed, Bob closed
  closing transaction 020000000001018984484a580b825b9972d7adb15050b3ab624ccd731946b3eeddb92f4e7ef6be0000000000ffffffff02605b030000000000160014bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb88e40b0000000000160014aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa0400483045022100d4eba6a09044945f613f1c5b6a648f4ce9e09afc10121d7bdc7e379bd6cf45d802207b257633fc512d5028e3feb7ed6f9768cfa604794e8f2657212efc8a95a07e230147304402207aca9930733b910b73e4ab0fe6221b30f28a206c1484285ebd55fea5d70c67c402201a25fdd909b41ccaa688329f22206272ed12ac554e4666fcf3f7e87074435aa20147522102380d1bce745fca652a82bf961ff104200d940461f65c2500fe60f9634c8518b121032c21c626ef3912f30e2ee43b8c0002d2ccc7e431a676033a0e940e731cafcdf752ae00000000
  it spends the funding output: OK
```

What the output shows:

- Alice opens a 1,000,000-satoshi channel and pushes 200,000 satoshis to
  Bob. The channel id is the funding txid in hash order; the txid is
  printed the usual reversed way.
- The two commitments of each state mirror each other: Alice's
  `to_local` is Bob's `to_remote`, and Alice's offered HTLCs are Bob's
  received ones. Alice, the funder, pays the fee: 1,810 satoshis with no
  HTLC outputs, 2,670 with two (172 weight units more each at 2,500
  satoshis per 1,000).
- After Bob fulfills the 50,000-satoshi HTLC and fails the 20,000 one,
  Bob has 250,000 satoshis.
- Bob's `commitment_signed` is lost. Alice forgets Bob's add, which no
  signature covered for her; Bob keeps it, since he signed it. After
  `channel_reestablish`, Bob sends the same add, byte for byte, and the
  signature again.
- Every message out of order is refused with its reason, and the
  refused messages change nothing.
- Bob's latest commitment, signed by both, spends the funding output
  under chapter 36's interpreter, so he could close alone at any time.
- Alice wants a 700-satoshi fee and accepts 600 to 900; Bob wants 500 and
  accepts 300 to 600. Bob counters 500, Alice offers 600, the midpoint,
  and Bob accepts it. The closing transaction pays Bob 220,000 satoshis
  and Alice 780,000 less the fee, and it spends the funding output.

The book's checks run this program in debug and release builds and
compare the output with
[bitcoin/fixtures/lightning-channel.out](../fixtures/lightning-channel.out).

## How the code is checked

### Checks in the example

The checks are in
[bitcoin/examples/checks/lightning-channel.coax](../examples/checks/lightning-channel.coax),
which loads the example:

- either side's HTLC, fulfilled or failed, leaves both nodes with the
  same balances, and each commitment's outputs and fee add up to the
  funding amount
- a `commitment_signed` with one byte of its signature changed is
  refused and changes nothing; the real one is then accepted

<!-- source: bitcoin/examples/checks/lightning-channel.coax check -->
```jai
// Either side offers an HTLC, both sides sign it, and the receiver
// fulfills or fails it. Afterwards both nodes agree on the balances, the
// amount moved only when fulfilled, and each commitment's outputs and
// fee spend the whole funding output. Each run is its own check: one
// check may visit only so many values at compile time.
payment :: (k: int) {
  a: [1] LnhNode; b: [1] LnhNode; p: [32] u8; h: [32] u8;
  lnh_open_pair(a[..], b[..]);
  lnh_preimage(10 + k, p[..], h[..]);
  amount := (k + 1) * 15000000;
  alice_offers := k % 2 == 0; fulfilled := k < 2;
  x := ifx alice_offers then a[..] else b[..]; y := ifx alice_offers then b[..] else a[..];
  #assert lnh_add(x, amount, h[..], 500 + k) == LNH_OK && lnh_dance(x, y) == LNH_OK;
  #assert a[0].n_htlcs == 1 && b[0].n_htlcs == 1;
  code := ifx fulfilled then lnh_fulfill(y, 0, p[..]) else lnh_fail(y, 0);
  #assert code == LNH_OK && lnh_dance(y, x) == LNH_OK;
  #assert a[0].n_htlcs == 0 && b[0].n_htlcs == 0;
  moved := ifx fulfilled then amount else 0;
  alice := 800000000 - (ifx alice_offers then moved else -moved);
  #assert lnh_agree(a[..], b[..], alice, 1000000000 - alice);
  c := lnh_build(a[..], true, a[0].local_number, LNH_NOW);
  #assert c.cm.n == 2 && c.cm.out[0].sat + c.cm.out[1].sat + c.cm.fee == 1000000;
}
#test "Alice pays Bob" #steps 8000000000 { payment(0); }
#test "Bob pays Alice" #steps 8000000000 { payment(1); }
#test "Alice's HTLC fails" #steps 8000000000 { payment(2); }
#test "Bob's HTLC fails" #steps 8000000000 { payment(3); }
// A commitment_signed with one byte of its signature changed is refused
// and changes nothing: the real one is accepted afterwards.
forged :: (k: int) {
  a: [1] LnhNode; b: [1] LnhNode; p: [32] u8; h: [32] u8; m: [LNH_MSG] u8;
  lnh_open_pair(a[..], b[..]);
  lnh_preimage(20 + k, p[..], h[..]);
  #assert lnh_add(a[..], 25000000, h[..], 500) == LNH_OK && lnh_commit(a[..]) == LNH_OK;
  #assert lnh_deliver(a[..], b[..], m[..]).code == LNH_OK;      // the add
  n := lnh_take(a[..], m[..]);
  m[34 + 17 * k] ^= 1;
  #assert lnh_receive(b[..], m[0..<n]) == LNH_BAD_SIGNATURE && b[0].local_number == 0 && b[0].out_n == 0;
  m[34 + 17 * k] ^= 1;
  #assert lnh_receive(b[..], m[0..<n]) == LNH_OK && b[0].local_number == 1;
  #assert lnh_flush(a[..], b[..]) == LNH_OK && !a[0].waiting;
}
#test "a forged commitment signature is refused: r" #steps 8000000000 { forged(0); }
#test "a forged commitment signature is refused: s" #steps 8000000000 { forged(3); }
```

```shell
coax/bin/coax test bitcoin/examples/checks/lightning-channel.coax
```

`coax test` runs 531 checks, including the loaded files' own checks, in
about five minutes on 2026-11-03.

### The library's own checks

`channel.coax` is an `#authority` file. Its checks compare the faster
key derivations and verification with chapters 106 and 21; open a
channel and compare both nodes' views of it; add, sign, fulfill and fail
HTLCs both ways; lose a `commitment_signed` and a `revoke_and_ack` and
reconnect; catch a node that lost state and a wrong secret; refuse
messages out of order at every stage; stop new HTLCs after `shutdown`;
and negotiate a closing fee.

```shell
coax/bin/coax test coax/lib/lightning/channel.coax
```

`coax test` runs 523 checks, including those of the files it loads, in
about four and a half minutes on 2026-11-03. Mutation testing: not run
(owner speed order).

### An independent model

[channel.py](../../coax/tests/lightning/channel.py) computes every line
the example prints with two Python nodes. It follows BOLT 2's rules
with a different design: each node keeps lists of changes (its own
proposed, signed and acked; the peer's proposed, acked and signed), and
each new commitment is the previous one with those changes applied. Its
messages are written from BOLT 2's field lists. Commitments and
signatures come from chapter 108's Python builder,
[bolt3_tx.py](../../coax/tests/lightning/bolt3_tx.py), and the two
transactions run under chapter 36's Python transliteration of Bitcoin
Core's interpreter. None of it shares code with Coax.

```shell
python3 -I coax/tests/lightning/channel.py --coax coax/bin/coax
```

On 2026-11-03 all 79 lines agree, and the example's output in native debug
and release builds equals them. Wasm was not run.

## What this chapter leaves out

- The network: the nodes exchange messages in memory, not over chapter
  105's encrypted connection.
- Watching the chain, closing alone, and penalties: chapter 110.
- Channel announcements and gossip: BOLT 7.
- `option_anchors`, `update_fee`, `update_fail_malformed_htlc`, the
  dual-funded `open_channel2`, splicing, quiescence, and the newer
  `closing_complete` negotiation.
- Routing: forwarding an HTLC to the next channel, and building and
  reading the onion and error packets (chapter 111).
- Timeouts: failing the channel when an HTLC nears its expiry.

## Summary

- The opener broadcasts the funding transaction only after it holds the
  accepter's signature of its first commitment.
- Each update is proposed, then signed into the peer's commitment, then
  revoked out of the old one, on both sides, before it is irrevocably
  committed.
- `commitment_signed` carries the commitment's signature and one per
  HTLC transaction; `revoke_and_ack` hands over the old commitment's
  secret and the next point.
- On reconnecting, uncovered updates are forgotten and
  `channel_reestablish` tells each side what to send again, or that it
  lost state.
- A mutual close negotiates the fee in `closing_signed` until both sign
  the same closing transaction.

## Dependencies

- [104. Messages and TLVs](ch104-messages-and-tlvs.md)
- [106. Commitment keys and revocation](ch106-commitment-keys-and-revocation.md)
- [107. Shachain](ch107-shachain.md)
- [108. Commitment and HTLC transactions](ch108-commitment-and-htlc-transactions.md)

## Sources

- [BOLT 2](https://github.com/lightning/bolts/blob/master/02-peer-protocol.md),
  "Channel Establishment v1", "Channel Close", "Normal Operation" and
  "Message Retransmission", at lightning/bolts revision `19146282`,
  pinned in
  [coax/tests/vectors/upstream/bolts/](../../coax/tests/vectors/upstream/bolts/02-peer-protocol.md).
- [BOLT 3](https://github.com/lightning/bolts/blob/master/03-transactions.md),
  "Closing Transaction", at the same revision.
- All pinned files are listed in [SOURCES.md](../../coax/tests/vectors/SOURCES.md).
