# Episode 245 Completion And Multi-Harness Orchestration

Date: 2026-07-01
Status: audit + advice. Companion to
`2026-07-01-khala-code-summary-and-analysis.md`. This doc audits exactly what
stands between the recorded first half of `docs/transcripts/245.md` and the
unrecorded completion segment (the italic script), then lays out a
minimal-change path that also moves toward orchestrating between multiple
harnesses (Codex, Claude Code, Khala). It flips no promise state and broadens
no public copy.
Execution: the P0–P3 sequence in §3.3 is consolidated into the unified
[`ROADMAP.md`](./ROADMAP.md); the full Claude chat-harness plan this doc
defers is `2026-07-01-claude-code-parity-and-codex-synergies.md`.

## 1. Where Episode 245 Stands

The recorded half (0:00–19:58) ends with the deterministic-delegation plan
filed as issues and handed to Codex. The italic completion segment requires,
on camera:

1. Issues #7798–#7801 closed on `main`.
2. Khala Code with the Fleet hotbar; a condensed sidebar fleet representation
   (accounts, active workers, free slots, auth blockers) opening into the
   main Fleet panel.
3. `Run delegate` showing the deterministic `khala.fleet.delegate` trace with
   the recovered-capacity line and an accepted assignment ref.
4. `Optimize delegation policy` loading the `khala-code-delegation-gepa` Gym
   proof (candidate refs, `metricValueBps`, admission, proposal,
   `decisionGrade=false`) plus the active-parameter readout.
5. `smoke:part2-ui` PASS output and its screenshots.
6. Ending on the Fleet-to-Gym screen.

### 1.1 Verified today (fresh runs at origin/main `0046f0d771`)

| Requirement | State | Evidence |
| --- | --- | --- |
| #7798, #7799, #7800, #7801 closed | DONE | `gh issue view` — all CLOSED; zero open issues repo-wide |
| `smoke:part2-ui` | PASS | Fresh run in a clean worktree: desktop + mobile, all three steps green, screenshots under `var/khala-code-desktop/part2-ui-recording-smoke` |
| Deterministic delegate trace renders | DONE | The smoke's Fleet screenshot shows the full module list with `advertise_capacity: recovered` and `1/1 accepted` |
| Gym proof pane renders | DONE | Gym screenshot shows `gated_proposal_ready`, `10000 bps`, `decisionGrade: false`, candidate/proposal refs, the Arbiter graph, and the caveat refs |
| Shell delegation diagnostic | PASS | `scripts/part2-delegation-smoke.ts`: full module sequence, no `0/1` dead-end |
| Fleet MCP bridge tests | PASS | `codex-fleet-mcp-bridge.test.ts` + `khala-tools mcp.test.ts`: 11/11 |

So the fixture/recording path the italic segment was written against is
green end to end. What is *not* yet proven is the thing the recording is
really about: the **live** single-message path.

### 1.2 The single-message path: how it works now, and what is unproven

Since the Codex-wrapper pivot, the default chat turn is a Codex app-server
thread — the desktop no longer injects Khala tools into the model. The
mechanism that makes "one casual prompt → fleet spinout" true again landed
hours ago (`0eebd41a7b`, documented in
`docs/khala-code/2026-07-01-message-triggered-codex-fleet-delegation.md`):

```text
user message
  -> Codex app-server thread/turn
  -> Codex tool router
  -> local MCP server "khala_fleet"  (pylon_ensure, codex_fleet_status, codex_spawn)
  -> codex_spawn
  -> khala.fleet.delegate            (the deterministic blueprint bundle)
  -> Pylon assignment -> isolated worker Codex home
```

Wiring facts (from `clients/khala-code-desktop/src/bun/`):

- The bridge auto-registers before the first default chat turn
  (`rpc-handlers.ts` → `maybeEnsureFleetMcpBridge()`), writing
  `mcp_servers.khala_fleet` into the user's Codex config through app-server
  `config/value/write` + `config/mcpServer/reload`. Opt-out:
  `KHALA_CODE_DESKTOP_FLEET_MCP_BRIDGE=0`.
- MCP approval mode is `prompt`, so message-triggered delegation asks at the
  Codex tool boundary before spawning.
- `codex_spawn` still runs the deterministic program, so the `0/1 available`
  category stays dead.
- Failure is fail-soft: a system note plus
  `blocker.local.khala_fleet_mcp_bridge.unavailable`.

**Honest verdict: code-complete, live-unproven.** In-repo coverage is the
config shape, the program against a mocked Pylon runner, and the mocked UI
smoke. Nothing yet exercises a real Codex app-server accepting those config
writes and a live model actually calling `khala_fleet.codex_spawn`. That is
the single rehearsal that decides whether the episode can show the casual
prompt working.

### 1.3 Residual gaps against the italic script

