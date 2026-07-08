# Proposal: fold Pylon into Khala Code as the primary surface

Date: 2026-07-08
Status: **ACCEPTED (owner, 2026-07-08).** Gate resolutions: (1)
daemon-cockpit shape (§3 rec C) — accepted; (2) TUI retirement —
accepted, gated on cockpit parity; (3) naming — **"Pylon" stays** (npm
continuity, no rebrand); (4) earning rails — **the Spark wallet is
preserved as a live rail** (owner: "so we can do cool shit with that");
the non-Spark earning/labor rails go to #8577's Wave-4 ask-first list.
Execution lanes filed: **PY-1 #8578** (pylon-core extraction + daemon +
typed RPC), **PY-2 #8579** (desktop cockpit parity, rides EN-5 #8574),
**PY-3 #8580** (TUI retirement, gated). Recorded in MASTER_ROADMAP
rev 6.3. Original proposal text below, unchanged.

## 1. The question

Pylon (`apps/pylon`, 143 src modules, published as `@openagentsinc/pylon`
1.0.5 with an OpenTUI interface) predates the Khala Code product. With
Tassadar/Psionic retired (#8576/#8577) its *compute-market/serving* half is
being removed — but Pylon was never only that. The owner asks: what else is
Pylon, and should the rest live directly inside Khala Code desktop?

## 2. What Pylon actually is, post-retirement (the durable ideas)

Decomposed from the current module inventory, minus the #8577 removal set
(psionic-\*, tassadar-\*, training-cockpit, serving-\*, gepa, frlm, m6/m7
preflights):

| # | Durable idea | Modules (representative) | Current consumer |
|---|---|---|---|
| P1 | **Fleet account custody + connect** — isolated per-account Codex/Claude homes, device-auth connect, readiness/quota/health ledgers, never touching `~/.codex` | account-connect/registry/quota/usage/status, codex-account-health\*, codex-custody-reprime | `khala fleet connect` CLI; Khala Code desktop "Connect account"; server dispatch gate reads the registry |
| P2 | **Local coding-delegation executor** — khala request → local Codex/Claude run with workspace materializer, turn reporters, PR publisher, second-pass reviewer, closeout receipts, dispatch-failure taxonomy | assignment, khala-dispatch/requester/spawn/burndown, codex/claude-agent-executor, workspace-materializer, virtual-merge-queue | Khala Code desktop fleet-run supervisor (via CLI subprocess); ops runbooks |
| P3 | **Presence/capacity publishing** — go-online, heartbeat, counted capacity refs (`capacity.coding.codex.available=N`), the thing that makes a machine a dispatch target | presence.ts, presence-\*-account-capacity | CLI + runbooks; the server admission gate |
| P4 | **MCP surface** — fleet tools over MCP for any agent host | khala-mcp, mcp-contract-import | Khala Code desktop already ships its own khala-fleet-mcp-server + codex/claude fleet MCP bridges (duplication) |
| P5 | **Wallet (Spark)** — the Lightning/Spark wallet runtime, backup/claim, self-test | spark-\* (wasm runtime, helper autostart, backup), wallet.ts, sat-number | Live payment rail (Spark is PRIMARY for agent/MPP payments); earning surfaces mostly dormant |
| P6 | **Earning / labor-market rails** — NIP-90 provider, nostr identity, tips, multi-earning ledger, work-requester | labor-market, labor, provider-nip90, nostr-identity, tips, multi-earning-ledger | POSTPONED program (docs bannered); code dormant |
| P7 | **Node/daemon + ops** — long-running `pylon node`, self-update, launch gates, dev-doctor, inventory, public activity, ssh/wsl detection | node/, self-update, launch-gates, operator, orchestration, coordinator | 24/7 standing-pylon runbook; npm distribution |
| P8 | **Forge dispatch protocol** | forge-dispatch-protocol, forge-verification-runner | Forge repo boundary (postponed) |
| P9 | **The TUI** — OpenTUI operator interface | (TUI layer) | Standalone contributor-app framing |

**The integration seam today:** Khala Code desktop does not import a Pylon
engine — `src/bun/pylon-service.ts` **spawns the Pylon CLI as a subprocess**
and decodes lifecycle wire-events from stdout (via
`@openagentsinc/agent-runtime-schema`). The desktop already carries fleet
cockpit UI, a fleet-run supervisor, account state reporting, and its own MCP
bridges. So the fold is already half-happened — informally, over a stringly
CLI boundary, with duplicated MCP surfaces and version-skew risk between the
app and whatever `apps/pylon` checkout/binary it finds.

## 3. Evaluation: where should each idea live?

Three integration shapes were considered for the engine:

- **A. Status quo+** (keep CLI subprocess, deepen UI): lowest effort, but
  permanently stringly — wire events over stdout, no typed services, version
  skew between app and CLI, duplicated MCP, two update surfaces.
- **B. In-process embed** (extract `packages/pylon-core`; desktop's Bun main
  imports it): fully typed Effect services/layers, one release surface — but
  standing capacity dies when the app quits, the Spark WASM wallet moves into
  the GUI process, and an engine crash takes the app with it.
- **C. Local daemon, desktop as cockpit** (engine runs as a supervised
  `node` process; desktop talks typed RPC): 24/7 capacity survives app
  quit/crash, matches the standing-pylon ops runbook, keeps wallet/executor
  blast radius out of the GUI — but requires a real typed RPC contract and
  daemon lifecycle management.

**Recommendation: C, enabled by B's refactor.** Extract the engine as typed
workspace packages (`pylon-core`: custody P1, executor P2, presence P3,
wallet P5 behind its own service boundary), then:

- **Khala Code desktop becomes the primary human surface** — the cockpit. It
  supervises a local engine daemon (start/stop/go-online from the Fleet
  pane), renders accounts/capacity/assignments/closeouts/receipts, and owns
  approvals. The existing `pylon-service.ts` subprocess seam is replaced by
  the typed RPC client against the daemon (same Effect schemas end to end —
  no stdout parsing).
- **Interactive one-shot runs** (a fleet run the user is watching) may run
  in-process through the same packages where daemon indirection adds nothing
  — the packages make both shapes cheap.
- **The daemon is the same engine headless** — on a server with no desktop,
  `pylon node` (or its successor) runs standalone exactly as today's burn
  runbook expects. Desktop-optional, never desktop-dependent.

Per-idea routing:

| Idea | Destination |
|---|---|
| P1 custody/connect | `pylon-core` package; **desktop Settings/Fleet is the primary connect UX**; `khala fleet connect` CLI stays as the paste-free headless front door; CX-2 mobile connect shares the same custody rail (already planned) |
| P2 executor | `pylon-core`; desktop fleet-run supervisor drives it over typed RPC; receipts/closeouts rendered in the Fleet pane |
| P3 presence | `pylon-core`; a **"Go online" toggle in Khala Code desktop** — a running (or daemon-backed) Khala Code IS a pylon; capacity chips already exist in the fleet cockpit UI |
| P4 MCP | **Consolidate to one MCP surface** owned by Khala Code (`khala-fleet-mcp-server`); retire Pylon's duplicate khala-mcp pathway |
| P5 wallet | Keep as its own service boundary inside the engine daemon (never in the GUI process); balances/receipts surfaced read-only in desktop Settings; payout custody stays on the existing bridge — unchanged |
| P6 earning/labor | **Do not fold now** — postponed program; code goes on the #8577 Wave-4 candidates list (owner decides removal vs dormancy) |
| P7 node/ops | The daemon IS this; self-update folds into the engine's release surface; dev-doctor/inventory become desktop diagnostics + CLI commands |
| P8 forge protocol | Leave at the Forge repo boundary; Wave-4 candidate |
| P9 TUI | **Retire the OpenTUI surface** once desktop cockpit parity is proven — the TUI is a second UI codebase for flows the desktop (and CLI) already cover; retiring it is real LOC reduction and removes a whole UI stack from the repo |

## 4. What this deliberately does NOT change

- **Org-cloud execution (CX-2..9)** — the mobile MVP's agent-computer lane
  is a separate, additive rail; this proposal covers *owner-local* capacity.
  The two meet only at the shared custody registry.
- **Server dispatch gates, token accounting, payments/credits** — untouched.
- **npm `@openagentsinc/pylon` continuity** — 1.0.5 users keep working; the
  package becomes the headless engine distribution. Whether it is eventually
  re-branded (e.g. "Khala Node") is an **owner naming gate** — do not rename
  in code or docs until decided.
- **The `khala` CLI onboarding front door** — stays; it's the documented
  community connect path.

## 5. Sequencing (rides existing programs, no new front)

1. **Now, inside #8577 (PRUNE):** the retirement waves shrink `apps/pylon`
   first — do the removal before the extraction so `pylon-core` is carved
   from a clean surface. Add P6/P8 (+P9 if accepted) to the Wave-4
   candidates list.
2. **Engine extraction (new lane under epic #8467/#8566 sequencing):**
   `packages/pylon-core` (custody/executor/presence/wallet services, typed
   RPC contract). Exit: desktop's `pylon-service.ts` subprocess+stdout seam
   deleted, replaced by the typed client; one MCP surface; CLI re-based on
   the same packages.
3. **Cockpit parity (rides EN-5 #8574):** the Fleet pane's account list,
   go-online, run supervision, and receipts land as **Effect Native
   surfaces** during the desktop conversion — do not build new Foldkit/React
   fleet UI first and convert later.
4. **TUI retirement (owner-gated):** after desktop parity receipts, archive
   the OpenTUI layer to backroom per the PRUNE convention.

## 6. Owner gates in this proposal

1. Accept/reject the **daemon-cockpit shape** (§3 recommendation C).
2. **TUI retirement** (P9) — yes/no.
3. **Naming**: Pylon stays the headless-engine brand vs re-brand under
   Khala; affects npm package and public docs, so explicitly gated.
4. P6 earning rails: dormant-in-tree vs archived-to-backroom.

If accepted, file: one engine-extraction lane, one cockpit-parity lane
(cross-linked to EN-5), one TUI-retirement lane (gated), and extend #8577's
Wave-4 list — then record the decision in MASTER_ROADMAP as a rev 6.x note.
