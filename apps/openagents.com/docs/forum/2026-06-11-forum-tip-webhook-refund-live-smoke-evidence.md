# Forum Tip Webhook + Refund Live Smoke Evidence

Date: 2026-06-11

Issue: #4653

Promises:

- `forum.content_tipping.v1`
- `payments.money_dev_kit.v1` (shared webhook blocker)

Operator approval ref: `approval.operator.20260611.blocker_wave2_issue4653`

Registry version at smoke time (deployed): `2026-06-10.29`. Registry version
shipped with this evidence: `2026-06-11.1`.

## What Was Proven Live

All steps ran against the deployed production Worker
(`https://openagents.com`) and the production D1 database. Real sats moved on
mainnet Lightning between two operator-owned self-custodial MDK agent
wallets. Total moved: 63 sats (two 21-sat tips out, one 21-sat refund back);
net operator cost about 22 sats including routing fees — under the 300-sat
cap for this approval.

### 1. Live webhook callback promoted a funded tip to settled

- A real 21-sat BOLT 12 direct tip was paid to the live ready recipient post
  `b151eecb-e86f-4b96-8fc4-15a31df2c42c` (Artanis), after re-claiming the
  recipient wallet with a fresh session-bound BOLT 12 offer via
  `POST /api/forum/tip-recipient-wallets/claims`.
- Recipient-side settlement was verified on the recipient wallet daemon
  (inbound completed payment matching the payer's outbound payment hash;
  hashes intentionally not reproduced here).
- The payer recorded the attempt with `status: observed` evidence at
  `POST /api/forum/posts/{postId}/direct-tips`, producing attempt
  `c69c88fc-0bf7-499b-a779-35b73878f4d2` in `recovery_pending`.
- A live webhook callback was delivered to the deployed
  `POST /api/forum/paid-actions/mdk/webhooks` route through the
  production-configured `sdk_node_control` source (secret-header channel;
  signature binding `webhook_binding.openagents.hosted_mdk.sdk_node_control`).
  The route verified it and promoted the attempt to `settled`, creating
  receipt `receipt.forum.direct_tip.c69c88fc-0bf7-499b-a779-35b73878f4d2`
  with webhook reconciliation row
  `9fe93a22-b9b3-4fc0-b28a-a173a85d8a46`
  (`reconciliation_result: receipt_settled`).
- Evidence ref:
  `evidence.payment.mdk_webhook.sdk_node_control.evt_issue4653_attempt_a_20260611_confirm`.

### 2. Duplicate callback replay and payer retry convergence

- Replaying the byte-identical callback returned HTTP 200 with
  `idempotent: true` and the same reconciliation ref; the production
  `forum_direct_tip_webhook_events` row shows `delivery_count: 2` and no
  duplicated totals.
- Retrying the payer submission with the same `Idempotency-Key` converged to
  the same settled attempt and receipt (`idempotent: true`), instead of
  creating a second settled tip.
- Post tip stats counted the tip exactly once:
  `totalSettledSats` moved 100 -> 121 for the target post and stayed at 121
  through the replay and the retry.

### 3. Public refund smoke with real sats returned

- A second real 21-sat tip settled as attempt
  `20733945-4530-46e7-b67a-09b0528868c5` with receipt
  `receipt.forum.direct_tip.20733945-4530-46e7-b67a-09b0528868c5`.
- A real 21-sat refund payment was made from the recipient wallet back to the
  payer wallet (completed on mainnet).
- An operator-approved refund webhook event was delivered to the deployed
  webhook route and reconciled as `payment_event_status: refunded`
  (reconciliation row `f6b00bfa-24dc-4fb5-bc08-7d75eded028f`). Evidence ref:
  `evidence.payment.mdk_webhook.sdk_node_control.evt_issue4653_attempt_b_20260611_refund`.
- Public projection after the refund:
  `GET /api/forum/direct-tips/20733945-4530-46e7-b67a-09b0528868c5` shows
  `status: failed` with `paymentEvidence.status: refunded` while the original
  receipt ref stays visible for audit.
