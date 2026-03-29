# Autopilot Desktop Panes

This document defines the active pane surfaces in `apps/autopilot-desktop` and how they are opened.

Deprecated simulation-only pane routes have been removed from the retained MVP runtime. The pane inventory below describes the active surfaces that remain available in current builds.

## Spacetime Rollout Semantics

Current docs reflect **Phase 1 (mirror/proxy semantics)** from `docs/SPACETIME_ROLLOUT_INDEX.md`:

- Sync bootstrap/token contracts are real and enforced.
- Replay-safe apply/checkpoint behavior is real and enforced.
- Presence/projection panes expose Spacetime-shaped state and stream ids while remote live subscription cutover completes.

Target **Phase 2** semantics (live remote subscriptions/reducers for ADR-approved domains) are tracked in:

- `docs/SPACETIME_ROLLOUT_INDEX.md`
- `docs/SPACETIME_SYNC_RELEASE_GATES.md`

## Pane Inventory

- `Autopilot Chat`
  - Chat-first pane with thread rail, transcript, composer input, and per-message status (`queued`, `running`, `done`, `error`).
  - Action: send prompt to local Autopilot lane.
- `Project Ops`
  - Native PM shell reserved for the Step 0 project-management slice.
  - Hidden by default behind `OPENAGENTS_ENABLE_PROJECT_OPS=1` until the PM stream-backed thin slice is ready.
  - Current shell shows feature-gate state, active default view, reserved PM stream grants, and the staged follow-up path for work items, cycles, and replay-safe projections.
  - Primary pane badge is `source: stream.pm.work_items.v1` because visible list/detail state comes from local replay-safe PM projection documents keyed by canonical PM stream ids.
  - PM sync/bootstrap diagnostics inside the pane may use `source: spacetime.sync.lifecycle`, but PM work-item values must not be labeled as live Spacetime authority during Phase 1.
  - Operator-visible PM rejection and recovery messages use stable `project_ops.*` error-code prefixes so invalid transitions, dependency failures, archived mutations, and checkpoint conflicts stay legible.
  - Action: read-only shell for now.
- `Codex Account`
  - Account auth and rate-limit controls (`account/read`, login start/cancel, logout, rate limits read).
- `Codex Models`
  - Model catalog visibility with default/hidden/reasoning capability details and reroute status.
- `Codex Config`
  - Config read/write/requirements and external agent config detect/import controls.
- `Codex MCP`
  - MCP server status listing, OAuth login for selected server, and config reload.
- `Codex Apps`
  - App connector list and update refresh flow.
- `Codex Labs`
  - Review start, command exec, collaboration modes, experimental features, and gated experimental APIs.
- `Codex Diagnostics`
  - Codex protocol observability pane with raw events, method counters, failure snapshots, and wire-log controls.
- `Provider Control`
  - Canonical provider shell pane with explicit state machine (`offline`, `connecting`, `online`, `degraded`) and preflight blockers.
  - The hotbar shell is now the default production shell; there is no Mission Control fullscreen-only production mode.
  - Owns `GO ONLINE` / `GO OFFLINE`, local runtime action, Apple FM smoke-test button when applicable, and provider inventory toggles.
  - Embeds the packaged `simple_fui_hud` asset through the shared native `RiveSurface` path as a settled hero surface inside the production pane, with truthful provider/runtime/wallet overlays sourced from app state.
  - Shows provider/runtime truth inline: `Mode`, `Model`, `Backend`, `Load`, `Control`, `Preflight`, blockers, last action/error, and advertised inventory rows.
  - `providers_online` remains sourced from Spacetime presence snapshots (`spacetime.presence:*` source tags) with identity-cardinality semantics from ADR-0002.
  - Online mode never auto-restores on launch in MVP; each app session requires a fresh explicit click before work intake begins.
  - Startup pane and hotbar slot `1`.
  - Action: toggle online/offline and runtime/inventory controls.
- `Provider Status`
  - Runtime status pane for heartbeat freshness, uptime, queue depth, and dependency state.
  - Shows canonical failure taxonomy class (`relay`, `execution`, `payment`, `reconciliation`) with concise diagnostics sourced from runtime/wallet/reconciliation authority.
  - Action: read-only operational visibility.
- `Tailnet Status`
  - Tailnet status pane for the current tailnet name and the auto-discovered device roster sourced from `tailscale status --json`.
  - Action: read-only operator visibility into the current Tailnet device set.
