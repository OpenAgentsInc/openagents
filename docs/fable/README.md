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
