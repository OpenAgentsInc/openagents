# MemoHarness and the current OpenAgents system: integration analysis

Date: 2026-07-20

Status: speculation and design analysis. This document is not dispatch
authority. It is not a product promise. It grounds every claim in the two source
documents and in named current-system files. It proposes where an agent memory
substrate could attach, and which existing parts it should extend.

Companion source documents:

- Paper summary:
  [`2026-07-18-memoharness-paper-summary.md`](./2026-07-18-memoharness-paper-summary.md)
- Prior-art integration analysis (targets the deprecated Blueprint direction):
  [`2026-07-18-memoharness-blueprint-integration-analysis.md`](./2026-07-18-memoharness-blueprint-integration-analysis.md)
- Related earlier note:
  [`2026-07-04-harness-optimization-evolve-the-harness-audit.md`](./2026-07-04-harness-optimization-evolve-the-harness-audit.md)

Note on the prior analysis: it targets the Blueprint kernel. Blueprint is
deprecated and archived in this workspace. This document treats that analysis as
prior art only. It re-targets the still-relevant ideas at the current OpenAgents
system.

## 1. What MemoHarness actually is

MemoHarness optimizes the agent harness, not the model weights. The harness is
the control layer around a fixed model. It sets what the model reads, which
tools it can call, how many calls run, what state survives between calls, and
what output is accepted (paper summary, executive summary and section 1).

MemoHarness has three parts (paper summary, section 1 and sections 2 to 6):

1. A six-dimension representation of the harness. The dimensions are context
   assembly (D1), tool interaction (D2), generation control (D3), orchestration
   (D4), memory management (D5), and output processing (D6). Each dimension is a
   bounded, editable control surface.
2. A dual-layer experience bank. A per-case layer records each execution: case
   features, the applied harness, the configuration delta, the full trajectory,
   reward, exact token cost, and a structured diagnosis. A global layer distills
   recurring failure clusters into patterns. Each pattern names the phenomenon,
   its supporting evidence, and the expected effect of a harness change.
   Retrieval returns a bounded slice, so controller context does not grow with
   the whole bank.
3. A one-shot, test-time controller. For a new unlabeled case, it retrieves the
   most similar past successes and failures by cosine similarity, plus
   feature-conditioned entries and the global patterns. It transforms the
   selected global harness into a case-specific harness, then runs that harness
   once. It uses no test label, no post-run feedback, no gradient update, and no
   second search loop.

Reward ranking is correctness first, then token cost as a tiebreak (paper
summary, section 3). Search starts from a minimal harness, so every added
behavior must be justified by execution evidence (paper summary, section 5).

### Reported benefits, separated from evidence

Claimed point estimates (paper summary, section 8):

- Terminal-Bench mean success 0.806, versus 0.722 for the strongest compared
  baseline (Codex).
- The selected harness also improves LiveCodeBench (0.900 to 0.967) and
  FinanceAgent (0.600 to 0.767).
- A Terminal-Bench harness transfers to six other models, mean gain +0.098.
- Cost is 6.89 US dollars versus 10.28 for Codex, because about 94 percent of
  the retrieved context is cached.

Evidence limits, stated by the paper itself (paper summary, sections 8 and 11):

- The main evaluation uses only 18 held-out tasks, with point estimates and no
  confidence intervals or significance tests.
- Some baselines compare whole released systems, not controlled harness swaps.
- The paper does not isolate the static optimized harness from the case-adapted
  harness, so the value of test-time adaptation is not quantified.
- The cost advantage depends on high cache reuse.

The correct reading is narrow. Execution experience is a credible substrate for
adaptive harness optimization. This paper is an encouraging prototype, not a
settled recipe or a general performance law.

## 2. Which retargeted lessons still apply, and which are obsolete

### Lessons that still apply, re-targeted at the current system

The prior analysis states a set of authority rules. The current system already
enforces the strongest of them, which makes them a good fit rather than a new
burden.

1. Experience is evidence, not authority. The agent trace store already carries
   this rule. Migration
   [`0228_agent_traces.sql`](../../apps/openagents.com/workers/api/migrations/0228_agent_traces.sql)
   states in its header: a trace is evidence only, and it grants no
   accepted-work, payout, settlement, or public-claim authority by itself. A
   MemoHarness memory record must keep the same status.
