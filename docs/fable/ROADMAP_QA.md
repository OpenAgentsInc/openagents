# ROADMAP_QA — The Fully Automated Khala Code QA Cycle

Date: 2026-07-02
Status: **active execution roadmap.** Successor to WS-6/WS-14 of
[`ROADMAP.md`](./ROADMAP.md) and direct implementation of §15.5 of the
[QA framework design](./2026-07-01-khala-code-desktop-qa-framework-design.md)
(post-roadmap addendum). The machine exists — drivers, oracles, seed corpus,
monkey, coverage ledger, fixture app-server, TLA+ specs, budgets all run
green on `main` today. **What does not exist is the ritual**: nothing runs
nightly, nothing gates merges on the visual tier, real-app lag is not yet
measurable, the headed native window has never been driven, and the live
tiers have never run armed. This roadmap closes every one of those gaps to
"fully implemented": all gates, all latency budgets, all tested use cases,
the scheduled loop, the explorers, and the live tiers.

**Owner arming (2026-07-02):** the owner has directed full execution with
no pending approvals — live tiers may run against the currently-logged-in
Codex/Claude accounts (isolated worker homes only, per invariants), and the
previously postponed explore-policy optimization lane is reopened here.
Spend-bearing runs remain no-spend own-capacity; money movement stays
owner-enveloped as always.