- `GPT-OSS Workbench`
  - GPT-OSS local inference workbench for the app-owned runtime seam.
  - Shows runtime reachability, configured model path, backend label, artifact presence, configured/ready/loaded model state, pane-owned prompt runs, and the last output preview.
  - Inputs: prompt, optional requested model, max tokens, temperature, top-k, and top-p.
  - Actions: refresh runtime, load/warm configured model, unload configured model, run prompt.
  - Explicit pane state machine: `loading`, `ready`, `error`.
- `Psionic Mesh`
  - Visualization-first GPT-OSS pane that turns Psionic runtime metrics into a synthetic decode field.
  - Shows a derived lattice, layer sweep, phase ribbons, and throughput/cache rings built from prompt/eval/load timings plus prompt/output token counts.
  - Explicitly labels the field as derived telemetry rather than raw tensor taps so the UI stays truthful about what the runtime exposes today.
  - Action: read-only visualization surface.
- `Contributor Beta`
  - Narrow external-contributor control surface for the admitted compiled-agent family.
  - Connects contributor identity, accepts the governed beta contract, runs the retained benchmark pack, captures governed runtime disagreement receipts, and submits bounded worker-role output for replay generation, ranking and labeling, validator scoring, or bounded module training.
  - Shows contributor identity, trust tier, environment class, capability summary, admitted family, contract versions, current worker role, accepted or review and rejected or quarantined counts, pending credit sats, credit-account linkage posture, and recent submission rows with digests and review reasons.
  - Makes the evidence boundary explicit: submissions can be accepted, quarantined, rejected, or routed for review, but the pane never grants promotion authority or live runtime authority to contributors.
  - Action: connect identity, accept contract, run benchmark pack, submit runtime receipt, cycle worker role, run bounded worker role.
- `Rive Preview`
  - Workbench pane for the packaged Rive asset registry using the shared native `RiveSurface` path.
  - Remains the debug/workbench surface even after the production Provider Control pane embeds the packaged HUD asset.
  - Controls: reload, previous asset, next asset, play/pause, restart, and fit mode.
  - Shows asset identity, render-path diagnostics, redraw/settled state, pointer capture, and first-frame metrics so new packaged assets can be verified without adding per-asset pane code.
  - Action: swap packaged assets and inspect runtime diagnostics.
- `Presentation`
  - Minimal slide-surface pane backed by the packaged `simple_fui_hud` Rive asset.
  - Fills the pane content area with the looping HUD animation using `contain` fit mode and no in-content controls, so fullscreen preserves the full composition instead of cropping width.
  - Exposes a fullscreen header action next to close; activating it promotes the pane into pane-level fullscreen, and `Esc` returns it to windowed mode.
  - Action: fullscreen toggle from pane chrome only.
- `Frame Debugger`
  - Live desktop cadence/debug pane for frame pacing, redraw pressure, and renderer timings.
  - Tracks rolling FPS, last/rolling frame interval, frame CPU phases, draw-call density, and the current redraw drivers coming from background pumps, chat, provider animation, and Rive surfaces.
  - Exists specifically to make render-loop lag visible from inside the app instead of inferring it from logs.
  - Action: read-only diagnostics surface.
- `Relay Connections`
  - Configured relay list with per-relay state (`connected`, `connecting`, `disconnected`, `error`), latency, last-seen, and last-error fields derived from provider-lane transport snapshots.
  - The default configuration should preinstall the OpenAgents-hosted Nexus as the primary relay, with a curated default public relay set visible and manageable alongside it.
  - Provider-mode NIP-90 intake should span the full configured reachable relay set and deduplicate repeated requests across relays; it is not restricted to Nexus-originated jobs.
  - Provider capability, feedback, and result events should fan out to every healthy configured relay by default; relay publish is best-effort and partial failures must remain visible.
  - The initial default public relay set should be chosen from relays where OpenAgents observes moderate-to-high recent NIP-90 job volume.
  - User-run Nexus deployments are assumed to be public/open relays by default. Closed/private relay posture is a future configuration path, not near-term product scope.
  - Inputs/actions: add relay (`wss://` validation), select row, retry selected, remove selected.
  - Retry is a reconnect attempt (`connecting`) only; connected state is set by relay transport health, not pane-local simulation.
  - Explicit pane state machine: `loading`, `ready`, `error`.
