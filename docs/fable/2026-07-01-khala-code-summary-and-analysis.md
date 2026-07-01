# Khala Code — Repo-Wide Summary And Analysis

Date: 2026-07-01
Status: synthesis document. Summarizes and analyzes everything discoverable in
this repository — code, docs, and the GitHub issue tracker — about **Khala
Code**. This doc flips no promise state, changes no runtime authority, and
broadens no public copy. It is a map plus an assessment, not a spec.

## 1. What Khala Code Is

Khala Code is the OpenAgents owner-local coding-agent product. As of
2026-07-01 its product statement is:

> Khala Code is a desktop/web wrapper around the user's local Codex install,
> with Khala swarm coordination layered around it.

The default coding harness requires a working Codex install and login
(`npm install -g @openai/codex`, `codex login`), and all default chat, thread,
slash-command, approval, MCP, plugin, skill, settings, and headless paths flow
through `codex app-server --stdio`. Khala Code does not reimplement Codex Core;
it adds the desktop shell, sidebar/thread navigation, Unified Inbox, Fleet and
Pylon swarm controls, Gym/proof panes, and headless JSONL automation around
that harness.

The product family spans three client surfaces plus one shared runtime:

- **Khala Code Desktop** — `clients/khala-code-desktop`, an Electrobun app
  (Bun host + Vite webview; also runs as a browser preview). The primary
  surface. App display name is literally "Khala Code".
- **Khala CLI** — `clients/khala-cli`, shipped npm package
  `@openagentsinc/khala` exposing the `khala` command (`khala fleet connect`,
  `khala fleet status`, `khala spawn`, `khala codex`, headless `khala code
  --json`).
- **Khala native macOS/iOS** — `clients/khala-macos` and
  `clients/khala-ios/Khala` (SwiftUI, bundle `com.openagents.khala`). Primarily
  the voice/chat client for the hosted Khala model, counted as the native arm
  of the family; the separate "Khala Desktop" macOS spec
  (`docs/desktop/2026-06-28-khala-desktop-spec.md`) is an adjacent
  Apple-FM/Pylon node product, not the Codex wrapper.
- **`@openagentsinc/khala-tools`** — `packages/khala-tools`, the
  provider-neutral Effect/Effect-Schema tool runtime (ADR 0012). Since the
  Codex-wrapper pivot it is supplemental/legacy on the desktop's default path,
  but remains the runtime for Khala-only swarm tools, fixtures, and fallback.

### 1.1 Naming disambiguation (read before citing anything)

Four nearby names cause real confusion; the repo history contains all of them:

1. **Khala (the platform/model)** — the hosted OpenAI-compatible inference
   endpoint (`openagents/khala`, `POST /api/v1/chat/completions`) that
   "behaves like one model but is an agent network underneath"
   (`docs/khala/khala.md`). Khala Code is a *client-side product*; Khala is
   the *hosted brain and market rail*. Hundreds of `khala` hits across the
   repo belong to the platform, not the coding product.
2. **`khala-code` (the deprecated model lane)** — in late June 2026 a hosted
   coding-tuned model variant id existed on the gateway (issues #6035, #6109,
   #6241). Khala is now a single model `openagents/khala`; the `khala-code`
   *model id* is deprecated and unrelated to the current product name.
3. **`KhalaCodex*` identifiers** — CLI/desktop TypeScript symbols like
   `KhalaCodexStatus`, `runKhalaCodexTask`. These are "Khala's local **Codex**
   integration", adjacent to but distinct from the product name.
4. **openagents.org / openagents-org** — an unrelated external "Slack for AI
   agents" project. External landscape research conflated it with our
   OpenAgents; the fleet-management spec carries an explicit identity warning.
   Nothing attributed to openagents.org is ours.

## 2. Timeline: How Khala Code Got Here

The entire Khala Code arc in the issue tracker runs **June 29 → July 1, 2026**
(~49 issues under the `khala-code` label, all closed at time of writing),
sitting on top of tool-runtime groundwork from June 22–29.

### Phase 0 — hosted `khala-code` model lane (June 22–25)

- #6035 fixed a 524 on long `khala-code` generations (Fireworks streaming
  usage-frame + buffering).
- #6109 wired the executed-verifier product path (`verified: true` impossible
  without executed verifier evidence).
- #6241 registered a `khala-code` Gym environment.
- This lane is **deprecated**: Khala is one model, `openagents/khala`.

