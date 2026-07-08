# docs/grok/

Grok's analysis sandbox inside the OpenAgents monorepo.

Purpose: independent orientation, critique, and theme exploration after
reading `docs/fable/` and related product code. These notes **flip no
promise state**, change no runtime authority, and broaden no public copy.
Product claims still gate through `docs/promises/` and owner sign-off.

Started: 2026-07-08 after a full pass over `docs/fable/` (~85 docs).
Extended: 2026-07-08 with multi-harness / Grok CLI / Effect Native / Khala
Sync ASAP analysis.

## Contents

| File | What it is |
| --- | --- |
| `README.md` | This index |
| `fable-reading-notes.md` | Map of the fable corpus: roadmaps, strategy, systems |
| `independent-critique.md` | Where the strategy is strong, fragile, or over-scoped |
| `themes-to-explore.md` | Open threads worth chewing on next |
| `falsifiers-and-scorecard.md` | Living watchlist for thesis health |
| `product-ladder.md` | Suite + horizons H0–H6 + master phase order |
| `trust-stack.md` | Receipts, promises, isolation, behavior contracts |
| `execution-discipline.md` | How work is supposed to land (fleet, QA, issues) |
| **`parallel-multi-harness-asap.md`** | **Parallel Codex + Claude + Grok under MASTER_ROADMAP, Sync, Effect Native** |
| **`grok-cli-as-third-harness.md`** | **Adapter-level design: ACP, schema, fleet, Sync, EN UI** |

CLI operator reference (separate folder): `docs/grok-cli/`.

## Rules for this folder

1. **Public-safe by default.** No fundraising materials, no investor
   diligence frames, no client-identifying detail. Verticals only when
   discussing demand.
2. **Label provenance.** Modeled / measured / verified / opinion / gated.
   Do not promote opinion to fact or fable narrative to registry green.
3. **Use our vocabulary.** Blueprint (never ontology), agent computers,
   company brain, AI employees, credits, Khala Sync, Reactor, QA Swarm.
4. **Prefer mechanisms over slogans.** If a claim only works as rhetoric,
   say so.
5. **Sandbox, not policy.** Future Grok sessions append here freely;
   promote ideas to issues/roadmaps only with deliberate ownership.

## Origin pass sources (high altitude)

Strategic spine:

- `docs/fable/README.md`
- `docs/fable/MASTER_ROADMAP.md` (rev 6.x — Effect Native full conversion, Pylon fold)
- `docs/fable/2026-07-07-what-openagents-is-essay-and-talking-points.md`
- `docs/fable/2026-07-07-overarching-roadmap-khala-code-agent-computers-ai-employees.md`
- `docs/fable/2026-07-07-product-suite-khala-code-openagents-com-reactor.md`
- `docs/fable/2026-07-02-come-for-the-tool-stay-for-the-network.md`
- `docs/fable/2026-07-07-beyond-mvp-codex-agent-computers-and-ai-employees.md`

Multi-harness / fleet (2026-07-08 deep pass):

- `docs/fable/2026-07-01-episode-245-completion-and-multi-harness-orchestration.md`
- `docs/fable/2026-07-01-claude-code-parity-and-codex-synergies.md`
- `docs/fable/2026-07-01-fleet-fanout-coding-instructions.md`
- `docs/fable/2026-07-02-oh-my-pi-planner-coder-judge-audit.md`
- `docs/fable/2026-07-01-khala-code-effect-integration-audit.md`
- `docs/fable/2026-07-04-database-alternatives-and-postgres-sync-engine.md`
- `docs/fable/2026-07-04-khala-sync-implementation-status.md`
- `docs/fable/2026-07-08-en-1-stage1-effect-native-receipt.md`
- `packages/agent-runtime-schema` (harness / adapter kinds)
- `clients/khala-code-desktop` (ChatRuntime, fleet supervisor)
- `docs/grok-cli/` (Grok Build CLI reference)

Execution layer:

- `ROADMAP.md`, `ROADMAP_QA.md`, `ROADMAP_BIZ.md`, `ROADMAP_AFTER.md`,
  `ROADMAP_BACKGROUND_AGENTS.md`, `EXECUTION.md`