- `Data Market`
  - Read-only market snapshot and operator-facing lifecycle pane for the current Data Market starter slice.
  - Shows relay-backed compatibility `DataAsset`, `AccessGrant`, `DeliveryBundle`, and `RevocationReceipt` rows plus recent lifecycle summaries and DS-backed publication posture.
  - Asset rows now surface packaging posture, visibility/sensitivity posture, and redacted Codex-export markers when present so shell-first publication is still legible in the UI.
  - This is the observability/control surface, not the primary seller authoring surface; DS listings and offers are the public market-facing publication layer, while DS-DVM request/result traffic is only the targeted fulfillment layer.
  - Action: refresh market snapshot.
- `Data Seller`
  - Dedicated conversational seller-authoring pane for the current Data Market MVP.
  - Owns the seller transcript, structured draft, exact preview, confirm-before-publish flow, published inventory summary, DS listing/offer-backed publication flow, incoming targeted request evaluation, payment-required issuance, delivery preparation/publication, and revoke/expire controls.
  - The pane is still mutation-capable, but it now doubles as the read-only visualization surface for shell/headless publication by showing package metadata, preview state, relay/kind/request flow, payment, delivery, and revocation truth from the same app-owned state machine.
  - Auto-provisions the first-party `autopilot-data-seller` and `autopilot-data-market-control` skills into the lane and uses typed `openagents.data_market.*` tools rather than generic pane poking.
  - Product truth is now relay-first: publication emits DS objects to relays, and the pane immediately reads the relay-backed local replica and compatibility lifecycle state back into the seller model.
  - Shell/headless parity is intentional: the same seller flow is also exposed through `autopilotctl data-market ...` and `autopilot_headless_data_market`.
  - Actions: seller draft/preview/confirm/publish, request payment, prepare delivery, issue delivery, revoke access.
- `Data Buyer`
  - Narrow buyer-side Data Market pane for selecting a visible asset/default offer from the current market snapshot and publishing a targeted DS-DVM request.
  - Tracks the current DS-DVM request, feedback/result observation, and local consume path for the delivered bundle.
  - The selected-asset card now shows bundle/posture context, including redacted Codex-export markers, so a buyer can see what kind of packaged asset is being requested before publishing.
  - This is not yet a full public discovery or procurement workstation; it is the buyer-side MVP lane needed to exercise the targeted access flow truthfully.
  - Actions: refresh market, select asset, publish targeted DS-DVM request, consume delivery.
- `Sync Health`
  - Spacetime sync diagnostics for connection state, subscription state, cursor progress, stale detection, replay count, and duplicate-drop count.
  - Source is lifecycle/apply telemetry (`spacetime.sync.lifecycle`) in current rollout phase.
  - Action: `Rebootstrap sync` for deterministic recovery lifecycle reset.
  - Explicit pane state machine: `loading`, `ready`, `error`.
- `Network Requests`
  - Buyer-side request composer for network submission.
  - Inputs: request type, payload, budget sats, timeout seconds (validated before submit).
  - MVP OpenAgents-posted public jobs should default to `race` resolution mode; future `windowed` mode can be added later for quality-sensitive jobs.
  - `Buy Mode` is now the constrained dedicated pane for the `v0.1` smoke-test lane: start/stop loop, one in-flight request, fixed `kind: 5050`, fixed `2 sats`, fixed cadence, and no generic request authoring UI.
  - Output: submitted request rows with request id and response stream linkage.
  - Explicit pane state machine: `loading`, `ready`, `error`.
- `Starter Jobs`
  - Secondary detail/debug pane for seed-demand metadata; starter jobs still appear in the normal `Job Inbox`, `Active Job`, and `Job History` surfaces with a visible source marker and are not separated from the main earn flow.
  - Initially this pane is populated only when connected to the OpenAgents-hosted Nexus; connecting through a third-party Nexus does not qualify for OpenAgents starter jobs.
  - Eligibility is determined by starter-demand policy; initial OpenAgents starter jobs target Autopilot users only.
  - Eligibility should come from OpenAgents-hosted-Nexus proof where available, not solely from a user-supplied Nostr client tag.
  - Stronger anti-spoofing attestation is roadmap hardening, not an MVP prerequisite for this pane.
  - Shows eligibility and payout sats per starter job.
  - Shows assignment lease state, aggressive start-confirm timer, and reassignment timer for hosted starter jobs when available.
  - Tracks completion with payout pointer linkage for wallet/history visibility.
  - Includes explicit dispatch safety controls: sats budget cap telemetry, inflight cap telemetry, and immediate kill-switch toggle.
  - Explicit pane state machine: `loading`, `ready`, `error`.
