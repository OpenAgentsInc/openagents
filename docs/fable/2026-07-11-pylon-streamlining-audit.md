# Pylon implementation audit — streamlining recommendations

- Date: 2026-07-11
- Author: Fable (workspace audit lane)
- Status: audit + recommendation input; not an execution claim
- Scope: `apps/pylon`, `packages/pylon-core`, the bundled
  `@openagentsinc/pylon-runtime` (Probe port), the server-side Pylon surfaces
  in `apps/openagents.com/workers/api`, and every legacy satellite that still
  carries the Pylon name
- Alignment targets:
  - [`docs/sol/MASTER_ROADMAP.md`](../sol/MASTER_ROADMAP.md) (rev 33)
  - [`docs/teardowns/2026-07-10-openagents-product-adaptation-analysis.md`](../teardowns/2026-07-10-openagents-product-adaptation-analysis.md)
    (the product recs doc), especially its "Pylon and local runtimes" section
  - [`2026-07-08-pylon-into-khala-code-proposal.md`](./2026-07-08-pylon-into-khala-code-proposal.md)
    (accepted, then partially superseded by the Sol reset — see §2)
- Open-issue context reviewed: #8566/#8574/#8597 (program/clients), #8640
  (Phase A live burn), #8686 CUT-06 (supervisor scope/ordering, deterministic
  fix landed at `d98abda795`, live receipt open), #8547 (Agent Computer),
  #8636 (owner-local vs managed-remote routing), CUT-01…CUT-27 plan in
  [`docs/sol/2026-07-11-openagents-coding-cutover-issue-plan.md`](../sol/2026-07-11-openagents-coding-cutover-issue-plan.md)

## 0. Executive summary

