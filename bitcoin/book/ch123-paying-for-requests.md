# 123. Paying for requests and verified work

Parts XVII and XIX built Lightning channels, invoices ([chapter 114](ch114-invoices.md)),
routing ([chapter 113](ch113-find-a-route-and-pay.md)), a shared BIP-39 seed
([chapter 121](ch121-one-seed-for-nostr-and-bitcoin.md)) and agent treasuries
([chapter 122](ch122-agent-treasuries.md)). This chapter connects those money
primitives to the real payment boundary that ships in OpenAgents: paying for
requests and independently verified Pylon work over **Bitcoin / Lightning /
Breez Spark** under `#authority` Coax policy.

OpenAgents keeps a clean architectural seam between **policy** and **wallet
effects**:
- **Money policy lives in `#authority` Coax** (`pay/wallet.coax`,
  `pay/wallet-rpc.coax`, [`pay/executor-settlement.coax`](../../pay/executor-settlement.coax)):
  pure integer satoshi bounds, verdict gating, idempotency, rolling 24-hour UTC
  caps, default-OFF owner arming (`NEEDS_OWNER.md`), and truthful receipt
  invariants—zero keys, zero sockets, zero surviving mutants.
- **Payment effects live in the host wallet adapter** (`apps/shared/host/apple/SparkWallet.swift`,
  `apps/openagents-desktop/Window+Wallet.swift`,
  `apps/openagents-desktop/Window+WalletRPC.swift`,
  `apps/openagents-desktop/page/walletpane.coax`, and
  `apps/openagents-cli/pylon-settle.coax` talking to the Breez Spark SDK): once
  policy returns `ES_RELEASE`, the host settles the Lightning invoice or Spark
  transfer and writes back the payment ID. On desktop (`--minimal` and `--full`),
  the sidebar Wallet pane displays live Spark balance and receive address
  alongside read-only NIP-06 Nostr key metadata (`m/44'/1237'/0'/0/0`), while
  agent-initiated spending through `WalletPay` in
  [`openagents/core/members.coax`](../../openagents/core/members.coax) is
  `#forall`-proven bypass-immune: even in Allow All / BYPASS / AUTO mode,
  `WalletPay` always requires explicit interactive owner approval.

```shell
coax/bin/coax run bitcoin/examples/paying-for-requests.coax
coax/bin/coax test pay/executor-settlement.coax
coax/bin/coax mutate pay/executor-settlement.coax --root-only
```

## By hand: three-leg settlement, rolling UTC caps, and refusal order

Suppose the owner arms the settlement policy with `per_payout_cap_sat = 500`,
`per_pubkey_daily_cap_sat = 1000`, and `global_daily_cap_sat = 1200`. On UTC
day `20778`, `900` sats have already been spent globally and `750` sats to this
worker pubkey:

1. **Worker leg (`200 sats`, `EXACT_REPLAY`):** `200 <= 500`, `750 + 200 = 950 <= 1000`,
   `900 + 200 = 1100 <= 1200`, `already_paid_leg = false`, `owner_armed = true`
   → `ES_RELEASE`.
2. **Validator leg (`50 sats`, `EXACT_REPLAY`):** Independent second-device
   validator passes the exact same cap checks → `ES_RELEASE`.
3. **Author royalty leg (`25 sats`, `EXACT_REPLAY`, `#13961`):** Module-author
   royalty leg passes per-leg caps (`ES_RELEASE`), and
   `es_admit_three_leg_split(200, 50, 25, 2500)` checks
   `25 * 10000 = 250,000 <= (200 + 50 + 25) * 2500 = 687,500` (`9.09% <= 25%`)
   → `ES_RELEASE`. Asking for a `100 sat` royalty on `(200, 50, 100)` gives
   `100 * 10000 = 1,000,000 > 350 * 2500 = 875,000` (`28.57% > 25%`) →
   `ES_REFUSE_BAD_INPUT`.
4. **Default-OFF mainnet arming:** If `owner_armed == false`, any valid payout
   returns `ES_HOLD_OWNER` before touching the Spark wallet.
5. **Duplicate delivery after restart:** If `already_paid_leg == true` for a
   `(receipt_id, leg)` pair, policy returns `ES_REFUSE_DUPLICATE` (zero
   double-spend).
6. **Failed or unverified verdicts:** `ES_VERDICT_FAILED` returns
   `ES_WITHHOLD_FAILED`; `ES_VERDICT_UNVERIFIED` returns `ES_HOLD_OWNER`.
7. **Rolling 24-hour UTC cap reset:** Requesting `350 sats` on day `20778`
   gives `900 + 350 = 1250 > 1200` → `ES_HOLD_OWNER`. On the next UTC day
   (`20779 > 20778`), `es_roll_period` resets both daily counters to `0`, so
   `350 <= 500` releases (`ES_RELEASE`). A stale or older day (`20777 < 20778`)
   never resets counters backwards.
