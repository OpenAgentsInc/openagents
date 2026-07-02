# oh-my-pi (omp) And The Planner/Coder/Judge Workflow — Audit For Khala Code

Date: 2026-07-02
Status: reference audit. Examines `can1357/oh-my-pi` (the `omp` coding agent,
tracked in the workspace `projects/manifest.txt` reference lane) and explains
(a) how it supports the "Fable 5 as planner/architect, GPT 5.5 as coder,
Fable 5 as judge" workflow the owner flagged, and (b) what Khala Code would
need to support that workflow as a primary supported path. Documentation-only;
flips no promise state, changes no runtime authority, broadens no public copy.
Companion docs: [`ROADMAP.md`](./ROADMAP.md) (WS-8/WS-9),
[`2026-07-01-claude-code-parity-and-codex-synergies.md`](./2026-07-01-claude-code-parity-and-codex-synergies.md)
(§4 synergies), and the episode-245 multi-harness doc (Axis A/B).

Source note: the repo was explored at upstream HEAD `9e0b6cd` via the GitHub
API because git/tarball transfers to GitHub were timing out from this machine
during the audit window. The local reference clone under
`projects/repos/oh-my-pi` should be completed by `./projects/sync.sh` when the
network path recovers; nothing in this audit depends on the local copy.
Per workspace policy, omp is patterns-only reference material: no vendored
code, no product-surface naming.

## 1. The Workflow Being Audited

Two owner-supplied field reports about omp:

> "fable 5 in omp w/gpt 5 xhigh advisor is literally AGI"

> "I'm having a lot of success using Fable xhigh as a planner/architect, using
> GPT 5.5 xhigh (subscription) as a coder, then Fable xhigh again as a judge.
> At API pricing, planning+judge costs are in the ~few dollar range compared
> to typical $50+ full round trips. … GPT 5.5 even at xhigh compared to
> Fable 5 is very cheap and very fast."

The pattern is a **three-role split across two models and two billing rails**:

1. **Planner/architect** — Fable 5 (Claude), `xhigh` reasoning, API-priced.
   Reads the repo, decomposes the objective, writes the plan.
2. **Coder** — GPT 5.5, `xhigh` reasoning, **subscription-routed** (ChatGPT
   plan via OpenAI Codex OAuth). Executes the bulk of tool-calling turns.
3. **Judge** — Fable 5 again, either as a per-turn **advisor** (the first
   quote) or an end-of-run **reviewer with a structured verdict**.

The economics work because token mass is wildly asymmetric: the coder burns
the overwhelming majority of tokens (tool loops, file reads, retries) on a
flat-rate subscription, while the planner and judge see only compact inputs
(objective + repo excerpts; transcript deltas + diffs) at metered API prices.
Hence "planning+judge ≈ a few dollars" versus "$50+ full round trips" when
everything rides the API meter. The screenshot the owner shared shows exactly
this: seven parallel `(task)` jobs, a phased todo list, a named-subagent
roster, and a footer reading `Fable 5 (VibeProxy)++ · med · 20.9%/1M · $28.25`
— a Fable primary with per-session cost and context telemetry, fanned out
over cheap workers.

## 2. What oh-my-pi Is