### Phase 1 — fleet control plane, epic #7590 (June 29)

First use of the product name. Moved the shell-driven Khala/Codex fleet
manager (the `/tmp/openagents_burst_pr_refill.sh` era) into the desktop:
fleet manager service + local event store (#7591), account
readiness/cooldowns (#7592), Pylon capacity/assignment reconciler (#7593),
token accounting replay panel (#7594), GitHub PR/issue queue planner (#7595),
live worker transcripts (#7596), deterministic manager resume (#7597), and
retirement of the temp shell loops (#7598). Source runbook:
`docs/ops/2026-06-29-khala-codex-fleet-manager-runbook.md` — which also
records the proven live result: multi-slot Codex fanout across ready
accounts producing ~20 merged PRs in one overnight run, with exact token
accounting and no-spend closeouts.

### Phase 1.5 — native runtime and UI buildout (June 29–30)

- ADR 0012 (`docs/adr/0012-adopt-khala-native-terminal-tool-runtime.md`)
  accepted `@openagentsinc/khala-tools`: Effect-Schema-first built-ins
  (`read`, `ls`, `glob`, `grep`, `edit`, `write`, `apply_patch`,
  `exec_command`, `write_stdin`, `ask_user`, `todo_write`, `view_image`,
  `web_fetch`, `web_search`, browser tools), four result lanes (model output /
  UI / private artifacts / public summary), typed authority presets, and an
  OpenAI-compatible adapter. Issues #7614–#7629 built it out.
- ADR 0013 adopted the ProseMirror-inspired command composer (#7639/#7643);
  #7750 rebuilt the sidebar as a true Foldkit/Effect component; #7608 ported
  the code-block renderer into `@openagentsinc/ui`.
- #7646 moved OpenRouter BYOK to hosted account/server-side routing so a
  user key can never bypass the hosted Khala metering/tracing boundary.
- Default-on Rampart PII redaction landed on the desktop chat boundary
  (`docs/khala/2026-06-30-khala-code-desktop-redaction.md`): per-session
  placeholder table, fail-soft model→heuristics→regex cascade; explicitly a
  privacy prefilter, not a security boundary.

### Phase 2 — Codex execution-boundary port, epic #7651 (June 30)

`docs/codex/2026-06-30-codex-to-khala-code-porting-audit.md` reframed the
question from "build a coding agent" (already built) to "port Codex's harder
execution-boundary machinery". Eleven lanes (A–K) landed in one fanout:
central hooked tool dispatcher (A/#7652), macOS Seatbelt sandbox with honest
`sandbox.enforced` reporting (C/#7653), atomic `apply_patch` + safety checks
(D/#7654), durable JSONL session rollouts + resume/fork (E/#7655), headless
JSONL `ThreadEvent` schema (F/#7656), compaction (G/#7657), MCP client +
`khala mcp-server` (H/#7658), tool planner + progressive disclosure
(I/#7659), PTY exec (J/#7660), feature-flag registry (K/#7661), and — last by
deliberate posture — permission policy + approval cache (B/#7662).

The recorded posture decision: **Khala Code runs permit-all ("YOLO") for
trusted local operators**; sandbox containment, not prompting, bounds risk
until the rest of the boundary matured. Explicit non-goals: Windows sandbox,
the full ratatui TUI, enterprise/MDM config layers, multi-provider wire
abstraction, Codex's own auth flows, and Codex cloud-tasks infra.

### Phase 3 — deterministic delegation + GEPA, epic #7730 (June 30)

Motivated by a real dead-end (`codex_spawn` failing with "0/1 available"
because capacity was never advertised), delegation became a deterministic
program: **`khala.fleet.delegate`** = `ensure_pylon → advertise_capacity →
select_account → prepare_work → dispatch → verify_closeout`, each module with
typed preconditions and deterministic fallbacks — never an LLM decision per
call. An offline Mutalisk (DSPy/GEPA) loop optimizes *parameters only*;
candidates are gated by the Effect authority and never auto-promote
(`docs/gepa/2026-06-30-gepa-usage-and-fleet-delegation-optimization-loop.md`).

### Phase 4 — Part 2 recording slice + Gym pane, epics #7755/#7756 (June 30)

The transcript-245 demo slice: one casual prompt steers the Codex fleet with
no dead-end; Mutalisk emits a `khala.fleet.delegation` candidate; a no-UI Gym
bridge projects an evidence-only admission (`gated_proposal_ready`,
`decisionGrade: false` — explicitly not automatic promotion). The read-only
**Gym pane** renders an Arbiter-style graph (prompt → delegate → Pylon →
Codex assignment → closeout; GD-0 → Mutalisk → candidate → admission →
Action Submission proposal), with the renderer extracted into
`@openagentsinc/arbiter-effect` (#7761). Runbook:
`docs/khala-code/2026-06-30-part2-recording-runbook.md`.

### Phase 5 — THE PIVOT: Codex wrapper, epic #7780 (July 1)

The defining product decision, recorded in
`docs/khala-code/2026-07-01-codex-harness-wrapper-port-audit.md`:

> Khala Code should pivot from "a Khala-native coding harness inspired by
> Codex" to "a direct desktop/web wrapper around the Codex harness, with
> Khala swarm coordination layered around it."

Rationale: Codex already owns the session model, approvals, sandboxing, tool
routing, plugin/skill/MCP policy, slash commands, app-server API, and rollout
mechanics. Rebuilding those in TypeScript makes Khala Code permanently drift
behind upstream. `codex app-server` becomes the local kernel; Khala Code
becomes a rich web-native client first and a swarm console second.

Fifteen children (#7781–#7795) were implemented and closed the same day:
Codex install/auth gate, app-server supervisor + typed JSON-RPC client,
thread/turn as the default chat runtime (session→thread mapping persisted in
`~/.khala-code/codex-sessions.json`), full `ThreadItem` renderer,
slash-command palette parity against the complete Codex TUI inventory,
Codex-typed approval/permission/sandbox/guardian decisions (no translation
through legacy Khala enums), Codex-owned models/config/usage/personality
settings, Codex-backed session sidebar, Codex ecosystem
(plugins/skills/MCP/apps/hooks) + Inbox diagnostics, demotion of the legacy
Khala-native runtime behind explicit flags, re-layering swarm delegation on
Codex sessions, headless JSONL on app-server, mechanical parity tests + live
smoke, Codex-required product copy, and the app-server gap matrix.

### Phase 6 — productization and parity follow-ups (July 1)

- Fleet UI action for deterministic delegation (#7798), durable
  `khala-code-delegation-gepa` Gym run seam (#7799), UI-started Mutalisk
  optimization (#7800), end-to-end transcript-245 UI smoke (#7801).
- Parity slices against the gap matrix: background terminals (#7803),
  workspace knowledge/skills/hooks/import (#7804), IDE mention/diff (#7805),
  preferences/appearance (#7806), side-agent/plan controls (#7807).
- Pylon runner generalization: persisted task DAGs (#7808), runner-neutral
  status/control (#7809), 409-contention retry in `khala spawn` (#7810).
- Recent main-branch commits continue the token-accounting thread (Khala
  token counter provenance, background token-usage sync, replay leaderboard
  privacy) and MCP wiring ("Wire Khala Fleet delegation through Codex MCP").

## 3. Architecture Today

### 3.1 Process and boundary layout

```text
Khala Code Desktop (Electrobun)
  Web UI (Vite webview or browser preview)
    sidebar + Codex thread rail        (Chat / Inbox / Fleet / Gym / Settings)
    transcript with ThreadItem cards, slash palette, composer HUD
    Fleet board graph, Gym proof pane
  Bun host
    Codex app-server client + supervisor   <- the harness kernel
    Codex harness/auth gate, parity contract, gap matrix
    Khala swarm/Pylon client (codex_spawn, fleet MCP bridge)
    on-device decider host (Apple FM / gpt-oss, optional)
    Rampart redaction boundary
  Local services
    codex app-server --stdio           (the user's own Codex install)
    pylon node (when swarm is enabled)
  User state
    CODEX_HOME or ~/.codex             (primary user Codex session)
    <pylon home>/accounts/codex/<ref>  (isolated worker Codex homes)
    ~/.khala-code/codex-sessions.json  (desktop session -> Codex thread map)
```

Three session categories are kept visibly distinct, with a hard safety rule:
Khala Code never runs `codex login` against the default `~/.codex` home
automatically (login flow start clears an active auth file). Fleet "connect
account" always means an isolated worker home, never the primary session.

### 3.2 Key code map

Desktop (`clients/khala-code-desktop/`):

- `src/bun/codex-app-server-client.ts`, `codex-app-server-chat-runtime.ts` —
  JSON-RPC supervisor/client and the default chat runtime over
  `thread/*` / `turn/*` / `item/*`.
- `src/bun/codex-harness-status.ts` — install/auth/home readiness gate.
- `src/bun/codex-thread-item-projector.ts` — `ThreadItem` variants and delta
  families into stable transcript cards (ids/statuses preserved).
- `src/bun/codex-parity-contract.ts`, `codex-app-server-gap-matrix.ts` —
  pinned-commit parity fixtures (reference Codex commit
  `db887d03e1f907467e33271572dffb73bceecd6b`).
- `src/bun/khala-codex-fleet-tools.ts` — `pylon_ensure`,
  `codex_fleet_status`, `codex_spawn`, `codexFleetPromoteThread()`, the
  deterministic `khala.fleet.delegate` runner.
- `src/bun/khala-chat-runtime.ts` — the legacy hosted-Khala/OpenRouter loop,
  flag-gated (`KHALA_CODE_DESKTOP_RUNTIME=khala_native_runtime`).
- `src/bun/codex-token-usage-telemetry.ts` — exact token telemetry for
  direct/local sessions.
- `src/ui/fleet-board-projection.ts` / `gym-graph-projection.ts` — typed
  public-safe projections (`openagents.khala_code.fleet_board_projection.v0`,
  `openagents.khala_code.gym_graph_projection.v0`).
- `src/shared/on-device-decider.ts` + Apple FM / gpt-oss backends — optional
  cheap private local model for small routing choices.
- ~40 test files pin the behavior: parity contract, slash commands, gap
  matrix, approvals, threads, settings, ecosystem, headless JSONL, fleet
  board, Gym proof, visual smokes.

Shared runtime (`packages/khala-tools/src/`): dispatcher with
hooks/lifecycle/accounting, permission policy + approval cache, macOS
process sandbox, atomic apply-patch, session rollout, MCP, feature flags,
redaction, and the `fleet-delegate-program`.

Delegation substrate (`apps/pylon/src/`): workspace materializer,
Codex/Claude agent executors with isolated per-account homes, approval queue
+ bounded auto-approval policy, assignment lifecycle NDJSON events, exact
token posting to `POST /api/pylon/codex/turns`, raw event-chunk archive.

Server side (`apps/openagents.com/workers/api/`): `token_usage_events`
(exact rows, `demand_source: khala_coding_delegation`), owner-only ATIF
traces, the public `khala-tokens-served` counter as a projection of exact
rows, and the Gym/Mutalisk admission routes.

### 3.3 What Khala adds versus what Codex owns

Codex owns: threads/turns/items, tools, shell/process execution, apply-patch
semantics, approvals/permissions/sandbox/guardian policy, slash-command
behavior, models/config/personality, MCP/plugins/skills/apps/hooks, session
storage, auth.

Khala Code owns: the Electrobun shell and web-native rendering (rich cards,
diffs, terminal panes), the Codex thread sidebar, the **Unified Inbox**
(approvals, MCP/auth blockers, worker closeouts, ecosystem diagnostics), the
**Fleet** layer (isolated worker Codex accounts, capacity/readiness, spawn,
promote-thread-to-swarm, token proof), **Gym/proof** surfaces, headless JSONL
correlation, redaction, and the deterministic delegation program with its
GEPA optimization loop.

The gap matrix names six small upstream-ready gaps
(`codex.app_server.gap.tui_preferences`, `.memory_and_import_management`,
`.side_agent_plan_controls`, `.ide_mentions_diff`,
`.windows_sandbox_read_roots`, `.background_terminals`) rather than cloning
TUI-local behavior into TypeScript.

## 4. Fleet Management And Swarm Delegation

`docs/khala-code/2026-06-30-khala-code-fleet-management-spec.md` defines the
product capability: manage many coding workers from one local surface. A
"fleet" is the owner's linked Codex (and Claude) accounts, each an isolated
local worker reached via Khala → Pylon → assignment, plus the native runtime.

The capability map's conclusion: **the engine is built; the remaining gaps
are surfaces.** Prioritized build list: Unified Inbox (shipped in the pivot),
fleet board/graph (shipped in v0), worker cards, supervised orchestration UI
over the watcher/merge-resolver/refill loop, a config scanner for
adopt-in-place (skills/MCP/instruction files across harnesses), a run
timeline/trace viewer, save-as-worker + template gallery, staged memory
governance with default human review, and event-triggered background
workers.

The delegation flow itself is public-safe by construction: the worker prompt
carries a bounded objective and public refs only (never the local
transcript); lifecycle events are `assertPublicProjectionSafe`-checked;
`codexFleetPromoteThread()` requires explicit context boundaries; raw Codex
events stay in owner-scoped private storage; and completion evidence is the
closeout checklist plus exact `token_usage_events` rows — counter movement
alone is never proof.

The optimization loop (Mutalisk/GEPA → candidate manifest → Gym ingest →
`gated_proposal_ready` → Action Submission proposal → owner approval) keeps
self-improvement evidence-gated: `decisionGrade` stays `false` until real
held-out/live evidence exists, and no optimizer output auto-promotes into
live delegation parameters.

## 5. Verification Culture

Khala Code's parity claim is enforced mechanically, not by aspiration:

- **Pinned reference commit.** All parity fixtures read the vendored Codex
  checkout (`projects/repos/codex`) at a pinned commit; changing the pin
  forces the contract, gap matrix, and doc to update in the same change.
- **Fixture suites** cover the app-server schema/method/notification
  inventory, `ThreadItem` variants, slash-command enum comparison, approval
  response bodies, thread lifecycle, settings/ecosystem RPC pass-through, and
  headless JSONL — none require a live Codex login.
- **Opt-in live smokes**: `smoke:codex-parity-live` (skip-safe by default;
  with the env opt-in it starts app-server, creates/resumes a thread, runs a
  harmless turn, interrupts, and shuts down), `smoke:codex-spawn-live`
  (guarded Pylon worker delegation), `smoke:part2-ui` and the Fleet/Gym
  visual smokes (UI geometry, no model turns).
- **Honest-state reporting** is a recurring pattern: `sandbox.enforced:
  false` until real enforcement landed; token rates reported as
  `pending`/`not_measured` rather than fabricated zeros; skipped smokes
  return structured skip reasons.

## 6. Analysis

### 6.1 Strengths

1. **The pivot was the right call, made fast, with receipts.** The June 30
   porting audit honestly concluded the native harness was strong prototype
   scaffolding but a permanent-drift liability; within ~24 hours the product
   re-founded itself on `codex app-server` with fifteen implemented issues, a
   pinned parity contract, and a gap matrix. The decision record → epic →
   implementation-log chain is unusually traceable.
2. **Differentiation is real and orthogonal.** Everything Khala Code adds
   (Inbox, Fleet, swarm delegation with isolated homes, exact token proof,
   Gym admission, headless correlation ids) sits *around* the harness rather
   than competing with it. That is defensible even as upstream Codex moves.
3. **Evidence discipline.** Exact-only token accounting, owner-only redacted
   traces, public counters as projections, closeout checklists, and
   never-auto-promote optimization are consistent across code, docs, and
   issue acceptance language.
4. **Deterministic delegation.** Replacing per-call LLM judgment with a typed
   module program (`khala.fleet.delegate`) plus offline parameter
   optimization is the correct split of authority, and it demonstrably killed
   the "0/1 available" dead-end class.
5. **Proven at real scale for its stage.** The fleet runbook records live
   multi-account, multi-slot fanout that merged ~20 PRs overnight with
   no-spend closeouts — the product is exercised on its own backlog.

### 6.2 Risks and tensions

1. **Upstream dependency concentration.** The default product path now
   requires OpenAI's Codex CLI, its auth, and its experimental app-server
   surface. Schema drift is mitigated by the pinned contract, but product
   availability is coupled to a third party's install base, pricing, and
   policy. The legacy native runtime is the hedge, and it is deliberately
   demoted — worth keeping healthy enough to be a real fallback.
2. **Parity-pin maintenance cost.** Every upstream slash-command or schema
   change breaks a fixture until mapped. That is the design working as
   intended, but it converts upstream velocity into recurring local toil;
   the named upstream gaps should actually be filed upstream to shrink the
   adapter surface.
3. **Security posture is still "trusted local operator".** Permit-all/YOLO
   plus owner-local `danger-full-access` for delegated Codex work is
   documented and bounded by sandbox containment, but it is not a posture
   that can be opened to less-trusted or remote-driven use without landing
   the deferred permission-policy wave as the default.
4. **Naming debt.** Khala (platform) vs Khala Code (product) vs the
   deprecated `khala-code` model lane vs `KhalaCodex*` symbols vs the
   unrelated openagents.org — every future doc and external conversation
   pays a small tax. The disambiguation in §1.1 should stay near the front
   of any public copy.
5. **Three-client sprawl.** Desktop (Electrobun), CLI (npm), and native
   macOS/iOS (SwiftUI) share the brand but not one UI stack; the fleet spec's
   "one visual map across desktop + CLI + macOS" gap is where divergence will
   show first.
6. **Docs lag the pivot at the edges.** Pre-pivot docs (the fleet spec, the
   porting audit, parts of the ops runbook) still describe the native runtime
   as substrate. Each carries updated framing notes, but a reader landing on
   a single doc can still leave with the June 30 architecture in mind.

### 6.3 Open threads worth tracking

- The remaining fleet-management surfaces: worker cards, supervised
  orchestration UI, config scanner ("adopt, don't import" across harnesses),
  run timeline/trace viewer, save-as-worker, memory governance,
  event-triggered workers (spec §4 items 3–9).
- Live progress streaming for long tools per the design in
  `docs/khala-code/2026-06-30-codex-spawn-live-progress-streaming-audit.md`
  (Pylon already emits real-time lifecycle NDJSON; the desktop consumer is
  the gap).
- Upstreaming the six named `codex.app_server.gap.*` items to Codex.
- Flipping deferred permission-policy defaults before any less-trusted use.
- Public product copy: the positioning doc gates copy changes; there is not
  yet a public promise record dedicated to "Khala Code wraps your Codex".

## 7. Source Index

Product dossier (`docs/khala-code/`):

- `2026-07-01-codex-harness-wrapper-port-audit.md` — the pivot decision
- `2026-07-01-codex-required-product-positioning.md` — product copy guardrail
- `2026-07-01-codex-parity-contract.md` — pinned parity contract
- `2026-07-01-codex-app-server-gap-matrix.md` — slash-command decisions
- `2026-07-01-codex-wrapper-implementation-log.md` — issue-by-issue log
- `2026-06-30-khala-code-fleet-management-spec.md` — fleet product spec
- `2026-06-30-codex-spawn-live-progress-streaming-audit.md` — streaming design
- `2026-06-30-part2-recording-runbook.md` — demo/proof runbook

Adjacent docs:

- `docs/codex/2026-06-30-codex-to-khala-code-porting-audit.md` — lanes A–K
- `docs/ops/2026-06-29-khala-codex-fleet-manager-runbook.md` — live fleet ops
- `docs/gepa/2026-06-30-gepa-usage-and-fleet-delegation-optimization-loop.md`
- `docs/gym/2026-06-30-mutalisk-khala-code-gym-integration-audit.md`
- `docs/khala/2026-06-30-khala-code-desktop-redaction.md` — Rampart layer
- `docs/khala/khala.md` — the hosted Khala platform (context)
- `docs/adr/0012-adopt-khala-native-terminal-tool-runtime.md`
- `docs/adr/0013-adopt-prosemirror-inspired-command-composer.md`
- `docs/desktop/2026-06-28-khala-desktop-spec.md` — the distinct SwiftUI app
- `docs/khala-cli/2026-06-26-khala-cli-roadmap.md` — CLI sibling surface
- `docs/tokens/2026-07-01-khala-code-direct-local-token-accounting-session.md`
- `docs/research/terminal-agents/2026-06-29-openagents-khala-tool-decisions.md`

Code:

- `clients/khala-code-desktop/` (+ `README.md`, ~40 test suites)
- `clients/khala-cli/`, `clients/khala-macos/`, `clients/khala-ios/Khala/`
- `packages/khala-tools/`
- `apps/pylon/src/` (executors, materializer, approval queue, lifecycle)
- `apps/openagents.com/workers/api/src/inference/` (token counters, Gym)

Issue epics (all closed): #7590 (fleet control plane), #7651 (Codex
execution-boundary port), #7730 (deterministic delegation), #7755/#7756
(Part 2 slice + Gym pane), #7780 (Codex wrapper pivot, children
#7781–#7795), plus follow-ups #7798–#7810. Deprecated model-lane history:
#6035, #6109, #6241.
