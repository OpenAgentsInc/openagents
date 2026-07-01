# docs/fable

Synthesis documents written by the Fable agent lane: repo-wide summaries and
analyses that map code, docs, and issue history for a given subject. These are
orientation/analysis artifacts — they flip no promise state, change no runtime
authority, and broaden no public copy.

Contents:

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
  shell migration on the `apps/autopilot-desktop` template → @effect/vitest
  + TestClock + guardrails.