- `Activity Feed`
  - Unified event stream across chat, jobs, wallet, network, and sync lanes.
  - Source badge is projection stream id (`stream.activity_projection.v1`), replay-safe and deduplicated.
  - Event rows carry stable event IDs and deterministic source tags to avoid duplicate replay rows.
  - Filters by domain (`all`, `chat`, `job`, `wallet`, `network`, `sync`) with selected-row details.
  - Continues to show observed network/job activity while offline so the app feels live before provider mode is enabled.
  - Explicit pane state machine: `loading`, `ready`, `error`.
- `Alerts and Recovery`
  - Deterministic incident queue across identity, wallet, relay, provider, and sync domains.
  - Rows track severity (`info`, `warning`, `critical`) and lifecycle (`active`, `acknowledged`, `resolved`).
  - Actions: run selected recovery, acknowledge selected alert, resolve selected alert.
  - Explicit pane state machine: `loading`, `ready`, `error`.
- `Settings`
  - MVP settings for primary Nexus/relay URL, backup relay set, wallet send-default sats, and provider queue-depth default (`1` active job initially).
  - Includes schema/version and identity path visibility.
  - Validation blocks invalid relay URL prefixes and impossible numeric ranges before save.
  - Save flow persists schema-backed settings and explicitly flags reconnect-required changes.
  - Explicit pane state machine: `loading`, `ready`, `error`.
- `Earnings & Jobs`
  - Canonical MVP earnings summary pane sourced from wallet/runtime/history lanes.
  - Shows sats today, lifetime sats, jobs today, last job result, current online uptime, and recent inbox/active/history summaries.
  - Includes loop-integrity SLO metrics: first-job latency, completion ratio, payout success ratio, and wallet confirmation latency.
  - Startup pane and hotbar slot `4`.
  - Actions: refresh metrics plus direct jump-off buttons for `Job Inbox`, `Active Job`, and `Job History`.
- `Buy Mode`
  - Dedicated buyer smoke-test pane for the fixed `kind: 5050` / `2 sats` loop.
  - Shows current loop summary, provider/work/payment state, and a visualization-first buyer ledger backed by NIP-90 payment facts.
  - Ledger rows make amount, fees, request identity, provider Nostr pubkey, and Lightning destination pubkey legible together, with degraded source labels when only projected evidence is available.
  - `OPENAGENTS_ENABLE_BUY_MODE` now only suppresses this pane/flow when explicitly disabled; the default shell assumes Buy Mode is available.
  - Action: start/stop loop and copy ledger.
- `NIP-90 Sent Payments`
  - Definitive buyer-side sent-payment report pane backed by the app-owned `stream.nip90_buyer_payment_attempts.v1` ledger.
  - Shows wallet-authoritative `payment_count`, `total_sats_sent`, `total_fee_sats`, `total_wallet_debit_sats`, selected window, connected relay count, deduped request count, degraded-binding count, and latest counted payment timestamp.
  - Presets: `Daily` (current local calendar day), rolling `24h`, `7d`, `30d`, plus `Custom` when absolute start/end boundaries have been supplied through pane inputs or control tooling.
  - Connected relays define the current request/result/invoice evidence scope, but definitive totals count only wallet-settled buyer sends after payment-pointer dedupe.
  - Actions: switch report window, cycle windows from keyboard/tooling, and copy the current report.
- `Buyer Race Matrix`
  - Visualization-first buyer pane for current NIP-90 provider competition.
  - Centers the current request and paints one vertical lane per provider pubkey with separate `SEL`, `RES`, `INV`, and `PAY` role chips when those roles split.
  - Uses current buyer flow state plus persisted provider-observation history to show result arrival, invoice arrival, payable-winner selection, relay provenance count, and loser reasons grounded in actual evidence.
  - Action: read-only live race visibility.
- `Seller Earnings Timeline`
  - Visualization-first seller pane for provider payouts over time.
  - Keeps wallet-confirmed Spark receives on the main horizontal rail and pushes settlement-observed / inferred rows into a visibly degraded secondary section.
  - Shows payout size, payer pubkey, settlement authority, and confirmation latency without making the operator read raw wallet/history rows.
  - Action: read-only payout visibility.