Execution update (2026-07-11): the #8640/CUT-06 burn is accepted and CUT-07
[#8687] followed this audit as the next short-term slice. CUT-07 changed only
the existing Khala Sync command ledger, projection, reader, and Desktop/mobile
adapters; it did not touch `apps/pylon/src/orchestration` or `src/node`. The
broader streamlining program below remains separately scoped and must not be
silently mixed into CUT-08/CUT-09 reliability work.

Pylon's fleet-execution core is real, current, and load-bearing: the
`orchestration/` + `node/` substrate (orchestration.sqlite runtime, mixed-kind
supervisor, durable planner, named-account capacity authority, exact
claimed-work runners, remote-intake seam, standing executor) is exactly the
"Pylon-owned" landed truth the Sol roadmap enumerates, and it is under active
P0 burn today (CUT-05/CUT-06 landed 2026-07-11).

Around that core, the implementation carries three distinct classes of
baggage:

1. **Dead product eras shipped inside the live npm package** — Spark/MDK
   wallet + tips + multi-earning (~10k LOC; wallet is owner-preserved but not
   isolated), NIP-90/nostr labor market, archived Psionic/Tassadar/training
   lanes, and Autopilot/Sarah-era social commands (`forum`, `ask-artanis`,
   `memories`) — all still wired into a 5,671-LOC hand-rolled `src/index.ts`
   dispatcher.
2. **A split-brain server authority** — the old D1 `pylon_api_*`
   assignment/presence/event ledger and the new Postgres `fleet_run_*`
   owner-scoped authority are both live, held side by side in
   `pylon-api-routes.ts`, with `pylon-codex-turn-ingest-routes.ts` actively
   double-writing both stacks, plus a third parallel delegation path (the
   `mutalisk-khala-delegation` gym routes).
3. **Structural drift from the product recs** — the recs doc says "Pylon
   should become the stable multi-engine supervisor behind Desktop rather
   than a second product shell," with a machine-readable lifecycle record,
   versioned protocol, and scoped capabilities. Today Pylon is still shaped
   like a standalone contributor product: its own cockpit-ish web routes, its
   own MCP path duplicating the frozen desktop's, a vendored bundle inside
   the deprecated `autopilot-desktop`, and a half-finished `pylon-core`
   extraction (PY-1 closed not-planned mid-flight).

The recommendation is a four-phase burn-down that never blocks the P0
CUT/#8640 lanes: (0) hygiene, (1) delete/quarantine dead product eras from
the package, (2) collapse the server to one FleetRun authority, (3) finish
the engine-shape work the recs doc requires (pylon-core completion, daemon
lifecycle record, one protocol, one MCP surface). Owner gates are listed
explicitly in §6.

## 1. Inventory — what "Pylon" is today

| Surface | Path | Size | State |
| --- | --- | --- | --- |
| CLI/daemon app | `apps/pylon/src` | 306 files, ~93.7k LOC (123 files flat at root; `index.ts` alone 5,671 LOC) | Live, published `@openagentsinc/pylon` 1.0.5 |
| Fleet core | `apps/pylon/src/orchestration` (+`src/node`) | ~29.7k + ~9.8k LOC | **Current valuable center**, active P0 burn |
| Extracted engine | `packages/pylon-core` | ~15.5k LOC (custody/executor/presence/rpc/shared) | Live; `apps/pylon` root files are largely re-export shims over it |
| Bundled runtime | `apps/pylon/packages/runtime` (`@openagentsinc/pylon-runtime`) | ~22.8k LOC, 101 files | Live (Apple-FM/Gemini backends, blueprint contracts, token usage) — direct Probe port with benchmark/GEPA weight |
| Apple FM bridge | `apps/pylon/swift/foundation-bridge` | 4 real files + **143 committed `.build/` cache files** | Live bridge, dirty tree |
| Tests | `apps/pylon/tests` + co-located + runtime | ~300 files, ~60k+ LOC | Mirror-of-src; includes tests for dead surfaces |
| Server routes | `apps/openagents.com/workers/api/src` (`pylon-api.ts`, `pylon-api-routes.ts`, `pylon-codex-turn-ingest-routes.ts`, `sarah-fleet-run-routes.ts`, `provider-account-pylon-routes.ts`, …) | ~12.3k LOC inside a 17.8k-LOC mega-router `index.ts` | Live, **dual D1/Postgres authority** (§3) |
| FleetRun authority | `packages/khala-sync-server/src/fleet-run-authority.ts` | ~3.6k LOC (+3.6k tests) | Live Postgres successor authority |
| Shared vocabulary | `packages/khala-fleet-intents` | ~1.2k LOC | Well-consolidated single intent vocabulary — the model to copy |
| Web routes | `apps/openagents.com/apps/start/src/routes/pylon/*`, `routes/pylons.tsx` | small | Legacy cockpit/stats surfaces outside the three-product rule |
| Vendored copy | `apps/autopilot-desktop/resources/pylon-node` (+ built `.app`s) | 4.1 MB bundle ×2 (untracked) | Deprecated-era Electrobun shell vendoring Pylon |
| OTA seed | `apps/oa-updates/pylon-dist` | **10 GB on disk** (untracked, only `.gitignore` tracked) | Live OTA mechanism; stale local asset accumulation |
| Docs | `apps/pylon/docs` (97 md), `docs/pylon` (3 md), `apps/openagents.com/docs/pylon` (26 md) | — | Root set current; app set carries probe-port/benchmark/proof archives; the openagents.com set is a **different, energy-era "Pylon"** (§4.6) |

Deprecated-client coupling: `clients/khala-code-desktop` is heavily entangled
(≈2,350 pylon lines across 98 files, including the `pylon-service.ts`
stdout-subprocess seam the PY-1 proposal wanted deleted); `clients/khala-cli`
moderately (dispatch strategy targeting Pylon capacity); `khala-mobile`/
`khala-ios` lightly; `khala-macos` and the deleted Electrobun
`openagents-desktop` stub not at all. All of these are frozen extraction
sources under the Sol greenfield decision, so the coupling is inert — but it
means "grep for consumers" overstates the live surface.

The greenfield `apps/openagents-desktop` reaches the fleet the right way
already: through Khala Sync schemas and the Runtime Gateway, plus a local
perimeter for Codex custody/device-auth (`codex-connect-host`,
`fleet-stage-control` in `service-topology.ts`). It does not consume
`/api/pylon/*` HTTP directly and holds no `pylon-core` dependency. That is
the destination pattern; nothing in the streamlining below may break it.

## 2. Where the roadmap already decided, and what changed under it

The 2026-07-08 fold-in proposal was accepted with four owner gates, then the
Sol reset re-cut the destination:

- **PY-1 (#8578, pylon-core extraction + daemon + typed RPC)** — closed
  `not planned`: "Superseded as an independent refactor program. Landed
  pylon-core packages remain." The extraction is therefore **half-done by
  decision**, not by accident: `packages/pylon-core` exists and is consumed,
  but `apps/pylon` still carries ~45k LOC of flat root modules and the
  monolith dispatcher.
- **PY-2 (#8579, Khala Code desktop cockpit)** — closed `not planned`, folded
  into OpenAgents Desktop #8574 ("one deep Fleet cockpit… not a separate
  Pylon product surface"). Khala Code desktop is now itself frozen, so its
  duplicated fleet MCP bridges and the `pylon-service.ts` subprocess seam are
  stranded legacy, not competing implementations.
- **PY-3 (#8580, OpenTUI retirement)** — closed `not planned`, deferred until
  #8574 parity. In practice the TUI is already effectively gone from
  `apps/pylon` (one vestigial `opentui` reference in `src/node/keybinds.ts`);
  the deferred issue is a docs/claims cleanup, not a code mountain.
- **Wallet gate** — the owner explicitly preserved the Spark wallet as a live
  rail ("so we can do cool shit with that"). Any streamlining plan must treat
  wallet **isolation** as the goal, never silent deletion. Note the standing
  rail policy: Spark is primary for agent/MPP payments; MDK is
  checkouts-secondary. (The in-tree `legacy-spark-wallet-migration.md` /
  `legacy-mdk-balance-recovery.md` docs describe storage/recovery migrations,
  not a Spark deprecation — do not misread them as one.)
- **Sol rev 31 boundary decisions that bind this audit:** the Desktop Runtime
  Gateway "is not a new public server or second Pylon" (decision 13); "Pylon
  is an engine, not a separate public desktop product or a second local
  authority" (D-track boundary); session identity never derives from the
  Pylon home (decision 18); provider/SCM credentials move behind a capability
  broker (decision 20). Streamlining must push Pylon toward
  engine-behind-gateway, not refresh its standalone product shell.

## 3. Findings

### 3.1 The fleet core is healthy and should be treated as untouchable during P0

`src/orchestration` + `src/node` contain everything the roadmap's "current
implementation truth" list attributes to Pylon: `store.ts` (4,948 LOC,
schema v12, bun:sqlite) as the canonical Pylon-home runtime;
`fleet-run-supervisor.ts` / `runtime-intent-supervisor.ts` (CUT-06
scope-fencing just landed); durable planner; owned capacity authority; exact
owned/Grok/managed-cloud runners; remote-intake + HTTP intake + intake
poller; standing executor; steering consumer/dispatcher; execution reporter
and receipts; the control server with typed `fleet_run.arm`/`fleet_run.disarm`
verbs. A store-bypass guard script runs before every test sweep. This is the
part that matches the product recs' target ("stable multi-engine supervisor")
and it is mid-burn for #8640/CUT — **no streamlining change should enter
these directories except through the CUT lanes that own them.**

Two structural debts inside the healthy core are worth naming for the
post-#8640 window (they are shape problems, not correctness problems):

- `src/index.ts` (5,671 LOC) dispatches by hand-rolled `args[0] === "…"`
  string matching over ~40 top-level commands. The machine-readable
  `cli-catalog.ts` exists, but dispatch, help, and catalog can drift
  independently.
- 123 modules sit flat at `src/` root — a mix of pylon-core re-export shims,
  live glue, and dead-era code — which makes the live surface illegible and
  is the main reason this audit needed a fine-toothed inventory at all.

### 3.2 Dead product eras are still wired into the shipped CLI

All of the following are reachable commands in the published
`@openagentsinc/pylon` binary today (verified in `src/index.ts` dispatch):

| Era | Commands | Backing modules (LOC) | Disposition signal |
| --- | --- | --- | --- |
| Payments/earnings | `wallet`, `balance`, `tip`, `tip-prefs`, `sweep-status`, `claim-tip-readiness`, `multi-earning` | `wallet.ts` (2,879), `spark-backup-helper.ts` (2,008), `spark-bun-storage.ts` (1,432), `spark-wasm-runtime.ts`, `generated/spark-wasm-b64.ts` (embedded WASM), `tips.ts`, `multi-earning-ledger.ts`; optional deps `@breeztech/breez-sdk-spark`, MDK shell-outs | Wallet: **owner-preserved rail** — isolate, don't delete. Tips/multi-earning: postponed-program remnants |
| Compute market | `provider` (go-online/approve-labor), NIP-90 serving | `provider-nip90.ts` (1,341), `labor.ts`, `labor-market.ts`, `stranger-probe.ts`, `nostr-identity.ts`; deps `@openagentsinc/nip90` + noble/scure crypto stack | Postponed program (P6 in the fold-in proposal); code dormant but shipped |
| Retired ML lanes | `training`, `psionic`, `tassadar cpu-transform-training` | `archived-psionic-qwen.ts`, `archived-tassadar-executor.ts` (both explicitly stubbed with `.archived` refs) | #8577 retirement executed the big removal but left these stubs + catalog entries |
| Sarah/Autopilot social | `forum`, `ask-artanis`, `memories` | dispatch at `index.ts:3711` + social modules; dep `autopilot-control-protocol` | Sarah surface removed at owner direction 2026-07-10; these are stranded feeders |
| Probe-era experiments | — | `frlm-conductor-execution.ts` (1,101), `gepa-capability.ts`, plus `packages/runtime` benchmark/GEPA/omega subtrees | No current consumer identified |

Cost of keeping them: they dominate the flat root file count, hold heavy
dependencies in the published package (Breez WASM blob, nostr crypto stack,
Autopilot protocol), pull ~10k+ LOC of paired tests through every
`bun test --max-concurrency=1` sweep, and make the CLI catalog read like four
abandoned products stapled to a fleet engine. The launch-gate policy
(`launch-gates.ts`) still gates releases on wallet/MDK/settlement evidence
refs, so payments-era code is also coupled into the release path of a
fleet-engine package.

### 3.3 The server has two live Pylon authorities plus a third delegation path

This is the highest-value correctness-adjacent finding:

1. **Old D1 ledger:** `pylon-api.ts` (3,108 LOC, D1-only) owns
   `pylon_api_assignments`, `pylon_api_events`, `pylon_api_presence`,
   `pylon_api_registrations`, `pylon_api_quarantines`.
2. **New Postgres authority:** `khala-sync-server/fleet-run-authority.ts`
   owns owner-scoped `fleet_run_*` (requests, intake claims/leases, work
   units, attempts, execution events/batches/acks, steering, usage evidence,
   control) — the FC-1/FC-3 substrate the roadmap calls closed.
3. **Both at once:** `pylon-api-routes.ts` (3,672 LOC) imports both schema
   families (≈35 old-assignment refs vs ≈31 fleet-run refs) and
   `pylon-codex-turn-ingest-routes.ts` (3,640 LOC) **double-writes** D1
   (`pylon_codex_raw_events`, `pylon_codex_raw_event_chunks`,
   `token_usage_events`, `agent_traces`) and the Postgres attempt authority
   in the same handlers.
4. **Third path:** the older Khala delegation "gym" surface
   (`/api/admin/khala/cloud/runtime-dispatch`, `/api/khala/cloud/
   runtime-turn-*`, `inference/gym/mutalisk-khala-delegation-*`) is a
   parallel dispatch authority predating FleetRun.
5. **Presence/capacity split:** D1 `pylon_api_presence` heartbeat + public
   capacity funnel run alongside the named-account lease/capacity authority
   (`/api/operator/provider-accounts/chatgpt-codex/leases*`) and the
   fleet-intents capacity rows.

The Cloudflare-exit context makes this sharper: the "D1" tables now live in
whatever the monolith's D1-compat layer is post-GCP-evacuation, meaning the
legacy stack is a compatibility shim on top of a migration on top of a
deprecated authority. Every new feature that touches both stacks (as the
turn-ingest routes do) doubles its failure modes — and the roadmap's own Sync
laws ("Server/Pylon authority decides claims… Khala Sync distributes typed
projections") assume exactly one authority per fact.

Sarah-named intake (`/api/sarah/fleet-runs`,
`sarah.coding_fleet_start.request.v1`) is explicitly retained by roadmap
decision 2 as the landed adapter for desktop/mobile FleetRun intake; it is
not cleanup-eligible, but new client state must target persona-neutral
contracts per the wontdo boundary.

### 3.4 Duplicated integration seams

- **MCP:** Pylon ships `khala-mcp`/`mcp-contract-import`; the frozen Khala
  Code desktop ships its own `khala-fleet-mcp-server` plus codex/claude
  fleet MCP bridges. The fold-in proposal already ruled: one MCP surface.
  With Khala Code frozen, the consolidation direction flips from the
  proposal's ("desktop owns it") to Pylon/engine owns it, with the desktop as
  a client through the Runtime Gateway.
- **Subprocess seam:** `khala-code-desktop/src/bun/pylon-service.ts` spawns
  the Pylon CLI and parses stdout wire-events — the exact "stringly" seam
  PY-1 was filed to delete. It survives only inside a frozen client; it must
  not be ported into `apps/openagents-desktop` (which currently, correctly,
  has no such seam).
- **Vendored engine:** `apps/autopilot-desktop` bundles a 4.1 MB
  `pylon-node/index.js` build (twice, counting the built `.app`), which is a
  second, version-skewed distribution channel of the engine inside a
  deprecated shell.

### 3.5 Distribution and tree hygiene

- 143 committed Swift `.build/` compiler-cache files under
  `swift/foundation-bridge/` (should be gitignored).
- `apps/oa-updates/pylon-dist` has accumulated **10 GB** of untracked local
  release assets; the OTA mechanism is live and correct, the local
  accumulation is pure disk waste.
- `apps/autopilot-desktop/{build,artifacts,resources/pylon-node}` holds
  ~211 MB of untracked build output.
- `openagents-worktrees/` did not register as gitignored in a
  `git check-ignore` probe (unlike `.worktrees/` and `.pylon*/`, which are
  correctly ignored) — verify and add an ignore rule.
- `apps/pylon` ships `src`, swift sources, runtime `src`, **and docs** to npm
  in `files`, so every stale doc rides every publish.

### 3.6 Docs: two unrelated "Pylons" and a large historical tail

- `docs/pylon/` (root, 3 files) is current — CUT-05/CUT-06 receipts.
- `apps/pylon/docs/` (97 files) mixes living runbooks with dated point-in-time
  audit snapshots (`2026-06-0x…`), the complete `probe-port/**` port-history
  archive, `benchmarks/**` (GEPA-era), and `proofs/m10-live-*/**`.
- `apps/openagents.com/docs/pylon/` (26 files, all 2026-06-06→08) documents a
  **different product concept** — the energy/interconnection/LDK-wallet
  "Pylon" era (power interconnection, flexible load, LSP, VSS). It collides
  on the name with the coding-fleet Pylon and predates the current program.

### 3.7 Web routes outside the product boundary

`apps/openagents.com/apps/start/src/routes/pylon/codex/assignments/…` and
`routes/pylons.tsx` (install/stats page over `/api/public/pylon-stats`) are
live-wired legacy cockpit surfaces. The Sol/product rule retains `/`,
`/forum`, `/promises` (plus infra exceptions); these routes — along with the
other non-product route folders observed (`autopilot`, `sites`, `training`,
`tassadar`, `artanis`, `gym`, …) — sit outside that boundary. They are
route-level cleanup, owner-gated only insofar as public URLs die.

### 3.8 Alignment gaps against the product recs doc

The recs doc's Pylon section is a concrete checklist. Current state:

| Recs requirement | Current state | Gap |
| --- | --- | --- |
| Stable multi-engine supervisor behind Desktop, not a second product shell | Supervisor core real; shell still standalone-product shaped (own web routes, own MCP, contributor framing) | Shape, not capability |
| Machine-readable lifecycle record (executable identity/version, protocol version, process generation, socket, readiness, update state, worker epoch, last transition) | Node control server + presence exist, but lifecycle facts are spread across presence heartbeat, `status`, launch gates, and self-update; no single typed record | Build the record; roadmap decision "daemon lifecycle is product state" (Codex teardown) |
| Idempotent start; serialized lifecycle mutations; bounded receipted stop/update/rollback | Self-update exists; no receipted rollback ledger | Fold into component-compatibility-ledger work (recs §7) |
| Discover/authenticate/version-check/elect/restart one compatible managed local service without stale PID/socket ambiguity | Known operational footgun (stale loopback node answering with old source; `PYLON_DISABLE_DAEMON_ROUTING=1` workaround documented in the repo runbook) | Election/version-handshake needed |
| Scoped client/device capabilities replacing shared local secret | Control server uses a local control token | Capability-scoped grants per client (also feeds roadmap decision 20 broker) |
| Typed quota/auth/rate-limit failures; named isolated account custody | **Done** — named-account capacity authority + custody in pylon-core | — |
| One conversation service shared by interactive/headless/SDK/remote adapters | Executors are harness-specific; no unified conversation service seam | Post-P0; owned by CUT lanes (#8691 agent graph, #8690 gateway events) |
| Local/remote transport adapters behind stable runtime identities | Remote intake + HTTP intake exist; identities are run/claim-scoped, host identity still implicit | Ties to roadmap decision 18 (host-independent sessions) |
| No provider-specific sidecar flags exposed to renderers/mobile | Holds today (desktop gateway is typed) | Keep as an oracle |

## 4. Recommendations

Ordered so that nothing blocks the active P0 CUT burn or the #8640 receipt.
Each phase is independently shippable.

### Phase 0 — hygiene (no behavior change, this week)

1. Gitignore and delete the committed `swift/foundation-bridge/.build/`
   cache (143 files).
2. Add/verify an ignore rule for `openagents-worktrees/`.
3. Delete `archived-psionic-qwen.ts`, `archived-tassadar-executor.ts`, their
   catalog entries (`tassadar cpu-transform-training`, `psionic`,
   `training`), and paired tests — they are self-labeled stubs.
4. Local disk (non-git): prune stale `apps/oa-updates/pylon-dist` assets
   (10 GB) and `apps/autopilot-desktop` build output (~211 MB).
5. Docs: move `apps/pylon/docs/probe-port/**`, `benchmarks/**`,
   `proofs/m10-live-*/**`, and the dated `2026-06-0x` snapshots to an
   `archive/` subtree (or backroom per convention); archive
   `apps/openagents.com/docs/pylon/` wholesale with a README note that it
   documents the retired energy-era Pylon concept, not the coding fleet.
6. Stop shipping `docs/` in the npm `files` array.

### Phase 1 — remove dead product eras from the shipped package (owner-gated items flagged)

1. **Delete the Sarah/Autopilot social commands** (`forum`, `ask-artanis`,
   `memories`) and the `autopilot-control-protocol` dependency. These feed a
   surface the owner removed on 2026-07-10.
2. **Quarantine the compute-market lane** (`provider` NIP-90 serving,
   `labor*`, `stranger-probe`, `multi-earning`, `tips`): move to a clearly
   named `src/postponed/` boundary or archive to backroom per the P6 Wave-4
   gate. Either way, remove the commands from the default CLI catalog and the
   noble/scure/nip90 deps from the default dependency set. `packages/nip90`
   and `apps/nostr-relay` stay (protocol library and relay are independent
   surfaces); only Pylon's dormant provider loop moves. **Owner gate:
   dormant-in-tree vs backroom** (unresolved gate 4 from the fold-in
   proposal).
3. **Isolate the wallet as its own service boundary** — this is the
   owner-preserved rail, so the move is extraction, not deletion: pull
   `wallet.ts`, `spark-*`, `breez-stdout-guard.ts`, and the embedded WASM
   blob into a dedicated package (e.g. `packages/pylon-wallet`) consumed by
   the daemon behind one typed service, per the fold-in proposal's P5 ruling
   ("its own service boundary inside the engine daemon (never in the GUI
   process)"). Make it an optional capability so the fleet engine, tests,
   and npm package don't carry Breez/MDK weight when custody isn't enabled.
   Untangle `launch-gates.ts` so fleet-engine releases don't gate on wallet
   evidence unless the wallet capability ships in that release.
4. Delete `frlm-conductor-execution.ts` / `gepa-capability.ts` and trim the
   `packages/runtime` benchmark/GEPA/omega subtrees after a consumer check —
   keep the Apple-FM/Gemini backends, blueprint contracts, and token-usage
   modules that are genuinely load-bearing.
5. Expected effect: roughly 15–20k LOC of src plus paired tests and several
   heavy deps out of the published package; the CLI catalog contracts to the
   fleet-engine surface (`fleet connect/status`, accounts, presence, node,
   khala, assignment, sessions, doctor/inventory, deploy, mcp, evidence).

### Phase 2 — one server authority (highest-value consolidation; sequence after #8640 Phase A receipt)

1. Declare Postgres `fleet_run_*` (via `fleet-run-authority.ts`) the sole
   coding-fleet authority — this is already the roadmap's stated spine.
2. Migrate the residual D1 `pylon_api_assignments`/`pylon_api_events`
   consumers in `pylon-api-routes.ts` onto work-unit/attempt records; keep
   the D1 ledger read-only for audit until a dated deletion gate, per the
   recs doc's rule that every compatibility layer gets an explicit deletion
   gate.
3. End the double-write in `pylon-codex-turn-ingest-routes.ts`: exact token
   rows (`token_usage_events`) and owner-only traces keep their store, but
   the assignment/attempt spine gets one writer; raw-event archives move
   behind the attempt authority reference rather than a parallel keyspace.
4. Retire the `mutalisk-khala-delegation` gym path
   (`/api/admin/khala/cloud/runtime-dispatch`, `runtime-turn-*`) or rebase it
   on FleetRun intake — it predates the authority and is a third dispatch
   truth.
5. Unify presence: fold D1 `pylon_api_presence` heartbeat state into the
   named-account capacity authority so "is this Pylon a dispatch target"
   has one answer; keep `/api/public/pylon-stats` and the capacity funnel as
   projections of it.
6. Split the pylon route families out of the 17.8k-LOC `workers/api/index.ts`
   mega-router into mounted modules while touching them (mechanical, but do
   it with the migration, not as a separate churn pass).

### Phase 3 — engine shape per the product recs (rides the CUT lanes)

1. **Finish the pylon-core consolidation PY-1 started:** `apps/pylon` becomes
   a thin CLI + daemon over `packages/pylon-core` (+ orchestration moved into
   core or a sibling `pylon-orchestration` package); kill the 123-flat-file
   layer by relocating live glue into feature folders and deleting shims once
   imports point at core. Replace the hand-rolled `args[0]` dispatcher with a
   catalog-driven registry so `cli-catalog.ts` is the single source for
   dispatch, help, and JSON output.
2. **Publish the machine-readable daemon lifecycle record** the recs doc
   requires (identity/version, protocol version, process generation,
   socket/transport, readiness, update state, worker epoch, last
   transition), and make `pylon node` startup idempotent with
   version-handshake election so the stale-loopback-node footgun (and its
   `PYLON_DISABLE_DAEMON_ROUTING=1` workaround) disappears.
3. **One MCP surface:** consolidate to a single engine-owned fleet MCP
   server; delete Pylon's duplicate pathway or the frozen desktop's bridges
   (whichever loses), and route desktop consumption through the Runtime
   Gateway rather than a second MCP client.
4. **Scoped capabilities instead of one local control token,** aligned with
   roadmap decision 20's capability broker: per-client/device grants for the
   control server, so desktop, CLI, and future mobile attach with distinct
   revocable authority.
5. Keep the boundary oracles: desktop renderer stays tokenless; no
   provider-specific sidecar flags reach renderers/mobile; session identity
   never derives from Pylon home (decision 18).

### Phase 4 — owner-gated retirements

1. Web routes: delete `routes/pylon/*` and `routes/pylons.tsx` (and decide
   the other non-product route folders) under the three-product rule; public
   URLs die, so this is an explicit owner call.
2. `apps/autopilot-desktop`: no tombstone exists but every signal says
   deprecated (private v0.0.1, vendored Pylon bundle, no consumers). Ask for
   the tombstone, then remove the vendored `pylon-node` build lane.
3. PY-3 residue: with OpenTUI already effectively out of the tree, close the
   loop by deleting the vestigial `node/keybinds.ts` reference and recording
   the retirement receipt when #8574 parity lands.
4. Naming (gate 3 of the fold-in proposal): "Pylon" stays per the owner's
   2026-07-08 ruling; nothing here renames anything.
5. Earning rails final disposition (gate 4): dormant-in-tree vs backroom for
   the quarantined P6 lane from Phase 1.

## 5. What this audit explicitly protects

- The `orchestration/` + `node/` fleet core and everything the CUT-01…CUT-27
  lanes own — no refactor enters those paths outside the owning issues while
  #8640/CUT is burning.
- The Spark wallet rail (owner-preserved 2026-07-08) — isolation and optional
  packaging only; no deletion, no rail-policy change.
- `/api/sarah/fleet-runs` FleetRun intake (roadmap decision 2 keeps it in
  force for the clients) and the `khala fleet connect` community front door.
- npm `@openagentsinc/pylon` continuity (published-package users keep
  working; Phase 1 changes the surface via deprecation-listed removals, so it
  is a major-version event when it ships).
- `packages/khala-fleet-intents` as the single cross-tier vocabulary, and
  `packages/nip90` / `apps/nostr-relay` as independent protocol surfaces.
- Exact-usage accounting invariants: `token_usage_events` rows stay
  exact-only through any store consolidation; counters remain projections.

## 6. Decision asks (NEEDS_OWNER candidates)

1. Phase 1.2 — compute-market/earning lane: dormant-in-tree quarantine vs
   backroom archive.
2. Phase 2 timing — confirm the D1→Postgres collapse waits for the #8640
   Phase A receipt (recommended) vs starting the read-only D1 freeze now.
3. Phase 4.1 — retire the public `/pylons` + `/pylon/...` web routes.
4. Phase 4.2 — tombstone `apps/autopilot-desktop`.
5. Phase 1 npm major — approve shipping the contracted CLI surface as the
   next major of `@openagentsinc/pylon`.

## 7. Method note

Evidence gathered 2026-07-11 from the repo at `e4ac9fb931` via three
parallel read-only exploration lanes (apps/pylon structure; server seams and
shared packages; legacy-baggage sweep), reconciled against MASTER_ROADMAP
rev 31, the product adaptation analysis, the fold-in proposal and its
PY-1/2/3 closure records, and the open issue set (#8547, #8566, #8574,
#8597, #8636, #8640, #8676/#8677, #8686–#8707). LOC figures are `wc -l`
approximations for orientation, not billing-grade metrics. This document
recommends; the Sol roadmap and owning issues decide.
