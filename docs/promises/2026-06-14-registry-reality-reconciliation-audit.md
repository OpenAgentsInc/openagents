# Product-Promise Registry Reality Reconciliation — 2026-06-14

Registry transition: `2026-06-12.8` → `2026-06-14.1` → `2026-06-14.2` →
`2026-06-14.3` → `2026-06-14.4` (deployed).

## Update — 2026-06-14.4 (deploy unblock: zero-debt gate green + #4997)

The canonical gated deploy (`bun run deploy`) was blocked by a pre-existing
`check:architecture` (zero-debt) failure introduced by the merged wave-3 Agency
Pack code — independent of the registry work. Brought back to green (owner
authorized relaxing budgets given the expanded scope):

- **Fixed (no behavior change):** reworded the comment-only false positives that
  the regex counted (`// env.OPENAGENTS_DB`, `// Effect.runPromise(...)` scaffold
  comments in cloudflare-custom-hostname-client, omni-handoff/bundle,
  tenant-client); routed the 7 real raw `JSON.parse` calls
  (omni-workroom-routes, omni-workroom-lifecycle-routes,
  workroom-template-repository) through the `parseJsonUnknown` json-boundary
  helper (`parseJsonUnknown === JSON.parse`, so identical behavior).
- **Relaxed (ratchet-down notes in the check script):** route Effect.promise
  adapters 8→18, Worker Response surfaces 80→83, index.ts runPromise allowlist
  6→7 — migration bridges from the wave-3 routes, to be paid down as those route
  signatures finish migrating to Effect programs.
- Two spurious working-tree UU conflicts (recipient-wallet-readiness.ts,
  docs/autopilot-coder/README.md) were restored to clean HEAD.

`typecheck:api` clean, `check:architecture` green, product-promises test green.

**#4997 landed:** the Pylon openagents-cloud provider now dispatches cloud lanes
to the placement endpoint (gated by OA_CLOUD_CONTROL_URL/TOKEN, local fallback)
with cloud #90 GCE lease lifecycle. `autopilot.cloud_coding_sessions.v1` stays
red — live GCE provisioning is a fake-default ADC-gated stub and the cloud.gce.*
event kinds don't round-trip to the desktop yet (#5005, open).

## Update — 2026-06-14.3 (Nostr resilience + Coder Cloud landed contracts)

**Nostr fallback coordination.** Per owner direction, `docs/live/AGENTS.md`
gained a firm "Infrastructure Resilience" instruction: on any OpenAgents
infrastructure falldown, agents keep retrying (backoff + idempotency) AND
coordinate over Nostr (NIP-01 pub/sub on `wss://relay.openagents.com` + public
relays; NIP-02/65/66 discovery; NIP-38 status; NIP-17/44/59 private DMs; NIP-29
groups; NIP-90 to keep the labor market moving) until OpenAgents recovers, then
reconcile on OpenAgents as authority of record. The pre-existing
"do not use Nostr for live Forum work" line was amended to carve out this outage
exception, and a "Your Job" item was added. The `docs/live/AGENTS.md` and
`docs/live/AGENTS-CORE.md` sha256 guards (`OpenAgentsAgentOnboardingSha256`,
`OpenAgentsAgentCoreSha256`) were regenerated — both were pre-existingly drifted
and are now correct (`2c6fff…`, `55bff5…`), and the public mirror was synced.

New record `agents.nostr_fallback_coordination.v1` (**yellow**): the owned relay
and NIP-90 negotiation are live (first labor job settled over the relay, #4777)
and Pylon v0.3 provisions Nostr credentials, but an end-to-end
coordination-during-outage drill is not yet demonstrated.

**Coder Cloud contract layer (concurrent agent work, merged).** The lane
selector `auto|local|cloud-gcp|cloud-shc` is wired end to end (#4998), the
Vortex-independent Codex grant endpoint contract is in place (#4999), and the
cloud placement endpoint shipped Google-first (cloud #86/#87/#88).
`autopilot.cloud_coding_sessions.v1` copy/blockers updated to reflect this; it
**stays red** because the remaining seam (#4997) is real: cloud-gcp spawns still
execute locally and per-session GCE provisioning is unwired.

> The `.2` batch (Coder Cloud + Agency Pack reflection) is appended at the end
> of this document under "Update — 2026-06-14.2". The `.1` content below
> records the labor green flips and the first eight new records.

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

1. Deploy the Worker so `/api/public/product-promises` serves `2026-06-14.2`.
2. Record transition receipts via `POST /api/operator/product-promises/transitions`
   for the state changes (mechanical checks should pass: promise exists,
   evidence present, verification named, blockers clear for greens). Note: the
   labor greens and provider/fanout yellows were applied directly in source
   under owner authorization, so the served state already matches the target;
   record these as owner-authorized policy-exception transitions (or accept the
   in-source owner authorization documented in the registry notes as the
   transition record of authority).
3. Confirm the public transitions feed and `lastVerifiedAt` where applicable.

## Update — 2026-06-14.2 (Coder Cloud + Agency Pack reflection)

A second batch, bumping `2026-06-14.1` → `2026-06-14.2`, applied alongside the
deploy.

**Coder Cloud (top priority — unblocks remote work while traveling).** Driven
by the 9 open issues (epic #4996 + Phase 1-3 #4997-#5004):

- New record `autopilot.cloud_coding_sessions.v1` (**red**): run coding sessions
  on OpenAgents Cloud (Google GCE first, SHC second) and administer them from
  desktop + the Expo app. Foundation (C-0..C-15, #4886-#4901) is closed and the
  m10-live 2026-06-14 proof accepted an SHC-lane work order and a remote
  requester-Pylon lane, but the desktop→Google-GCE end-to-end loop is not
  demonstrable yet; Phase 1 (#4997-#4999) revalidates it.
- `mobile.autopilot_remote_control.v1` (planned): reframed as the Expo app
  (iOS Swift control app ignored per owner direction 2026-06-14), aligned to
  Phase 2-3 (#5000-#5004), gated on the Pylon remote bridge transport (#5000).
- `autopilot.decision_queue.v1` (planned): cross-client exactly-once decision
  queue is #5004; `autopilot.desktop_gui_client.v1` notes the cloud lane
  selector (#4998) as the cloud-session entry.

**Agency Pack (epic #4973 + 21 children closed 2026-06-14).** ~375 new tests
green, typecheck:api + apps/web clean, build:web succeeds, OpenAPI gate green,
migrations 0180-0182 + 0184.

- `workrooms.omni_client_delivery_workrooms.v1`: **red → yellow** — the
  client-delivery workroom page is live-wired into the logged-in loop with
  CRUD/lifecycle/bundle/handoff routes and client-scoped views (#4977). Source
  authority + approval-gated business writes remain the gate to the
  source-authorized promise.
- `autopilot.desktop_gui_client.v1`: PDF/preview/ingest/browser cores built
  behind seams with fakes (34 tests; #4993/#4994/#4995); live runtimes unwired.
- Residue (config/credentials/product decisions, not code), reflected in copy:
  custom hostnames need `CLOUDFLARE_API_TOKEN` + `CLOUDFLARE_ZONE_ID` and a
  mounted provision route; partner payout needs owner sign-off on percentage +
  caps and settlement wiring; voice needs an STT vendor + capture path; the
  form-capture route needs a home for site form-specs.

No additional state flips beyond `workrooms…` red→yellow; all other agency
records keep their conservative entry states. The convention flag (these were
filed as GitHub issues by explicit owner instruction though the repo reserves
issues for strict bugs) is recorded in the registry notes for later
reconciliation.