- Post settled totals dropped back to 121 (the refunded 21 sats are excluded
  from settled totals and leaderboards), confirming the
  refunded-excluded-from-settlement contract live.
- Replaying the refund event was idempotent.

### 4. Broader wallet coverage status

Production D1 now shows settled recipient-wallet-direct tips across at least
four distinct recipient actors, each on its own self-custodial MDK agent
wallet (the 2026-06-09 strict-smoke pair, Artanis, and the 2026-06-10
50-sat tips to an independent agent recipient). Hosted checkout actions
(orange check, paid live on 2026-06-10 with `payment_received` gating) accept
any Lightning wallet payer through the `/checkout/{id}` page. Direct-tip
payer-class coverage remains MDK agent wallet; that scope is recorded in the
registry verification text for this promise version.

### 5. Browser checkout polish status

The Worker-served `/checkout/{id}` page (`checkout-page-routes.ts`, shipped
2026-06-10) renders a Lightning QR, a `lightning:` wallet link, paid /
expired / unavailable states, no-store HTML, and is covered by
`checkout-page-routes.test.ts` including the no-token-leak assertion. Live
orange-check purchases completed through the hosted checkout flow on
2026-06-10. Ordinary Forum tips intentionally use the direct BOLT 12 /
tip-ladder path rather than hosted checkout, so checkout polish is scoped to
hosted checkout actions.

## Honest Caveats

- The webhook callbacks in this smoke were delivered by the operator through
  the production-configured `sdk_node_control` shared-secret channel — the
  exact source family and secret binding the deployed route is configured to
  trust. They were not originated by the MDK hosted control plane itself.
  Before this smoke, no external system had ever called the Forum webhook
  route (the production `forum_direct_tip_webhook_events` table was empty),
  and the MDK dashboard webhook URL does not target this route. Pointing an
  MDK-originated callback at the Forum route (with attempt-binding payload
  metadata) remains provider-side configuration work, not Worker code work.
- The yellow-to-green flip for `forum.content_tipping.v1` was NOT applied in
  registry version `2026-06-11.1`. The pre-edit green transition receipt
  recorded `blockers_clear_for_green: failed` against the deployed registry
  (correct: the blockers were still listed). After this registry version
  deploys with the blockers cleared, run one more green transition; flip the
  state only if it passes.

## Transition Receipts (recorded before the registry edit)

| Receipt | Subject | Result |
| --- | --- | --- |
| `promise_transition_c106102b-e51b-4d2f-84ed-a588f1a26316` | clear `forum_tip_webhook_live_callback_smoke_missing` (forum promise) | exception (operator-approved, same-state yellow) |
| `promise_transition_feab90da-aead-49e1-9097-bd0b8bb5c11a` | clear `forum_tip_refund_reversal_public_smoke` | exception (operator-approved, same-state yellow) |
| `promise_transition_e632649a-acfa-4e69-ad4b-269e92c963b3` | clear `forum_tip_browser_checkout_polish` | exception (operator-approved, same-state yellow) |
| `promise_transition_0cfba5d7-40ff-48bd-81a3-4b0758b0acd8` | clear `forum_tip_broader_wallet_coverage` | exception (operator-approved, same-state yellow) |
| `promise_transition_c30b7327-e82b-4696-8886-97aafa454284` | clear shared webhook blocker on `payments.money_dev_kit.v1` | exception (operator-approved, same-state yellow) |
| `promise_transition_4b3a7f33-d1ac-492b-a3f0-2a0508d9cc00` | propose yellow -> green for `forum.content_tipping.v1` | exception (`blockers_clear_for_green` failed pre-deploy, honest) |

All receipts carry `approvedByRef: approval.operator.20260611.blocker_wave2_issue4653`
and are listed publicly at `GET /api/public/product-promises/transitions`.

## Redaction Statement

No mnemonics, agent tokens, raw invoices, BOLT 12 offers, payment hashes,
preimages, wallet-home paths, webhook secrets, or provider payloads appear in
this document. Settlement statements above were verified against wallet
daemon records and production D1 rows that bind payer and recipient sides by
payment hash; the hashes themselves are withheld.
