# Khala Code Desktop QA Framework — Design

Date: 2026-07-01
Status: design doc. Specifies a testing framework/agent that can drive Khala
Code Desktop programmatically — defined scenarios and free-explore mode,
headless and headed, computer-use and typed programmatic access — with
expectations at every phase, plus the formal-verification, measurement, and
optimization layers around it. Grounded in a fresh audit of what already
exists. **See §15 for the post-roadmap implementation-status addendum
(2026-07-02).** Companion to the other `docs/fable/` analyses. This doc flips no
promise state and broadens no public copy.

## 0. The One-Paragraph Design

Do not build a new QA system. **Aim the one we already shipped at our own
desktop app, and close six specific gaps.** `apps/qa-runner`
(`@openagentsinc/qa-runner@0.1.0`, MIT, npm-published) is already an
autonomous QA agent: an LLM ReAct brain plus a scripted deterministic brain,
typed action schema, Playwright/terminal/container/native-macOS backends, a
distiller that turns a live session into a committed re-runnable e2e test,
commitments → CONFIRMED/REFUTED/INCONCLUSIVE verification, ATIF traces with
redaction, an HTTP control API, and GEPA failure-learning. Khala Code Desktop
is already automatable three ways: a full-surface HTTP RPC bridge, an in-page
`window.khalaCodeDesktop` automation API, and a proven Playwright+Vite smoke
harness. The framework below is mostly a **marriage contract** between those
two systems — a desktop driver for qa-runner, one scenario format that runs
through every access mode, an oracle catalog, a determinism layer, and a
formal-model tier for the state machines that deserve it. Everything new is
small; the leverage is in composition. And because Khala Code Desktop becomes
the flagship native-desktop QA target, every gap we close here directly
advances the QA-agent product line (#6181 "out-ship Factory").

Cross-connections: gaps G1/G2 are hard prerequisites for Lane D of
`2026-07-01-fleet-fanout-coding-instructions.md`, and the `schema`/
`consistency` oracles cannot exist until the Schema-first RPC contract from
Phase 1 of `2026-07-01-khala-code-effect-integration-audit.md` lands. The
G-gaps and P-phases here are scheduled as the QA workstream in the unified
[`ROADMAP.md`](./ROADMAP.md).

## 1. What Already Exists (Audit Summary)

### 1.1 The QA agent (`apps/qa-runner` — shipped, epics #6174/#6181/#6206)

- **Two brains.** `scriptedBrain` executes a deterministic step list; live
  mode (`khala-session.ts`/`khala-driver.ts`) is a ReAct loop — one typed
  action per turn over any OpenAI-compatible endpoint (default hosted Khala;
  BYO model; `--fake-model` for zero-network runs). The goal-driven live loop
  *is* the free-explore mode; scripted is the defined mode.
- **Typed action vocabulary** (Effect Schema, `khala-action.ts`): `navigate,
  click, type, readText, waitFor, screenshot, assert, terminal_run, done,
  fail` — with one bounded corrective re-prompt on unparseable actions, then
  honest failure.
- **Backends behind one `QaRunResult` schema**: local Chromium (Playwright),
  terminal/TUI (PTY + snapshot asserts, asciicast video), container,
  **native macOS desktop** (AX tree via System Events + `screencapture` +
  click/type synthesis, armed by `QA_NATIVE_DESKTOP=1`), Cloudflare Browser
  Rendering/Sandbox.
- **Evidence**: video, Playwright trace.zip, per-step screenshots,
  `result.json`, deterministic public-safe `session_trace.v1`, ATIF traces
  published to `openagents.com/trace/{uuid}` (+ compare view), and the
  **distiller** that emits a committed `*.e2e.test.ts` from a passing live
  session — the anti-Factory differentiator.
- **Honesty machinery**: commitments declared up front, post-run verify
  (CONFIRMED requires observed evidence from *this* run), public-safety
  tripwires (`assertPublicSafeResult`), condition-based waits (no sleeps),
  artifacts flush on crash (`Effect.ensuring`), honest exit codes.
- **Ops**: HTTP control daemon (bearer auth, real runs armed via
  `QA_CONTROL_ARM_REAL=1`), PR-evidence commenting, evals comparison runner,
  failure-learning → GEPA candidates, Tier-1 warning-only pre-push smoke +
  Tier-2 async full matrix on the owned GCE runner (no GitHub Actions).
- **Probe computer-use** (`packages/probe/.../computer-use/`): the same
  browser/terminal/fs tool family exposed as LLM tools with an action
  timeline; unit-testable with injected fakes.

### 1.2 The desktop's automation surfaces (`clients/khala-code-desktop`)

- **HTTP preview RPC bridge** (`src/bun/index.ts`): with
  `KHALA_CODE_DESKTOP_OPEN_WINDOW=0` the app runs headless (no native
  window) while `Bun.serve` exposes `GET /health` and `POST /rpc/<method>`
  with `{args:[...]}` — dispatching into the **same 57-method handler table**
  the native window uses: app/harness status, full thread/turn lifecycle,
  fleet status/delegate/promote, approvals, settings/config,
  ecosystem/MCP/skills/plugins, fs/mentions, background terminals, slash
  commands, token summaries. **No auth today**, and streamed `chatTurnEvent`
  pushes are dropped over HTTP (final responses only).
- **`window.khalaCodeDesktop`** (in-page, `main.ts`): every RPC method plus
  automation helpers — state readers (`messages()`, `gymState()`,
  `composerStatus()`, `isPending()`, `threadSwitchPerformance()`) and
  drivers (`setComposerDraft`, `submitComposer`, `stopTurn`, `reset`,
  `loadGymProof`, `simulateLargePaste`, `stageAttachmentForSmoke`).
- **Proven Playwright harness pattern** (scripts/): Bun-spawned Vite +
  headless Chromium + `page.route("**/rpc/*")` fixture mocking; probe
  library for geometry, focus, reduced-motion, canvas/WebGL pixels, and
  public-safe text; viewport matrix (desktop 1280x800, mobile 390x844,
  dark, reduced-motion); screenshots + `summary.json` under ignored `var/`.
- **Headless JSONL mode** (`--json`): real Codex-backed turns, stderr JSONL
  events with correlation ids, single stdout result, structured failure on
  missing Codex, `KHALA_CODE_HEADLESS_INTERRUPT_AFTER_MS` automation hook.
- **Selector inventory**: rich, stable `id` + `data-khala-*` /
  `data-hotbar-action` / `data-thread-id` / `data-state` attributes — good
  anchors for both Playwright and vision agents.
- **One perf benchmark** (`bench:thread-switch`): tunable mock latencies,
  samples read via `threadSwitchPerformance()` (click→optimistic-render,
  click→full-render, cache hits).
- **Prior art for determinism**: `apps/autopilot-desktop/src/testing/`
  (deterministic-env, synthetic-event-service, app-replica) — the sibling
  desktop app's harness; `@effect/vitest`/TestClock used in the Worker's MPP
  tests.

### 1.3 The honest gaps

1. The UI shell is **imperative DOM, not Foldkit** — no central Model/fold
   to read or model-check; state lives in module-level variables.
2. **No streaming over the HTTP bridge** — live-turn observation requires
   the in-page API or native socket.
3. **No auth / no read-only mode on the bridge** — fine for localhost
   dogfood, unacceptable for productization.
4. **No native-window automation in the desktop repo** — headed testing of
   the real Electrobun window is unwired (qa-runner's macOS AX backend
   exists but has never been pointed at Khala Code).
5. **No property-based, model-based, or formal layer** — `fast-check` absent
   repo-wide; no TLA+ specs; `app-shell.test.ts` partially asserts on
   source-code text (a smell standing in for real DOM tests).
6. **Coverage is unmeasured** — nothing tracks which RPC methods, slash
   commands, panels, settings keys, or item-card variants a test run
   actually exercised.

## 2. Design Principles

1. **One scenario, many drivers.** A test is written once against an
   abstract driver interface and must run through every access mode. Mode
   disagreement is itself a bug signal (N-version testing).
2. **Determinism first, then exploration.** Every flake is either a real bug
   or a harness bug; there is no third category. Explore mode generates
   candidates; the distiller freezes what it finds into deterministic
   regression tests.
3. **Honest evidence only** (inherited from qa-runner): commitments up
   front, verify from observed evidence, no fake green, artifacts always
   flush, uncertainty never rounds up.
4. **Public-safety is an oracle, not a policy doc.** Every mode asserts the
   tripwires (no secrets/paths/raw prompts in DOM, projections, traces).
5. **Formal models inform; they never authorize.** Per the workspace
   invariant contract: narrow checkable contracts, bounded models,
   counterexamples become regression tests, runtime policy never weakened to
   make a model pass.
6. **Measure everything the harness touches.** Every run emits perf samples
   and a coverage ledger; budgets fail loudly; trends feed the optimization
   loop.

## 3. Architecture: Four Access Modes, One Driver Contract

```text
                       +--------------------------------------+
                       |        Scenario / Explore layer      |
                       |  typed scenarios | LLM brain | monkey |
                       +-------------------+------------------+
                                           |
                              KhalaCodeQaDriver (Effect service)
                                           |
        +----------------+----------------+----------------+----------------+
        |                |                 |                |
   Mode P: RPC      Mode D: DOM       Mode V: Vision    Mode H: Headless
   HTTP /rpc/*      Playwright over   qa-runner brain   `--json` JSONL
   (+ SSE events)   Vite or preview   + screenshots     CLI contract
   headless, typed  server; real UI;  + native macOS    real Codex turns
   Schema oracles   mocked or real    AX backend for
                    backend           the HEADED
                                      Electrobun window
        |                |                 |                |
        +----------------+--------+--------+----------------+
                                  |
                     Oracle catalog (section 6)
                     Evidence: QaRunResult + ATIF + coverage ledger
```

**Mode P — programmatic.** An Effect client (`KhalaCodeRpcClient`) that
speaks `POST /rpc/<method>` and decodes every response with Effect Schema.
This is the fastest, most headless mode: full thread/turn/fleet/settings
lifecycle without a browser at all. It is also the *reference semantics* —
what the RPC layer says the app state is.

**Mode D — DOM.** Playwright drives the real webview UI (via Vite or the
preview server) with either mocked RPC (fixtures, today's pattern) or the
real Bun host behind it. This tests what users see: rendering, focus,
keyboard, hotbar, panels, palette, composer. The existing probe library
(geometry/focus/reduced-motion/pixels/public-safe text) is the assertion
vocabulary.

**Mode V — vision / computer-use.** qa-runner's live brain drives the app
from screenshots and the AX tree, no selectors: headless via its Chromium
backend pointed at the preview URL, **headed via its native macOS AX backend
pointed at the real Electrobun window** (`QA_NATIVE_DESKTOP=1`). This is the
only mode that tests what a human actually experiences — native menus,
window chrome, real WKWebView behavior — and it is the productized-agent
mode.

**Mode H — headless JSONL.** The `--json` CLI contract as a black-box
automation surface: prompt in, JSONL events + final JSON out, interrupt
hook. Cheap to fuzz, cheap to schema-check, and the natural CI smoke for the
Codex harness path.

**The driver contract.** One Effect service interface with implementations
per mode:

```ts
interface KhalaCodeQaDriver {
  boot(opts: BootOptions): Effect<AppHandle, BootError>        // headless|headed, fixture|live backend
  act(action: QaAction): Effect<Observation, ActError>          // superset of qa-runner's action schema
  read(query: StateQuery): Effect<StateSnapshot, ReadError>     // typed state reads (RPC | window API | AX)
  events(): Stream<AppEvent>                                    // turn events, console errors, crashes
  metrics(): Effect<MetricsSnapshot>                            // perf samples, coverage counters
  shutdown(): Effect<Artifacts>                                 // always flushes evidence
}
```

`QaAction` extends qa-runner's schema with desktop verbs (`rpc_call`,
`hotbar`, `slash_command`, `approve`, `thread_select`); `StateQuery` unifies
"what the app believes" across modes so the consistency oracle (§6) can
compare them.

## 4. Defined Scenarios: Expectations At Every Phase

A scenario is a typed, versioned Effect Schema document — not code — so the
same scenario runs in every mode, distills from explore sessions, and diffs
cleanly in review:

```ts
KhalaCodeQaScenario = {
  id: "scenario.khala_code.thread_lifecycle.v1",
  modes: ["rpc", "dom", "vision", "headless"],       // which drivers must pass
  backend: "fixture" | "live_codex" | "live_fleet",  // gating tier
  phases: [
    { name: "boot",
      act:   [{ kind: "boot", headless: true, backend: "fixture" }],
      expect: [{ oracle: "schema", query: "codingStatus", decode: "CodingStatus" },
               { oracle: "invariant", id: "no_console_errors" }] },
    { name: "start-thread",
      act:   [{ kind: "rpc_call", method: "codexThreadStart" }],
      expect: [{ oracle: "consistency", left: "rpc:codexThreadList",
                 right: "dom:#thread-sidebar [data-thread-id]" },
               { oracle: "perf", metric: "thread_start_ms", budget: 400 }] },
    { name: "submit-turn",
      act:   [{ kind: "type", target: "#composer-input", text: "hello" },
              { kind: "submit_composer" }],
      expect: [{ oracle: "event", within_ms: 5000, match: "message_done" },
               { oracle: "public_safe_dom" }] },
    ...
  ],
  commitments: ["thread survives reload", "no legacy dead-end text"],
}
```

Rules:

- **Every phase has expectations.** A phase without an oracle is rejected by
  the scenario loader.
- **Commitments ride to the verifier** — the run's verdict uses qa-runner's
  CONFIRMED/REFUTED/INCONCLUSIVE semantics, so a scenario that "mostly
  worked" cannot report green.
- **Backend tiers gate spend and risk**: `fixture` runs everywhere
  (pre-push, CI); `live_codex` is skip-safe-by-default with env arming
  (exactly like `smoke:codex-parity-live` today); `live_fleet` requires a
  live Pylon and is the recording-rehearsal tier.
- **The seed corpus is mechanical**: one lifecycle scenario per roadmap RPC
  group (threads, turns, approvals, settings/config, models/personality,
  ecosystem, fs/mentions/attachments, background terminals, slash commands,
  token summaries, fleet, FleetRun, sessionCatalog, forum panel, inbox
  routing, gym source projections, plans/billing, headless event sources, and
  qaMetrics), one per hotbar panel, one per `ThreadItem` card variant
  (replayed from the parity fixtures), one per slash command (the registry
  already enumerates them — coverage is a loop, not a wishlist).
- **Q4.1 implementation artifact**: `packages/khala-qa-harness/src/seed-corpus.ts`
  exports `KHALA_CODE_QA_SEED_SCENARIOS`, a fixture RPC fetch, and
  `KHALA_CODE_QA_SEED_CORPUS_MANIFEST` grouped by RPC area, hotbar panel,
  `ThreadItem` variant, and slash command so the coverage ledger can count
  coverage directly. See `docs/qa/khala-code-mechanical-corpus.md`.
- **Q4.2 implementation artifact**: `clients/khala-code-desktop/src/bun/codex-thread-item-fixtures.ts`
  pins one shared render fixture per parity-contract `ThreadItem` variant.
  The seed corpus replays those fixtures through the `thread_items` group and
  records the fixture source in the manifest. See
  `docs/qa/khala-code-thread-item-coverage.md`.
- **Q4.4 implementation artifact**: `packages/khala-qa-harness/src/seed-corpus.ts`
  exports `KHALA_CODE_QA_ERROR_STATE_CASES`, one fixture-tier scenario per
  named degradation case, plus invariant oracles for typed degraded state,
  no console errors, and no data loss. See
  `docs/qa/khala-code-error-state-corpus.md`.
- **Q4.5 implementation artifact**: `packages/khala-qa-harness/src/cross-mode.ts`
  runs one `modes: ["rpc", "dom"]` scenario document through Mode P and
  deterministic fixture Mode D, evaluates per-phase `consistency` oracles for
  thread list, fleet counts, Gym state, and runtime badges, and emits a
  first-disagreement bug payload with both mode states attached. See
  `docs/qa/khala-code-cross-mode-consistency.md`.

## 5. Explore Mode: The Free-For-All That Cannot Lie

Two explorers, sharing the driver and oracles:

**5.1 The seeded monkey (deterministic, cheap, always on).** A seeded PRNG
walker over the *enabled* action space: visible clickable elements
(harvested from `data-*` selectors + the AX tree), the slash-command
registry, hotbar slots, composer input with fuzz corpora, RPC calls with
schema-generated arguments. Seed + action log are recorded, so any crash
replays exactly. This is where "everything needs to behave normally when
clicked around" gets enforced at scale: thousands of random interaction
sequences per night on the fixture backend, with the oracle set (§6) as the
tripwire. No LLM, no tokens, no flake excuses.

**5.2 The LLM explorer (qa-runner live brain).** Goal-directed free-roam
("exercise the fleet panel until something surprises you", "try to make the
composer lose input", "find a way to see text that should be redacted") with
the full oracle set attached. Its two superpowers over the monkey: it reads
meaning (it can judge "this error message is unhelpful" or "this state looks
inconsistent"), and its passing discoveries feed the **distiller** — an
explore session that finds a bug becomes, after the fix, a committed
deterministic scenario. Explore → distill → regress is the framework's
learning loop.

**Coverage feedback closes the loop.** Both explorers receive the coverage
ledger (§7) as steering input: unvisited RPC methods, unexercised slash
commands, never-rendered item variants, unclicked selectors. Exploration is
biased toward the frontier, so coverage climbs instead of resampling the
happy path.

## 6. The Oracle Catalog

Oracles are the framework's product. Every mode evaluates the applicable
subset on every phase:

| Oracle | What it asserts | Source |
| --- | --- | --- |
| `schema` | Every RPC response / JSONL event / bridge payload decodes against its Effect Schema; unknown fields flagged | new `KhalaCodeRpcClient` schemas (derive from `rpc.ts` types) |
| `consistency` | RPC state ≡ DOM state ≡ AX state (thread lists, fleet counts, gym state, runtime badges) | driver `read()` across modes |
| `invariant` | No console errors/unhandled rejections; no zombie processes after shutdown; runtimeMode label always present; approval cards always answerable; legacy runtime only behind flags with banner | Playwright console/page-error hooks; process table; DOM |
| `public_safe` | qa-runner tripwire + the part2 unsafe-text pattern over DOM text, traces, screenshots-adjacent metadata: no tokens, no local paths, no raw prompts | existing `assertPublicSafeResult`, `part2UiUnsafeTextPattern` |
| `visual` | Geometry (no clipping/overlap), focus rings, reduced-motion, nonblank canvas, screenshot diffs vs blessed baselines per viewport | existing probe library + a new baseline store |
| `perf` | Budgets on named metrics: thread-switch (exists), turn-start latency, first-render, panel-open, memory ceiling, JSONL event lag | `threadSwitchPerformance` pattern generalized (§8) |
| `a11y` | AX tree completeness for interactive elements; keyboard-only completion of every defined scenario | Playwright accessibility snapshot + native AX backend |
| `event` | Expected turn events arrive in order within deadlines; interrupts actually interrupt | `events()` stream (needs bridge SSE, §9 G1) |
| `crash` | App survives; on any crash the seed/action-log/artifacts flush and the run REFUTES | qa-runner ensuring pattern |

## 7. The Coverage Ledger

A per-run, mergeable JSON artifact counting what was exercised:

- RPC methods called (of the 57) and with how many distinct argument shapes;
- slash commands dispatched (of the full Codex registry) and their
  availability states;
- hotbar slots / panels opened; settings keys written; approval decision
  kinds sent;
- `ThreadItem` variants rendered (the parity contract enumerates them);
- selectors clicked / screens screenshotted (for the vision mode);
- lines/branches via Bun's coverage where cheap.

The nightly report is the union across all modes and both explorers, with
the frontier list feeding §5's steering. **A coverage class that stays at
zero for a week becomes an issue automatically** — that is the honest
version of "full coverage": measured, visible, and owned, not asserted.

## 8. Determinism, Measurement, And The Effect Layer

- **Fixture app-server.** The chat-runtime tests already fake the Codex
  app-server; promote that into a first-class **fixture Codex process** (a
  small Bun binary speaking the app-server JSON-RPC protocol from recorded
  notification scripts, including approvals and background terminals). This
  gives Modes P/D/H a fully deterministic live-shaped backend — the single
  biggest flake-killer available.
- **Deterministic env harness.** Port
  `apps/autopilot-desktop/src/testing/deterministic-env.ts` (+
  synthetic-event-service, app-replica) into a shared
  `packages/khala-qa-harness`; adopt `TestClock` for time-dependent logic
  (cooldowns, heartbeat freshness, interrupt timers) instead of real sleeps.
- **Metrics registry.** Generalize `threadSwitchPerformanceSample` into a
  `qaMetrics()` reader on the window API and a `qaMetrics` RPC method: named
  counters/timers the app records anyway (turn latency, render marks via
  rAF/idle callbacks, cache hits, memory via CDP in Mode D). Budgets live in
  scenarios; trends live in the nightly report. This is the "measure
  everything, optimize everything" substrate — you cannot optimize what the
  harness cannot read.
  The first registry schema is `openagents.khala_code.qa_metrics.v1`, with
  samples for thread switch render/RPC timings, `turn_start.latency_ms`,
  `first_render.ms`, `panel.open_ms`, and `cache.hit`. The initial data-backed
  budgets are `budget.khala_code.cockpit_render.50_cards.v1`,
  `budget.khala_code.lifecycle_event_to_card.p95.v1`, and
  `budget.khala_code.supervisor_tick.25_target.v1`; scenario `perf` oracles
  consume the same budget records and fail when fixture samples exceed them.
- **Shared harness package.** The three smoke scripts each carry duplicated
  Vite/waitForHttp/probe helpers; extract once into the harness package so
  every new scenario is ~20 lines, not 400.

## 9. Property-Based, Model-Based, And Formal Tiers

**9.1 Property-based (add `fast-check`).** Highest-value properties:
composer editing (any sequence of type/paste/slash/attach operations never
loses committed input and never desyncs preview from draft), thread-item
projector (any interleaving of `item/*` deltas produces a consistent card;
already fixture-tested, generalize the generator), markdown/diff renderers
(no crash, no HTML injection on arbitrary input).

**9.2 Model-based testing (the fold without Foldkit).** Define explicit
Effect Schema state machines — *models* — for the app's core lifecycles,
even though the shell is imperative DOM:

- thread lifecycle: `none → starting → ready → turn_active → interrupted |
  completed → resumed/forked/archived/deleted`;
- approval lifecycle: `requested → answered(decision) | superseded | turn_interrupted`;
- fleet delegate program: the six modules with their precondition/fallback
  edges;
- app-server supervisor: `idle → starting → ready → restarting → disposed`.

fast-check's model-based mode then generates command sequences against the
*real app via Mode P* and asserts the app's observable state stays
bisimilar to the model. Every divergence is either an app bug or a model
bug — both are wins. **This is also the argument for migrating the shell to
Foldkit over time**: Foldkit's `main.ts`-importable pure `update` makes the
model and the implementation the same artifact, collapsing this tier's
maintenance cost. Recommend: new panels get written as Foldkit programs;
the shell migrates opportunistically.

**9.3 Formal verification (bounded, per the workspace contract).** TLA+ is
for design-level state machines with interleaving; use it exactly where
that bites:

- **`khala.fleet.delegate`**: model the six modules, the recovery ladder
  (`dispatch → advertise_capacity` loop), bounded retry counters, and
  concurrent delegate runs against one Pylon. Properties: no dead-end state
  (the `0/1 available` class is unreachable), termination under bounded
  retries, no capacity oversubscription (active assignments ≤ advertised).
  This program is small, typed, and already burned us once — it is the
  perfect first spec.
- **Approval protocol**: no lost approvals (every server request gets
  exactly one typed response or a turn-interrupt), no response forgery to a
  stale request id.
- **Session/thread mapping**: desktop sessionId ↔ Codex threadId persistence
  across crash/reload never orphans or double-binds a thread.

Discipline (from the workspace `INVARIANTS.md`): one invariant per spec, a
checked config, counterexamples converted into scenario fixtures, and the
model never authorizes runtime behavior. Skip formal methods everywhere
else — the oracle catalog is the broader net.

## 10. Optimization Loops (Testing The Tester)

- **Explore-policy GEPA.** qa-runner already has `failure-learning-gepa.ts`.
  The explorer's textual policy (its goal prompts, action-selection
  heuristics, frontier weighting) is a GEPA-optimizable parameter set with a
  crisp metric: new-coverage-per-action and confirmed-bugs-per-thousand
  actions. Same rules as the delegation loop: offline optimization, Gym
  admission, candidates never auto-promote.
- **Perf regression as optimization input.** The metrics registry (§8)
  yields per-commit trends; budgets catch regressions, and the same data
  ranks optimization targets (worst p95s first) instead of guessing.
- **Scenario portfolio pruning.** Track per-scenario yield (bugs caught /
  runtime cost); the nightly matrix orders scenarios by yield so the
  cheapest most-catching tests run first (Tier-1 pre-push stays under its
  bound honestly).

Implementation seam: `packages/khala-qa-harness/src/explore-policy-gepa.ts`
keeps GEPA explore-policy candidates offline, Gym-admitted, and
non-self-promoting, then ranks the scenario portfolio by confirmed-bug yield
per runtime cost.

## 11. Build List (Phased, Minimal-First)

**G-gaps (framework spine):**

1. **G1 — Bridge events + auth.** Add `GET /rpc/events` (SSE) carrying
   `chatTurnEvent` + console/crash events over the preview bridge, and a
   loopback bearer token + optional read-only mode. Small; unblocks Mode P
   turn observation and is a productization prerequisite. (The no-auth
   full-surface bridge is fine for local dogfood but must not ship beyond
   it.)
2. **G2 — `KhalaCodeRpcClient` + schema oracles.** Typed Effect client for
   all 57 methods with response Schemas; the `schema` and `consistency`
   oracles fall out of it.
3. **G3 — qa-runner desktop backend.** `khala-desktop-backend.ts` in
   qa-runner: boots the app (`OPEN_WINDOW=0`, fixture or live backend),
   composes the Chromium surface + RPC client + JSONL; headed variant arms
   the existing native macOS AX backend against the real Electrobun window.
4. **G4 — Scenario DSL + driver service + shared harness package**
   (`packages/khala-qa-harness`): extract the duplicated Vite/probe helpers,
   implement the four drivers, load/run/report scenarios, wire the
   verifier + distiller.
5. **G5 — Fixture Codex app-server process** (§8) for deterministic
   live-shaped runs.
6. **G6 — Coverage ledger** (§7) with the frontier report.

**Phases:**

- **P0 (days): G2 + G4-lite.** RPC client, scenario schema, the DOM driver
  reusing today's smoke pattern, oracle set {schema, invariant,
  public_safe, visual, crash}, and the mechanical seed corpus for threads +
  fleet + settings. Immediate payoff: replaces `app-shell.test.ts`'s
  source-text assertions with real driven tests.
- **P1 (week): G1 + G5 + seeded monkey.** Deterministic explore nights on
  the fixture backend; event oracle; perf metrics registry + budgets.
- **P2 (week): G3 + LLM explorer + distiller wiring.** Vision mode headless
  and headed; explore → distill → regress loop live; coverage ledger
  steering both explorers.
- **P3: model-based + formal tier.** fast-check models for thread/approval
  lifecycles; the `khala.fleet.delegate` TLA+ spec; counterexample →
  fixture pipeline.
- **P4: optimization + productization.** GEPA explore-policy loop; nightly
  matrix on the owned GCE runner (Tier-2 pattern); then §12.

## 12. Productization Path

Everything above doubles as product development for the QA agent line
(#6181 was explicitly "out-ship Factory"; #6191 shipped the OSS package;
there is a named first prospective customer in the feature-request doc):

- Khala Code Desktop becomes the **flagship native-desktop QA target** —
  the demo where the agent drives a real Electrobun app headed via the AX
  backend, finds a seeded bug, and commits the distilled regression test.
  That story (native desktop + distill-to-committed-test + honest verdicts)
  is precisely the driver-breadth lane (#6186) Factory's droid-control
  does not cover.
- The **driver contract + scenario DSL** generalize: any Electrobun/Tauri/
  web app with an RPC seam gets Mode P; anything with a URL gets Modes D/V.
  "Bring your app, get a scenario corpus, an explore night, and a coverage
  ledger" is a sellable unit whose evidence chain (ATIF traces, videos,
  verdicts, receipts) already exists.
- The **owner-gated seams stay owner-gated**: run receipts → settlement
  (#6188) and skill emission remain INERT until deliberately flipped; the
  framework must keep producing evidence that would survive that flip
  (exact accounting, public-safe traces, verifiable verdicts).

## 13. Invariants To Keep

- No GitHub-hosted CI: Tier-1 bounded pre-push + Tier-2 owned-runner
  pattern stays.
- The bridge's full-mutation surface never ships beyond loopback without
  G1's auth; the QA framework itself must not become the unauthenticated
  remote-control hole.
- Live tiers stay skip-safe-by-default with explicit env arming; fixture
  tiers never touch `~/.codex`, real accounts, or spend.
- Public-safety tripwires run in every mode; screenshots and traces are
  evidence and must be redaction-checked like any projection.
- Explore-mode discoveries become deterministic scenarios or they didn't
  happen; "the agent saw it once" is INCONCLUSIVE, not CONFIRMED.
- Formal models and optimizer candidates inform; the Effect authority and
  the owner gate admit.

## 14. Bottom Line

The perfect testing framework for Khala Code Desktop is mostly assembled
from parts this repo already shipped: qa-runner's brains, backends,
distiller, verifier, and evidence chain on one side; the desktop's
full-surface RPC bridge, in-page automation API, probe library, and
headless JSONL contract on the other. What is genuinely new is small and
high-leverage: an events+auth upgrade to the bridge, a typed RPC client, a
desktop backend for qa-runner, one scenario format across four drivers, a
fixture app-server, a coverage ledger, and a bounded formal tier for the
three state machines that deserve it. Build it in that order, let the
seeded monkey and the LLM explorer climb the coverage frontier every
night, distill what they find into committed regressions — and the same
work productizes the QA agent with our own desktop app as its hardest,
best demo.

## 15. Addendum — Post-Roadmap Status (2026-07-02)

Written after the unified [`ROADMAP.md`](./ROADMAP.md) desktop-fleet push
closed (WS-17 readiness gate shipped; ROADMAP_AFTER not started). This
section audits what of the framework above **actually exists on `main`**,
what a fresh hands-on run of every runnable tier produced today
(origin/main `63c5c43b26`), what that run caught, and the honest answer to
"can we start fully automated QA against Khala Code Desktop now?"

### 15.1 What got built (design → main)

Every G-gap and nearly every WS-6 task from §11 is implemented and on
`main`:

| Design item | Artifact on `main` | Status |
| --- | --- | --- |
| G1 bridge auth + events | `clients/khala-code-desktop/src/bun/index.ts` — per-boot bearer (`x-khala-code-preview-token`, printed at boot), typed `rpc_unauthorized` / `rpc_read_only` rejections, `GET /rpc/events` SSE | **Live** (exercised today, §15.2) |
| G2 typed RPC client + schema/consistency oracles | `packages/khala-qa-harness/src/rpc-client.ts` | **Live** |
| G3 qa-runner desktop backend | `apps/qa-runner/src/khala-desktop-backend.ts` + `native-desktop-backend.ts` (headed AX, `QA_NATIVE_DESKTOP=1`) | Built; headed variant **never yet pointed at the real packaged app** |
| G4 scenario DSL + driver + shared package | `packages/khala-qa-harness` (`scenario.ts`, `driver.ts`, `rpc-driver.ts`, `runner.ts`, `desktop-smoke-helpers.ts`, `deterministic-env.ts`) | **Live** |
| G5 fixture Codex app-server | `src/bun/fixture-codex-app-server.ts` (+ test); harness `real-app-fetch.ts` composes the real RPC handlers over it in-process — Mode P without a browser | **Live** |
| G6 coverage ledger + frontier | `coverage-ledger.ts`; monkey night emits `artifacts/monkey-night-coverage-ledger.json` (RPC methods × argument shapes, hotbar panels, approval kinds) | **Live**; nightly union + zero-for-a-week auto-issue **not scheduled anywhere** |
| T6.7 seed corpus | `seed-corpus.ts` + `KHALA_CODE_QA_SEED_CORPUS_MANIFEST` | **Live** |
| T6.8 seeded monkey | `monkey-explorer.ts`, `monkey-night.ts` CLI | **Live** |
| T6.9 LLM explorer | `explorer-brain.ts` (`deterministic_fixture` tier proven; `live_llm` tier typed, needs a model) | Partial |
| T6.10 live smokes | `apps/pylon` `smoke:fleet-run-live` / `smoke:fleet-run-sustained`, skip-safe, full arming contract printed on skip | **Live** (unarmed skip verified today) |
| T6.11/T6.12 property + model-based | fast-check in harness; `model-based.ts` | **Live** |
| T6.13 formal tier | `specs/khala-fleet-delegate/FleetDelegateSupervisor.tla`, `specs/approval-protocol/`, `specs/session-thread-mapping/`, + mutation specs | **Live** (bounded) |
| T6.14 perf registry + budgets | `src/shared/qa-metrics.ts` — `qaMetrics` RPC + the three budgets (`cockpit_render.50_cards`, `lifecycle_event_to_card.p95`, `supervisor_tick.25_target`) | Built; **webview samples still not reachable from the RPC in real runs** (the known T6.14 gap) |
| T6.15 GEPA explore-policy | `explore-policy-gepa.ts` | Built, postponed lane (owner 2026-07-02) |

### 15.2 Fresh run evidence (all executed 2026-07-02)

- `bun run --cwd packages/khala-qa-harness test` — **50/50 pass**, 8 files,
  ~2s (scenario runner, seed corpus, coverage ledger, monkey, model-based,
  RPC client, explore-policy, explorer brain).
- `bun src/monkey-night.ts --runs 10 --steps 32` — **10/10 pass**, coverage
  ledger artifact written, seed+log replay refs recorded.
- `bun test tests/*.test.ts` in the desktop — **495/495 pass**, 64 files,
  ~5s (one non-reproducing single-test error in one of three suite runs;
  by principle 2 every flake is a bug — worth chasing when it recurs).
- `bun test src/khala-desktop-backend.test.ts` in qa-runner — 6/6 pass.
- **Mode P against the real bridge**: `KHALA_CODE_DESKTOP_OPEN_WINDOW=0
  bun src/bun/index.ts` boots headless on :50021; `/health` OK; RPC with
  the boot token succeeds; RPC without it returns typed
  `rpc_unauthorized`; `GET /rpc/events` streams. G1 is real, not aspirational.
- `smoke:fleet-run-live` unarmed — clean structured skip naming the full
  arming contract (`PYLON_FLEET_RUN_LIVE_ARM=1` + pins).

### 15.3 What the run caught (the framework doing its job)

Both Mode D visual smokes were **red on `main`** when this audit started —
`smoke:part2-ui` and `smoke:cockpit-visual` timed out, despite both being
green at origin/main on 07-01 (episode-245 doc §1.1). Root causes, found
by driving the real UI under Playwright with per-step diagnostics:

1. **`smoke:part2-ui`**: the cockpit now requires the `fleetRunList` RPC
   (WS-3 fleet-run parity). The smoke's mock table predated it; the
   default-500 made the cockpit render `Could not load fleet status:
   fleetRunList failed with 500` and `Worker Codex accounts` never
   appeared. Fixed in this change (mock added).
2. **`smoke:cockpit-visual`**: the T13.2 Foldkit cockpit embed
   deliberately removed the condensed sidebar counts element
   (`data-khala-code-fleet-counts`) — `tests/app-shell.test.ts:877` pins
   the *absence* — but the smoke's `expectCountLabel` + geometry oracle
   still required it. Fixed in this change (stale assertions removed,
   consistent with the pinned removal).

Both smokes are green again, and `bun run verify` (typecheck + 495 tests +
UI build + bun build) passes from a clean worktree.

Two meta-findings matter more than the individual fixes:

- **Nothing runs Modes D/V on any cadence.** The visual smokes are not in
  `verify`, not in any pre-push tier, and no nightly/cron exists for them
  or for the monkey night (repo-wide grep: zero automation references).
  Two roadmap lanes landed UI changes and the visual tier silently broke
  within a day. The machine is built; **the loop is not running.**
- **Cockpit robustness gap (product bug, not test bug):** one failed RPC
  (`fleetRunList`) blanks the entire cockpit, including account data that
  arrived successfully from `codexFleetStatus`. The cockpit should render
  what it has and degrade the run section. This is exactly the §6
  `consistency`-oracle class the framework exists to catch.

### 15.4 The honest gap: fixture-green ≠ the app the owner uses

The owner-observed reality (laggy app, broken interactions) is consistent
with this audit: the fixture tiers are green while the tiers that would
see what the owner sees are either red-until-today, unwired, orunscheduled:

1. **Lag is not yet measurable in real runs.** The `qaMetrics` registry
   and budgets exist, but webview samples are not reachable from the RPC
   in real (non-fixture) runs — the T6.14 bun-side sample bridge is the
   single most leveraged missing piece given the lag complaint. Until it
   lands, the perf oracle literally cannot see the slowness.
2. **The headed real window has never been driven.** qa-runner's native
   macOS AX backend + `khala-desktop-backend.ts` exist, but no run has
   pointed them at the actual packaged Electrobun window
   (`QA_NATIVE_DESKTOP=1`). Everything Mode D/V tested so far is the Vite
   preview, not the WKWebView the owner uses.
3. **Mode H needs a real Codex login** (structured failure without it) and
   the live tiers (T6.10) need owner-armed env — both by design, both
   unexercised in this audit.
4. **No scheduled loop**: no nightly monkey night, no coverage-frontier
   union report, no auto-issue on zero-for-a-week coverage classes, no
   visual smokes in any gate.

### 15.5 Verdict: can fully automated QA start now?

**Yes — the fixture tier can start today, unattended, with commands that
were all proven in this audit:**

```sh
bun run --cwd packages/khala-qa-harness test          # scenario/oracle tier
bun run --cwd packages/khala-qa-harness monkey:night  # seeded monkey + ledger
bun run --cwd clients/khala-code-desktop verify        # typecheck + suite + build
bun run --cwd clients/khala-code-desktop smoke:part2-ui
bun run --cwd clients/khala-code-desktop smoke:cockpit-visual
bun run --cwd clients/khala-code-desktop smoke:composer-visual
bun run --cwd apps/pylon smoke:fleet-run-live          # skip-safe until armed
```

What "starting for real" requires, in leverage order — now executed as
[`ROADMAP_QA.md`](./ROADMAP_QA.md) (epic #8051, issues #8012–#8050):

1. **Schedule the loop** (the cheapest, highest-value step): a nightly
   owned-runner job (Tier-2 pattern, no GitHub-hosted CI) running the
   block above plus the coverage-ledger union/frontier report, and the
   visual smokes added to the pre-push or merge gate so UI lanes cannot
   silently break Mode D again. Everything §15.3 caught would have been
   caught the night it landed.
2. **Land the T6.14 real-run perf bridge** (webview → bun sample path) so
   thread-switch, turn-start, panel-open, and cockpit-render samples flow
   from the app the owner actually uses, against the existing budgets.
   This converts "it feels laggy" into ranked regression data — the §10
   optimization loop is inert without it.
3. **One headed AX run** against the packaged Electrobun app to prove Mode
   V end-to-end (it is also the productization demo, §12).
4. **Arm the live tiers** once (owner sitting): `smoke:fleet-run-live`
   with two real issues, then `smoke:fleet-run-sustained` — the fleet-side
   half of the framework has never run in anger.
5. **Wire the LLM explorer's live brain** (qa-runner live mode against the
   Mode P driver) and start the explore → distill → regress loop with the
   coverage frontier as steering.

The bottom line restated for this milestone: the marriage contract §0
called for has been signed — drivers, oracles, corpus, monkey, ledger,
fixture backend, formal specs all exist and run. What does not yet exist
is the *ritual*: nothing runs the suite nightly, nothing gates merges on
the visual tier, and the perf oracle cannot yet see real-app lag. The
framework caught two real regressions the first time someone actually ran
it end to end — which is both the proof it works and the proof it must be
put on a schedule.
