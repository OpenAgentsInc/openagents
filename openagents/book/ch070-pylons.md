# 70. Pylons

> **Status: partly implemented.** The `#authority` external Pylon onboarding gate
> ([`psionic/lib/pylon/onboard.coax`](../../psionic/lib/pylon/onboard.coax)),
> `#authority` 3-leg settlement ledger (`Worker + Validator + Module Author` in
> [`psionic/lib/peer/pay.coax`](../../psionic/lib/peer/pay.coax) and
> [`pay/executor-settlement.coax`](../../pay/executor-settlement.coax)),
> live relay rehearsal (`openagents pylon verify-relay` in
> [`apps/openagents-cli/pylon-onboard.coax`](../../apps/openagents-cli/pylon-onboard.coax)),
> and live 3-leg settlement runner (`apps/openagents-cli/pylon-settle.coax`) run
> today and are grounded on `cpu` by
> [`openagents/examples/pylon-onboard-settle.coax`](../examples/pylon-onboard-settle.coax)
> and [`openagents/examples/pylon-onboard-settle.out`](../examples/pylon-onboard-settle.out).
> Remaining: public external-operator fleet scaling and open market bidding.

## Premise

Our own hardware is small. This chapter adds everyone else's: a Pylon is a
program you run on your computer that serves models to the network and pays
you in Bitcoin.

## Draft

The first AI product from this team was a network for selling spare compute.
[Episode 290](../../docs/transcripts/290.md) tells the story: you ran a
program on a gaming computer, it served an open model, and you were paid in
Bitcoin. At its height about 300 people ran it at once. It worked, and it
failed for one reason. Nobody was buying. The inference was not the best, the
cheapest, the fastest or the most private.

[Episode 174](../../docs/transcripts/174.md) names that failure and proposes
agents as the buyers. [Episodes 203](../../docs/transcripts/203.md),
[214](../../docs/transcripts/214.md) and [221](../../docs/transcripts/221.md)
rebuild the network under the name Pylon. Episode 290 relaunches it and says
who it is for: people with strong computers at home that sit idle most of the
day, who would be both sellers and buyers. Gamers.

Demand is different this time because the buyer is in this book—and because
the **OpenAgents Executor Network** (`#13912`–`#13928`) turns consumer Macs
and NVIDIA GPUs into a verifiable execution, validation, and decentralized
training mesh rather than an unverified token relay:

- **Three `NIP-PYLON` lanes:** `cj-executor` (runs digest-pinned `exec-vm-v1` /
  `wasm-exec-v1` programs after passing a local self-test), `cj-validate`
  (a second distinct physical device independently replays traces via
  [Psionic ch234](../../psionic/book/ch234-exact-replay-as-a-verification-class.md)
  and publishes signed NIP-32 kind `1985` verdicts), and `cj-training`
  (runs bounded Local-SGD / DiLoCo routing-adapter windows whose updates pass
  a 6-tier Byzantine verifier and the Quarantine Optimizer in
  [Psionic ch272–274](../../psionic/book/ch272-bounded-training-windows-and-diloco.md)).
- **Live Bitcoin mainnet two-leg settlement:** Verified across an Apple M5 Max
  worker and an NVIDIA RTX 4080 Linux validator over a live Coax Nostr relay,
  automatically paying both legs on Bitcoin mainnet via Breez Spark (`200 sats`
  worker + `50 sats` validator, `0` sat fee, zero double-spend on restart),
  gated by `#authority` rolling 24h UTC caps and reconciled into signed kind
  `30201` pool aggregates ([W4.3 mainnet settlement record](../../docs/verification/executor-w43-mainnet-settlement.md),
  [Bitcoin ch123](../../bitcoin/book/ch123-paying-for-requests.md),
  [Psionic ch238](../../psionic/book/ch238-pay-for-verified-work.md)).
- **Ambient decision demand:** The front makes decisions on every turn, coding
  agents invoke exact weight modules, and every guard and shopkeeper in the
  OpenAgents Metaverse makes decisions too (chapter 74).