Delivery mechanics are [`EXECUTION.md`](./EXECUTION.md): one GitHub issue
per task (issue map in §12; epic #8051), one PR per issue, clean worktrees, verify-green
before review, merged-to-`main` is the only "done". Delegability grades as
in ROADMAP.md (HIGH = fleet worker with bounded prompt; MED = tight issue +
reviewer attention; LOW = supervisor).

## 0. Ground Truth (what already runs, verified 2026-07-02)

All fresh-run evidence lives in §15.2–15.3 of the QA design doc. Summary:
harness suite 50/50; desktop suite 495/495 (~5s); 10×32 monkey night clean
with coverage-ledger artifact; qa-runner desktop backend 6/6; Mode P against
the real HTTP bridge live (headless boot, bearer enforced, SSE streaming);
visual smokes red-then-fixed (`fleetRunList` mock gap, retired sidebar
counts element) — proof both that the framework catches real regressions
and that nothing was running it. The two commits that closed the audit:
`d9793d1607` (smoke fixes), `4bb6910040` (addendum).

## 1. Dependency Spine

```text
QA-1 The Loop (nightly + gates)  ── unblocks trend/report/auto-issue lanes
        |
   QA-2 Real-run latency (perf bridge → budgets → sweep → fixes)
        |
QA-4 Use-case corpus ──► QA-8 Gates hard-fail (schema oracle, arch scan, TLC)
        |
QA-3 Headed Mode V ──► QA-6 Explorers (LLM brain, distill loop, monkey scale)
        |
QA-5 Live tiers (armed NOW; weekly cadence lands after QA-1)
        |
QA-7 Product fixes the QA run already found (cockpit degradation, flake hunt)
        |
QA-9 Planner/Coder/Judge workflow (build + test; role registry first)
        |
QA-10 QA Swarm productization (epic #8071; consumes Q1/Q2/Q6 outputs)
```

QA-1.1 (the nightly job) and Q2.1 (the perf bridge) are the two
foundations; everything else fans out. QA-5.1 (armed live smoke) has no
code dependency and starts immediately under the owner arming.

## 2. QA-1 — The Loop: scheduled automation and merge gates

The single highest-leverage workstream. Everything §15.3 caught would have
been caught the night it landed if this existed.

| Task | Description | Deps | Delegable |
| --- | --- | --- | --- |
| Q1.1 | **Nightly QA matrix on the owned runner** (Tier-2 pattern, no GitHub-hosted CI): one scheduled job runs, in order — harness suite, desktop `verify`, all fixture visual smokes (`part2-ui`, `cockpit-visual`, `composer-visual`), monkey night (≥1000 total actions), model-based tier, property tier — then writes a dated public-safe run report (JSON + markdown) and the merged coverage ledger. Any failure files/updates a strict-form issue automatically with seed/log/artifact refs. | — | MED |
| Q1.2 | **Visual smokes gate merges.** Bounded Tier-1 pre-push (or merge-queue) step for changes touching `clients/khala-code-desktop` or `packages/ui`: the three fixture visual smokes must pass. Warning-only for the first week, then hard-fail. Time-box the step (<5 min) and keep it skip-safe for non-desktop changes. | — | HIGH |
| Q1.3 | **Coverage union + frontier + auto-issue** (T6.6 completion): nightly union of all per-run ledgers; frontier report (unvisited RPC methods, unexercised slash commands, never-rendered item variants, unclicked selectors) published in the run report; any coverage class at zero for 7 consecutive days ⇒ auto-issue. | Q1.1 | HIGH |
| Q1.4 | **Flake policy + quarantine**: every intermittent failure is a bug (design principle 2 — there is no third category). Nightly job retries a failure once; if it flips, it is quarantined into a tracked list with an auto-issue carrying seeds/logs, never silently retried green. First target: the 1-in-3 desktop-suite single-test error observed 2026-07-02 (§15.2). | Q1.1 | MED |
| Q1.5 | **QA status surface**: the nightly report published as a public-safe JSON artifact + summary (owned-runner artifact store or committed `docs/qa/reports/`), with pass/fail, coverage counts, perf trends, and live-tier status — the owner reads one page to know whether the app is healthy. | Q1.1, Q1.3 | MED |

## 3. QA-2 — Real-run latency: make lag measurable, then kill it

The owner-observed lag is invisible to the framework today because webview
perf samples are not reachable from the RPC in real runs (the known T6.14
gap). This workstream is the lag lever.

| Task | Description | Deps | Delegable |
| --- | --- | --- | --- |
| Q2.1 | **The bun-side sample bridge** (T6.14 completion): webview `qaMetrics()` samples flow to the Bun host continuously (preview bridge message or SSE back-channel) and out the `qaMetrics` RPC in real runs — preview AND packaged app. Fixture test + a live assertion that a real thread-switch produces a real sample through the RPC. | — | MED |
| Q2.2 | **The full latency budget family as data** (extends the v1 budgets): `budget.khala_code.startup_interactive.v1` (cold boot → interactive < 3000ms), `thread_switch.optimistic.v1` (< 100ms), `thread_switch.full.v1` (< 400ms), `turn_start.first_event.v1` (< 400ms), `composer.keystroke_echo.p95.v1` (< 16ms), `panel.open.v1` (< 150ms), `sse.event_to_ui.p95.v1` (< 250ms), `transcript.scroll_dropped_frames.v1` (< 5%), `app_server.spawn_ready.v1` (< 2000ms) — plus the existing cockpit-render/lifecycle-p95/supervisor-tick three. Encoded as data, consumed by scenario `perf` oracles, evaluated in the nightly. | Q2.1 | HIGH |
| Q2.3 | **Lag profiling sweep of the real app**: instrumented real-bridge sessions (Mode P + Mode D on the built webview, then packaged) collecting every budget's samples under realistic load (long transcript, 50-card cockpit, streaming turn); rank worst p95s; file one optimization issue per offender with the sample evidence attached. This is §10's "perf regression as optimization input" made real. | Q2.1 | MED |
| Q2.4 | **Memory/leak/zombie oracle**: RSS + JS heap ceiling after a monkey night (`memory.rss_after_monkey_night.v1` < 1.5GB, no monotonic growth across runs); zero orphan processes after driver `shutdown()` (invariant oracle), wired into the nightly. | Q1.1 | HIGH |
| Q2.5 | **Perf trend reporting + regression auto-issue**: per-budget nightly trends in the Q1.5 report; any budget regression ⇒ auto-issue with the offending samples. | Q1.1, Q2.2 | HIGH |
| Q2.6 | **Burn down the ranked lag offenders** (umbrella): child issues from Q2.3's ranking, fixed to budget-green — includes the known full-re-render-per-event and module-level-state classes from the Effect audit if they surface in the ranking. Definition of done: every Q2.2 budget green in the nightly for 7 consecutive days on the real app. | Q2.3 | MED |

## 4. QA-3 — Headed native Mode V: drive the real window

Everything Mode D tested so far is the Vite preview. The owner uses the
packaged Electrobun window; the AX backend exists and has never been
pointed at it.

| Task | Description | Deps | Delegable |
| --- | --- | --- | --- |
| Q3.1 | **First headed AX run**: qa-runner native macOS backend (`QA_NATIVE_DESKTOP=1`) drives the packaged app (`electrobun build` output): boot, hotbar navigation, composer type/submit against fixture backend, screenshot; committed runbook with the exact invocation. | — | MED |
| Q3.2 | **Headed scenario subset weekly**: the seed-corpus smoke subset through the vision driver on the packaged app; AX-tree a11y oracle (every interactive element present in the AX tree; keyboard-only completion of one defined scenario). | Q3.1, Q4.1 | MED |
| Q3.3 | **Screenshot baseline store + diff oracle**: blessed baselines per viewport (desktop/mobile, dark, reduced-motion), redaction-checked like any projection; diffs fail the visual oracle. | Q3.1 | MED |
| Q3.4 | **The flagship demo** (productization, #6181 lane): seeded-bug hunt on the packaged app headed — agent finds the bug from screenshots/AX, distiller emits the committed regression test. Recorded as the qa-runner product demo. | Q3.1, Q6.2 | MED |

## 5. QA-4 — The use-case corpus: every surface, every state

"Everything needs to behave normally when clicked around" becomes an
enumerated, loader-enforced corpus. A phase without an oracle is rejected.

| Task | Description | Deps | Delegable |
| --- | --- | --- | --- |
| Q4.1 | **Complete the mechanical corpus — one lifecycle scenario per RPC group**: threads (start/list/read/rename/archive/delete/fork/compact/resume), turns (start/interrupt/steer), approvals (request→every decision kind), settings/config, models/personality, ecosystem (MCP servers, skills, plugins, apps, hooks), fs/mentions/attachments, background terminals, slash commands, token summaries, fleet (status/delegate/promote), fleet-run (start/status/control/list — pause/resume/drain/stop each), sessionCatalog, forum panel (browse/post/tip surfaces against the host proxy fixture), inbox routing (every flag kind), gym pane, plans/billing panel, headless events, qaMetrics. | — | HIGH |
| Q4.2 | **ThreadItem variant coverage**: one render scenario per parity-contract variant, replayed from the pinned fixtures; rendered-variant counts flow to the coverage ledger. | — | HIGH |
| Q4.3 | **Slash-command sweep**: one scenario per registry command, including unavailable/disabled states; registry enumeration keeps the loop mechanical (coverage is a loop, not a wishlist). | — | HIGH |
| Q4.4 | **Error-state corpus**: Codex binary missing; auth expired; Pylon offline; single-RPC failure with partial degradation (the Q7.1 contract); corrupt session-state recovery; MCP server down; network loss mid-turn; interrupt mid-tool-call; app-server crash + supervisor restart. Every case has expected typed degraded UI, no console errors, no data loss. | — | HIGH |
| Q4.5 | **Cross-mode consistency runs**: the corpus runs through Mode P and Mode D with the `consistency` oracle (RPC state ≡ DOM state for thread lists, fleet counts, gym state, runtime badges); any mode disagreement is a bug (N-version testing). | Q4.1 | MED |
| Q4.6 | **Console-error oracle everywhere**: every Mode D smoke and scenario asserts zero unexpected console errors / unhandled rejections; boot-time RPC failures in fixtures are deliberately mocked or expected — the current 500-noise class dies here. | — | HIGH |

## 6. QA-5 — Live tiers (owner-armed 2026-07-02: GO)

Skip-safe stays the default in CI; the armed runs below execute now under
the owner direction, against the currently-logged-in accounts, isolated
worker homes only, no-spend own-capacity closeouts, exact-row evidence.

| Task | Description | Deps | Delegable |
| --- | --- | --- | --- |
| Q5.1 | **Armed `smoke:fleet-run-live`**: target 2, two real issues, two distinct PRs, verify-green closeouts, exact `token_usage_events` rows, public `khala-tokens-served` delta reconciliation, zero duplicate claims. Evidence archived in the run report and the issue. | — (armed) | MED |
| Q5.2 | **Armed `smoke:fleet-run-sustained`**: ≥5 workers for ≥30 minutes, ≥2 observed refills, zero duplicate claims, cooldown rotation observed. | Q5.1 | MED |
| Q5.3 | **Codex live parity + Mode H live**: `smoke:codex-parity-live` armed against the logged-in Codex install; headless `--json` JSONL run with a real turn; JSONL schema oracle on every event; gaps recorded in the parity matrix. | — (armed) | MED |
| Q5.4 | **Claude harness live smoke**: one live Claude chat turn through the desktop runtime (pill → `claude_runtime`), approval callback exercised, exact token row lands via the Claude turn reporter path; scenario asserts the runtime badge and closeout diagnostics. | — (armed) | MED |
| Q5.5 | **Weekly armed cadence**: the owned runner runs Q5.1/Q5.3/Q5.4 weekly (Q5.2 monthly) with request-count caps, honest skip reporting when accounts are cold, and evidence rolled into Q1.5. | Q5.1–Q5.4, Q1.1 | MED |

## 7. QA-6 — Explorers and the learning loop

| Task | Description | Deps | Delegable |
| --- | --- | --- | --- |
| Q6.1 | **LLM explorer live brain**: wire qa-runner's live ReAct brain to the Mode P/D drivers (hosted Khala default, BYO model, `--fake-model` CI path); goals seeded from the coverage frontier ("exercise what the ledger says is untouched"). | Q1.3 | MED |
| Q6.2 | **Explore → distill → regress**: passing explore sessions distill into committed deterministic scenarios via the existing distiller; a discovery that cannot be distilled is INCONCLUSIVE, not shipped. First distilled regression committed = done. | Q6.1 | MED |
| Q6.3 | **Monkey night at scale**: nightly ≥5000 actions across fixture backends including the 18-worker cockpit state; claim-invariant oracle on every cockpit night; any crash's seed + action log auto-committed as a replay fixture. | Q1.1 | HIGH |
| Q6.4 | **Explore-policy GEPA reopened** (was T6.15, postponed; reopened by owner directive 2026-07-02): offline optimization of explorer goal prompts/frontier weighting on new-coverage-per-action and confirmed-bugs-per-1000-actions; Gym-admitted; candidates never auto-promote. | Q6.1, Q6.3 | MED |

## 8. QA-7 — Product fixes the QA run already found

These are product bugs, not test bugs — the first yield of actually running
the framework (§15.3–15.4).

| Task | Description | Deps | Delegable |
| --- | --- | --- | --- |
| Q7.1 | **Cockpit partial degradation**: one failed RPC (`fleetRunList` today) must not blank the whole cockpit — render what arrived, degrade the failed section to a typed error chip with retry. Scenario + regression in the corpus (Q4.4 case). | — | MED |
| Q7.2 | **Boot-RPC degraded states**: every boot-time RPC (`harnessSettingRead`, `sessionCatalog`, `events`, `claudeApprovalPending`, `fleetRunList`, `codexFleetStatus`) has a typed degraded UI state and no unhandled console error on failure. | Q7.1 | MED |
| Q7.3 | **Hunt the intermittent suite error**: reproduce the 1-in-3 single-test error from 2026-07-02, fix it (TestClock where real timing is the cause), and add it to the Q1.4 quarantine ledger as the first solved case. | — | MED |
| Q7.4 | **Real-bridge CI smoke** (closes the in-process vs productized gap): boot the real Bun host headless with `KHALA_CODE_CODEX_COMMAND` pointed at the fixture app-server binary; run the seed-corpus Mode P scenarios over actual HTTP + bearer + SSE (not the in-process `real-app-fetch` shortcut); wire into the nightly. | Q4.1 | MED |

## 9. QA-8 — Gates hard-fail and guardrails (WS-14 completion)

| Task | Description | Deps | Delegable |
| --- | --- | --- | --- |
| Q8.1 | **Arch scan hard-fail** (T14.2 completion): the report-only scan over `clients/khala-code-desktop` + `packages/khala-tools` (`JSON.parse … as`, bare `catch{}`, direct env reads, `Date.now()` in logic, stray `Effect.runPromise`, `setTimeout` kills) promotes to hard-fail in `verify`. | — | HIGH |
| Q8.2 | **Schema oracle in the gate**: fixture-tier runs decode every RPC response against the `rpc.ts` schemas with unknown-field flagging; a schema-drift failure blocks merge like any test. | Q4.1 | HIGH |
| Q8.3 | **TLC in the loop**: bounded TLC model checks for the three specs + mutation specs run in the nightly; the counterexample→fixture pipeline documented and exercised once end-to-end. | Q1.1 | MED |
| Q8.4 | **Public-safety oracle in every mode**: `assertPublicSafeResult` / the part2 unsafe-text pattern runs on DOM text, traces, JSONL, and screenshot-adjacent metadata in every smoke and scenario, headed included. | — | HIGH |
| Q8.5 | **Coverage floor gate**: after one week of Q1.3 baselines, enforce "a PR may not reduce the covered-RPC-method count" — soft-warn for a week, then hard. | Q1.3 | MED |
| Q9.1 | [#8052](https://github.com/OpenAgentsInc/openagents/issues/8052) | Typed model-role registry (architect/coder/judge/advisor) |
| Q9.2 | [#8053](https://github.com/OpenAgentsInc/openagents/issues/8053) | Plan-first chat flow (/architect plan card) |
| Q9.3 | [#8054](https://github.com/OpenAgentsInc/openagents/issues/8054) | Judge-on-diff verdict card in chat |
| Q9.4 | [#8055](https://github.com/OpenAgentsInc/openagents/issues/8055) | Advisor runtime (turn-level second model, bounded) |
| Q9.5 | [#8057](https://github.com/OpenAgentsInc/openagents/issues/8057) | Per-role economics surface (exact rows, honest dollars) |
| Q9.6 | [#8058](https://github.com/OpenAgentsInc/openagents/issues/8058) | architect-coder-judge preset + promise record |
| Q9.7 | [#8059](https://github.com/OpenAgentsInc/openagents/issues/8059) | Planner/Coder/Judge under the QA regime (prove it exists) |

## 9b. QA-9 — Planner/Coder/Judge workflow: build it, wire it, prove it

Source: [`2026-07-02-oh-my-pi-planner-coder-judge-audit.md`](./2026-07-02-oh-my-pi-planner-coder-judge-audit.md)
(gaps G1–G6, adopted into this roadmap by owner direction 2026-07-02 —
"the roadmap includes building and testing this workflow and making sure it
exists"). The fleet-scale halves already landed (T9.4 plan-then-fan-out,
T9.5 second-pass reviewer, harness pill, workerKind); this workstream builds
the single-session chat-first expression — Fable/Claude as
architect and judge, the user's subscription-routed Codex as coder, an
optional turn-level advisor — and puts the whole trio under the same QA
regime as everything else in this roadmap. omp is patterns-only reference:
no vendored code or naming, no gray-proxy provider entries (Fable routes
through legitimate Anthropic rails only; subscription no-resale stays
non-waivable), and verify-command/deterministic-program authority is never
weakened — advisors and judges are advisory data under it.

| Task | Description | Deps | Delegable |
| --- | --- | --- | --- |
| Q9.1 | **Typed model-role registry** (adapts omp §2.1): `openagents.khala_code.model_roles.v1` — `{ role: architect\|coder\|judge\|advisor, harness: codex\|claude\|khala, model?, effort?: minimal…xhigh }` as a persisted schema-first setting with a Settings surface; consumed by chat, fleet dispatch, and the reviewer; effort maps harness-natively (Codex app-server config, Claude SDK thinking). The harness pill grows from "which engine chats" to "which engine plays which role". | — | HIGH |
| Q9.2 | **Plan-first chat flow** (adapts omp §2.4; promotes landed T9.4 into the default surface): a composer plan-mode toggle / `/architect` runs the architect role (Claude plan mode, read-only) against the `claude_plan_fanout_dag.v1` contract and renders an approvable plan card; approval dispatches small plans as in-thread coder turns and large plans as a FleetRun; the plan artifact persists with the session. | Q9.1 | MED |
| Q9.3 | **Judge-on-diff in chat** (adapts omp §2.5; surfaces landed T9.5): after a coder turn or worker closeout produces a diff, the judge role renders a structured verdict card (`accept\|request_changes\|replan`, P0–P3 findings with file/line anchors and per-finding confidence); "request changes" feeds the annotate-diff steering loop (T15.1). Verify commands remain the only merge authority; the verdict is advisory data. | Q9.1 | MED |
| Q9.4 | **Advisor runtime** (adapts omp §2.3; the largest new piece): `KhalaAdvisorRuntime` as an Effect service — a Claude session with its own context consuming turn deltas of the active Codex thread, read-only workspace inspection, severity-typed advisories (`nit\|concern\|blocker`); `nit` batches to a transcript card, `concern\|blocker` injects steering via `codexTurnSteer`. Control-theory invariants ported as code, not prompt: emission guard (dedupe + noise filter), `immuneTurns` interrupt budget, reset on compaction/thread-switch, advisor-never-a-peer, separate exact token rows for advisor usage. | Q9.1, WS-8 P2 | MED |
| Q9.5 | **Per-role economics surface** (adapts omp §2.7): `role_ref` attribution on exact token rows (desktop telemetry + Pylon turn reports), API-metered roles priced from the model catalog, per-session honest rendering — "coder: subscription-covered · architect+judge: $X.YZ"; projections from exact rows only, `not_measured` when a rail's pricing is unknown. | Q9.1 | HIGH |
| Q9.6 | **One-command preset + promise record**: `khala code --preset architect-coder-judge` and a Settings preset card — coder = the user's existing Codex login, architect/judge = the user's own Anthropic auth (API key or Claude login via the SDK; never a proxy), advisor optional-on; candidate promise `khala_code.architect_coder_judge.v1` filed through `docs/promises/` (copy-gated; no public copy until the flow is verifiable end-to-end). | Q9.1–Q9.5 | HIGH |
| Q9.7 | **The workflow under the QA regime — prove it exists**: scenario-corpus additions for every new surface (role-registry RPC group, plan card approve/reject, judge verdict card incl. every verdict kind, advisor advisories incl. the interrupt budget and dedupe guard) on the fixture tier (fixture Codex app-server + fixture Claude); coverage-ledger counting for the new surfaces; cross-mode consistency; and a skip-safe env-armed `smoke:architect-coder-judge-live` that runs one real plan → coder turn → judge verdict end-to-end with per-role exact token rows and public-safe projections. Green in the nightly = the workflow demonstrably exists and keeps existing. | Q9.2–Q9.5, Q4.1 | MED |

## 9c. QA-10 — QA Swarm productization (pointer)

The machine this roadmap builds is also a sellable product. The product plan
— packages, the shareable evidence URL, the arbiter-effect swarm board, the
three-effect 3D scene, customer-one dogfood at Khala Code, the named-first-
customer sales motion, and third-party onboarding — lives in
[`2026-07-02-qa-swarm-product-plan.md`](./2026-07-02-qa-swarm-product-plan.md)
(workstream QS1–QS10, issues #8061–#8070 under epic #8071). QS consumes this
roadmap's outputs (Q1.1 nightly, Q1.5 status surface, Q2 budgets, Q6
explorers); it never duplicates them.

## 10. Milestones

- **QM1 — The loop runs.** Nightly matrix green three consecutive nights;
  visual smokes gate desktop merges; coverage frontier published. (Q1.*)
- **QM2 — Lag is a number.** Real-run samples flowing; full budget family
  evaluated nightly; the sweep's offender list filed. (Q2.1–Q2.3)
- **QM3 — The real window is driven.** Headed AX run green on the packaged
  app; screenshot baselines blessed. (Q3.1, Q3.3)
- **QM4 — Live tiers proven.** Q5.1 + Q5.3 + Q5.4 evidence archived; weekly
  cadence scheduled. (QA-5)
- **QM5 — The loop learns.** First explore-session-distilled regression
  committed; monkey at ≥5000 actions/night; claim invariant clean. (QA-6)
- **QM6 — Nothing merges unguarded.** Arch scan, schema oracle, console
  oracle, public-safety oracle, and coverage floor all hard-fail;
  every Q2.2 budget green on the real app for 7 consecutive days. (QA-8, Q2.6)
- **QM7 — The trio is a surface.** Plan→code→judge green in fixture
  scenarios and in the armed live smoke; role economics rendered from exact
  rows; advisor bounded and accounted. (QA-9)

## 11. Invariants (unchanged, restated for this roadmap)

- Fixture tiers never touch `~/.codex`, real accounts, or spend; live tiers
  are skip-safe by default and env-armed deliberately (the owner arming
  covers the Q5 runs and the weekly cadence, not silent standing authority
  for anything spend-bearing).
- Isolated worker homes for every live run; the owner's live `~/.codex` and
  `~/.claude` are never written by connect/test flows.
- Exact-only token accounting; counter movement alone is never evidence;
  every live-tier claim reconciles to `token_usage_events` rows.
- Public-safety tripwires run in every mode; screenshots and reports are
  redaction-checked projections.
- Explore discoveries become deterministic scenarios or they didn't happen.
- Optimizer candidates (Q6.4) are offline, Gym-admitted, never
  auto-promoted; runtime policy is never weakened to make a model or a
  budget pass.
- No GitHub-hosted CI: Tier-1 bounded pre-push + Tier-2 owned-runner only.
- Every landing: relevant suites + `check:deploy` green; clean worktrees;
  issues close only via merged PRs.

## 12. Issue Map

One GitHub issue per task, filed 2026-07-02 (see the epic for the live
checklist). This table is patched with the real numbers at filing time.

Epic: [#8051](https://github.com/OpenAgentsInc/openagents/issues/8051)

| Task | Issue | Title |
| --- | --- | --- |
| Q1.1 | [#8012](https://github.com/OpenAgentsInc/openagents/issues/8012) | Nightly QA matrix on the owned runner |
| Q1.2 | [#8013](https://github.com/OpenAgentsInc/openagents/issues/8013) | Visual smokes gate desktop merges |
| Q1.3 | [#8014](https://github.com/OpenAgentsInc/openagents/issues/8014) | Coverage union, frontier report, zero-coverage auto-issue (T6.6 completion) |
| Q1.4 | [#8015](https://github.com/OpenAgentsInc/openagents/issues/8015) | Flake policy and quarantine ledger |
| Q1.5 | [#8016](https://github.com/OpenAgentsInc/openagents/issues/8016) | Public-safe QA status surface |
| Q2.1 | [#8017](https://github.com/OpenAgentsInc/openagents/issues/8017) | Real-run perf sample bridge (T6.14 completion) |
| Q2.2 | [#8018](https://github.com/OpenAgentsInc/openagents/issues/8018) | Full latency budget family as data |
| Q2.3 | [#8019](https://github.com/OpenAgentsInc/openagents/issues/8019) | Lag profiling sweep of the real app |
| Q2.4 | [#8020](https://github.com/OpenAgentsInc/openagents/issues/8020) | Memory, leak, and zombie-process oracle |
| Q2.5 | [#8021](https://github.com/OpenAgentsInc/openagents/issues/8021) | Perf trend reporting and regression auto-issue |
| Q2.6 | [#8022](https://github.com/OpenAgentsInc/openagents/issues/8022) | Burn down the ranked lag offenders to budget-green |
| Q3.1 | [#8023](https://github.com/OpenAgentsInc/openagents/issues/8023) | First headed AX run against the packaged app |
| Q3.2 | [#8024](https://github.com/OpenAgentsInc/openagents/issues/8024) | Weekly headed scenario subset with a11y oracle |
| Q3.3 | [#8025](https://github.com/OpenAgentsInc/openagents/issues/8025) | Screenshot baseline store and visual diff oracle |
| Q3.4 | [#8026](https://github.com/OpenAgentsInc/openagents/issues/8026) | Flagship demo: seeded-bug hunt, distilled regression |
| Q4.1 | [#8027](https://github.com/OpenAgentsInc/openagents/issues/8027) | Complete the mechanical scenario corpus (every RPC group) |
| Q4.2 | [#8028](https://github.com/OpenAgentsInc/openagents/issues/8028) | ThreadItem variant render coverage |
| Q4.3 | [#8029](https://github.com/OpenAgentsInc/openagents/issues/8029) | Slash-command scenario sweep |
| Q4.4 | [#8030](https://github.com/OpenAgentsInc/openagents/issues/8030) | Error-state scenario corpus |
| Q4.5 | [#8031](https://github.com/OpenAgentsInc/openagents/issues/8031) | Cross-mode consistency runs (Mode P == Mode D) |
| Q4.6 | [#8032](https://github.com/OpenAgentsInc/openagents/issues/8032) | Console-error oracle in every Mode D run |
| Q5.1 | [#8033](https://github.com/OpenAgentsInc/openagents/issues/8033) | Armed smoke:fleet-run-live (2 workers, real closeouts) |
| Q5.2 | [#8034](https://github.com/OpenAgentsInc/openagents/issues/8034) | Armed smoke:fleet-run-sustained (>=5 workers, >=30 min) |
| Q5.3 | [#8035](https://github.com/OpenAgentsInc/openagents/issues/8035) | Codex live parity + Mode H JSONL live run |
| Q5.4 | [#8036](https://github.com/OpenAgentsInc/openagents/issues/8036) | Claude harness live smoke through the desktop |
| Q5.5 | [#8037](https://github.com/OpenAgentsInc/openagents/issues/8037) | Weekly armed live-tier cadence |
| Q6.1 | [#8038](https://github.com/OpenAgentsInc/openagents/issues/8038) | LLM explorer live brain wired to the desktop drivers |
| Q6.2 | [#8039](https://github.com/OpenAgentsInc/openagents/issues/8039) | Explore -> distill -> regress loop live |
| Q6.3 | [#8040](https://github.com/OpenAgentsInc/openagents/issues/8040) | Monkey night at scale (>=5000 actions nightly) |
| Q6.4 | [#8041](https://github.com/OpenAgentsInc/openagents/issues/8041) | Explore-policy GEPA loop (reopened by owner 2026-07-02) |
| Q7.1 | [#8042](https://github.com/OpenAgentsInc/openagents/issues/8042) | Cockpit partial degradation on single-RPC failure |
| Q7.2 | [#8043](https://github.com/OpenAgentsInc/openagents/issues/8043) | Typed degraded states for every boot-time RPC |
| Q7.3 | [#8044](https://github.com/OpenAgentsInc/openagents/issues/8044) | Hunt the intermittent desktop-suite error |
| Q7.4 | [#8045](https://github.com/OpenAgentsInc/openagents/issues/8045) | Real-bridge CI smoke (HTTP + bearer + SSE, not in-process) |
| Q8.1 | [#8046](https://github.com/OpenAgentsInc/openagents/issues/8046) | Architecture scan hard-fail (T14.2 completion) |
| Q8.2 | [#8047](https://github.com/OpenAgentsInc/openagents/issues/8047) | Schema oracle in the merge gate |
| Q8.3 | [#8048](https://github.com/OpenAgentsInc/openagents/issues/8048) | TLC model checks in the nightly + counterexample pipeline |
| Q8.4 | [#8049](https://github.com/OpenAgentsInc/openagents/issues/8049) | Public-safety oracle in every mode |
| Q8.5 | [#8050](https://github.com/OpenAgentsInc/openagents/issues/8050) | Coverage floor gate |