- `Settlement Ladder`
  - Operator-first per-request proof pane for answering exactly where a NIP-90 payment flow stopped.
  - Shows six proof rungs: request observed, result observed, invoice observed, buyer payment pointer assigned, seller settled, and buyer wallet confirmed.
  - Keeps seller-wallet settlement proof distinct from buyer-wallet confirmation proof so adjacent stages do not imply one another.
  - Action: read-only proof visibility.
- `Key Ledger`
  - Operator table for NIP-90 payment activity by actor with explicit namespace separation between Nostr keys and Lightning destinations.
  - Shows sats sent, sats received, jobs won, invoices emitted, settlement failures, average latency, and a tiny recent-activity sparkline for each actor row.
  - Includes a focus card that summarizes the current top actor and points operators toward atlas/replay drill paths.
  - Action: read-only actor visibility.
- `Settlement Atlas`
  - Constellation-style graph of buyer-to-provider NIP-90 payment edges using canonical Nostr identities from the fact ledger.
  - Encodes edge thickness from sats volume and edge glow from recency while keeping facts without both sides of identity proof out of the graph and counted as degraded.
  - Includes a focus edge card for the hottest current buyer/provider relationship and its request/payment evidence anchor.
  - Action: read-only graph visibility.
- `Spark Replay`
  - Scrubbable replay pane for a single NIP-90 request race from request publication through buyer payment and seller settlement.
  - Uses persisted request facts plus provider-observation history first, with lower-confidence segments visibly marked when replay steps are derived/backfilled.
  - Includes `Prev`, `Auto`, and `Next` controls plus per-step evidence detail so the replay is inspectable rather than decorative.
  - Action: scrub replay steps.
- `Relay Choreography`
  - Relay-aware pane that keeps current relay health separate from persisted NIP-90 relay-hop evidence.
  - Shows buyer and provider anchors with relay nodes in the middle, request publish fanout, result ingress, and invoice ingress threads only when those relay URLs are actually present in the payment ledger.
  - Wallet confirmation is kept explicitly request-scoped in the detail card so the UI does not imply per-relay settlement proof.
  - Action: read-only relay/evidence visibility.
- `Log Stream`
  - Replay-safe runtime logs for provider, buyer, wallet, and mirrored trace output.
  - Independent terminal scroll/copy surface rather than an inline Mission Control log box.
  - Hotbar slot `5`.
  - Action: copy all logs.
- `Job Inbox`
  - Deterministic intake pane for incoming NIP-90 requests with stable request IDs and replay-safe ordering.
  - Remains visible while offline in preview mode so the user can see reachable market activity before opting into provider mode.
  - Shows requester, capability, demand source (`open-network` vs `starter-demand`) with a visible source badge or star, price, ttl, validation state, and decision state per request.
  - Offline rows are read-only and visibly marked as preview/unclaimable until the user clicks `Go Online`.
  - Auto-accept is the default provider policy for matching jobs while online.
  - Actions: select request, view why it auto-accepted/rejected, and apply manual accept/reject override only for debug, hold, or policy-tuning cases.
- `Active Job`
  - In-flight job lifecycle pane for one selected job (`received -> accepted -> running -> delivered -> paid`).
  - Source badge is lifecycle projection stream id (`stream.earn_job_lifecycle_projection.v1`), non-authoritative for settlement.
  - Shows append-only execution log events, request demand source, invoice/payment linkage, and failure reason when present.
  - Actions: advance stage, abort job (disabled when runtime lane does not support cancel).
- `Job History`
  - Deterministic receipt/history pane for completed/failed jobs with immutable metadata.
  - Source badge is lifecycle projection stream id (`stream.earn_job_lifecycle_projection.v1`), while wallet reconciliation remains payout truth.
  - Includes status/time filters, job-id search, and pagination.
  - Row model includes `job_id`, `status`, `demand source` with visible marker, `completed timestamp`, `result hash`, `payment pointer`, and unpaid/lost-race reason when present.
- `Agent Profile and State`
  - SA profile/state/goals pane for `39200`, `39201`, and `39203` event visibility.
  - Actions: publish profile, publish encrypted state, update goals snapshot.
  - Explicit pane state machine: `loading`, `ready`, `error`.