1. **Live message-triggered spawn is unrehearsed** (§1.2). This is the only
   hard blocker candidate.
2. **The Gym "Optimize delegation policy" button is fixture-backed.** Both
   it and "Load demo proof" load the canned bridge proof
   (`main.ts` → `loadGymDemoOptimization()`); no live Mutalisk run starts
   from the UI. The italic script already describes this as the
   preview/proof path, so it is recordable as written — but say "preview"
   on camera, not "a live optimizer just ran".
3. **The condensed sidebar fleet representation needs a look.** The italic
   asks for account/worker/slot/auth-blocker counts visible at the sidebar
   level before opening the Fleet panel. The hotbar + Fleet panel render in
   the smoke; whether the condensed entry carries those counts was not
   verified in this audit. If it is icon-only today, that is a small,
   contained UI addition — not a blocker if the shot order becomes "click
   Fleet, read the top-of-panel summary".
4. **Live `Run delegate` needs a live Pylon.** The Fleet panel's delegate
   runner is wired to the real backend (`codexFleetDelegateRun` →
   `spawnCodexInstances` → real Pylon CLI); fixture mode needs no pins,
   real-work mode locally refuses until `repo`/`commit`/`verify` pins are
   present. For an on-camera accepted assignment ref, a live Pylon with a
   ready connected account must be up.

## 2. Advice: Complete The Episode With What Exists

Minimal-change recommendation: **change nothing before recording; rehearse
one live path; pick shots by what the rehearsal proves.**

### 2.1 Pre-recording rehearsal checklist (~30 minutes)

Run from a clean worktree at current `origin/main`:

```sh
# 1. Fixture confidence (already proven, cheap to re-run)
bun run --cwd clients/khala-code-desktop smoke:part2-ui
bun clients/khala-code-desktop/scripts/part2-delegation-smoke.ts

# 2. Live fleet preconditions
khala fleet status                       # >=1 ready isolated account
$PYLON provider go-online && $PYLON presence heartbeat

# 3. THE decisive rehearsal: casual prompt in default Codex mode
bun run --cwd clients/khala-code-desktop dev
#   type: "Test delegating a bounded analysis task to one Codex worker.
#          Do not change code."
#   expect: Codex MCP tool item for khala_fleet.codex_spawn -> approval
#           prompt -> deterministic delegate trace -> accepted assignment
```

If step 3 works: record the full italic segment plus the casual prompt —
that is the strongest possible close (the exact prompt that failed at 4:12
in the recorded half now succeeding through the deterministic program).

If step 3 fails: the likely culprits, in order — the pinned Codex build
rejecting `mcp_servers.*` config writes over app-server; the reload not
picking up the server; the `bun` command path inside the MCP server spawn;
or Pylon not live. The fail-soft blocker ref will say which. In that case
record the italic segment exactly as scripted (Fleet panel `Run delegate` is
real-backend and does not depend on the MCP bridge), and hold the casual
prompt for the next episode. The script's claims stay honest either way:
the dead-end category is gone because the *program* is deterministic, and
the panel demonstrates it.

### 2.2 Narration guardrails

- The Gym pane: call it what it is — a preview-backed proof of the loop
  (`decisionGrade=false`, `gated_proposal_ready`, owner approval still
  required). The italic script already does this well.
- Do not claim live GEPA optimization ran from the button; it loads the
  public-safe fixture proof.
- The accepted assignment ref on screen should come from a real fixture
  delegate run (no-spend closeout), which the panel supports today.

## 3. The Multi-Harness Direction

The owner's instinct — a toggle between "Codex mode" and "Khala mode", and a
dropdown that can point a request at Codex, Claude Code, or Khala with smart
routing — maps onto the existing architecture unusually cleanly, because the
pivot accidentally built the right seam. The key is to recognize there are
**two independent selector axes**, and conflating them is what would create a
mess:

**Axis A — the chat harness.** Who runs *this conversation*: which engine
owns the thread, tools, approvals, and session state of the chat the user is
typing into.

**Axis B — the delegation target.** Who runs the *spawned workers* when the
conversation (or the Fleet panel) fans work out through
`khala.fleet.delegate`.

These already exist separately in the code:

| Axis | Today | Mechanism |
| --- | --- | --- |
| A: chat harness | `codex_harness` (default) vs `claude_runtime` vs `khala_native_runtime` (legacy) | persisted setting, env-gated overrides, `runtimeMode` already carried on every RPC backend projection and tool catalog |
| B: delegation target | Codex only (`codex_agent_task`) | `codex_spawn` → `khala.fleet.delegate` → Pylon dispatch |

### 3.1 What the audit says about the Claude Code lane

Claude Code delegation is much closer than "not finalized" suggests —
roughly **80% of Codex-lane parity, production-live for the bounded lane,
zero open issues**:

