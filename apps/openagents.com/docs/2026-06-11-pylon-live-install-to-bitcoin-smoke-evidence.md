# Pylon Live Install-To-Bitcoin Smoke Evidence (live_small_sats)

Date: 2026-06-11

Issue: `OpenAgentsInc/openagents#4658`

Registry version during run: `2026-06-11.3` (worker deployed at that version).

Operator approval: `approval.operator.20260611.blocker_wave2_issue4658`
(spend cap 500 sats total, payout amount cap 150 sats).

## Result

The `live_small_sats` mode of the install-to-bitcoin smoke contract
(`workers/api/src/pylon-install-to-bitcoin-smoke.ts`) ran end to end on a
real machine (operator macOS host) against production `openagents.com`.
The projection computed from the live ref bundle returned:

- `status: live_settled_bitcoin_ready`
- `blockerRefs: []`
- `settledBitcoinClaimAllowed: true`
- `redactionScanPassed: true`
- all nine steps (`install`, `register`, `heartbeat`, `wallet`,
  `assignment`, `closeout`, `payment`, `settlement`,
  `public_projection`) in state `passed`

## Run shape

1. Fresh install: `bun pm pack` of `@openagentsinc/pylon@0.3.0-rc2` (with
   `@openagentsinc/nip90` and `@openagentsinc/tassadar-executor` tarball
   overrides), installed into a clean temp prefix with a brand-new
   dedicated `PYLON_HOME`. `pylon bootstrap --json` passed platform and
   bin checks.
2. Fresh agent identity registered via `POST /api/agents/register`
   (display name `pylon-4658-smoke`); all agent-side calls used that
   bearer credential. No existing Pylon registration or home was touched.
3. The installed binary itself ran the agent-side chain:
   `pylon presence register`, `pylon provider go-online`,
   `pylon presence heartbeat`, `pylon wallet status`,
   `pylon wallet report-readiness`, and
   `pylon wallet request-payout-target-admission`.
4. MDK wallet: a brand-new wallet was created in its original wallet home
   (never a mnemonic restore) and funded small over Lightning from the
   operator edge payer wallet, which itself runs in its preserved original
   funded wallet home per the #4657 send-readiness rule.
   `MDK_WALLET_PORT` was explicitly pinned for every daemon.
5. Paid assignment chain on production: operator-dispatched assignment
   (`payable_pending_settlement`, controlled dispatch gate `ready`),
   worker accept -> progress -> artifacts -> worker closeout, operator
   closeout to `accepted_work`.
6. Payment: the hosted-MDK programmatic payout adapter returned
   `hosted_mdk_programmatic_payouts_disabled` on the production account,
   so the payout used the established `mdk_agent_wallet` adapter pattern
   (same as the 2026-06-08 settled receipts): a real 21-sat Lightning
   payment from the operator edge payer wallet to the fresh Pylon
   wallet's payout target, verified by balance movement on both wallets,
   followed by Pylon-side `payment-receipts` and `settlement-status`
   events and the operator settlement bridge.
7. Public settlement receipt
   `receipt.nexus_pylon.settlement.assignment_public_install_to_bitcoin_20260611033900`
   is retrievable at its public route and reports
   `receiptKind: settlement_recorded`, `realBitcoinMoved: true`,
   `movementMode: real_bitcoin`, state `settled`, `amountSats: 21`.

## Ref bundle

| Stage | Refs |
| --- | --- |
| install | `install.public.pylon.0_3_0_rc2.local_tarball_4658` |
| register | `registration.pylon.24819249b4634a4c9d5e`, `pylon.24819249b4634a4c9d5e` |
| heartbeat | `heartbeat.public.pylon.24819249b4634a4c9d5e.seq_1` |
| wallet | `pylon_event.wallet_readiness.28d0939c-dffe-490e-a62c-2f956937998a`, `wallet_home.mdk_agent_wallet.original_funded_wallet_home` |
| payout readiness | `pylon_event.payout_target_admission.de633e63-1b7f-481e-a5ea-a5e92d286f54`, `payout_target.public.install_to_bitcoin.admitted_4658` |
| assignment | `assignment.public.install_to_bitcoin.20260611033900` (lease active through closeout) |
| closeout | `closeout.public.install_to_bitcoin.operator_accepted`, `pylon_event.worker_closeout.f4829fcd-5e28-45ea-9f0c-3cf38d26fa7e`, `accepted_work.public.install_to_bitcoin.echo_4658` |
| payment | `pylon_event.payment_receipt.9ebecfd4-c95d-44fe-80d8-6bdfa5b30a6b`, `payment.redacted.mdk_agent_wallet.4ca9c1b4e2ef2d4d95adbec7` |
| settlement | `receipt.nexus_pylon.settlement.assignment_public_install_to_bitcoin_20260611033900`, `pylon_event.settlement_status.4175a524-90f5-49ab-8c47-86df0fe125a0` |
| public projection | `route:/api/public/nexus-pylon/receipts/receipt.nexus_pylon.settlement.assignment_public_install_to_bitcoin_20260611033900`, `route:/api/pylons/pylon.24819249b4634a4c9d5e` |
| approval / caps | `approval.operator.20260611.blocker_wave2_issue4658`, `spend_cap.bitcoin_satoshis.150`, `spend_cap.total.bitcoin_satoshis.500`, `amount.bitcoin_satoshis.21` |

## Spend accounting

| Movement | Sats |
| --- | --- |
| Operator funding of the fresh Pylon wallet (Lightning, JIT channel fee absorbed by receiver) | 100 |
| Accepted-work payout to the Pylon wallet | 21 |
| Total operator spend (cap 500) | 121 |
| Hosted MDK treasury spend | 0 |

No mnemonics, raw invoices, payment hashes, preimages, bearer tokens, or
wallet-home paths are recorded in this document or in any public ref.

## Transition receipts

- New: `promise_transition_b9d568b5-0d02-476b-8205-9503f9060744`
  (`pylon.install_without_wallet_knowledge.v1`, red -> yellow, result
  `passed`, recorded against registry `2026-06-11.3` before the registry
  edit).
- Superseded: `promise_transition_73746398-0096-4962-b0c6-060e81fc70c4`
  (2026-06-10, the #4657 MDK restore re-scope receipt; the registry still
  showed red when this run started, so the new receipt is the operative
  red -> yellow evidence and cites the old one).

## Honest remainder (named blocker for green)

`blocker.product_promises.install_to_bitcoin_self_serve_without_operator_staging_missing`:
this run required operator staging (operator-funded wallet, operator
assignment dispatch, operator closeout, operator payout execution because
hosted-MDK programmatic payouts are disabled on the production account).
Green requires the same chain to run self-serve for a stranger with no
Bitcoin wallet knowledge and no operator in the loop.