- `Agent Schedule and Tick`
  - SA schedule/tick pane for `39202`, `39210`, and `39211` operational controls.
  - Actions: apply heartbeat schedule, publish manual tick request, inspect latest tick result.
  - Explicit pane state machine: `loading`, `ready`, `error`.
- `Trajectory Audit`
  - SA trajectory pane for session/step verification and replay-safe audit visibility.
  - Actions: open session context, cycle step filter, verify trajectory hash.
  - Explicit pane state machine: `loading`, `ready`, `error`.
- `CAST Control`
  - Charms CAST operations pane for status, check/prove/sign/inspect controls, and broadcast safety gating.
  - Actions: refresh status, run check, run prove, run sign/broadcast path, run spell inspection, toggle broadcast armed state.
  - Explicit pane state machine: `loading`, `ready`, `error`.
- `Agent Skill Registry`
  - SKL discovery pane for manifest/version/search tracking (`33400`, `33401`, optional `6390`).
  - Actions: discover skills, inspect manifest, install selected skill version.
  - Explicit pane state machine: `loading`, `ready`, `error`.
- `Skill Trust and Revocation`
  - SKL trust gate pane for trust-tier visibility, attestations, kill-switch, and revocation state.
  - Actions: refresh trust, inspect attestations, toggle kill switch, revoke skill.
  - Explicit pane state machine: `loading`, `ready`, `error`.
- `Credit Desk`
  - AC lifecycle pane for intent/offer/envelope/spend controls (`39240` to `39243`).
  - Actions: publish intent, publish offer, publish envelope, authorize spend.
  - Explicit pane state machine: `loading`, `ready`, `error`.
- `Credit Settlement Ledger`
  - AC settlement/default pane for payout verification and default handling (`39244`, `39245`).
  - Actions: verify settlement, emit default notice, emit reputation label.
  - Explicit pane state machine: `loading`, `ready`, `error`.
- `Nostr Keys (NIP-06)`
  - Shows identity path, `npub`, masked `nsec`, masked mnemonic, and key controls.
  - Secrets are masked by default, reveal is timed, and copy emits explicit custody warning copy.
  - Explicit pane state machine: `loading`, `ready`, `error`.
  - Regenerate immediately triggers dependent wallet refresh.
  - Actions: regenerate keys, reveal/hide secrets, copy `nsec`.
- `Spark Lightning Wallet`
  - Shows wallet connectivity, balances, addresses, invoice creation, payment sending, and recent payment status.
  - All provider earnings settle into this wallet first in MVP; external wallet usage happens through withdraw/pay-invoice flow rather than payout routing configuration.
  - Explicit pane state machine: `loading` (awaiting first refresh), `ready`, `error`.
  - Actions: refresh wallet, generate receive addresses, copy Spark address, create invoice, send payment.
- `Create Lightning Invoice`
  - Dedicated pane for creating receive invoices separate from pay flow.
  - Inputs: invoice sats (required), description (optional), expiry seconds (optional).
  - Outputs: generated invoice text, copy action, and QR payload field.
  - Explicit pane state machine: `loading`, `ready`, `error`.
- `Pay Lightning Invoice`
  - Dedicated payment pane for paying a Lightning invoice/payment request.
  - Inputs: payment request (required), send sats (optional).
  - Available regardless of provider online/offline state; withdraw does not require leaving earn mode first.
  - Explicit pane state machine: `loading`, `ready`, `error`.
  - Action: pay invoice (`Enter` submit and button submit are equivalent).

## Packaged Rive Assets

- Runtime `.riv` assets live under `apps/autopilot-desktop/resources/rive/`.
- The app-owned manifest and deterministic lookup path live in `apps/autopilot-desktop/src/rive_assets.rs`.
- Each manifest entry owns the packaged asset id, runtime path, description, and default artboard/scene handles; the renderer/runtime path stays shared in `wgpui`.
- `Rive Preview` cycles through the manifest with `Prev asset` and `Next asset`; `rive_hud_viewer` accepts `--asset <id>` and `--list-assets`.
- The current registry includes the primary HUD asset plus a second deterministic fixture file so multi-asset bring-up is proven before a distinct second production asset lands.
- `Presentation` always uses the primary `simple_fui_hud` packaged asset and does not expose manifest cycling.
- To add another asset: copy the `.riv` file into `apps/autopilot-desktop/resources/rive/`, add one manifest entry in `apps/autopilot-desktop/src/rive_assets.rs`, and verify it through `Rive Preview` or `cargo run -p autopilot-desktop --bin rive_hud_viewer -- --asset <id>`.