- A real executor exists (`apps/pylon/src/claude-agent-executor.ts`,
  workflow `claude_agent_task`, driven by `@anthropic-ai/claude-agent-sdk`
  with a PreToolUse workspace-escape deny hook).
- Isolated per-account homes (`.claude*` sibling homes with
  `claude setup-token` OAuth tokens injected per account) — the same
  never-clobber model as Codex.
- Per-account counted capacity refs (`capacity.coding.claude.account.*`),
  dispatch-gate keying, `khala request --workflow claude_agent_task`, the
  MCP/CLI workflow enums, a claude-supervisor for N-worker pools, and exact
  token rows (`pylon-claude-own-capacity` / `openagents/pylon-claude`) into
  the same `token_usage_events` ledger. 21/21 executor/capacity tests pass.

The real deltas to Codex parity, ranked by whether the dropdown needs them:

Needed for a credible "delegate to Claude Code" option:

1. **Connect UX** — `khala fleet connect` is Codex-device-auth only; Claude
   accounts are hand-assembled. A `khala fleet connect --harness claude`
   path (wrapping `claude setup-token` into an isolated home) is the
   biggest UX gap.
2. **Fleet tooling is Codex-named** — the desktop ships
   `khala-codex-fleet-tools.ts` only; `khala-burndown.ts` hard-codes
   `codex_agent_task`; the deterministic program's precondition/blocker
   vocabulary is Codex-specific (`advertised_codex_capacity`,
   `no_available_codex_capacity`).

Deferrable (bounded lane is fine without them):

3. Execution posture — Claude delegated runs are bounded-workspace
   deny-by-default; only Codex has the owner-local full-access posture.
4. No PR publisher analogue; local-verification closeouts only.
5. Observability — no raw-event chunks and no ATIF trace ingest yet. The
   bounded Pylon-Claude lane does emit an exact assignment-turn token row to
   `/api/pylon/claude/turns`; closeout now exposes whether that row was
   reported, missing, unconfigured, or failed to post.

**T9.7 landing note (2026-07-02):** the desktop-fleet slice of Claude closeout
depth is shipped. `apps/pylon/src/claude-agent-executor.ts` accepts both
snake_case and camelCase SDK usage fields, attempts the Claude turn reporter for
every completed SDK session before terminal closeout, and adds public-safe refs
such as `result.public.pylon.claude_agent_task.token_usage_reported` plus typed
blockers for `token_usage_missing`, `token_usage_report_failed`, and
`token_usage_reporter_unconfigured`. The CI smoke now requires the reporter
route. The posture decision is intentionally conservative: public/fleet Claude
assignments do not use `bypassPermissions`; PR publishing and raw-event/ATIF
ingest remain deferred until Claude workers graduate from bounded
local-verification work to PR delivery.

### 3.2 Recommended shape: one selector per axis

**Axis A UI: a harness pill on the composer/settings — "Codex | Claude | Khala".**
Minimal change: promote the existing env gate to a persisted desktop setting
that `rpc-handlers.ts` reads as the default, keep env vars as overrides, and
render the current `runtimeMode` that responses already carry. "Khala mode" is
the honest name for `khala_native_runtime` — hosted Khala routing with the
native tool loop. This directly delivers the toggle without silent fallback:
Codex mode is where local parity lives, Claude mode is the Agent SDK harness,
and Khala mode keeps the legacy banner visible.

**Axis B UI: a delegation-target field on `codex_spawn` and the Fleet
delegate form — "codex | claude | auto".** This is where Claude Code
arrives first and cheapest, because the server workflow, executor, capacity
refs, and dispatch gate all exist:

- Generalize the spawn tool: either add `workerKind: "codex" | "claude" |
  "auto"` to `codex_spawn` (and eventually rename the tool `fleet_spawn`,
  keeping `codex_spawn` as an alias), or add a sibling `claude_spawn`.
  Parameter on one tool is better — it keeps the MCP surface small and lets
  `auto` exist.
- Parameterize `khala.fleet.delegate` by worker kind: same module skeleton
  (`ensure_pylon → advertise_capacity → select_account → prepare_work →
  dispatch → verify_closeout`), with the capacity/blocker vocabulary keyed
  by kind (`advertised_{kind}_capacity`, `no_available_{kind}_capacity`)
  and dispatch selecting `codex_agent_task` vs `claude_agent_task`. The
  program's determinism and recovery ladder is exactly why adding a second
  harness is safe: control flow stays code, only the account pool and
  workflow id vary.