Episode 290 calls this having ambient compute as a primitive. The Executor
Network capstone ([executor-network-flywheel.md](../../docs/books/capstones/executor-network-flywheel.md))
proves the end-to-end loop on our own hardware; scaling to independent outside
operators is what 1.0 tracks.

## What we build

A plan for what this chapter ships once the draft is approved:

- A Pylon you install with one command (`openagents pylon`), which passes a
  digest self-test and advertises decision, `cj-executor`, `cj-validate`, and
  `cj-training` lanes.
- Routing through the gateway and `NIP-PYLON` relays with distinct-device
  exact replay verification and Byzantine-safe training quarantine.
- Receipt-first two-leg Breez Spark / Lightning payouts under rolling daily
  caps, with a reconciled public kind `30201` settled feed (`paid_msat.bitcoin`).

It builds on what exists today:

- [`psionic/lib/pylon`](../../psionic/lib/pylon)
- [`psionic/lib/mesh`](../../psionic/lib/mesh)
- [`psionic/lib/peer/pay.coax`](../../psionic/lib/peer/pay.coax), [`psionic/lib/peer/settled-feed.coax`](../../psionic/lib/peer/settled-feed.coax), and [`pay/executor-settlement.coax`](../../pay/executor-settlement.coax)
- [`psionic/lib/peer/training-window.coax`](../../psionic/lib/peer/training-window.coax), [`psionic/lib/peer/train-verify.coax`](../../psionic/lib/peer/train-verify.coax), and [`psionic/lib/peer/quarantine-opt.coax`](../../psionic/lib/peer/quarantine-opt.coax)
- [`coax/lib/gateway/pylon.coax`](../../coax/lib/gateway/pylon.coax)
- [`apps/openagents-cli/pylon.coax`](../../apps/openagents-cli/pylon.coax), [`apps/openagents-cli/pylon-onboard.coax`](../../apps/openagents-cli/pylon-onboard.coax), and [`apps/openagents-cli/pylon-settle.coax`](../../apps/openagents-cli/pylon-settle.coax)
- [`docs/connect-a-pylon.md`](../../docs/connect-a-pylon.md) and [`docs/pylon/external-operator-quickstart.md`](../../docs/pylon/external-operator-quickstart.md)
- [`coax/lib/lightning/pay.coax`](../../coax/lib/lightning/pay.coax)

On the protocol ([chapter 65](ch065-one-schema-three-carriages.md)):

| Record | Schema | Carriage | Today |
| --- | --- | --- | --- |
| A Pylon's announcement | `openagents.pylon.v1` `Beacon` ([spec 27.1](../../docs/protocol/openagents-protocol-spec.md#271-openagentspylonv1)) | Nostr kind 30200, public (NIP-PYLON) | Spec sketch; no `.proto` file |
| Service receipt and pool totals | `openagents.pylon.v1alpha1` `ServiceReceipt`, `PoolAggregate` ([`mesh.proto`](../../proto/openagents/pylon/v1alpha1/mesh.proto)) | Nostr kinds 3201 and 30201 | Schema file: a projection of NIP-PYLON's JSON. The signed JSON stays the authority |

### The chapter's check

- **Source:** [`psionic/lib/pylon/onboard.coax`](../../psionic/lib/pylon/onboard.coax),
  [`psionic/lib/peer/pay.coax`](../../psionic/lib/peer/pay.coax),
  [`pay/executor-settlement.coax`](../../pay/executor-settlement.coax),
  [`apps/openagents-cli/pylon-onboard.coax`](../../apps/openagents-cli/pylon-onboard.coax),
  [`apps/openagents-cli/pylon-settle.coax`](../../apps/openagents-cli/pylon-settle.coax),
  [`openagents/examples/pylon-onboard-settle.coax`](../examples/pylon-onboard-settle.coax),
  and the committed stdout transcript [`openagents/examples/pylon-onboard-settle.out`](../examples/pylon-onboard-settle.out).
