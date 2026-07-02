# docs/fable

Synthesis documents written by the Fable agent lane: repo-wide summaries and
analyses that map code, docs, and issue history for a given subject. These are
orientation/analysis artifacts — they flip no promise state, change no runtime
authority, and broaden no public copy.

As of 2026-07-01 the folder also carries the **unified execution layer**:
every recommendation and roadmap across the eight analysis docs is
consolidated into one roadmap and one operating procedure, cross-linked from
each source doc.

## Start Here

- [`ROADMAP.md`](./ROADMAP.md) — the single consolidated roadmap: 17
  workstreams (WS-1…WS-17), ~80 issue-sized tasks with hard dependencies and
  delegability grades, a five-wave parallelization plan for multi-agent
  fan-out, milestones M1–M6, and the merged non-negotiable invariants.
  Headline shape: two small foundations land first — the Schema-first
  contracts spine (Effect audit Phase 1) and wiring the dormant Pylon
  orchestration store as the one state model (Orca Priority 1 = fan-out
  Lanes A1/B1) — then work fans out wide across the fleet-run engine, the
  work planner + claim registry, the cockpit UI, the QA framework, the
  Effect process spine, the Claude chat harness, multi-harness routing, the
  status spine, the mobile companion, Artanis elevation, the staged Foldkit
  migration, and continuous guardrails — gated at the end by the "clean
  2B-token day" acceptance run.
- [`ROADMAP_BIZ.md`](./ROADMAP_BIZ.md) — the consolidated business
  fulfillment engine roadmap (2026-07-02): the end-to-end funnel from simple
  intake at `/business` through qualify, convert (rate card,
  payment→workspace→service-promise), provision (corpus ingestion + redaction
  + private compute tier), fulfill (workroom surfacing, review gates,
  document/site/email/campaign pipelines, fulfillment agents), prove (KPI
  dashboards, handoff portal, case studies), retain, and multiply
  (white-label, referral). Nine workstreams, 45 issues (#8074–#8118) under
  epic #8073, harmonized with ROADMAP_AFTER (this roadmap is the plumbing
  under AW-0). Meditation source:
  [`2026-07-02-business-fulfillment-engine-meditations.md`](./2026-07-02-business-fulfillment-engine-meditations.md).