`omp` (npm `@oh-my-pi/pi-coding-agent`, https://omp.sh) is Can Bölük's fork of
Mario Zechner's Pi (`badlogic/pi-mono`): a Bun/TypeScript terminal coding
agent with ~55k lines of Rust natives (search/shell/AST/PTY/BPE in-process,
no fork-exec), 32 built-in tools, LSP and DAP integration, hash-anchored
edits, a real browser, and four entry points (TUI, one-shot `-p`, RPC, ACP).
It is a **single harness that calls provider APIs directly** — the opposite
architectural choice from Khala Code, which wraps existing harnesses (Codex
app-server, Claude Agent SDK) and adds fleet coordination around them. That
difference drives most of the gap analysis in §5.

The relevant subsystems, from the upstream docs and prompts:

### 2.1 Model roles — routing by intent, per-role reasoning effort

`omp` assigns models to typed roles resolved from `settings.modelRoles`
(docs/models.md):

- Roles: `default`, `smol`, `slow`, `vision`, `plan`, `designer`, `commit`,
  `tiny`, `task`, `advisor`.
- Every role value may carry a thinking suffix:
  `:off|minimal|low|medium|high|xhigh`.
- Launch overrides (`--smol`, `--slow`, `--plan`), mid-session `/model`,
  `Ctrl+P` cycling, per-role **fallback chains** (`retry.fallbackChains`) for
  429/quota walls, and **path-scoped model sets** per repo.

So "Fable planner + GPT coder + Fable judge" is literally three lines of
config: `modelRoles.plan: <fable>:xhigh`,
`modelRoles.default: openai-codex/gpt-5.5:xhigh`,
`modelRoles.advisor: <fable>:xhigh` (and/or `slow` for the reviewer agent).

### 2.2 Mixed billing rails — subscription OAuth next to API keys

The provider catalog (40+ providers) explicitly tags auth modes: `oauth`
(sign in with a provider account), `plan` (coding-plan subscription), `local`.
**OpenAI Codex is an OAuth provider** — the coder role rides the user's
ChatGPT subscription, not a metered key. Anthropic OAuth, GitHub Copilot,
Cursor, and a dozen coding plans sit beside plain API keys, and
`~/.omp/agent/models.yml` accepts **custom providers** speaking any of seven
wire protocols. The `VibeProxy` in the owner's screenshot is such a custom
provider entry (it is not part of the omp repo) — a local gateway exposing
Fable 5 to the harness. Round-robin credential stacking and per-role fallback
chains complete the picture: one session can mix a subscription-routed coder
with API-priced planner/judge, exactly the economics in §1.

### 2.3 The advisor — a second model watching every turn

`docs/advisor-watchdog.md` is the most distinctive mechanism, and the direct
subject of the "w/gpt 5 xhigh advisor" quote (roles are symmetric — the owner
runs Fable primary with GPT advisor; the tweet's workflow inverts it):

- The advisor is a **full second agent** with its own model (role `advisor`),
  its own context, and its own (default read-only: `read`/`grep`/`glob`)
  tools, attached to the primary session.
- After each primary turn it receives only the **new transcript delta**
  (including thinking and tool intent), inspects the workspace itself, and
  emits at most one note per update through an `advise` tool.
- Notes carry severity: `nit` (batched, non-interrupting), `concern` /
  `blocker` (**interrupting** — delivered through the steering channel, can
  abort in-flight tools, and auto-resume the primary). An `immuneTurns`
  budget (default 3) prevents interrupt storms; an **emission guard**
  enforces dedupe and drops content-free "LGTM" noise in code, not prompt.
- Notes land in the primary transcript as
  `<advisory severity="…" guidance="weigh, don't blindly obey">` — the
  primary weighs advice, it does not obey it.
- `WATCHDOG.md` (review priorities) and `WATCHDOG.yml` (a **roster** of named
  advisors, each with its own model, tool grant, and specialization prompt)
  are discovered project-and-user-wide like context files.
- Advisor usage is separately metered: its turns persist to
  `<session>/__advisor.jsonl`, `/advisor status` reports its token/cost
  totals, and `omp stats` attributes them like any subagent.
- `advisor.syncBacklog` bounds how far the primary may run ahead of review
  (off/1/3/5 turns, 30s cap) — a tunable lockstep-to-async dial.

This is a **turn-granularity judge**: it catches drift while the coder is
mid-run, not after the diff is done.

### 2.4 Plan mode and the bundled `plan` agent

Plan mode enforces read-only operation via prompt constraints
(`plan-mode-subagent.md`: never create/edit/delete, never state-changing
commands) plus a swapped read-only tool subset, with the `plan` model role
active. The bundled `plan` agent (prompts/agents/plan.md) is a software
architect: it must spawn `explore` subagents for independent areas, and must
produce a plan "executable without re-exploration" with a fixed structure
(Summary / Changes / Sequence / Edge Cases / Verification / Critical Files).
Approved plans are handed to executing subagents as references.

### 2.5 The judge as reviewer — schema-typed verdicts

The bundled `reviewer` agent (prompts/agents/reviewer.md, model `pi/slow`,
thinking high) yields a **schema-validated verdict object**:
`overall_correctness: correct|incorrect`, `confidence: 0–1`, and `findings[]`
each with title, body, **priority P0–P3**, per-finding confidence, and file/
line anchors. `/review` spawns dedicated reviewer subagents that sweep
branches, single commits, or uncommitted work in parallel. The parent reads
the verdict as data (`agent://<id>/…` JSON paths), not prose.

### 2.6 `task` fan-out — typed, isolated, async subagents

The `task` tool (docs/tools/task.md) spawns batches of subagents — bundled
(`explore`, `plan`, `designer`, `reviewer`, `tester`, `librarian`, `sonic`,
`task`) or project/user-defined markdown agents with frontmatter `model`,
`thinkingLevel`, `tools`, `spawns`, and `output` (a per-agent result schema).
Runs are async jobs (the "waiting on 7 jobs" tree in the screenshot), bounded
by a semaphore, optionally **workspace-isolated** via a Rust isolation PAL
(APFS/Btrfs/ZFS clones, overlayfs, ProjFS, reflink, copy fallback) with
patch-capture or branch-merge back to the parent. Subagents finish through a
hidden `yield` tool producing typed output readable at `agent://<id>`;
transcripts stay inspectable at `history://<id>`; `irc` gives live agents a
peer channel. Per-agent model overrides mean the fan-out itself can be
role-priced (e.g. `sonic` = `pi/smol` for mechanical work).

### 2.7 Cost and context telemetry

The TUI footer carries live context share and session dollar cost; `/advisor
status` and `omp stats` attribute usage per advisor/subagent/session/project.
The user can *see* the planner/judge dollars staying small while the
subscription coder does the tonnage — the observability half of the
economics claim.

## 3. Why This Combination Works (And Its Fragility)

Structurally, omp turns "use the right model for each cognitive job" into
configuration rather than orchestration code:

1. **Role-typed model routing with per-role effort** makes the trio cheap to
   express and swap (`Ctrl+P`, fallback chains when a rail 429s).
2. **Subscription rails beside API rails in one catalog** puts the token-mass
   asymmetry to work: metered dollars only where judgment lives.
3. **The advisor is architecturally honest**: separate context (no
   contamination), read-only by default (no authority), interrupt semantics
   with rate limits (bounded noise), code-enforced dedupe, and separate
   accounting. It is judgment as a *stream*, not a gate.
4. **Schema-typed reviewer verdicts** make the judge composable — a verdict
   is data a program can route on, which is the same design conclusion Khala
   Code reached independently with T9.5.
5. **Isolated fan-out with typed yields** keeps parallel coders from
   colliding and keeps results machine-readable.

Fragilities worth naming before adopting anything:

- **Rail dependence.** The whole cost story rides on OpenAI keeping xhigh
  GPT 5.5 inside the ChatGPT-plan Codex rail (the owner's own caveat: "less
  than 24hrs since the re-release … longevity unclear"). Provider ToS,
  quotas, or pricing can invalidate it overnight. Fallback chains soften but
  do not remove this.
- **Unofficial proxies.** `VibeProxy`-style gateways that re-front
  subscription model access sit in ToS gray zones. Khala Code must route
  Fable through legitimate Anthropic auth (API key or the user's own Claude
  login via the Agent SDK), never a gray proxy, and our own no-resale
  invariant for subscription accounts stays non-waivable.
- **Advisor cost creep.** A frontier advisor reads every turn delta; on long
  sessions that is real API spend. omp mitigates with delta-only rendering,
  dedupe, and re-prime-on-compaction; any Khala adoption needs the same
  discipline plus our exact-token accounting.
- **Advice vs authority.** omp keeps the advisor non-binding by prompt
  framing ("weigh, don't blindly obey"). Khala Code's stronger stance —
  verify commands and deterministic programs as the only authority — should
  not be weakened to match; the advisor pattern must slot in *under* it.

## 4. What Khala Code Already Has

The striking finding: **Khala Code already implements the fleet-scale version
of this exact workflow.** Landed and verified in the repo at audit time:

- **Axis A / Axis B multi-harness.** The composer harness pill
  ("Codex | Claude | Khala", T8.5 SHIPPED) selects the chat harness; the
  `workerKind: codex|claude|auto` field (T4.4/T9.2, with the T9.3
  classifier-aware `auto` parameter layer) selects delegation targets.
- **Planner: plan-then-fan-out (T9.4, landed).**
  `clients/khala-code-desktop/src/bun/claude-plan-fanout.ts` decodes Claude
  plan-mode output as `openagents.khala_code.claude_plan_fanout_dag.v1`,
  validates public-safe refs / dependency cycles, converts it into a
  `plan_dag` FleetRun work source, and dispatches dependency-free nodes to
  Codex workers first. This *is* "Fable plans, GPT codes" — at fleet scale.
- **Judge: second-pass reviewer (T9.5, landed).**
  `apps/pylon/src/claude-second-pass-reviewer.ts` runs a Claude review after
  verify-green and feeds the merge policy an advisory structured verdict;
  the sibling desktop contract
  `openagents.khala_code.claude_plan_fanout_review.v1` types
  `accept | request_changes | replan`. Verify commands remain the authority —
  a stronger invariant than omp's prompt-level framing.
- **Subscription-rail coder, natively.** Khala Code's entire product premise
  is wrapping the user's own Codex login — the coder role is *already*
  subscription-routed, with isolated per-account homes
  (`<pylon home>/accounts/codex/<ref>`), multi-account concurrency
  (`khala fleet connect`), and the same for Claude accounts (T9.1).
- **Exact-only token accounting** with per-turn rows, owner-only traces, and
  public counters as projections — stronger provenance than omp's local
  stats, though (see G5) not yet *role-attributed* or dollar-denominated in
  the UI.
- **Typed fan-out substrate.** FleetRun + claim registry + deterministic
  `khala.fleet.delegate` program + closeout checklists parallel omp's task
  tool, with a stronger duplicate-work story (claims) and a weaker
  in-session ergonomics story (no `agent://`-style typed yield paths in chat).

What Khala Code does **not** have is the *single-session, chat-first*
expression of the trio — the thing that makes omp feel like "literally AGI"
in one terminal window:

1. No **advisor runtime**: nothing watches the active Codex thread turn-by-
   turn with a second model and injects steering advisories. Our judge fires
   at closeout, after the work.
2. No **model-role registry**: roles (planner/coder/judge/advisor) with
   per-role model + reasoning effort are not a typed, persisted setting; the
   harness pill selects an engine for the whole conversation.
3. Plan-then-fan-out is a **Fleet feature, not the default chat flow** — a
   casual chat prompt does not get a plan-first phase unless the user drives
   the Fleet surface.
4. No **per-role economics display**: token rows are exact but not
   dollar-projected or split "subscription-covered vs API-metered" anywhere
   a user decides from.

## 5. Gap Analysis: Making Planner/Coder/Judge A Primary Khala Code Workflow

Ordered by leverage; each gap names the omp mechanism it adapts and the
existing roadmap seam it extends. These are proposals for roadmap intake, not
scheduled work.

### G1 — Typed model-role registry (adapts §2.1; extends WS-8/T8.5, T1.3)

A shared Effect Schema setting, e.g.
`openagents.khala_code.model_roles.v1`:
`{ role: architect|coder|judge|advisor, harness: codex|claude|khala, model?,
effort?: minimal|low|medium|high|xhigh }` — persisted like the harness pill,
surfaced in Settings, consumed by chat, fleet dispatch, and the reviewer.
Mapping effort is harness-native: Codex reasoning effort via app-server
config; Claude via SDK `thinking`/effort options. The pill grows from "which
engine chats" to "which engine plays which role". Delegability: HIGH once the
schema is agreed (rides T1.1's contract discipline).

### G2 — Advisor runtime for the desktop chat (adapts §2.3; new, the largest piece)

A `KhalaAdvisorRuntime` as an Effect service: a Claude (Fable) session with
its own context that consumes **turn deltas** of the active Codex thread
(the thread-item projector already yields neutral turn events; render deltas
to markdown like omp's `formatSessionHistoryMarkdown`), runs read-only
inspection against the workspace, and emits severity-typed advisories
(`nit|concern|blocker`). Delivery: `nit` → batched transcript card;
`concern|blocker` → injected steering via the existing `codexTurnSteer` RPC.
Port omp's control-theory pieces as invariants, not suggestions: emission
guard (code-enforced dedupe + noise filter), `immuneTurns` interrupt budget,
reset-on-compaction/thread-switch, advisor-never-a-peer, separate exact token
rows for advisor usage. Config via a WATCHDOG-style discovery
(`ADVISOR.md` / roster yml) folded into the fleet spec's adopt-in-place
config scanner. UI: advisory cards with severity chips; Inbox rollup for
blockers. Delegability: MED (one strong lane + supervisor review on the
steering seam). Depends on: WS-8 Phase 1–2 (Claude runtime, approvals),
T1.2/T5.2 streaming consumer.

### G3 — Plan-first chat flow (adapts §2.4; extends T9.4 into the default surface)

Promote plan-then-fan-out from the Fleet panel into the chat composer: a
plan-mode toggle (or `/architect`) that runs the architect role
(Claude plan mode, read-only, `plan_dag` contract already typed) and renders
the DAG as an approvable plan card; on approval, nodes dispatch either as a
single Codex turn in-thread (small plans) or a FleetRun (large plans). The
plan artifact persists with the session like omp's plan file. Delegability:
MED. Depends on: T9.4 (landed), T3.3/T5.x for the run-from-chat surface.

### G4 — Judge-on-diff in chat (adapts §2.5; extends T9.5 + T15.1)

Bring the second-pass reviewer to the desktop surface: after a coder turn or
worker closeout produces a diff, run the judge role and render the structured
verdict (`accept|request_changes|replan`, P0–P3-style findings with file/line
anchors — extend the review schema with priorities/confidence, which omp's
reviewer shows earn their keep) as a card whose "request changes" feeds the
annotate-diff steering loop (T15.1). Verify commands remain the only merge
authority; the verdict stays advisory data. Delegability: HIGH once T15.1's
diff renderer lands.

### G5 — Per-role economics surface (adapts §2.7; extends T5.4, exact-token spine)

Attribute exact token rows by role (`role_ref` on the desktop telemetry and
Pylon turn reports), price API-metered roles from the model catalog, and
render per-session: "coder: subscription-covered · architect+judge: $X.YZ".
This makes the §1 economics *visible and honest* — projections from exact
rows only, `not_measured` when a rail's pricing is unknown, never synthesized
counters. Delegability: HIGH.

### G6 — One-command preset (product packaging; extends WS-9 + onboarding)

`khala code --preset architect-coder-judge` (and a Settings preset card):
coder = the user's existing Codex login (already the product default),
architect/judge = Claude via the user's Anthropic auth (API key or their own
Claude login through the SDK — never a proxy), advisor optional-on. Publishes
the workflow as copy-gated product surface only through `docs/promises/`
(a candidate promise: `khala_code.architect_coder_judge.v1`) — no public
copy broadened until the flow is verifiable end-to-end. Delegability: HIGH
after G1–G4.

### Explicit non-goals

- No vendoring of omp code, prompts, or naming (same rule as Orca:
  patterns-only).
- No gray-proxy provider entries for subscription model access; Fable routes
  through legitimate Anthropic rails only, and subscription no-resale remains
  non-waivable.
- No weakening of the deterministic-delegation or verify-command authority to
  make advisory judgment "feel" stronger; omp's advisor slots under our
  invariants, not beside them.
- The legacy Khala-native runtime does not grow a third parallel
  planner/judge implementation; roles bind to the two real harnesses.

## 6. Sequencing Against The Roadmap

All of §5 lives naturally in **WS-9 (multi-harness routing and synergies)**
with UI landings in WS-5/WS-15:

1. **G1** first (small, unblocks everything; pairs with the T8.x Claude
   phases already in flight).
2. **G3 + G4** next — they are thin surfaces over landed T9.4/T9.5 and
   deliver the visible "plan → code → verdict" loop in chat.
3. **G2** as its own lane once WS-8 Phase 2 (approvals/steering) is merged —
   it is the differentiating piece and the riskiest seam (steering channel).
4. **G5** in parallel with T5.4 gauges; **G6** last, gated on the promise
   record.

The end state is strictly stronger than omp's single-terminal version: the
same three-role loop, but with claims (no duplicate work), verify-command
authority (no vibes-based acceptance), exact token provenance (no
self-reported costs), isolated worker homes, and the option to scale the
coder role from one subscription login to a fleet of them.

## 7. Source Index

Upstream (`can1357/oh-my-pi` @ `9e0b6cd`, via GitHub API):

- `README.md` — product overview, provider/auth catalog, tool inventory
- `docs/advisor-watchdog.md` — advisor runtime, WATCHDOG.md/yml, emission
  guard, syncBacklog, cost attribution
- `docs/models.md` — `models.yml`, model roles, thinking suffixes, fallback
  chains, canonical equivalence, auth resolution order
- `docs/task-agent-discovery.md`, `docs/tools/task.md` — subagent discovery,
  batch fan-out, isolation PAL, typed yields, lifecycle
- `packages/coding-agent/src/prompts/agents/plan.md`, `reviewer.md`;
  `src/prompts/system/plan-mode-subagent.md`; `src/task/agents.ts`

Khala Code (this repo, verified at `399b077c27`):

- `clients/khala-code-desktop/src/bun/claude-plan-fanout.ts` (T9.4 contracts)
- `apps/pylon/src/claude-second-pass-reviewer.ts` (T9.5)
- `clients/khala-code-desktop/src/bun/claude-*.ts` (WS-8 runtime set)
- `docs/fable/ROADMAP.md` (WS-8/WS-9, T4.4/T8.5/T9.1–T9.7)
- `docs/fable/2026-07-01-claude-code-parity-and-codex-synergies.md` (§4)
- `docs/fable/2026-07-01-episode-245-completion-and-multi-harness-orchestration.md` (§3)
- `docs/fable/2026-07-01-khala-code-summary-and-analysis.md` (product map)