- **Trace:** audits default-OFF mainnet arming (`onboard_unarmed_blocker=mainnet_unarmed`),
  admits a 3-leg settlement (`750` worker + `200` validator + `50` author = `1000`
  sats, `500` bps cap, `legs=3 released_sat=1000`), resets the rolling UTC daily
  cap cleanly on day `20779`, and counts only `real_bitcoin_moved == true` rows
  (`truthful_settled_sats=250`).
- **Refusal:** same-device validation is refused (`same_dev_can_val=false`);
  replaying an already-settled author leg is refused with `ES_REFUSE_DUPLICATE`
  (`dup_refused=true`); royalties exceeding `max_royalty_bps` are refused with
  `ES_REFUSE_BAD_INPUT` (`over_bps_refused=true`); exceeding the daily cap holds
  with `ES_HOLD_OWNER` (`same_day_held=true`).
- **Independent check:** `coax/bin/coax run openagents/examples/pylon-onboard-settle.coax`
  and `--release` match [`openagents/examples/pylon-onboard-settle.out`](../examples/pylon-onboard-settle.out),
  beside live `openagents pylon verify-relay` (#13963) and 3-leg settlement E2E
  against `nostr/relay/bin/relay` (#13961).

Planned paths: none. Pylons are [`psionic/lib/pylon`](../../psionic/lib/pylon).

## Sources

- **Episodes:**
  - [Episode 290, *Enter the Verse*](../../docs/transcripts/290.md): 2:30 to 5:30 and 24:00 to 26:00, Pylons for gamers
  - [Episode 1, *Intro*](../../docs/transcripts/001.md): an open platform tied to a compute network
  - [Episode 174, *GPUtopia 2.0*](../../docs/transcripts/174.md): sellers without buyers
  - [Episode 178, *Swarm Inference*](../../docs/transcripts/178.md): paid inference between two machines
  - [Episode 201, *Fracking Apple Silicon*](../../docs/transcripts/201.md): idle Apple hardware
  - [Episode 203, *Pylon and Nexus*](../../docs/transcripts/203.md)
  - [Episode 214, *Compute Market*](../../docs/transcripts/214.md): a compute market beta
  - [Episode 221, *Pylon Launch*](../../docs/transcripts/221.md): the Pylon launch
  - [Episode 223, *Pay the People*](../../docs/transcripts/223.md): payout bottlenecks
  - [Episode 237, *You Must Construct Additional Pylons*](../../docs/transcripts/237.md): construct additional Pylons
- **Old code at `coder-v1.0.0`, crates:** `pylon`, `psionic`, `inference`, `compute-workbench`
- **NIPs:** [NIP-PYLON](../../nostr/nips/openagents/NIP-PYLON.md), [NIP-CAP](../../nostr/nips/openagents/NIP-CAP.md), [NIP-X402](../../nostr/nips/openagents/NIP-X402.md), [NIP-HOST](../../nostr/nips/openagents/NIP-HOST.md)
- **Also:**
  - Psionic's chapters on the Pylon service and Nostr From Scratch's on its events, listed in [the books' shared topics](../../docs/books/README.md)
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P11, P12

## Open questions for the owner

1. As public operators join beyond our own machines, should per-job payouts
   stay fixed (starting at `200 sats` worker + `50 sats` validator for exact
   replay) or move to open market bidding under daily caps?
2. What trust/reputation threshold graduates a new public Pylon from Tier-2
   redundant replication to sampled validation?
3. Beyond our own coding agents and the Metaverse, who is the first external
   buyer of verified execution or DiLoCo training capacity?

## Source listing

Run the Chapter 70 example in [`openagents/examples/pylon-onboard-settle.coax`](../examples/pylon-onboard-settle.coax) (committed transcript [`openagents/examples/pylon-onboard-settle.out`](../examples/pylon-onboard-settle.out)):

<!-- source: openagents/examples/pylon-onboard-settle.coax pylon-onboard-settle -->
```jai
pos_run :: () -> bool {
  // 1. External Pylon onboarding readiness audit: unarmed default-OFF vs same-device vs armed.
  base := PoInputs.{
    has_nostr_key = true, executor_selftest_ok = true, tamper_refused_ok = true,
    modules_installed = 8, relay_configured = true, distinct_validator_device = true,
    wallet_ready = true, mainnet_armed = false, daily_cap_sat = 5000,
  };
  r_unarmed := po_evaluate(base);
  same_dev := base; same_dev.distinct_validator_device = false;
  r_same := po_evaluate(same_dev);
  armed := base; armed.mainnet_armed = true;
  r_armed := po_evaluate(armed);
  print("onboard_unarmed_blocker=% same_dev_can_val=% armed_can_mainnet=%\n",
    po_blocker_name(r_unarmed.blocker), r_same.can_run_validator, r_armed.can_settle_mainnet);

  // 2. Three-leg escrow admission (750 worker + 200 validator + 50 author = 1000 sats, 500 bps).
  pol := VpExecPolicy.{
    es = EsPolicy.{owner_armed = true, per_payout_cap_sat = 1000, per_pubkey_daily_cap_sat = 1000, global_daily_cap_sat = 1000},
    mainnet_armed = true,
    owner_policy_digest = POS_POL
  };
  l: VpExecLedger;
  pair := vp_exec_admit_replay_pair(l, pol, POS_REC, POS_VRD, POS_WRK, POS_VAL, ExwReplayOutcome.exact_replay, 750, 200, 20778, "2026-11-21", "bitcoin");
  au := vp_exec_admit_author_leg(pair.ledger, pol, POS_REC, POS_VRD, POS_AUT, ES_VERDICT_EXACT_REPLAY, 750, 200, 50, 500, 20778, "2026-11-21", "bitcoin");
  l = au.ledger;
  print("three_leg_ok=% legs=% released_sat=%\n",
    pair.worker.code == ES_RELEASE && pair.validator.code == ES_RELEASE && au.code == ES_RELEASE,
    l.n_legs, l.released_sat);

  // 3. Refusals: duplicate author leg refused, over-cap royalty (51/1001 > 500 bps) refused, cap hit on same UTC day -> held.
  dup := vp_exec_admit_author_leg(l, pol, POS_REC, POS_VRD, POS_AUT, ES_VERDICT_EXACT_REPLAY, 750, 200, 50, 500, 20778, "2026-11-21", "bitcoin");
  over_bps := es_admit_three_leg_split(750, 200, 51, 500);
  same_day := vp_exec_admit_author_leg(l, pol, POS_REC2, POS_VRD, POS_AUT, ES_VERDICT_EXACT_REPLAY, 750, 200, 50, 500, 20778, "2026-11-21", "bitcoin");
  next_day := vp_exec_admit_author_leg(l, pol, POS_REC2, POS_VRD, POS_AUT, ES_VERDICT_EXACT_REPLAY, 750, 200, 50, 500, 20779, "2026-11-22", "bitcoin");
  print("dup_refused=% over_bps_refused=% same_day_held=% next_day_reset=%\n",
    dup.code == ES_REFUSE_DUPLICATE, over_bps == ES_REFUSE_BAD_INPUT,
    same_day.code == ES_HOLD_OWNER, next_day.code == ES_RELEASE);

  // 4. Truthful settled-sat accumulator ignores simulated/dry-run rows.
  acc := po_accumulate_settled_sat(0, 1000, false);
  acc = po_accumulate_settled_sat(acc, 250, true);
  print("truthful_settled_sats=%\n", acc);

  return !r_unarmed.can_settle_mainnet && !r_same.can_run_validator && r_armed.can_settle_mainnet &&
         l.n_legs == 3 && l.released_sat == 1000 && dup.code == ES_REFUSE_DUPLICATE &&
         over_bps == ES_REFUSE_BAD_INPUT && same_day.code == ES_HOLD_OWNER &&
         next_day.code == ES_RELEASE && acc == 250;
}
```

Output (`coax/bin/coax run openagents/examples/pylon-onboard-settle.coax`, identical in `--release`):

```text
onboard_unarmed_blocker=mainnet_unarmed same_dev_can_val=false armed_can_mainnet=true
three_leg_ok=true legs=3 released_sat=1000
dup_refused=true over_bps_refused=true same_day_held=true next_day_reset=true
truthful_settled_sats=250
ok=true
```