2. Optimization is proposal only, and promotion is independent. This maps onto
   the release-gate and evidence-only run pattern that the Blueprint kernel
   still expresses in-repo, and onto the general repository invariant that a
   trace, model output, or roadmap does not grant an action (root
   `AUTHORITY.md` and this app's contract).
3. Private traces stay private. Public projections use refs, digests, and
   redacted metrics. The current trace tripwire enforces this at ingest (see
   section 4).
4. Memory must be correctable and deletable. The trace store already carries
   `content_digest` dedup and a `training_consent` column that defaults to
   withheld (migration
   [`0229_agent_trace_data_market.sql`](../../apps/openagents.com/workers/api/migrations/0229_agent_trace_data_market.sql)).
   A memory layer must respect consent and support deletion.
5. Separate per-case evidence from distilled global patterns, and keep retrieval
   bounded. The per-case layer already exists as the trace store. The global
   pattern layer does not exist yet.
6. Rank correctness first, cost second, and keep exact token and cache evidence.
   The token ledger already records exact usage (see section 4).

### Lessons that are now obsolete or must be dropped

- Do not target the deprecated Blueprint workspace, and do not revive
  Blueprint as a company brain. The master roadmap lists
  Blueprint-as-company-brain maturation as a non-goal without a new bounded
  owner decision
  ([`docs/sol/MASTER_ROADMAP.md`](../sol/MASTER_ROADMAP.md), non-goals list).
- Do not treat the in-repo `workers/api/src/blueprint` kernel as the memory
  home. Its Program Run, Module Version, Optimizer Run, and Release Gate schemas
  are a useful governance pattern to imitate, but the current coding runtime and
  the trace store are the concrete memory seams, not that kernel.
- The prior analysis said new persistence must use Cloud SQL and Cloud Storage,
  and must not add D1 or R2. The live trace store and the Codex raw-event
  archive already use the legacy D1 plus R2 path (migrations 0228 to 0243). This
  is an unresolved infrastructure tension, not a settled target. A new memory
  store must follow the current infrastructure authority in the app contract,
  which names Google Cloud as the sole production authority. See section 6.

## 3. Where MemoHarness memory plugs into the current architecture

There is no live cross-run or cross-session agent memory today. The Codex
executor mints a fresh thread per run and has no resume path
([`apps/pylon/src/codex-agent-executor.ts`](../../apps/pylon/src/codex-agent-executor.ts)).
The assignment state machine persists only evidence refs
([`apps/pylon/src/assignment.ts`](../../apps/pylon/src/assignment.ts)). Each
delegated coding session is single-shot. This is the gap MemoHarness memory
would fill. Three seams fit best.

### Seam A (best fit): cross-run memory for the Pylon and Codex coding loop

This seam gives the coding runtime a repo-scoped and task-scoped memory across
runs. It uses MemoHarness dimensions D1 (context) and D5 (memory).

Concrete parts that already exist:

- A durable transcript source-of-record. Every completed Codex turn posts its
  raw event stream to the ingest route
  ([`apps/openagents.com/workers/api/src/pylon-codex-turn-ingest-routes.ts`](../../apps/openagents.com/workers/api/src/pylon-codex-turn-ingest-routes.ts)),
  posted by
  [`packages/pylon-core/src/executor/codex-turn-reporter.ts`](../../packages/pylon-core/src/executor/codex-turn-reporter.ts).
  The store keys rows on `content_digest`, `session_ref`, `assignment_ref`, and
  `turn_index` (migrations
  [`0238_pylon_codex_raw_events.sql`](../../apps/openagents.com/workers/api/migrations/0238_pylon_codex_raw_events.sql)
  and
  [`0243_pylon_codex_raw_event_chunks.sql`](../../apps/openagents.com/workers/api/migrations/0243_pylon_codex_raw_event_chunks.sql)).
  Today this archive is write-only. Nothing reads it back into a later turn. It
  is the natural source to distill memory from.
- An unwired primitive kit under `apps/pylon/src/tas/`. It includes repo memory
  ([`repo-memory.ts`](../../apps/pylon/src/tas/repo-memory.ts): `recallForRepo`,
  `mergeObservation`, facts typed as convention, layout, command, or note),
  session memory
  ([`session-memory.ts`](../../apps/pylon/src/tas/session-memory.ts): salience
  and recency scoring), team memory
  ([`team-memory.ts`](../../apps/pylon/src/tas/team-memory.ts): scope and
  visibility), a token-budgeted context packer
  ([`context-assembly.ts`](../../apps/pylon/src/tas/context-assembly.ts)), and a
  ranking primitive
  ([`semantic-retrieval.ts`](../../apps/pylon/src/tas/semantic-retrieval.ts):
  cosine similarity and top-k over items that already carry embeddings). These
  modules are pure and unit-tested, but no runtime imports them.

What memory would be written here: after a run closes out, an offline compiler
would derive redacted, owner-scoped repo facts (conventions, layout, working
commands) and task outcomes with a coarse six-dimension diagnosis. It would
write them as `repo-memory.ts` and `session-memory.ts` records that reference
the trace and the raw-event archive by ref and digest, never by raw content.

What memory would be read here: at dispatch, the executor would call
`recallForRepo`, rank with `semantic-retrieval.ts`, pack a bounded slice with
`context-assembly.ts`, and inject that slice into the initial Codex prompt. The
executor would record what it recalled in the run trace. This matches the
paper's D1 and D5 changes without adding any new authority.

### Seam B (substrate): the redacted agent-trace store as the per-case experience layer

The redacted ATIF trace store is already the per-case experience layer that
MemoHarness needs. It should be extended, not duplicated.

- The schema is
  [`packages/atif/src/trace-schema.ts`](../../packages/atif/src/trace-schema.ts),
  pinned at `ATIF-v1.7`. A trajectory has ordered steps. An agent step can carry
  a reasoning summary (`reasoning_content`) and tool labels (`function_name`,
  `arguments`), plus per-step metrics (prompt tokens, completion tokens, cost).
  These are the public-safe projection, not raw prompts.
- Migration
  [`0236_agent_traces_demand_attribution.sql`](../../apps/openagents.com/workers/api/migrations/0236_agent_traces_demand_attribution.sql)
  adds `demand_kind` and `demand_source` as dedicated columns. Large
  trajectories move to R2 (migration
  [`0230_agent_trace_trajectory_r2.sql`](../../apps/openagents.com/workers/api/migrations/0230_agent_trace_trajectory_r2.sql)).

MemoHarness maps onto this cleanly. A `HarnessExecutionExperience` record should
reference an existing trace UUID and the exact token rows, plus a
six-dimension diagnosis and a success flag. It should not copy the trajectory.
The MemoHarness global layer (`HarnessPatternCandidate`) is the missing piece. It
would be a small new table of patterns, distilled offline from consented traces,
each pattern holding supporting success and failure refs and applicability
bounds.

### Seam C (local): external memory for the on-device Apple FM chat

The on-device chat is the strongest case for an external memory harness, because
the model is small and its context is tiny.

- The desktop shell assembles the prompt in
  [`apps/openagents-desktop/src/renderer/shell.ts`](../../apps/openagents-desktop/src/renderer/shell.ts).
  `buildOpenAgentsAppleFmPrompt` flattens the live note list and truncates it to
  `APPLE_FM_PROMPT_MAX_CHARS = 3900` characters. It drops the oldest turns first
  and always keeps the latest user message. There is no retrieval, no
  summarization, and no persistence.
- The design note
  [`docs/apple-fm/2026-07-20-free-on-device-inference-ide-analysis.md`](../apple-fm/2026-07-20-free-on-device-inference-ide-analysis.md)
  describes the model as about a 3-billion-parameter local model with a single
  bounded prompt, capped at 4000 characters by the desktop bridge. It states the
  model cannot ingest a repository and cannot remember anything after the chat.
  It reserves an optional semantic-retrieval seat that is currently disabled
  (`retrieval_disabled`), and it states the rule that no embeddings are
  generated, stored, or uploaded.

What memory would be written and read here: a small, on-device memory of
conversation summaries and repo facts. The host would recall a bounded slice
with the same `context-assembly.ts` packer and `semantic-retrieval.ts` selector,
then inject it into `buildOpenAgentsAppleFmPrompt` before truncation. This fills
the reserved seat. It must remain strictly local. It must not upload embeddings,
summaries, or the memory itself.

## 4. Hard constraints the memory must respect

1. Redacted, owner-only trace privacy. The trace tripwire rejects, and does not
   hide, unsafe values before persistence. The tripwire finding codes are
   `secret_material`, `wallet_or_payment_material`, `local_path`, `pii_email`,
   and `raw_provider_model_id`
   ([`packages/atif/src/trace-schema.ts`](../../packages/atif/src/trace-schema.ts)).
   A separate scrubber
   ([`packages/atif/src/redaction.ts`](../../packages/atif/src/redaction.ts))
   removes keys, mnemonics, tokens, paths, and personal data before the
   tripwire. Auto-captured traces default to `owner_only` and return 404 to a
   non-owner, so existence is not revealed
   ([`apps/openagents.com/INVARIANTS.md`](../../apps/openagents.com/INVARIANTS.md),
   Agent Trace Store). Memory must reuse these guards. It must never store raw
   prompts, secrets, local paths, or wallet material.
2. Local-only for the Apple FM path. The corpus rule forbids generating,
   storing, or uploading embeddings on that path. On-device memory must stay on
   the device. It must not become an upload channel.
3. Exact-usage and authority boundaries. Public counters derive only from exact
   `token_usage_events` rows. Synthetic accounting is forbidden (ADR
   [`0009-count-served-tokens-from-exact-usage-ledger-rows.md`](../adr/0009-count-served-tokens-from-exact-usage-ledger-rows.md)).
   The ledger must not store raw prompts, tool arguments, provider payloads, or
   paths
   ([`apps/openagents.com/INVARIANTS.md`](../../apps/openagents.com/INVARIANTS.md),
   Canonical Token Usage Ledger). Reading or writing memory must not create
   usage rows, and must not grant payout, settlement, or public-claim authority.
4. Multi-tenant and owner-scope isolation. Trace rows carry `owner_user_id`, and
   team memory carries scope and visibility
   ([`team-memory.ts`](../../apps/pylon/src/tas/team-memory.ts)). Consent
   defaults to withheld. A distilled global pattern must not inherit access to
   the private cases that supported it. One owner scope must not read another
   owner's memory. This mirrors the single-owner execution rule already stated
   in the coding-delegation runbook.

These rules match the MemoHarness privacy invariant already written into the
current specs. The web trust surface spec states that no raw private
experiences, patterns, prompts, transcript text, provider tool output,
embeddings, retrieval queries, private scores, secrets, credentials, or
filesystem paths may enter public-safe MemoHarness projections
([`specs/web/openagents-com-trust-surface.product-spec.md`](../../specs/web/openagents-com-trust-surface.product-spec.md)).

## 5. Build versus reuse

The system already has most of a memory substrate. MemoHarness ideas should
extend it, not replace it.

- The per-case experience layer already exists as the redacted ATIF trace store
  plus the exact token ledger. Reuse it. Do not build a parallel trajectory
  store.
- The recall, ranking, and packing primitives already exist under
  `apps/pylon/src/tas/`. They are pure and tested but unwired. Wire them. Do not
  write new ranking math.
- The durable transcript source-of-record already exists as the Codex raw-event
  archive. Read from it offline. Do not add a second transcript store.
- The product design vocabulary already exists. The cognee audit at the
  workspace root recommends a `remember`, `recall`, `improve`, and `forget`
  vocabulary and a two-tier lifecycle of fast session memory then durable
  promoted memory (`projects/2026-05-21-cognee-memory-blueprint-audit.md`,
  reference only). Reuse that shape.
- MemoHarness is already named in the current specs. Eight ProductSpec and
  AssuranceSpec files reference a dual-layer private experience bank with
  `HarnessExecutionExperience` and `HarnessPatternCandidate` records, for
  example
  [`specs/desktop/full-auto.product-spec.md`](../../specs/desktop/full-auto.product-spec.md)
  and
  [`specs/mobile/mobile-any-host-fleet-controller.product-spec.md`](../../specs/mobile/mobile-any-host-fleet-controller.product-spec.md).
  This analysis should align with those records rather than invent a new format.

The only genuinely new parts are: a distillation compiler that turns consented
traces and raw-event archives into memory records, a small global-pattern table,
a read path that injects a bounded recalled slice into the Codex prompt and the
Apple FM prompt, and the consent and deletion lifecycle that ties them together.

## 6. Risks, open questions, and a small first slice

Risks and open questions:

- Infrastructure target. The current trace and raw-event stores use D1 and R2,
  but the app contract names Google Cloud as the sole production authority. A new
  memory store must resolve this before it persists anything durable.
- Consent semantics. `training_consent` defaults to withheld. Memory reuse of a
  trace needs an explicit consent rule, or it must draw only from the owner's own
  runs for the owner's own benefit.
- Redaction sufficiency on read-back. The tripwire rejects rather than redacts.
  Memory must not reintroduce raw material when it packs a recalled slice into a
  prompt. Each recalled item needs its own safety check.
- Cross-scope leakage. A global pattern must prove it carries no owner-private
  content before any shared use.
- Retrieval quality without stored embeddings. `semantic-retrieval.ts` takes
  embeddings as input. The coding loop needs an embedding source. The Apple FM
  path must compute any embedding locally, or use a non-embedding recall order.
- Weak upstream evidence. The paper reports 18 tasks and no confidence
  intervals. Treat MemoHarness as a design hypothesis. Require an offline
  static-versus-adapted comparison before any live use.
- Premature integration. The TAS kit has no live caller. Wiring it is real work
  and real risk. Start offline.

Proposed first slice, kept small and honest:

- Build a repo-fact memory for the Codex coding loop only.
- Derive redacted, owner-scoped repo facts offline from existing closeout
  evidence and consented traces. Store them as `repo-memory.ts` records that
  reference the trace and archive by ref and digest.
- At dispatch, recall a bounded slice, pack it with `context-assembly.ts`, and
  inject it into the initial Codex prompt behind a default-off flag.
- Record the recalled refs in the run trace. Change no authority. Add no usage
  rows.
- Measure task success and token cost with the flag off and on, offline first,
  then in shadow. Keep the ATIF trace store, the token ledger, and the release
  discipline as the factual authorities.

This slice tests the core MemoHarness claim, that recalled execution experience
improves a fixed model, while it respects every current privacy and authority
rule.

## References

Source documents:

- [`docs/research/2026-07-18-memoharness-paper-summary.md`](./2026-07-18-memoharness-paper-summary.md)
- [`docs/research/2026-07-18-memoharness-blueprint-integration-analysis.md`](./2026-07-18-memoharness-blueprint-integration-analysis.md)
- [`docs/research/2026-07-04-harness-optimization-evolve-the-harness-audit.md`](./2026-07-04-harness-optimization-evolve-the-harness-audit.md)

Current-system files inspected:

- [`apps/pylon/src/codex-agent-executor.ts`](../../apps/pylon/src/codex-agent-executor.ts)
- [`apps/pylon/src/assignment.ts`](../../apps/pylon/src/assignment.ts)
- [`apps/pylon/src/tas/repo-memory.ts`](../../apps/pylon/src/tas/repo-memory.ts)
- [`apps/pylon/src/tas/session-memory.ts`](../../apps/pylon/src/tas/session-memory.ts)
- [`apps/pylon/src/tas/team-memory.ts`](../../apps/pylon/src/tas/team-memory.ts)
- [`apps/pylon/src/tas/context-assembly.ts`](../../apps/pylon/src/tas/context-assembly.ts)
- [`apps/pylon/src/tas/semantic-retrieval.ts`](../../apps/pylon/src/tas/semantic-retrieval.ts)
- [`apps/openagents.com/workers/api/src/pylon-codex-turn-ingest-routes.ts`](../../apps/openagents.com/workers/api/src/pylon-codex-turn-ingest-routes.ts)
- [`packages/pylon-core/src/executor/codex-turn-reporter.ts`](../../packages/pylon-core/src/executor/codex-turn-reporter.ts)
- [`packages/atif/src/trace-schema.ts`](../../packages/atif/src/trace-schema.ts)
- [`packages/atif/src/redaction.ts`](../../packages/atif/src/redaction.ts)
- [`apps/openagents.com/workers/api/src/token-ledger-store.ts`](../../apps/openagents.com/workers/api/src/token-ledger-store.ts)
- [`apps/openagents-desktop/src/renderer/shell.ts`](../../apps/openagents-desktop/src/renderer/shell.ts)
- [`docs/apple-fm/2026-07-20-free-on-device-inference-ide-analysis.md`](../apple-fm/2026-07-20-free-on-device-inference-ide-analysis.md)
- [`apps/openagents.com/INVARIANTS.md`](../../apps/openagents.com/INVARIANTS.md)
- [`docs/adr/0009-count-served-tokens-from-exact-usage-ledger-rows.md`](../adr/0009-count-served-tokens-from-exact-usage-ledger-rows.md)
- [`specs/web/openagents-com-trust-surface.product-spec.md`](../../specs/web/openagents-com-trust-surface.product-spec.md)
- [`specs/desktop/full-auto.product-spec.md`](../../specs/desktop/full-auto.product-spec.md)
- [`specs/mobile/mobile-any-host-fleet-controller.product-spec.md`](../../specs/mobile/mobile-any-host-fleet-controller.product-spec.md)
- Migrations `0137`, `0228`, `0229`, `0230`, `0232`, `0236`, `0238`, `0243`
  under
  [`apps/openagents.com/workers/api/migrations/`](../../apps/openagents.com/workers/api/migrations/)

Reference only, outside this repository:

- `projects/2026-05-21-cognee-memory-blueprint-audit.md` at the workspace root.