## Source Badges

Panes with mutable status/state render a top-right badge in the form `source: ...`.
Badge semantics:

- `source: runtime`
  - Values are expected from runtime/sync/network lanes.
- `source: wallet`
  - Values are expected from Spark wallet lane/API state.
- `source: local`
  - Values are local app/device state only.
- `source: runtime+wallet`
  - Values combine runtime telemetry and wallet totals.
- `source: runtime+local`
  - Values combine runtime snapshots and local operator state.
- `source: runtime+wallet+local`
  - Values aggregate multiple lanes plus local app events.
- `source: spacetime.sync.lifecycle`
  - Values are derived from sync lifecycle/apply telemetry state.
- `source: stream.*`
  - Values are backed by replay-safe projection stream rows for the named stream id.

Current pane badge mapping:

- `Autopilot Chat`: `source: local`
- `Project Ops`: `source: stream.pm.work_items.v1`
- `Codex Account`: `source: codex`
- `Codex Models`: `source: codex`
- `Codex Config`: `source: codex`
- `Codex MCP`: `source: codex`
- `Codex Apps`: `source: codex`
- `Codex Labs`: `source: codex`
- `Codex Diagnostics`: `source: codex`
- `Provider Control`: `source: runtime`
- `Provider Status`: `source: runtime`
- `Tailnet Status`: `source: tailnet`
- `GPT-OSS Workbench`: `source: runtime`
- `Psionic Mesh`: `source: runtime`
- `Earnings & Jobs`: `source: runtime+wallet+receipts`
- `Relay Connections`: `source: runtime`
- `Sync Health`: `source: spacetime.sync.lifecycle`
- `Network Requests`: `source: runtime`
- `Buy Mode`: `source: buy`
- `Seller Earnings Timeline`: `source: stream.nip90_payment_facts.v1`
- `Settlement Ladder`: `source: stream.nip90_payment_facts.v1`
- `Key Ledger`: `source: stream.nip90_payment_facts.v1`
- `Settlement Atlas`: `source: stream.nip90_payment_facts.v1`
- `Spark Replay`: `source: stream.nip90_payment_facts.v1`
- `Relay Choreography`: `source: runtime.relay_connections + stream.nip90_payment_facts.v1`
- `Log Stream`: `source: log`
- `Starter Jobs`: `source: runtime`
- `Activity Feed`: `source: stream.activity_projection.v1`
- `Alerts and Recovery`: `source: runtime`
- `Settings`: `source: local`
- `Job Inbox`: `source: runtime`
- `Active Job`: `source: stream.earn_job_lifecycle_projection.v1`
- `Job History`: `source: stream.earn_job_lifecycle_projection.v1`
- `Nostr Keys (NIP-06)`: `source: local`
- `Agent Profile and State`: `source: runtime`
- `Agent Schedule and Tick`: `source: runtime`
- `Trajectory Audit`: `source: runtime`
- `CAST Control`: `source: runtime+local`
- `Agent Skill Registry`: `source: runtime`
- `Skill Trust and Revocation`: `source: runtime`
- `Credit Desk`: `source: runtime`
- `Credit Settlement Ledger`: `source: runtime`
- `Spark Lightning Wallet`: `source: wallet`
- `Create Lightning Invoice`: `source: wallet`
- `Pay Lightning Invoice`: `source: wallet`

## Opening Panes

- Hotbar:
  - `1` opens `Provider Control`.
  - `2` opens `Nostr Keys (NIP-06)`.
  - `3` opens `Spark Lightning Wallet`.
  - `4` opens `Earnings & Jobs`.
  - `5` opens `Log Stream`.
  - `K` opens the command palette.