8. **Truthful receipts:** `es_receipt_truthful(real_bitcoin_moved, settled_sat,
   has_payment_id)` returns `false` if a caller claims `real_bitcoin_moved =
   true` without both `settled_sat > 0` and a real Spark/Lightning payment ID.

## The `#authority` three-leg settlement policy

[`pay/executor-settlement.coax`](../../pay/executor-settlement.coax) (`101/101`
mutants killed, `0` survived) defines the satoshi upper bound (`21,000,000 BTC`),
the maximum module-author royalty cap (`25%` / `2500 bps`), the three settlement
legs (`WORKER`, `VALIDATOR`, and `AUTHOR`), the verdicts, and the policy/period
structs:

<!-- source: pay/executor-settlement.coax types -->
```jai
ES_MAX_SAT :: 2_100_000_000_000_000;
ES_MAX_AUTHOR_ROYALTY_BPS :: 2500;

ES_LEG_WORKER :: 1;
ES_LEG_VALIDATOR :: 2;
ES_LEG_AUTHOR :: 3;

ES_VERDICT_EXACT_REPLAY :: 1;
ES_VERDICT_TRAINING_ACCEPTED :: 2;
ES_VERDICT_FAILED :: 3;
ES_VERDICT_UNVERIFIED :: 4;

ES_RELEASE :: 0;
ES_HOLD_OWNER :: 1;
ES_WITHHOLD_FAILED :: 2;
ES_REFUSE_DUPLICATE :: 3;
ES_REFUSE_BAD_INPUT :: 4;

EsPolicy :: struct {
  owner_armed: bool;
  per_payout_cap_sat: int;
  per_pubkey_daily_cap_sat: int;
  global_daily_cap_sat: int;
}

EsPeriodState :: struct {
  utc_day: int;
  global_spent_sat: int;
  pubkey_spent_sat: int;
}

EsPayoutRequest :: struct {
  leg: int;
  verdict: int;
  amount_sat: int;
  utc_day: int;
  already_paid_leg: bool;
}
```

The admission function checks inputs, duplicate leg delivery, verdict status,
owner arming, and rolling daily caps in fixed fail-closed order, `es_admit_three_leg_split`
enforces the bounded author royalty share across `(worker, validator, author)`, and
`es_receipt_truthful` prevents dry-runs or mocks from ever inflating settled
Bitcoin totals:

<!-- source: pay/executor-settlement.coax policy -->
```jai
// Monotonic rolling 24-hour UTC period reset: advances to a newer utc_day by
// zeroing daily counters; never resets backwards on stale/out-of-order timestamps.
es_roll_period :: (st: EsPeriodState, req_utc_day: int) -> EsPeriodState #effects() {
  if req_utc_day > st.utc_day {
    return EsPeriodState.{
      utc_day = req_utc_day,
      global_spent_sat = 0,
      pubkey_spent_sat = 0
    };
  }
  return st;
}

// Decides whether a WORKER, VALIDATOR, or AUTHOR leg payout may release, must be held for
// owner review, must be withheld on failed verdict, or refused as duplicate/invalid.
es_admit_payout :: (pol: EsPolicy, st: EsPeriodState, req: EsPayoutRequest) -> int #effects(traps) {
  if req.leg != ES_LEG_WORKER && req.leg != ES_LEG_VALIDATOR && req.leg != ES_LEG_AUTHOR { return ES_REFUSE_BAD_INPUT; }
  if req.amount_sat <= 0 || req.amount_sat > ES_MAX_SAT { return ES_REFUSE_BAD_INPUT; }
  if req.utc_day <= 0 { return ES_REFUSE_BAD_INPUT; }
  if req.already_paid_leg { return ES_REFUSE_DUPLICATE; }
  if req.verdict == ES_VERDICT_FAILED { return ES_WITHHOLD_FAILED; }
  if req.verdict != ES_VERDICT_EXACT_REPLAY && req.verdict != ES_VERDICT_TRAINING_ACCEPTED {
    return ES_HOLD_OWNER;
  }
  if !pol.owner_armed { return ES_HOLD_OWNER; }
  cur := es_roll_period(st, req.utc_day);
  if pol.per_payout_cap_sat > 0 && req.amount_sat > pol.per_payout_cap_sat { return ES_HOLD_OWNER; }
  if pol.per_pubkey_daily_cap_sat > 0 && cur.pubkey_spent_sat + req.amount_sat > pol.per_pubkey_daily_cap_sat {
    return ES_HOLD_OWNER;
  }
  if pol.global_daily_cap_sat > 0 && cur.global_spent_sat + req.amount_sat > pol.global_daily_cap_sat {
    return ES_HOLD_OWNER;
  }
  return ES_RELEASE;
}

// Three-leg split & royalty cap admission (Worker + Validator + Module Author, #13961).
// Enforces positive worker/validator legs, non-negative author royalty within ES_MAX_SAT,
// max_royalty_bps bounded by ES_MAX_AUTHOR_ROYALTY_BPS (25%), and author/total <= max_royalty_bps/10000.
es_admit_three_leg_split :: (worker_sat: int, validator_sat: int, author_sat: int, max_royalty_bps: int) -> int #effects(traps) {
  if worker_sat <= 0 || validator_sat <= 0 || author_sat < 0 { return ES_REFUSE_BAD_INPUT; }
  if max_royalty_bps < 0 || max_royalty_bps > ES_MAX_AUTHOR_ROYALTY_BPS { return ES_REFUSE_BAD_INPUT; }
  rem := ES_MAX_SAT - worker_sat;
  if validator_sat > rem { return ES_REFUSE_BAD_INPUT; }
  if author_sat > rem - validator_sat { return ES_REFUSE_BAD_INPUT; }
  total := worker_sat + validator_sat + author_sat;
  // Overflow-safe check of author_sat * 10000 <= total * max_royalty_bps:
  // since total <= 2.1e15 and max_royalty_bps <= 2500, total * max_royalty_bps <= 5.25e18 < 2^63-1!
  if author_sat * 10000 > total * max_royalty_bps { return ES_REFUSE_BAD_INPUT; }
  return ES_RELEASE;
}

// Truthful settlement receipt invariant: real_bitcoin_moved may be true ONLY when
// settled_sat > 0 AND a non-empty settlement payment_id was returned by Spark/LN.
es_receipt_truthful :: (real_bitcoin_moved: bool, settled_sat: int, has_payment_id: bool) -> bool #effects() {
  if real_bitcoin_moved {
    return settled_sat > 0 && has_payment_id;
  }
  return true;
}
```