- `auto` = Khala smart routing. The server already has the seams
  (`coding-workflow-classifier.ts`, `autopilot-work-adapter-selection.ts`);
  `auto` initially can be a trivial local rule (prefer the kind with free
  advertised slots), then graduate to the server classifier, then become a
  GEPA-optimizable *parameter* of the delegation program — never its
  control flow. That is the same DSPy split the episode teaches.

  **T9.3 update (2026-07-02):** `auto` now consumes a structured
  workflow-classification hint (`codex_agent_task`, `claude_agent_task`,
  `cloud_coding_session`, or `none`) when one is already present from the
  typed classifier seam. The deterministic delegation program scores only
  available Codex/Claude slots, with Gym-admitted parameter knobs for the
  classifier confidence threshold, classifier bonus slots, and tie-breaker.
  No prose is parsed, and no optimizer candidate can change the control-flow
  modules or self-promote at runtime.

This gives the owner's dropdown its full meaning without inverting the
pivot: **the chat harness stays a wrapper decision (Axis A); "or whomever
else" is a delegation decision (Axis B).** A Codex-mode chat can spawn
Claude workers; a Khala-mode chat can spawn Codex workers; nothing about
the parity contract changes.

### 3.3 Minimal-change sequence

- **P0 (record episode 245): change nothing.** Default Codex mode + the
  `khala_fleet` MCP bridge + the Fleet panel is the story. Run the §2.1
  rehearsal to pick shots.
- **P1 (days): the Axis A toggle.** Persisted harness setting + composer
  pill for `codex_harness`/`claude_runtime`/`khala_native_runtime`, "Khala
  mode" naming, runtime badge on responses (already emitted — just render it). Also
  surface the condensed sidebar fleet counts if the rehearsal shows they
  are missing (§1.3.3).
- **P2 (this week): the Axis B target.** `workerKind` through
  `codex_spawn`/`khala.fleet.delegate`/the Fleet delegate form with
  `codex | claude`; `khala fleet connect --harness claude`; de-Codex-name
  the shared fleet tooling (`khala-codex-fleet-tools.ts` →
  fleet-tools with a kind parameter; fix the `khala-burndown.ts`
  hard-coding). The Claude executor and server rails need no changes for
  bounded fixture/checkout work. This is the same work item as Lane B4 of
  `2026-07-01-fleet-fanout-coding-instructions.md` — implement it once,
  there, with the FleetRun `workerKind` enum.
- **P3 (partly landed in T9.3): `auto` and Khala-as-router.** Local
  free-slot rule → server classifier → GEPA-optimized routing parameters,
  admission-gated through the same Gym path the episode demos. The
  classifier-biased parameter layer is in; broader Khala-as-router and
  deferrable Claude gaps (full-access posture decision, PR publisher,
  ATIF/per-turn rows) remain for when Claude workers graduate from
  analysis/bounded work to PR delivery.

**T9.4 landing note (2026-07-02):** the first concrete Axis-A/Axis-B
crossover now exists as a typed handoff rather than an implicit prompt
pattern. A Claude plan-mode session can emit a bounded task DAG, Desktop
validates it as public-safe structured data, and FleetRun accepts it as
`work_source = plan_dag`. The supervisor keeps the deterministic control-flow
role: it claims dependency-free nodes, dispatches them to Codex workers, and
only unlocks dependent nodes after closeout. Claude's returned
`accept | request_changes | replan` review verdict is represented as an
advisory contract, not as merge or dispatch authority.

### 3.4 Invariants to keep while doing this

- Isolated worker homes for every harness; never touch `~/.codex` or the
  owner's live `~/.claude` session from connect flows.
- The MCP approval prompt stays at the Codex tool boundary for
  message-triggered delegation.
- The Codex parity contract stays scoped to the Codex chat harness; a
  harness toggle must not smuggle the legacy runtime back in as a silent
  fallback (today's explicit-flag + banner behavior is right — the toggle
  should keep the visible labeling).
- Exact-only token accounting per lane (`pylon-codex-own-capacity` /
  `pylon-claude-own-capacity`); counters remain projections.
- Optimizer candidates (including future routing parameters) never
  auto-promote; Action Submission + owner approval always gate admission.

## 4. Bottom Line

Episode 245's completion segment is recordable now: every issue it cites is
closed, both recording smokes pass fresh at origin/main, the Fleet panel's
deterministic delegate runner is wired to the real backend, and the Gym
proof pane renders the full admission story. The one unproven shot is the
one that matters most — the casual chat prompt triggering
`khala_fleet.codex_spawn` through the brand-new Codex MCP bridge — so
rehearse exactly that before rolling, and fall back to the Fleet-panel path
(which tells the same deterministic-program story honestly) if the live MCP
leg misbehaves. Then take the harness question in two small steps that fit
what is already built: a persisted Codex/Khala chat-harness toggle first,
and a `codex | claude | auto` worker-kind parameter through the
deterministic delegation program second — Claude Code is ~80% of the way
there on the delegation axis already, and the deterministic program is
precisely the seam that makes adding harnesses safe.