- Command Palette (`K`):
  - `Autopilot Chat` -> opens `Autopilot Chat`.
  - `Codex Account` -> opens `Codex Account`.
  - `Codex Models` -> opens `Codex Models`.
  - `Codex Config` -> opens `Codex Config`.
  - `Codex MCP` -> opens `Codex MCP`.
  - `Codex Apps` -> opens `Codex Apps`.
  - `Codex Labs` -> opens `Codex Labs`.
  - `Codex Diagnostics` -> opens `Codex Diagnostics`.
  - `Provider Control` -> opens `Provider Control`.
  - `Provider Status` -> opens `Provider Status`.
  - `Tailnet Status` -> opens `Tailnet Status`.
  - `GPT-OSS Workbench` -> opens `GPT-OSS Workbench`.
  - `Psionic Mesh` -> opens `Psionic Mesh`.
  - `Presentation` -> opens `Presentation`.
  - `Earnings & Jobs` -> opens `Earnings & Jobs`.
  - `Relay Connections` -> opens `Relay Connections`.
  - `Sync Health` -> opens `Sync Health`.
  - `Network Requests` -> opens `Network Requests`.
  - `Buy Mode` -> opens `Buy Mode`.
  - `Log Stream` -> opens `Log Stream`.
  - `Starter Jobs` -> opens `Starter Jobs`.
  - `Activity Feed` -> opens `Activity Feed`.
  - `Alerts and Recovery` -> opens `Alerts and Recovery`.
  - `Settings` -> opens `Settings`.
  - `Job Inbox` -> opens `Job Inbox`.
  - `Active Job` -> opens `Active Job`.
  - `Job History` -> opens `Job History`.
  - `Agent Profile and State` -> opens `Agent Profile and State`.
  - `Agent Schedule and Tick` -> opens `Agent Schedule and Tick`.
  - `Trajectory Audit` -> opens `Trajectory Audit`.
  - `CAST Control` -> opens `CAST Control`.
  - `Agent Skill Registry` -> opens `Agent Skill Registry`.
  - `Skill Trust and Revocation` -> opens `Skill Trust and Revocation`.
  - `Credit Desk` -> opens `Credit Desk`.
  - `Credit Settlement Ledger` -> opens `Credit Settlement Ledger`.
  - `Identity Keys` -> opens `Nostr Keys (NIP-06)`.
  - `Spark Wallet` -> opens `Spark Lightning Wallet`.
  - `Create Lightning Invoice` -> opens `Create Lightning Invoice`.
  - `Pay Lightning Invoice` -> opens `Pay Lightning Invoice`.

## Behavior Notes

- Chat, Codex Account, Codex Models, Codex Config, Codex MCP, Codex Apps, Codex Labs, Codex Diagnostics, Provider Control, Provider Status, Tailnet Status, GPT-OSS Workbench, Psionic Mesh, Rive Preview, Presentation, Relay Connections, Sync Health, Network Requests, Earnings & Jobs, Buy Mode, Log Stream, Starter Jobs, Activity Feed, Alerts and Recovery, Settings, Job Inbox, Active Job, Job History, Agent Profile and State, Agent Schedule and Tick, Trajectory Audit, CAST Control, Agent Skill Registry, Skill Trust and Revocation, Credit Desk, Credit Settlement Ledger, identity, wallet, create-invoice, and pay-invoice panes are singletons: opening again brings the existing pane to front.
- Wallet worker updates are shared across wallet-related panes.
- When a new invoice is created in the wallet pane, that invoice is prefilled into send/payment request inputs.
- Fullscreen pane presentation is reversible: invoking close on a fullscreen pane demotes it back to windowed before a second close removes it.

## Codex Tool Control

Panes and CAD can be manipulated through Codex `item/tool/call` requests in the `openagents.*` namespace.

Supported tool names:

- `openagents.pane.list`
- `openagents.pane.open`
- `openagents.pane.focus`
- `openagents.pane.close`
- `openagents.pane.set_input`
- `openagents.pane.action`
- `openagents.cad.intent`
- `openagents.cad.action`

Execution behavior:

- `openagents.*` calls are auto-executed by desktop and auto-responded through Codex lane.
- Non-OpenAgents tool calls remain pending for manual response.

Full contract and examples:

- [`docs/codex/CODEX_PANE_CAD_TOOLING.md`](/Users/christopherdavid/code/openagents/docs/codex/CODEX_PANE_CAD_TOOLING.md)
- [`docs/codex/CAD_CHAT_BUILD_IMPLEMENTATION.md`](/Users/christopherdavid/code/openagents/docs/codex/CAD_CHAT_BUILD_IMPLEMENTATION.md)
- [`docs/codex/CAD_CHAT_BUILD_RELEASE_RUNBOOK.md`](/Users/christopherdavid/code/openagents/docs/codex/CAD_CHAT_BUILD_RELEASE_RUNBOOK.md)