## Runnable example and expected transcript

[`bitcoin/examples/paying-for-requests.coax`](../examples/paying-for-requests.coax)
runs the three-leg settlement trace (worker, validator, and module author with
the 25% royalty cap check), all guarded refusals, the rolling UTC day reset,
and the truthful receipt gate, matching
[`bitcoin/fixtures/paying-for-requests.out`](../fixtures/paying-for-requests.out)
byte for byte in debug and release builds:

```text
worker 200 sat: release
validator 50 sat: release
author 25 sat: release
three-leg split 200/50/25 (<=25%): release
three-leg split 200/50/100 (>25% cap): refuse_bad_input
unarmed mainnet: hold_owner
duplicate leg: refuse_duplicate
failed verdict: withhold_failed
unverified verdict: hold_owner
over global daily cap: hold_owner
next UTC day reset: release
receipt real+id: true real+no_id: false dry_run: true
```

## Live Bitcoin mainnet qualification and checks

- **Unit & mutation checks:** `coax/bin/coax test pay/executor-settlement.coax`
  and `coax/bin/coax mutate pay/executor-settlement.coax --root-only` (`101/101`
  mutants killed, `0` survived).
- **Live two-device Bitcoin mainnet qualification (W4.3, `#13922`) and three-leg
  extension (`#13961`):** Verified end-to-end across two distinct physical
  machines (Apple M5 Max worker + `coderos-4080` RTX 4080 Linux validator over
  a live Coax Nostr relay) settling on **Bitcoin mainnet via Breez Spark
  (`0.27.2`)**—`200 sats` to the worker + `50 sats` to the validator, `0` sat
  fee, verified recipient mainnet balances, zero double-spend on restart, and a
  reconciled signed `NIP-PYLON` kind `30201` pool aggregate reporting
  `paid_msat.bitcoin = 250000`. `#13961` extended `pay/executor-settlement.coax`
  and `apps/openagents-cli/pylon-settle.coax` to a third `ES_LEG_AUTHOR`
  royalty leg capped at `25%` (`2500 bps`); broader agent-economy `NIP-REG` /
  `NIP-EVAL` / `NIP-PAY` live payout wiring is tracked in open issue `#14060`.
  See [the W4.3 mainnet settlement verification record](../../docs/verification/executor-w43-mainnet-settlement.md),
  [Psionic chapter 238](../../psionic/book/ch238-pay-for-verified-work.md), and
  [Nostr chapter 82](../../nostr/book/ch082-pylons-environments-and-deploys.md).
- **Protocol schema:** `VerifiedPayoutIntent` and `SettlementReceipt` in
  [`proto/openagents/executor/v1alpha1/settlement.proto`](../../proto/openagents/executor/v1alpha1/settlement.proto).
