# Product-Promise Registry Reality Reconciliation — 2026-06-14

Registry transition: `2026-06-12.8` → `2026-06-14.1`.

This audit reconciles the public product-promise registry
(`apps/openagents.com/workers/api/src/product-promises.ts`) with what actually
shipped and settled since registry `2026-06-12.8`, and with the imminent Monday
2026-06-15 decentralized-training launch. It records the state changes applied,
the new promise records added, the copy refreshes, and the receipt-first
transitions that the operator must still record at/after deploy.

## Governance note (read first)

State changes on this registry are **receipt-first**: the discipline
(`proof.claim_upgrade_receipts.v1`) is that an operator records a transition at
`POST /api/operator/product-promises/transitions` against the *served* version,
the route mechanically checks the record, and only then does the source flip.

The state flips in this edit (labor ×2 → green, provider/fanout ×2 → yellow)
were applied **under explicit owner authorization (2026-06-14)** ahead of the
operator-route transition receipts, because the underlying settlement evidence
(#4777 / #4781 / #4783 bundles, with real escrow reserve/release receipts,
NIP-90 event refs, validator verdicts, and ledger settlement) is complete and
public-safe. The matching `promise_transition_<uuid>` receipts are **still to be
recorded** by the operator against the deployed `2026-06-14.1` version — they
were not fabricated here. There is no CI deploy (empty `.github/workflows/`), so
this source edit does not change the live API until a manual deploy; the live
public claim and the receipt recording should land together.

## State changes applied (owner-authorized, receipt recording pending)

| promiseId | from → to | Evidence | Pending operator action |
| --- | --- | --- | --- |
| `labor.forum_work_requests.v1` | yellow → **green** | #4777 bundle: workRequest `b74bb55c…`, forum_topic `098e36a8…`, reserve/release escrow receipts, kind-5934/7000/6934 events, state `settled` | record transition receipt vs `2026-06-14.1` |
| `labor.nostr_negotiation_market.v1` | yellow → **green** | #4777 bundle: job `215ffa0b…`, quote `3d7ec6bb…`, acceptance `3cecbc2c…`, result event, 1 sat moved requester→provider on the audited ledger | record transition receipt vs `2026-06-14.1` |
| `provider.compliant_usage_labor.v1` | red → **yellow** | #4777: independent provider Pylon `e3a6991c…` executed output-only in a bounded sandbox, validator re-ran `bun test` (1 pass), public closeout `fe1ee748…`. Settled on the credit ledger, not yet the external reliable-tips ladder | record transition receipt; green still needs ladder-settled external payout |
| `autopilot.control_center_fanout_marketplace.v1` | red → **yellow** | #4783 P7 lane-C: real Autopilot work order `f374a475…` with owned capacity dark fanned out to the market (`432420e6…`), independent provider executed + validator-accepted, escrow settled; server-side `customerOptIn` gate enforced | record transition receipt; green still needs self-serve fanout + plugin marketplace beyond `code_task` |

`artanis.labor_requester.v1` stays **yellow**: the request_labor surface is built
and gated, but no unattended Artanis labor request has settled (blockers
unchanged).

## New promise records added (conservative entry states)

These product surfaces shipped (wave-3 #4977–#4995 + desktop/mobile clients) and
now make user/agent-facing claims with no prior record. All enter at conservative
states; none claim green.

| promiseId | state | What it covers |
| --- | --- | --- |
| `autopilot.desktop_gui_client.v1` | yellow | Electrobun + Foldkit desktop shell; connects to a local Pylon, renders sessions/decisions/timeline, dispatches bounded loopback actions. Local-only; remote/cloud + pricing/distribution gated. |
| `mobile.autopilot_remote_control.v1` | planned | RN/Expo remote-control app; spec + roadmap (#4902–#4948) filed, scaffold not built. Local-native build + self-hosted OTA per owner mandate. |
| `workrooms.omni_client_delivery_workrooms.v1` | red | Operator-gated client-delivery workroom data model + lifecycle/template routes (#4977). No customer-facing UI, source authority, or approval-gated writes yet. |
| `autopilot_sites.native_email_sequences.v1` | yellow | Operator-gated email campaign/sequence authoring + enrollment (#4983/#4984). No send-service integration or deliverability proof. |
| `autopilot_sites.custom_tenant_hostnames.v1` | yellow | Operator-gated tenant custom-hostname registration/verification/resolution (#4988/#4989) + Cloudflare custom-hostname client (config owner-gated). No self-serve UI or automated SSL. |
| `autopilot_sites.partner_payout_ledger.v1` | red | Operator-gated partner-payout ledger + transition routes (#4986). No attribution policy, settlement dispatch, or partner-facing projection. |
| `autopilot.cloud_credits_ui.v1` | yellow | Foldkit credits panel (balance + cost preview) embedded in the workroom page (#4985). Presentational; purchase/spend backend not wired. |
| `mobile.voice_session_evidence_transcript_ingest.v1` | red | Voice-session evidence contracts + read-only projections (#4992). No ingestion endpoint, transcription service, or proposal/approval loop. |

## Copy refreshes (state unchanged)

- `training.monday_decentralized_training_launch.v1` (**red**): copy updated from
  "Episode 236 says a launch is targeted for Monday" to reflect that the launch
  is **imminent (Monday 2026-06-15)** with contributor join-lifecycle /
  device-admission contracts landed (#4848–#4854) and the SHC+Pylon fallback
  closeout route deployed (m10-live 2026-06-14). Stays red: no run identifier,
  participant admission, work/validation/payment receipts exist yet.
- `training.public_distributed_training_run.v1` (**red**),
  `pylon.largest_decentralized_training_claim.v1` (**red**),
  `models.tassadar_percepta_executor.v1` (**red**),
  `pylon.v0_3_multi_earning_node.v1` (**red**): evidence/copy refreshed
  (W3 student-program report 2026-06-14 as research-only; m10 proofs; per-mode
  green/yellow inventory). States unchanged.
- Existing `workrooms.source_authorized_business_objects.v1`,
  `mobile.voice_approval_companion.v1`, `autopilot.decision_queue.v1`,
  `sites.referral_bitcoin_stream.v1`, `autopilot.agentic_labor_products.v1`:
  wave-3 issues added as precursor/foundation evidence; states unchanged.

## Caveats preserved

- **Settled ≠ external sats.** The first labor settlements moved 1 sat each via
  the credit ledger, not the external reliable-tips ladder. The labor market
  rails are green (request→settle lifecycle proven); broad external-payout
  earning is the next gate (provider.compliant_usage_labor stays yellow).
- **Monday launch has not happened.** All `training.*` launch promises stay
  red/yellow until the run produces public run state + accepted-work +
  validation + settlement receipts. Rails-ready is not launched.
- **Receipt-first still owed.** The four state flips above need their
  operator-route transition receipts recorded against `2026-06-14.1` at deploy.

## Operator checklist at deploy

1. Deploy the Worker so `/api/public/product-promises` serves `2026-06-14.1`.
2. Record transition receipts via `POST /api/operator/product-promises/transitions`
   for the four state changes above (mechanical checks should pass: promise
   exists, state differs, evidence present, verification named, blockers clear
   for the two greens).
3. Confirm the public transitions feed lists them and `lastVerifiedAt` updates.
