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