- [`ROADMAP_QA.md`](./ROADMAP_QA.md) — the active QA execution roadmap
  (2026-07-02): everything needed to run the fully automated Khala Code QA
  cycle — the scheduled nightly loop and merge gates, the real-run latency
  budget family and lag burndown, headed native Mode V, the complete
  use-case scenario corpus, owner-armed live tiers, the explorer/distill
  learning loop, and hard-fail guardrails. Eight workstreams, 39 issues
  (#8012–#8050) under epic #8051, with the issue map in its §12.
- [`2026-07-02-qa-swarm-product-plan.md`](./2026-07-02-qa-swarm-product-plan.md)
  — the QA Swarm product plan: the ROADMAP_QA machine packaged as a sellable
  autonomous-QA product (swarm runs, committed distilled tests, honest
  verdicts, the shareable StarCraft-blue evidence board via arbiter-effect +
  three-effect, customer #1 = Khala Code Desktop, the named-first-customer
  demo-PR sales motion). Workstream QS1–QS10, issues #8061–#8070, epic #8071.
- [`2026-07-02-site-speed-lane-spec.md`](./2026-07-02-site-speed-lane-spec.md)
  — spec for the standing site-speed lane on the deployed website (landing
  page first): ground-truth baseline (2.7 KB SPA shell, 4.1 MB/1.07 MB-br
  monolithic bundle, live-at-read counter API, WebSocket counter feed), the
  mark taxonomy, a three-mode methodology (lab matrix with isolation
  experiments incl. block-the-counter, edge probes, Analytics-Engine RUM),
  ranked hypotheses, named budgets, and phases P0–P5. Separate lane from
  ROADMAP_QA/QA Swarm.
- [`EXECUTION.md`](./EXECUTION.md) — how the roadmap is executed: Artanis
  (fleet-manager role) supervises; **Khala Code fleet delegation is the
  primary mechanism**; one GitHub issue per roadmap task, closed only via a
  reviewed PR merged to `main`, built in a clean worktree; final review by
  the supervisor or a tightly-controlled subagent, never the authoring
  worker; every delegated run's tokens verified through the exact
  `token_usage_events` chain (`POST /api/pylon/{codex,claude}/turns` →
  ledger → `GET /api/public/khala-tokens-served`) into the public
  `openagents.com/stats` counters. The run doubles as the final stress test
  of the Khala Code fleet system before outside users — fleet bugs are
  fixed in-flight as first-class work.

## Analysis Docs

- `2026-07-01-khala-code-summary-and-analysis.md` — everything in this repo
  (code, docs, issues) about Khala Code: identity and naming disambiguation,
  the June 29 → July 1 timeline through the Codex-wrapper pivot (epic #7780),
  current architecture, fleet/swarm delegation, verification culture, and an
  assessment of strengths, risks, and open threads.
- `2026-07-01-episode-245-completion-and-multi-harness-orchestration.md` —
  audit of what stands between the recorded half of transcript 245 and its
  unrecorded completion segment (fresh smoke evidence, the live
  message-triggered `khala_fleet` MCP path, residual gaps), a pre-recording
  rehearsal checklist, and a minimal-change plan for orchestrating between
  harnesses: a Codex/Khala chat-harness toggle (Axis A) and a
  `codex | claude | auto` delegation-target parameter through the
  deterministic `khala.fleet.delegate` program (Axis B), grounded in the
  ~80%-parity state of the Claude Code delegation lane.
- `2026-07-01-khala-code-desktop-qa-framework-design.md` — design for the
  Khala Code Desktop testing framework/agent: four access modes (typed RPC,
  DOM/Playwright, vision/computer-use headed+headless, headless JSONL) behind
  one driver contract, a typed scenario DSL with per-phase expectations, a
  seeded-monkey + LLM free-explore mode with a coverage ledger, an oracle
  catalog (schema/consistency/visual/perf/public-safety), a determinism layer
  (fixture Codex app-server, TestClock), property/model-based and bounded
  TLA+ tiers, GEPA optimization loops, and the productization path built on
  the shipped `apps/qa-runner` (@openagentsinc/qa-runner) substrate.
- `2026-07-01-fleet-fanout-coding-instructions.md` — implementation handoff
  for bulletproof fleet fan-out steered from Khala Code: one command starts a
  sustained N-worker run (FleetRun record + refill supervisor + `khala_fleet`
  MCP verbs), a claim registry and typed work planner kill the June 29
  duplicate-PR class, the Fleet screen becomes a cockpit
  (pause/resume/drain/stop, worker cards with live lifecycle, account cards
  with ticking rate limits and reconnect, throughput gauges, Inbox flags),
  full RPC/QA parity per the QA framework doc, and a "clean 2B-token day"
  live acceptance protocol with a definition-of-done checklist.
- `2026-07-01-khala-code-effect-integration-audit.md` — deep Effect-usage
  audit of Khala Code Desktop and everything it consumes, grounded in the
  Effect v4 source (`projects/repos/effect-smol`), its `.patterns/` rules,
  and `effect-solutions`. Headline: the desktop links Effect but doesn't
  adopt it (5/55 files import it; zero services/layers/Config/Scope/Clock;
  five hand-rolled subprocess implementations; an unvalidated 57-method RPC
  contract; a 2,598-line vanilla-DOM shell) and takes the imperative escape
  hatch out of every Effect/Foldkit surface it consumes. Includes a v4
  best-practices baseline, ranked debt with failure modes, and a four-phase
  plan: Schema-first contracts → scoped process/protocol services
  (ChildProcess, CodexAppServer, PylonService, config) → staged Foldkit
  shell migration copying the `apps/autopilot-desktop` patterns into
  `clients/khala-code-desktop` (the only active desktop target;
  autopilot-desktop is postponed reference material) → @effect/vitest
  + TestClock + guardrails.
- `2026-07-01-orca-analysis-and-adoption-plan.md` — how to think about Orca
  (`stablyai/orca`, MIT reference at `projects/repos/orca`): a terminal
  multiplexer + worktree manager + SQLite message bus whose breadth comes
  from PTY/glyph heuristics, versus our typed/verified/exact-accounted
  spine. Audits the five-port adoption scoreboard (runner registry live;
  the orchestration task-DAG store built+tested but dormant; dashboard
  mock-only; Artanis verbs partial; mobile companion never filed) and lays
  out the adoption order: wire the dormant orchestration store as the
  FleetRun spine, unify on the runner-neutral status contract end to end,
  build the mobile companion as an E2EE-paired DO-relayed allowlisted
  projection (observe/notify/approve/steer; rough desktop parity minus
  terminals/design-mode/local execution), then the annotate-diff review
  loop — while explicitly not copying PTY status detection,
  trust-the-summary completion, unclaimed parallelism, or 30-harness
  breadth.
- `2026-07-01-claude-code-parity-and-codex-synergies.md` — bringing Claude
  Code (Claude Agent SDK) up to Codex parity as a Khala Code Desktop chat
  harness, then the crossovers. Grounded in the SDK types
  (`@anthropic-ai/claude-agent-sdk@0.3.172`) and a desktop seam map. Covers
  the missing `ChatRuntime` abstraction, a Codex↔Claude protocol mapping
  table (thread↔session, turn↔user→result span, approvals↔canUseTool), a
  phased bring-up (harness abstraction → minimal Claude runtime as the
  desktop's first real Effect service → approvals/telemetry/MCP/settings →
  sidebar/slash parity, behind a "Codex | Claude | Khala" composer pill),
  an Effect-wrapping cheat sheet (query→Stream, control methods→service
  methods, SDKMessage→Schema, canUseTool→bridged callback), and the
  synergies: Fable/Claude plan-mode decomposition + review delegating
  coding to Codex through the deterministic delegation program and fleet.
- `2026-07-01-product-promises-khala-code-launch-alignment.md` — registry
  audit + launch alignment: reconciles the product-promise registry
  (`2026-06-29.5`, 120 records — versioned before the Khala Code arc
  existed) against the unified roadmap and the **released Episode 245**
  (the Khala Code launch video; the fleet-demo transcript the other fable
  docs call "245" is now the unreleased draft `24X1.md`). Maps every 245
  claim to its registry record, traces the "coding agent pays you"
  escalation across episodes 220–244 (Ep 228 "Get Paid to Code" is the
  direct ancestor of the 245 economics loop; Ep 222's launch-truth-contract
  header is the claim-sheet pattern 245 should repeat), proposes the
  owner-gated
  `khala_code.*` promise family (wrapper product yellow; plans, trace
  capture, plugins, and revenue-share planned), stale-record
  reconciliations (incl. withdrawing the retired-Expo mobile record), the
  roadmap↔promise pairings, and the Khala-Code-as-gateway funnel across
  the registry's green substrate. **Implemented as registry `2026-07-01.1`**
  (owner-directed, no green flips; see the doc's §4 status banner).
- `2026-07-01-promissory-nongreen-assault-runbook.md` — the PROMISSORY
  standing runbook: one repeatable formula any agent (or ten concurrently)
  follows to claim and assault the next non-green product promise. Scoring
  formula with a swappable campaign throughline (currently Khala Code
  launch), a race-free claim protocol (GitHub issue `PROMISSORY:
  <promiseId>` + orchestration-store work claims), the per-promise assault
  ladder (audit → blocker decomposition → implement → registry evidence
  pass), strict state-flip rules (never green — make green a five-minute
  owner decision), a concurrent-safe registry edit protocol, and the fleet
  mass-dispatch template with refill and stop conditions.
- `2026-07-02-khala-code-install-path-audit.md` — audit of the install story
  under the Khala Code reorientation: the heavy-clone failure (agents told to
  build Khala Code from source full-clone the ~460 MB monorepo history when a
  ~40 MB `--depth 1` clone suffices), the Pylon-only served
  `openagents.com/INSTALL.md`, and the absence of any root install file.
  Implemented in the same change: a canonical root `INSTALL.md` (Khala Code
  and Pylon only, per owner direction) linked prominently from the README, a
  Khala-Code-first rewrite of the served INSTALL.md, an install-first
  AGENTS.md callout, and an AGENTS.md slim-down that moves the Pylon and
  Sites/commerce reference blocks into new served companion files `PYLON.md`
  and `SITES.md` (SURFACES.md pattern) with sync-script, SHA-pin, and
  copy/redaction-gate coverage updated.
- `2026-07-02-forum-starcraft-theme-consolidation-audit.md` — audit of the
  Forum surface across web and Khala Code Desktop under the owner mandate
  "uniform StarCraft blue everywhere, no light/dark mode." Finds the desktop
  forum hotbar panel already renders natively on the khala palette
  (dark-only, `--oa-color-khala-*`), while the website forum still carries
  the bespoke phpBB-style skin: 18 local `--color-forum-*` tokens with
  light+dark values, a System/Light/Dark selector (the site's only theme
  toggle), a hardcoded blue-gradient header, an amber-accent OG image, and
  zero shared-design-system usage. Plan: repoint the forum tokens to khala
  values, delete the theme machinery outright, consolidate the header and
  OG card, add the missing palette-guard test, and verify web/desktop
  convergence — structure and behavior unchanged, one swoop.
- `2026-07-02-openagents-com-blog-docs-starcraft-theme-reset-audit.md` —
  audit of the `/blog` and `/docs` surfaces on `openagents.com`: both still
  render the old flat Vortex-era shell (hardcoded hex, amber accent, no
  `khala-*` energy layer, no `packages/ui` primitives) despite the
  StarCraft-theme centralization pass covering the hero/scene pages. Lays
  out the zero-based reset: delist all five legacy blog posts and nine of
  ten docs pages via a render-time `listed` filter (URLs stay live for
  direct links), replace the blog index with a single placeholder post
  "Introducing Khala Code" (July 2, 2026), reduce docs to a Khala
  Code-oriented Overview, and restyle both pages onto the Protoss energy
  layer with token-backed `packages/ui` components — one implementation
  issue, one swoop.
- `2026-07-01-artanis-fleet-administrator-audit.md` — audit of Artanis (the
  autonomous operator/administrator persona): its split architecture (Khala
  operator chat + Gemini cron ticks), current see-vs-do capability (the
  #6359 "see-but-not-act gap" closed for the bounded no-spend lane), the two
  visions (Vision A: administrator of the shared org fleet; Vision B:
  per-user fleet manager / Artanis-as-a-Service), the owner-intent history
  reconstructed from past sessions (birth → first autonomy → rebirth on
  Khala → the "Artanis is your boss" inversion → the 10×-tokens mission),
  recorded challenges (headless fabrication → Blueprint-signature
  governance, truncation → RLM composition, duplicate forum identities), and
  a five-priority path to fleet administrator: make authority scope
  first-class, finish the Blueprint-signature gates, wire onto the one
  orchestration/status spine, resolve identity + raise autonomy
  deliberately, then productize AaaS — dovetailing with the fan-out, Orca,
  and Claude-parity plans.
