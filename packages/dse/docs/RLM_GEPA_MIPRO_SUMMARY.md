# RLM, GEPA, and MIPRO in OpenAgents (Docs + History Map)

Generated: 2026-02-08
Repo: https://github.com/OpenAgentsInc/openagents (branch `main`)

This document inventories where **RLM**, **GEPA**, and **MIPRO/MIPROv2** are mentioned in tracked documentation (Markdown), and links to the relevant docs and commits.

Notes:
- Search is **case-insensitive substring** matching (so `RLM` also matches `FRLM`, and `MIPRO` also matches `MIPROv2`).
- Counts below are **matching lines**, not total occurrences.
- Git history uses `git log -S<TERM>` (pickaxe), scoped to `*.md`/`*.mdx`.

## Current Tree Statistics

- `RLM`: 52 files, 681 matching lines
- `GEPA`: 24 files, 79 matching lines
- `MIPRO`: 30 files, 111 matching lines
- Union: 67 files mentioning at least one of the terms

## Concept Summary (What These Mean In This Repo)

### RLM / FRLM

- **RLM (Recursive Language Model)** is described as an **iterative prompt-execute loop**: the model can emit commands (like `RUN ...`) that are executed locally, with results fed back into the loop until it emits `FINAL ...`. See `SYNTHESIS_EXECUTION.md` for the canonical high-level description and CLI examples (`pylon rlm ...`). ([RLM/FRLM section](https://github.com/OpenAgentsInc/openagents/blob/main/SYNTHESIS_EXECUTION.md#L435))
- **FRLM (Federated RLM)** extends RLM by **fanning out sub-queries across backends/lane types** (local, cloud APIs, and swarm/NIP-90 jobs). See `SYNTHESIS_EXECUTION.md` for the canonical description. ([FRLM paragraph](https://github.com/OpenAgentsInc/openagents/blob/main/SYNTHESIS_EXECUTION.md#L441))
- In the OpenAgents stack, **Adjutant uses DSPy decision pipelines** (complexity/delegation/RLM trigger) to decide when to route into RLM-style execution for deep/large-context tasks, and records those decisions for later evaluation/optimization. See `crates/adjutant/docs/README.md` for where this is documented. ([RLM Integration](https://github.com/OpenAgentsInc/openagents/blob/main/crates/adjutant/docs/README.md#L408))
- Separately from “execution mode,” some Autopilot planning docs use **“RLM-style”** to mean an **evaluation/reward abstraction** (multiple weighted signals aggregated into a scalar reward) used by an optimization loop. That “RLM-as-evaluator” usage shows up in `docs/autopilot/*synergies*.md` and `docs/autopilot/AUTOPILOT_OPTIMIZATION_PLAN.md`.

### MIPRO / MIPROv2

- **MIPROv2** is the default “baseline” optimizer in multiple docs: compile/optimize signatures/modules by generating candidate instructions (and demos) and selecting what scores best on a metric. ([MIPROv2 section](https://github.com/OpenAgentsInc/openagents/blob/main/crates/dsrs/docs/OPTIMIZERS.md#L50))
- In the self-improving loop narrative, **MIPROv2 is auto-triggered** when rolling accuracy drops and enough labeled examples exist (Adjutant performance tracking + background optimization). See `SYNTHESIS_EXECUTION.md` and `crates/adjutant/docs/README.md`. ([auto-triggers MIPROv2](https://github.com/OpenAgentsInc/openagents/blob/main/SYNTHESIS_EXECUTION.md#L423), [Self-Improving Autopilot](https://github.com/OpenAgentsInc/openagents/blob/main/crates/adjutant/docs/README.md#L502))

### GEPA

- **GEPA** is documented as a reflective/evolutionary optimizer that uses **text feedback + trace reflection** and **Pareto-style dominance/frontier tracking** (often positioned as useful for multi-objective tradeoffs like quality vs. cost/latency). ([GEPA section](https://github.com/OpenAgentsInc/openagents/blob/main/crates/dsrs/docs/OPTIMIZERS.md#L243))
- In OpenAgents framing docs, GEPA is usually mentioned alongside MIPROv2 as “the compiler/optimizer layer” for improving signature behavior without retraining base models. See `docs/dspy/openagents-usage.md`. ([MIPROv2 + GEPA plan](https://github.com/OpenAgentsInc/openagents/blob/main/docs/dspy/openagents-usage.md#L53))

### Relationship (DSPy/dsrs × Optimizers × Execution)

- `docs/dspy/report.md` explicitly frames a stack: **DSPy** as the programming/modeling layer, **MIPRO/GEPA** as compile-time optimizers, and **RLMs** as an inference-time strategy (and notes DSPy’s `dspy.RLM` module, as of mid-January 2026). ([Executive picture](https://github.com/OpenAgentsInc/openagents/blob/main/docs/dspy/report.md#L3))
- The glossary anchors terminology used across docs: “Optimizer” includes **MIPROv2/COPRO/GEPA/Pareto**, and `rlm` is a canonical delegation target. ([Optimizer + DelegationTarget](https://github.com/OpenAgentsInc/openagents/blob/main/docs/GLOSSARY.md#L71))

## Fast Entry Points (Recommended Reading Order)

- [SYNTHESIS_EXECUTION.md](https://github.com/OpenAgentsInc/openagents/blob/main/SYNTHESIS_EXECUTION.md)
  - Title: OpenAgents: System Guide
  - Latest: [220768e7fa](https://github.com/OpenAgentsInc/openagents/commit/220768e7fa0b987e4c98b5f8f1e64c767c6b6e55) 2026-02-04
- [docs/GLOSSARY.md](https://github.com/OpenAgentsInc/openagents/blob/main/docs/GLOSSARY.md)
  - Title: Glossary
  - Latest: [685e31e758](https://github.com/OpenAgentsInc/openagents/commit/685e31e758a2e1625b8ef02e8cc702fbfeff4841) 2026-02-04
- [docs/dspy/report.md](https://github.com/OpenAgentsInc/openagents/blob/main/docs/dspy/report.md)
  - Title: How DSPy, MIPRO, GEPA, and RLMs fit together
  - Latest: [980939b8cb](https://github.com/OpenAgentsInc/openagents/commit/980939b8cb28be6a98d1cb4d0b5370c3acc454ed) 2026-01-25
- [docs/dspy/openagents-usage.md](https://github.com/OpenAgentsInc/openagents/blob/main/docs/dspy/openagents-usage.md)
  - Title: DSPy in OpenAgents: Why It Matters and How We Use It
  - Latest: [980939b8cb](https://github.com/OpenAgentsInc/openagents/commit/980939b8cb28be6a98d1cb4d0b5370c3acc454ed) 2026-01-25
- [crates/dsrs/docs/OPTIMIZERS.md](https://github.com/OpenAgentsInc/openagents/blob/main/crates/dsrs/docs/OPTIMIZERS.md)
  - Title: Optimizers
  - Latest: [626ff23a58](https://github.com/OpenAgentsInc/openagents/commit/626ff23a58219806cbd6afdbb48ec3949ccc762f) 2026-01-13
- [crates/rlm/docs/README.md](https://github.com/OpenAgentsInc/openagents/blob/main/crates/rlm/docs/README.md)
  - Title: RLM Paper Replication Infrastructure
  - Latest: [685e31e758](https://github.com/OpenAgentsInc/openagents/commit/685e31e758a2e1625b8ef02e8cc702fbfeff4841) 2026-02-04
- [crates/frlm/docs/README.md](https://github.com/OpenAgentsInc/openagents/blob/main/crates/frlm/docs/README.md)
  - Title: FRLM - Federated Recursive Language Models
  - Latest: [685e31e758](https://github.com/OpenAgentsInc/openagents/commit/685e31e758a2e1625b8ef02e8cc702fbfeff4841) 2026-02-04

## Complete Doc Inventory (Current Tree)

Each entry lists:
- file link
- title (first `# ...` heading)
- per-term counts, linking to the first matching line
- oldest + latest commit touching the file

### Root Docs

- [AGENTS.md](https://github.com/OpenAgentsInc/openagents/blob/main/AGENTS.md)
  - Title: OpenAgents: Agent Contract (READ THIS FIRST)
  - Matches: `RLM`: [4 lines](https://github.com/OpenAgentsInc/openagents/blob/main/AGENTS.md#L9)
  - Oldest: [e460aca030](https://github.com/OpenAgentsInc/openagents/commit/e460aca030ad77edb920dbc5552122b0c105aa9f) 2025-09-23
  - Latest: [b7308e7a6b](https://github.com/OpenAgentsInc/openagents/commit/b7308e7a6b936a8407faf7e26b4f6dcdd6003813) 2026-02-08
- [SYNTHESIS.md](https://github.com/OpenAgentsInc/openagents/blob/main/SYNTHESIS.md)
  - Title: OpenAgents: The Agentic OS
  - Matches: `RLM`: [3 lines](https://github.com/OpenAgentsInc/openagents/blob/main/SYNTHESIS.md#L526), `GEPA`: [1 lines](https://github.com/OpenAgentsInc/openagents/blob/main/SYNTHESIS.md#L496), `MIPRO`: [2 lines](https://github.com/OpenAgentsInc/openagents/blob/main/SYNTHESIS.md#L496)
  - Oldest: [8c69db92df](https://github.com/OpenAgentsInc/openagents/commit/8c69db92df7f16ed4e331925e13361a9626eb482) 2025-12-24
  - Latest: [685e31e758](https://github.com/OpenAgentsInc/openagents/commit/685e31e758a2e1625b8ef02e8cc702fbfeff4841) 2026-02-04
- [SYNTHESIS_EXECUTION.md](https://github.com/OpenAgentsInc/openagents/blob/main/SYNTHESIS_EXECUTION.md)
  - Title: OpenAgents: System Guide
  - Matches: `RLM`: [19 lines](https://github.com/OpenAgentsInc/openagents/blob/main/SYNTHESIS_EXECUTION.md#L51), `GEPA`: [1 lines](https://github.com/OpenAgentsInc/openagents/blob/main/SYNTHESIS_EXECUTION.md#L397), `MIPRO`: [2 lines](https://github.com/OpenAgentsInc/openagents/blob/main/SYNTHESIS_EXECUTION.md#L397)
  - Oldest: [3f733583db](https://github.com/OpenAgentsInc/openagents/commit/3f733583db41a7684f5dd64f7c03a46105f83a13) 2026-01-08
  - Latest: [220768e7fa](https://github.com/OpenAgentsInc/openagents/commit/220768e7fa0b987e4c98b5f8f1e64c767c6b6e55) 2026-02-04

### docs/

- [docs/AGENT_FOUNDATIONS.md](https://github.com/OpenAgentsInc/openagents/blob/main/docs/AGENT_FOUNDATIONS.md)
  - Title: Agent Foundations (OpenAgents)
  - Matches: `RLM`: [12 lines](https://github.com/OpenAgentsInc/openagents/blob/main/docs/AGENT_FOUNDATIONS.md#L26), `GEPA`: [2 lines](https://github.com/OpenAgentsInc/openagents/blob/main/docs/AGENT_FOUNDATIONS.md#L170), `MIPRO`: [2 lines](https://github.com/OpenAgentsInc/openagents/blob/main/docs/AGENT_FOUNDATIONS.md#L170)
  - Oldest: [685e31e758](https://github.com/OpenAgentsInc/openagents/commit/685e31e758a2e1625b8ef02e8cc702fbfeff4841) 2026-02-04
  - Latest: [685e31e758](https://github.com/OpenAgentsInc/openagents/commit/685e31e758a2e1625b8ef02e8cc702fbfeff4841) 2026-02-04
- [docs/GLOSSARY.md](https://github.com/OpenAgentsInc/openagents/blob/main/docs/GLOSSARY.md)
  - Title: Glossary
  - Matches: `RLM`: [1 lines](https://github.com/OpenAgentsInc/openagents/blob/main/docs/GLOSSARY.md#L96), `GEPA`: [1 lines](https://github.com/OpenAgentsInc/openagents/blob/main/docs/GLOSSARY.md#L78), `MIPRO`: [1 lines](https://github.com/OpenAgentsInc/openagents/blob/main/docs/GLOSSARY.md#L78)
  - Oldest: [685e31e758](https://github.com/OpenAgentsInc/openagents/commit/685e31e758a2e1625b8ef02e8cc702fbfeff4841) 2026-02-04
  - Latest: [685e31e758](https://github.com/OpenAgentsInc/openagents/commit/685e31e758a2e1625b8ef02e8cc702fbfeff4841) 2026-02-04
- [docs/PAPER.md](https://github.com/OpenAgentsInc/openagents/blob/main/docs/PAPER.md)
  - Title: (no H1 title found)
  - Matches: `RLM`: [85 lines](https://github.com/OpenAgentsInc/openagents/blob/main/docs/PAPER.md#L5), `MIPRO`: [10 lines](https://github.com/OpenAgentsInc/openagents/blob/main/docs/PAPER.md#L40)
  - Oldest: [685e31e758](https://github.com/OpenAgentsInc/openagents/commit/685e31e758a2e1625b8ef02e8cc702fbfeff4841) 2026-02-04
  - Latest: [685e31e758](https://github.com/OpenAgentsInc/openagents/commit/685e31e758a2e1625b8ef02e8cc702fbfeff4841) 2026-02-04
- [docs/PROJECT_OVERVIEW.md](https://github.com/OpenAgentsInc/openagents/blob/main/docs/PROJECT_OVERVIEW.md)
  - Title: OpenAgents Repository Map
  - Matches: `RLM`: [3 lines](https://github.com/OpenAgentsInc/openagents/blob/main/docs/PROJECT_OVERVIEW.md#L64)
  - Oldest: [685e31e758](https://github.com/OpenAgentsInc/openagents/commit/685e31e758a2e1625b8ef02e8cc702fbfeff4841) 2026-02-04
  - Latest: [685e31e758](https://github.com/OpenAgentsInc/openagents/commit/685e31e758a2e1625b8ef02e8cc702fbfeff4841) 2026-02-04
- [docs/ROADMAP.md](https://github.com/OpenAgentsInc/openagents/blob/main/docs/ROADMAP.md)
  - Title: MVP "Add Next" Priorities
  - Matches: `RLM`: [26 lines](https://github.com/OpenAgentsInc/openagents/blob/main/docs/ROADMAP.md#L15), `MIPRO`: [1 lines](https://github.com/OpenAgentsInc/openagents/blob/main/docs/ROADMAP.md#L280)
  - Oldest: [685e31e758](https://github.com/OpenAgentsInc/openagents/commit/685e31e758a2e1625b8ef02e8cc702fbfeff4841) 2026-02-04
  - Latest: [685e31e758](https://github.com/OpenAgentsInc/openagents/commit/685e31e758a2e1625b8ef02e8cc702fbfeff4841) 2026-02-04
- [docs/adr/ADR-0004-lane-taxonomy.md](https://github.com/OpenAgentsInc/openagents/blob/main/docs/adr/ADR-0004-lane-taxonomy.md)
  - Title: ADR-0004: Lane Taxonomy and Naming
  - Matches: `RLM`: [1 lines](https://github.com/OpenAgentsInc/openagents/blob/main/docs/adr/ADR-0004-lane-taxonomy.md#L111)
  - Oldest: [b05412601a](https://github.com/OpenAgentsInc/openagents/commit/b05412601a02b2e5cea83d25e9d66f585f0cc55c) 2026-01-13
  - Latest: [685e31e758](https://github.com/OpenAgentsInc/openagents/commit/685e31e758a2e1625b8ef02e8cc702fbfeff4841) 2026-02-04
- [docs/adr/ADR-0007-tool-execution-contract.md](https://github.com/OpenAgentsInc/openagents/blob/main/docs/adr/ADR-0007-tool-execution-contract.md)
  - Title: ADR-0007: Tool Execution Contract (Adapters vs Runtime vs Refine)
  - Matches: `RLM`: [1 lines](https://github.com/OpenAgentsInc/openagents/blob/main/docs/adr/ADR-0007-tool-execution-contract.md#L106)
  - Oldest: [3b78d3c397](https://github.com/OpenAgentsInc/openagents/commit/3b78d3c39769a11fd4fa7728974659f6729c5e02) 2026-01-13
  - Latest: [685e31e758](https://github.com/OpenAgentsInc/openagents/commit/685e31e758a2e1625b8ef02e8cc702fbfeff4841) 2026-02-04
- [docs/adr/ADR-0009-planir-canonical-schema.md](https://github.com/OpenAgentsInc/openagents/blob/main/docs/adr/ADR-0009-planir-canonical-schema.md)
  - Title: ADR-0009: PlanIR Canonical Schema and Unification
  - Matches: `RLM`: [1 lines](https://github.com/OpenAgentsInc/openagents/blob/main/docs/adr/ADR-0009-planir-canonical-schema.md#L21)
  - Oldest: [6d5fa50b01](https://github.com/OpenAgentsInc/openagents/commit/6d5fa50b01189a4f45ff25ced5d3580eda694e12) 2026-01-13
  - Latest: [685e31e758](https://github.com/OpenAgentsInc/openagents/commit/685e31e758a2e1625b8ef02e8cc702fbfeff4841) 2026-02-04
- [docs/adr/ADR-0010-decision-pipeline-gating.md](https://github.com/OpenAgentsInc/openagents/blob/main/docs/adr/ADR-0010-decision-pipeline-gating.md)
  - Title: ADR-0010: Decision Pipeline Gating and Counterfactual Recording
  - Matches: `RLM`: [2 lines](https://github.com/OpenAgentsInc/openagents/blob/main/docs/adr/ADR-0010-decision-pipeline-gating.md#L13)
  - Oldest: [6d5fa50b01](https://github.com/OpenAgentsInc/openagents/commit/6d5fa50b01189a4f45ff25ced5d3580eda694e12) 2026-01-13
  - Latest: [685e31e758](https://github.com/OpenAgentsInc/openagents/commit/685e31e758a2e1625b8ef02e8cc702fbfeff4841) 2026-02-04
- [docs/adr/INDEX.md](https://github.com/OpenAgentsInc/openagents/blob/main/docs/adr/INDEX.md)
  - Title: ADR Index
  - Matches: `RLM`: [1 lines](https://github.com/OpenAgentsInc/openagents/blob/main/docs/adr/INDEX.md#L16)
  - Oldest: [b05412601a](https://github.com/OpenAgentsInc/openagents/commit/b05412601a02b2e5cea83d25e9d66f585f0cc55c) 2026-01-13
  - Latest: [f9612ff718](https://github.com/OpenAgentsInc/openagents/commit/f9612ff718d0ff3efa8861d623eaf7f348ad01d8) 2026-02-06
- [docs/archive/LEGACY_DOCS.md](https://github.com/OpenAgentsInc/openagents/blob/main/docs/archive/LEGACY_DOCS.md)
  - Title: Legacy Documentation Mapping
  - Matches: `RLM`: [2 lines](https://github.com/OpenAgentsInc/openagents/blob/main/docs/archive/LEGACY_DOCS.md#L89)
  - Oldest: [2c25d558af](https://github.com/OpenAgentsInc/openagents/commit/2c25d558afa7de1dda0690c4dc6effdff092c12c) 2026-02-04
  - Latest: [685e31e758](https://github.com/OpenAgentsInc/openagents/commit/685e31e758a2e1625b8ef02e8cc702fbfeff4841) 2026-02-04
- [docs/archive/autopilot-desktop-self-improver-plan.md](https://github.com/OpenAgentsInc/openagents/blob/main/docs/archive/autopilot-desktop-self-improver-plan.md)
  - Title: Autopilot Desktop Self-Improver Plan (DSPy/DSRS)
  - Matches: `GEPA`: [3 lines](https://github.com/OpenAgentsInc/openagents/blob/main/docs/archive/autopilot-desktop-self-improver-plan.md#L29), `MIPRO`: [3 lines](https://github.com/OpenAgentsInc/openagents/blob/main/docs/archive/autopilot-desktop-self-improver-plan.md#L29)
  - Oldest: [2c25d558af](https://github.com/OpenAgentsInc/openagents/commit/2c25d558afa7de1dda0690c4dc6effdff092c12c) 2026-02-04
  - Latest: [2c25d558af](https://github.com/OpenAgentsInc/openagents/commit/2c25d558afa7de1dda0690c4dc6effdff092c12c) 2026-02-04
- [docs/archive/autopilot-migration-plan.md](https://github.com/OpenAgentsInc/openagents/blob/main/docs/archive/autopilot-migration-plan.md)
  - Title: Autopilot Architecture Migration Plan
  - Matches: `RLM`: [2 lines](https://github.com/OpenAgentsInc/openagents/blob/main/docs/archive/autopilot-migration-plan.md#L176)
  - Oldest: [2c25d558af](https://github.com/OpenAgentsInc/openagents/commit/2c25d558afa7de1dda0690c4dc6effdff092c12c) 2026-02-04
  - Latest: [2c25d558af](https://github.com/OpenAgentsInc/openagents/commit/2c25d558afa7de1dda0690c4dc6effdff092c12c) 2026-02-04
- [docs/autopilot/AUTOPILOT_OPTIMIZATION_PLAN.md](https://github.com/OpenAgentsInc/openagents/blob/main/docs/autopilot/AUTOPILOT_OPTIMIZATION_PLAN.md)
  - Title: Autopilot Optimization Plan (DSE-first, Horizons/Monty-inspired)
  - Matches: `RLM`: [13 lines](https://github.com/OpenAgentsInc/openagents/blob/main/docs/autopilot/AUTOPILOT_OPTIMIZATION_PLAN.md#L5), `MIPRO`: [4 lines](https://github.com/OpenAgentsInc/openagents/blob/main/docs/autopilot/AUTOPILOT_OPTIMIZATION_PLAN.md#L43)
  - Oldest: [412fe7793f](https://github.com/OpenAgentsInc/openagents/commit/412fe7793ff79d717a5c9c5eb986af7ff0accfa3) 2026-02-06
  - Latest: [f962acff8c](https://github.com/OpenAgentsInc/openagents/commit/f962acff8c6895d66a988a9b47068b14e6acd706) 2026-02-06
- [docs/autopilot/MVP_USER_STORIES.md](https://github.com/OpenAgentsInc/openagents/blob/main/docs/autopilot/MVP_USER_STORIES.md)
  - Title: Autopilot MVP User Stories
  - Matches: `GEPA`: [1 lines](https://github.com/OpenAgentsInc/openagents/blob/main/docs/autopilot/MVP_USER_STORIES.md#L180)
  - Oldest: [a8ddecf4bd](https://github.com/OpenAgentsInc/openagents/commit/a8ddecf4bd2812588eea8e8991571b22ed52c9fa) 2026-02-07
  - Latest: [a8ddecf4bd](https://github.com/OpenAgentsInc/openagents/commit/a8ddecf4bd2812588eea8e8991571b22ed52c9fa) 2026-02-07
- [docs/autopilot/anon-chat-execution-plane.md](https://github.com/OpenAgentsInc/openagents/blob/main/docs/autopilot/anon-chat-execution-plane.md)
  - Title: Autopilot Chat Execution Plane (Convex-First MVP)
  - Matches: `GEPA`: [4 lines](https://github.com/OpenAgentsInc/openagents/blob/main/docs/autopilot/anon-chat-execution-plane.md#L50)
  - Oldest: [0af4699162](https://github.com/OpenAgentsInc/openagents/commit/0af4699162a1e972f5f5f7cea65a996aaeb292d6) 2026-02-07
  - Latest: [e91451e0ea](https://github.com/OpenAgentsInc/openagents/commit/e91451e0eac4413dfef896ba2a5d92a758d6f8ed) 2026-02-07
- [docs/autopilot/bootstrap-plan.md](https://github.com/OpenAgentsInc/openagents/blob/main/docs/autopilot/bootstrap-plan.md)
  - Title: Autopilot Bootstrap (DB-Backed, Effect Schema) Plan
  - Matches: `GEPA`: [1 lines](https://github.com/OpenAgentsInc/openagents/blob/main/docs/autopilot/bootstrap-plan.md#L419)
  - Oldest: [e27f9e20b6](https://github.com/OpenAgentsInc/openagents/commit/e27f9e20b65f0a9eeba24fe556a8741b851a0887) 2026-02-05
  - Latest: [9a49407bd4](https://github.com/OpenAgentsInc/openagents/commit/9a49407bd4b4f5b981a31346247cecf5034fa9d8) 2026-02-07
- [docs/autopilot/horizons-synergies.md](https://github.com/OpenAgentsInc/openagents/blob/main/docs/autopilot/horizons-synergies.md)
  - Title: Horizons and OpenAgents Autopilot / Effect / DSE: Synergies and Learnings
  - Matches: `RLM`: [14 lines](https://github.com/OpenAgentsInc/openagents/blob/main/docs/autopilot/horizons-synergies.md#L5), `MIPRO`: [10 lines](https://github.com/OpenAgentsInc/openagents/blob/main/docs/autopilot/horizons-synergies.md#L5)
  - Oldest: [076731089a](https://github.com/OpenAgentsInc/openagents/commit/076731089adb69cc46da91f1369aa03cd8af087b) 2026-02-06
  - Latest: [f9612ff718](https://github.com/OpenAgentsInc/openagents/commit/f9612ff718d0ff3efa8861d623eaf7f348ad01d8) 2026-02-06
- [docs/autopilot/microcode-synergies.md](https://github.com/OpenAgentsInc/openagents/blob/main/docs/autopilot/microcode-synergies.md)
  - Title: Microcode and OpenAgents Autopilot / Effect / DSE: Synergies and Learnings
  - Matches: `RLM`: [1 lines](https://github.com/OpenAgentsInc/openagents/blob/main/docs/autopilot/microcode-synergies.md#L156)
  - Oldest: [2e2ae422a7](https://github.com/OpenAgentsInc/openagents/commit/2e2ae422a766d83a40823426dc004443bf86462f) 2026-02-06
  - Latest: [2e2ae422a7](https://github.com/OpenAgentsInc/openagents/commit/2e2ae422a766d83a40823426dc004443bf86462f) 2026-02-06
- [docs/autopilot/rlm-synergies.md](https://github.com/OpenAgentsInc/openagents/blob/main/docs/autopilot/rlm-synergies.md)
  - Title: RLMs (“Recursive Language Models”) and OpenAgents Autopilot / Effect / DSE: Synergies and Integration Plan
  - Matches: `RLM`: [29 lines](https://github.com/OpenAgentsInc/openagents/blob/main/docs/autopilot/rlm-synergies.md#L1), `GEPA`: [1 lines](https://github.com/OpenAgentsInc/openagents/blob/main/docs/autopilot/rlm-synergies.md#L129), `MIPRO`: [1 lines](https://github.com/OpenAgentsInc/openagents/blob/main/docs/autopilot/rlm-synergies.md#L129)
  - Oldest: [f962acff8c](https://github.com/OpenAgentsInc/openagents/commit/f962acff8c6895d66a988a9b47068b14e6acd706) 2026-02-06
  - Latest: [f962acff8c](https://github.com/OpenAgentsInc/openagents/commit/f962acff8c6895d66a988a9b47068b14e6acd706) 2026-02-06
- [docs/autopilot/spec.md](https://github.com/OpenAgentsInc/openagents/blob/main/docs/autopilot/spec.md)
  - Title: Autopilot (Simplified Spec)
  - Matches: `GEPA`: [2 lines](https://github.com/OpenAgentsInc/openagents/blob/main/docs/autopilot/spec.md#L34)
  - Oldest: [8f6bb5d4ae](https://github.com/OpenAgentsInc/openagents/commit/8f6bb5d4ae47c3f76a34b39bc4c01f84a0f0cf4f) 2026-02-05
  - Latest: [9a49407bd4](https://github.com/OpenAgentsInc/openagents/commit/9a49407bd4b4f5b981a31346247cecf5034fa9d8) 2026-02-07
- [docs/dspy/openagents-usage.md](https://github.com/OpenAgentsInc/openagents/blob/main/docs/dspy/openagents-usage.md)
  - Title: DSPy in OpenAgents: Why It Matters and How We Use It
  - Matches: `RLM`: [2 lines](https://github.com/OpenAgentsInc/openagents/blob/main/docs/dspy/openagents-usage.md#L37), `GEPA`: [4 lines](https://github.com/OpenAgentsInc/openagents/blob/main/docs/dspy/openagents-usage.md#L11), `MIPRO`: [4 lines](https://github.com/OpenAgentsInc/openagents/blob/main/docs/dspy/openagents-usage.md#L11)
  - Oldest: [980939b8cb](https://github.com/OpenAgentsInc/openagents/commit/980939b8cb28be6a98d1cb4d0b5370c3acc454ed) 2026-01-25
  - Latest: [980939b8cb](https://github.com/OpenAgentsInc/openagents/commit/980939b8cb28be6a98d1cb4d0b5370c3acc454ed) 2026-01-25
- [docs/dspy/report.md](https://github.com/OpenAgentsInc/openagents/blob/main/docs/dspy/report.md)
  - Title: How DSPy, MIPRO, GEPA, and RLMs fit together
  - Matches: `RLM`: [18 lines](https://github.com/OpenAgentsInc/openagents/blob/main/docs/dspy/report.md#L1), `GEPA`: [19 lines](https://github.com/OpenAgentsInc/openagents/blob/main/docs/dspy/report.md#L1), `MIPRO`: [18 lines](https://github.com/OpenAgentsInc/openagents/blob/main/docs/dspy/report.md#L1)
  - Oldest: [e05c58c635](https://github.com/OpenAgentsInc/openagents/commit/e05c58c63562f2675d3951aff5a51a7f4ec2e807) 2026-01-25
  - Latest: [980939b8cb](https://github.com/OpenAgentsInc/openagents/commit/980939b8cb28be6a98d1cb4d0b5370c3acc454ed) 2026-01-25
- [docs/logs/20260125-2359-audit.md](https://github.com/OpenAgentsInc/openagents/blob/main/docs/logs/20260125-2359-audit.md)
  - Title: Codebase Audit (2026-01-25 23:59)
  - Matches: `GEPA`: [1 lines](https://github.com/OpenAgentsInc/openagents/blob/main/docs/logs/20260125-2359-audit.md#L80), `MIPRO`: [1 lines](https://github.com/OpenAgentsInc/openagents/blob/main/docs/logs/20260125-2359-audit.md#L79)
  - Oldest: [159367223f](https://github.com/OpenAgentsInc/openagents/commit/159367223fe7f7fc9431487fc26fff71f5176545) 2026-01-26
  - Latest: [f5c8a1e636](https://github.com/OpenAgentsInc/openagents/commit/f5c8a1e63633183e04f17d48f2893ed82e340d26) 2026-01-26
- [docs/logs/20260126/1705-clippy-analysis.md](https://github.com/OpenAgentsInc/openagents/blob/main/docs/logs/20260126/1705-clippy-analysis.md)
  - Title: Clippy Status (workspace)
  - Matches: `RLM`: [6 lines](https://github.com/OpenAgentsInc/openagents/blob/main/docs/logs/20260126/1705-clippy-analysis.md#L27)
  - Oldest: [d647cbbe94](https://github.com/OpenAgentsInc/openagents/commit/d647cbbe94a008a40348908dabc48f3c7362b306) 2026-01-26
  - Latest: [526c04a60e](https://github.com/OpenAgentsInc/openagents/commit/526c04a60e6bf3ac6196e82c681f1d2e3a2e3932) 2026-01-29
- [docs/nostr/NOSTR_AUDIT.md](https://github.com/OpenAgentsInc/openagents/blob/main/docs/nostr/NOSTR_AUDIT.md)
  - Title: Nostr Audit (OpenAgents)
  - Matches: `RLM`: [5 lines](https://github.com/OpenAgentsInc/openagents/blob/main/docs/nostr/NOSTR_AUDIT.md#L44)
  - Oldest: [df157ab2aa](https://github.com/OpenAgentsInc/openagents/commit/df157ab2aa6a163f2924661a22244674363ad418) 2026-01-27
  - Latest: [2c25d558af](https://github.com/OpenAgentsInc/openagents/commit/2c25d558afa7de1dda0690c4dc6effdff092c12c) 2026-02-04
- [docs/nostr/SPARK_AUDIT.md](https://github.com/OpenAgentsInc/openagents/blob/main/docs/nostr/SPARK_AUDIT.md)
  - Title: Spark Audit (OpenAgents)
  - Matches: `RLM`: [2 lines](https://github.com/OpenAgentsInc/openagents/blob/main/docs/nostr/SPARK_AUDIT.md#L120)
  - Oldest: [93cac29c1d](https://github.com/OpenAgentsInc/openagents/commit/93cac29c1dd47d551702c42717cd8faa9cf83e50) 2026-01-27
  - Latest: [93cac29c1d](https://github.com/OpenAgentsInc/openagents/commit/93cac29c1dd47d551702c42717cd8faa9cf83e50) 2026-01-27
- [docs/open-protocols/OPEN_PROTOCOLS_LAUNCH_PLAN.md](https://github.com/OpenAgentsInc/openagents/blob/main/docs/open-protocols/OPEN_PROTOCOLS_LAUNCH_PLAN.md)
  - Title: Open Protocols Launch Plan (Sequential)
  - Matches: `RLM`: [1 lines](https://github.com/OpenAgentsInc/openagents/blob/main/docs/open-protocols/OPEN_PROTOCOLS_LAUNCH_PLAN.md#L249)
  - Oldest: [2c25d558af](https://github.com/OpenAgentsInc/openagents/commit/2c25d558afa7de1dda0690c4dc6effdff092c12c) 2026-02-04
  - Latest: [220768e7fa](https://github.com/OpenAgentsInc/openagents/commit/220768e7fa0b987e4c98b5f8f1e64c767c6b6e55) 2026-02-04
- [docs/transcripts/openagents/TOPICS.md](https://github.com/OpenAgentsInc/openagents/blob/main/docs/transcripts/openagents/TOPICS.md)
  - Title: OpenAgents Transcripts — Topic Index
  - Matches: `RLM`: [9 lines](https://github.com/OpenAgentsInc/openagents/blob/main/docs/transcripts/openagents/TOPICS.md#L16), `GEPA`: [2 lines](https://github.com/OpenAgentsInc/openagents/blob/main/docs/transcripts/openagents/TOPICS.md#L152), `MIPRO`: [2 lines](https://github.com/OpenAgentsInc/openagents/blob/main/docs/transcripts/openagents/TOPICS.md#L152)
  - Oldest: [05bcc9e5f4](https://github.com/OpenAgentsInc/openagents/commit/05bcc9e5f48036ef950e246256f311e9ddeaa564) 2026-02-01
  - Latest: [05bcc9e5f4](https://github.com/OpenAgentsInc/openagents/commit/05bcc9e5f48036ef950e246256f311e9ddeaa564) 2026-02-01

### crates/README.md/

- [crates/README.md](https://github.com/OpenAgentsInc/openagents/blob/main/crates/README.md)
  - Title: OpenAgents Crates
  - Matches: `RLM`: [6 lines](https://github.com/OpenAgentsInc/openagents/blob/main/crates/README.md#L14)
  - Oldest: [9ecc867190](https://github.com/OpenAgentsInc/openagents/commit/9ecc8671905470acd03ae1dc6b0592e24758d956) 2025-12-10
  - Latest: [d794a8cbaa](https://github.com/OpenAgentsInc/openagents/commit/d794a8cbaa6b9fc7bcbef13875cb28ed03c0efea) 2026-01-27

### crates/adjutant/

- [crates/adjutant/docs/DSPY-INTEGRATION.md](https://github.com/OpenAgentsInc/openagents/blob/main/crates/adjutant/docs/DSPY-INTEGRATION.md)
  - Title: DSPy Integration
  - Matches: `RLM`: [16 lines](https://github.com/OpenAgentsInc/openagents/blob/main/crates/adjutant/docs/DSPY-INTEGRATION.md#L249), `MIPRO`: [11 lines](https://github.com/OpenAgentsInc/openagents/blob/main/crates/adjutant/docs/DSPY-INTEGRATION.md#L3)
  - Oldest: [b4e14fd865](https://github.com/OpenAgentsInc/openagents/commit/b4e14fd8657510f19288d536ba5c804dfd33b76b) 2026-01-08
  - Latest: [38e113e365](https://github.com/OpenAgentsInc/openagents/commit/38e113e365c892b3175fa8350ee6d33bfffd748b) 2026-01-13
- [crates/adjutant/docs/README.md](https://github.com/OpenAgentsInc/openagents/blob/main/crates/adjutant/docs/README.md)
  - Title: Adjutant
  - Matches: `RLM`: [35 lines](https://github.com/OpenAgentsInc/openagents/blob/main/crates/adjutant/docs/README.md#L40), `MIPRO`: [4 lines](https://github.com/OpenAgentsInc/openagents/blob/main/crates/adjutant/docs/README.md#L230)
  - Oldest: [0ac34688f2](https://github.com/OpenAgentsInc/openagents/commit/0ac34688f251fd92f1199df80ab89c0c6905e969) 2026-01-08
  - Latest: [685e31e758](https://github.com/OpenAgentsInc/openagents/commit/685e31e758a2e1625b8ef02e8cc702fbfeff4841) 2026-02-04
- [crates/adjutant/docs/TIERED-EXECUTOR.md](https://github.com/OpenAgentsInc/openagents/blob/main/crates/adjutant/docs/TIERED-EXECUTOR.md)
  - Title: Tiered Executor
  - Matches: `MIPRO`: [2 lines](https://github.com/OpenAgentsInc/openagents/blob/main/crates/adjutant/docs/TIERED-EXECUTOR.md#L280)
  - Oldest: [0ac34688f2](https://github.com/OpenAgentsInc/openagents/commit/0ac34688f251fd92f1199df80ab89c0c6905e969) 2026-01-08
  - Latest: [687510c96d](https://github.com/OpenAgentsInc/openagents/commit/687510c96de3a326e6c23aaedc2ce6658dd2669e) 2026-01-12

### crates/autopilot/

- [crates/autopilot/docs/MVP_COMPREHENSIVE.md](https://github.com/OpenAgentsInc/openagents/blob/main/crates/autopilot/docs/MVP_COMPREHENSIVE.md)
  - Title: Autopilot MVP (Comprehensive)
  - Matches: `RLM`: [1 lines](https://github.com/OpenAgentsInc/openagents/blob/main/crates/autopilot/docs/MVP_COMPREHENSIVE.md#L199)
  - Oldest: [5ad60ffc72](https://github.com/OpenAgentsInc/openagents/commit/5ad60ffc729bc80c1636c76e494205e69e856b16) 2026-01-12
  - Latest: [5ad60ffc72](https://github.com/OpenAgentsInc/openagents/commit/5ad60ffc729bc80c1636c76e494205e69e856b16) 2026-01-12

### crates/compute/

- [crates/compute/docs/FRLM_TOOLS.md](https://github.com/OpenAgentsInc/openagents/blob/main/crates/compute/docs/FRLM_TOOLS.md)
  - Title: FRLM Tools Integration
  - Matches: `RLM`: [21 lines](https://github.com/OpenAgentsInc/openagents/blob/main/crates/compute/docs/FRLM_TOOLS.md#L1)
  - Oldest: [090bc1c15e](https://github.com/OpenAgentsInc/openagents/commit/090bc1c15e6a749948d1e0396327559c1ac8ac5c) 2026-01-04
  - Latest: [3c1256adf5](https://github.com/OpenAgentsInc/openagents/commit/3c1256adf534cf7ee96ba5790dda73374308cbf9) 2026-01-12

### crates/dsrs/

- [crates/dsrs/docs/ARCHITECTURE.md](https://github.com/OpenAgentsInc/openagents/blob/main/crates/dsrs/docs/ARCHITECTURE.md)
  - Title: dsrs Architecture
  - Matches: `GEPA`: [2 lines](https://github.com/OpenAgentsInc/openagents/blob/main/crates/dsrs/docs/ARCHITECTURE.md#L221), `MIPRO`: [2 lines](https://github.com/OpenAgentsInc/openagents/blob/main/crates/dsrs/docs/ARCHITECTURE.md#L209)
  - Oldest: [5550e1c107](https://github.com/OpenAgentsInc/openagents/commit/5550e1c10729eaa6320adb731f3c8b064a3f26a9) 2026-01-09
  - Latest: [685e31e758](https://github.com/OpenAgentsInc/openagents/commit/685e31e758a2e1625b8ef02e8cc702fbfeff4841) 2026-02-04
- [crates/dsrs/docs/ARTIFACTS.md](https://github.com/OpenAgentsInc/openagents/blob/main/crates/dsrs/docs/ARTIFACTS.md)
  - Title: MVP Artifacts
  - Matches: `MIPRO`: [1 lines](https://github.com/OpenAgentsInc/openagents/blob/main/crates/dsrs/docs/ARTIFACTS.md#L372)
  - Oldest: [634f5b6277](https://github.com/OpenAgentsInc/openagents/commit/634f5b62774f588c9d0ea95a75771839a0af0ff5) 2026-01-13
  - Latest: [685e31e758](https://github.com/OpenAgentsInc/openagents/commit/685e31e758a2e1625b8ef02e8cc702fbfeff4841) 2026-02-04
- [crates/dsrs/docs/COMPILER-CONTRACT.md](https://github.com/OpenAgentsInc/openagents/blob/main/crates/dsrs/docs/COMPILER-CONTRACT.md)
  - Title: Compiler Contract (Wave 3)
  - Matches: `GEPA`: [1 lines](https://github.com/OpenAgentsInc/openagents/blob/main/crates/dsrs/docs/COMPILER-CONTRACT.md#L28), `MIPRO`: [2 lines](https://github.com/OpenAgentsInc/openagents/blob/main/crates/dsrs/docs/COMPILER-CONTRACT.md#L28)
  - Oldest: [5550e1c107](https://github.com/OpenAgentsInc/openagents/commit/5550e1c10729eaa6320adb731f3c8b064a3f26a9) 2026-01-09
  - Latest: [dd2e4a0d9b](https://github.com/OpenAgentsInc/openagents/commit/dd2e4a0d9b841b856c3b224215a33a093e79647d) 2026-01-13
- [crates/dsrs/docs/DSPY_ROADMAP.md](https://github.com/OpenAgentsInc/openagents/blob/main/crates/dsrs/docs/DSPY_ROADMAP.md)
  - Title: DSPy Roadmap (OpenAgents)
  - Matches: `RLM`: [3 lines](https://github.com/OpenAgentsInc/openagents/blob/main/crates/dsrs/docs/DSPY_ROADMAP.md#L49)
  - Oldest: [ddf6d1c9d9](https://github.com/OpenAgentsInc/openagents/commit/ddf6d1c9d99195c415d71954757d752aea220c17) 2026-01-12
  - Latest: [592cd606a3](https://github.com/OpenAgentsInc/openagents/commit/592cd606a3b56164bca832d783e4260814a4f04e) 2026-01-13
- [crates/dsrs/docs/EVALUATION.md](https://github.com/OpenAgentsInc/openagents/blob/main/crates/dsrs/docs/EVALUATION.md)
  - Title: Evaluation System (Wave 5)
  - Matches: `MIPRO`: [1 lines](https://github.com/OpenAgentsInc/openagents/blob/main/crates/dsrs/docs/EVALUATION.md#L304)
  - Oldest: [b90c789487](https://github.com/OpenAgentsInc/openagents/commit/b90c789487675852d991922db3d07118c8bdfe99) 2026-01-09
  - Latest: [dd2e4a0d9b](https://github.com/OpenAgentsInc/openagents/commit/dd2e4a0d9b841b856c3b224215a33a093e79647d) 2026-01-13
- [crates/dsrs/docs/MARKETPLACE.md](https://github.com/OpenAgentsInc/openagents/blob/main/crates/dsrs/docs/MARKETPLACE.md)
  - Title: DSPy Training Data Marketplace
  - Matches: `MIPRO`: [1 lines](https://github.com/OpenAgentsInc/openagents/blob/main/crates/dsrs/docs/MARKETPLACE.md#L33)
  - Oldest: [1413c16c71](https://github.com/OpenAgentsInc/openagents/commit/1413c16c710c574678939525c1060dbef0a0e87b) 2026-01-09
  - Latest: [dd2e4a0d9b](https://github.com/OpenAgentsInc/openagents/commit/dd2e4a0d9b841b856c3b224215a33a093e79647d) 2026-01-13
- [crates/dsrs/docs/METRICS.md](https://github.com/OpenAgentsInc/openagents/blob/main/crates/dsrs/docs/METRICS.md)
  - Title: Metrics
  - Matches: `RLM`: [1 lines](https://github.com/OpenAgentsInc/openagents/blob/main/crates/dsrs/docs/METRICS.md#L856), `GEPA`: [1 lines](https://github.com/OpenAgentsInc/openagents/blob/main/crates/dsrs/docs/METRICS.md#L78), `MIPRO`: [2 lines](https://github.com/OpenAgentsInc/openagents/blob/main/crates/dsrs/docs/METRICS.md#L829)
  - Oldest: [dff775ddd8](https://github.com/OpenAgentsInc/openagents/commit/dff775ddd86263a7393981185ccd7f7f7a08109f) 2026-01-13
  - Latest: [e1fa267770](https://github.com/OpenAgentsInc/openagents/commit/e1fa2677709755c1b61f5297dc8cd8d753e384ad) 2026-01-13
- [crates/dsrs/docs/MODULES.md](https://github.com/OpenAgentsInc/openagents/blob/main/crates/dsrs/docs/MODULES.md)
  - Title: Modules
  - Matches: `RLM`: [18 lines](https://github.com/OpenAgentsInc/openagents/blob/main/crates/dsrs/docs/MODULES.md#L334), `MIPRO`: [2 lines](https://github.com/OpenAgentsInc/openagents/blob/main/crates/dsrs/docs/MODULES.md#L672)
  - Oldest: [dff775ddd8](https://github.com/OpenAgentsInc/openagents/commit/dff775ddd86263a7393981185ccd7f7f7a08109f) 2026-01-13
  - Latest: [685e31e758](https://github.com/OpenAgentsInc/openagents/commit/685e31e758a2e1625b8ef02e8cc702fbfeff4841) 2026-02-04
- [crates/dsrs/docs/OPTIMIZERS.md](https://github.com/OpenAgentsInc/openagents/blob/main/crates/dsrs/docs/OPTIMIZERS.md)
  - Title: Optimizers
  - Matches: `GEPA`: [12 lines](https://github.com/OpenAgentsInc/openagents/blob/main/crates/dsrs/docs/OPTIMIZERS.md#L243), `MIPRO`: [9 lines](https://github.com/OpenAgentsInc/openagents/blob/main/crates/dsrs/docs/OPTIMIZERS.md#L50)
  - Oldest: [dff775ddd8](https://github.com/OpenAgentsInc/openagents/commit/dff775ddd86263a7393981185ccd7f7f7a08109f) 2026-01-13
  - Latest: [626ff23a58](https://github.com/OpenAgentsInc/openagents/commit/626ff23a58219806cbd6afdbb48ec3949ccc762f) 2026-01-13
- [crates/dsrs/docs/README.md](https://github.com/OpenAgentsInc/openagents/blob/main/crates/dsrs/docs/README.md)
  - Title: dsrs - Rust DSPy
  - Matches: `RLM`: [2 lines](https://github.com/OpenAgentsInc/openagents/blob/main/crates/dsrs/docs/README.md#L128), `GEPA`: [3 lines](https://github.com/OpenAgentsInc/openagents/blob/main/crates/dsrs/docs/README.md#L57), `MIPRO`: [3 lines](https://github.com/OpenAgentsInc/openagents/blob/main/crates/dsrs/docs/README.md#L57)
  - Oldest: [5550e1c107](https://github.com/OpenAgentsInc/openagents/commit/5550e1c10729eaa6320adb731f3c8b064a3f26a9) 2026-01-09
  - Latest: [dd2e4a0d9b](https://github.com/OpenAgentsInc/openagents/commit/dd2e4a0d9b841b856c3b224215a33a093e79647d) 2026-01-13
- [crates/dsrs/docs/SIGNATURES.md](https://github.com/OpenAgentsInc/openagents/blob/main/crates/dsrs/docs/SIGNATURES.md)
  - Title: Signatures (Wave 4)
  - Matches: `RLM`: [33 lines](https://github.com/OpenAgentsInc/openagents/blob/main/crates/dsrs/docs/SIGNATURES.md#L811), `GEPA`: [1 lines](https://github.com/OpenAgentsInc/openagents/blob/main/crates/dsrs/docs/SIGNATURES.md#L15), `MIPRO`: [5 lines](https://github.com/OpenAgentsInc/openagents/blob/main/crates/dsrs/docs/SIGNATURES.md#L15)
  - Oldest: [2d25cb28cf](https://github.com/OpenAgentsInc/openagents/commit/2d25cb28cf51d99b72b430e241d9d8bd6b1ad429) 2026-01-09
  - Latest: [6a49fbf870](https://github.com/OpenAgentsInc/openagents/commit/6a49fbf8709d6ba86b590fdce841ae81274fc3b5) 2026-01-29
- [crates/dsrs/docs/TOOLS.md](https://github.com/OpenAgentsInc/openagents/blob/main/crates/dsrs/docs/TOOLS.md)
  - Title: Tools
  - Matches: `RLM`: [21 lines](https://github.com/OpenAgentsInc/openagents/blob/main/crates/dsrs/docs/TOOLS.md#L5)
  - Oldest: [dff775ddd8](https://github.com/OpenAgentsInc/openagents/commit/dff775ddd86263a7393981185ccd7f7f7a08109f) 2026-01-13
  - Latest: [99fefea85f](https://github.com/OpenAgentsInc/openagents/commit/99fefea85f75a8259616556839de6a55bd263fc1) 2026-01-14

### crates/frlm/

- [crates/frlm/docs/ARCHITECTURE.md](https://github.com/OpenAgentsInc/openagents/blob/main/crates/frlm/docs/ARCHITECTURE.md)
  - Title: FRLM Architecture
  - Matches: `RLM`: [21 lines](https://github.com/OpenAgentsInc/openagents/blob/main/crates/frlm/docs/ARCHITECTURE.md#L1), `GEPA`: [1 lines](https://github.com/OpenAgentsInc/openagents/blob/main/crates/frlm/docs/ARCHITECTURE.md#L404), `MIPRO`: [1 lines](https://github.com/OpenAgentsInc/openagents/blob/main/crates/frlm/docs/ARCHITECTURE.md#L404)
  - Oldest: [84273d5a42](https://github.com/OpenAgentsInc/openagents/commit/84273d5a42eae80748bcf6b4d3cc388ea50e8843) 2026-01-04
  - Latest: [3c1256adf5](https://github.com/OpenAgentsInc/openagents/commit/3c1256adf534cf7ee96ba5790dda73374308cbf9) 2026-01-12
- [crates/frlm/docs/README.md](https://github.com/OpenAgentsInc/openagents/blob/main/crates/frlm/docs/README.md)
  - Title: FRLM - Federated Recursive Language Models
  - Matches: `RLM`: [34 lines](https://github.com/OpenAgentsInc/openagents/blob/main/crates/frlm/docs/README.md#L1)
  - Oldest: [84273d5a42](https://github.com/OpenAgentsInc/openagents/commit/84273d5a42eae80748bcf6b4d3cc388ea50e8843) 2026-01-04
  - Latest: [685e31e758](https://github.com/OpenAgentsInc/openagents/commit/685e31e758a2e1625b8ef02e8cc702fbfeff4841) 2026-02-04
- [crates/frlm/docs/TOOLS.md](https://github.com/OpenAgentsInc/openagents/blob/main/crates/frlm/docs/TOOLS.md)
  - Title: FRLM SubQuery Tools Support
  - Matches: `RLM`: [11 lines](https://github.com/OpenAgentsInc/openagents/blob/main/crates/frlm/docs/TOOLS.md#L1)
  - Oldest: [090bc1c15e](https://github.com/OpenAgentsInc/openagents/commit/090bc1c15e6a749948d1e0396327559c1ac8ac5c) 2026-01-04
  - Latest: [090bc1c15e](https://github.com/OpenAgentsInc/openagents/commit/090bc1c15e6a749948d1e0396327559c1ac8ac5c) 2026-01-04

### crates/gateway/

- [crates/gateway/README.md](https://github.com/OpenAgentsInc/openagents/blob/main/crates/gateway/README.md)
  - Title: Gateway
  - Matches: `RLM`: [7 lines](https://github.com/OpenAgentsInc/openagents/blob/main/crates/gateway/README.md#L662)
  - Oldest: [e30475cdb4](https://github.com/OpenAgentsInc/openagents/commit/e30475cdb4c93f5b336b3a58f567d5681dbdae59) 2026-01-08
  - Latest: [b0468e0f2a](https://github.com/OpenAgentsInc/openagents/commit/b0468e0f2aefc06dc3bdde705dbad60ccb23aeec) 2026-01-16

### crates/manatap/

- [crates/manatap/docs/CHAIN_VISUALIZER_SPEC.md](https://github.com/OpenAgentsInc/openagents/blob/main/crates/manatap/docs/CHAIN_VISUALIZER_SPEC.md)
  - Title: Mana Tap: DSPy Chain Visualizer Spec
  - Matches: `GEPA`: [1 lines](https://github.com/OpenAgentsInc/openagents/blob/main/crates/manatap/docs/CHAIN_VISUALIZER_SPEC.md#L627), `MIPRO`: [1 lines](https://github.com/OpenAgentsInc/openagents/blob/main/crates/manatap/docs/CHAIN_VISUALIZER_SPEC.md#L627)
  - Oldest: [664ed0f578](https://github.com/OpenAgentsInc/openagents/commit/664ed0f57800e0cfe66ca663f2597dc0afd38819) 2026-01-12
  - Latest: [664ed0f578](https://github.com/OpenAgentsInc/openagents/commit/664ed0f57800e0cfe66ca663f2597dc0afd38819) 2026-01-12

### crates/nostr/

- [crates/nostr/client/docs/DVM_CLIENT.md](https://github.com/OpenAgentsInc/openagents/blob/main/crates/nostr/client/docs/DVM_CLIENT.md)
  - Title: DvmClient - NIP-90 Data Vending Machine Client
  - Matches: `RLM`: [6 lines](https://github.com/OpenAgentsInc/openagents/blob/main/crates/nostr/client/docs/DVM_CLIENT.md#L73)
  - Oldest: [acbd4b3441](https://github.com/OpenAgentsInc/openagents/commit/acbd4b3441a043f65c02aa720d7b1c5f137a2108) 2026-01-07
  - Latest: [acbd4b3441](https://github.com/OpenAgentsInc/openagents/commit/acbd4b3441a043f65c02aa720d7b1c5f137a2108) 2026-01-07
- [crates/nostr/core/README.md](https://github.com/OpenAgentsInc/openagents/blob/main/crates/nostr/core/README.md)
  - Title: nostr/core
  - Matches: `GEPA`: [6 lines](https://github.com/OpenAgentsInc/openagents/blob/main/crates/nostr/core/README.md#L527)
  - Oldest: [c8a10a3c50](https://github.com/OpenAgentsInc/openagents/commit/c8a10a3c5040bc1f087b624c464fed6ea92259fa) 2025-12-20
  - Latest: [7df96b6e7a](https://github.com/OpenAgentsInc/openagents/commit/7df96b6e7a918fb546b77e8d6175c76ed884626a) 2026-01-11

### crates/pylon/

- [crates/pylon/docs/CLI.md](https://github.com/OpenAgentsInc/openagents/blob/main/crates/pylon/docs/CLI.md)
  - Title: CLI Reference
  - Matches: `RLM`: [16 lines](https://github.com/OpenAgentsInc/openagents/blob/main/crates/pylon/docs/CLI.md#L609)
  - Oldest: [7c8bd1addd](https://github.com/OpenAgentsInc/openagents/commit/7c8bd1addd1f4cff7d1d1e607ebc6607a8f9f1c0) 2025-12-28
  - Latest: [b0468e0f2a](https://github.com/OpenAgentsInc/openagents/commit/b0468e0f2aefc06dc3bdde705dbad60ccb23aeec) 2026-01-16

### crates/rlm/

- [crates/rlm/docs/ARCHITECTURE.md](https://github.com/OpenAgentsInc/openagents/blob/main/crates/rlm/docs/ARCHITECTURE.md)
  - Title: Architecture
  - Matches: `RLM`: [21 lines](https://github.com/OpenAgentsInc/openagents/blob/main/crates/rlm/docs/ARCHITECTURE.md#L5)
  - Oldest: [1976ae8644](https://github.com/OpenAgentsInc/openagents/commit/1976ae8644ac50b594eb8fa14b263c8041f31740) 2026-01-05
  - Latest: [3c1256adf5](https://github.com/OpenAgentsInc/openagents/commit/3c1256adf534cf7ee96ba5790dda73374308cbf9) 2026-01-12
- [crates/rlm/docs/DSPY.md](https://github.com/OpenAgentsInc/openagents/blob/main/crates/rlm/docs/DSPY.md)
  - Title: DSPy Integration
  - Matches: `RLM`: [27 lines](https://github.com/OpenAgentsInc/openagents/blob/main/crates/rlm/docs/DSPY.md#L3), `MIPRO`: [3 lines](https://github.com/OpenAgentsInc/openagents/blob/main/crates/rlm/docs/DSPY.md#L63)
  - Oldest: [8cda787387](https://github.com/OpenAgentsInc/openagents/commit/8cda7873877c0b823c900665f8990ce40577364e) 2026-01-06
  - Latest: [3c1256adf5](https://github.com/OpenAgentsInc/openagents/commit/3c1256adf534cf7ee96ba5790dda73374308cbf9) 2026-01-12
- [crates/rlm/docs/METHODS.md](https://github.com/OpenAgentsInc/openagents/blob/main/crates/rlm/docs/METHODS.md)
  - Title: Methods
  - Matches: `RLM`: [10 lines](https://github.com/OpenAgentsInc/openagents/blob/main/crates/rlm/docs/METHODS.md#L3)
  - Oldest: [1976ae8644](https://github.com/OpenAgentsInc/openagents/commit/1976ae8644ac50b594eb8fa14b263c8041f31740) 2026-01-05
  - Latest: [9d0a720987](https://github.com/OpenAgentsInc/openagents/commit/9d0a720987c0c7cdff4685dce82a8d4f1e436b6b) 2026-01-13
- [crates/rlm/docs/PROVENANCE.md](https://github.com/OpenAgentsInc/openagents/blob/main/crates/rlm/docs/PROVENANCE.md)
  - Title: Provenance Tracking
  - Matches: `RLM`: [7 lines](https://github.com/OpenAgentsInc/openagents/blob/main/crates/rlm/docs/PROVENANCE.md#L3)
  - Oldest: [ee9c53f932](https://github.com/OpenAgentsInc/openagents/commit/ee9c53f932769a8a5503f7c71059b5b61ed6e0ea) 2026-01-06
  - Latest: [ee9c53f932](https://github.com/OpenAgentsInc/openagents/commit/ee9c53f932769a8a5503f7c71059b5b61ed6e0ea) 2026-01-06
- [crates/rlm/docs/README.md](https://github.com/OpenAgentsInc/openagents/blob/main/crates/rlm/docs/README.md)
  - Title: RLM Paper Replication Infrastructure
  - Matches: `RLM`: [39 lines](https://github.com/OpenAgentsInc/openagents/blob/main/crates/rlm/docs/README.md#L1)
  - Oldest: [1976ae8644](https://github.com/OpenAgentsInc/openagents/commit/1976ae8644ac50b594eb8fa14b263c8041f31740) 2026-01-05
  - Latest: [685e31e758](https://github.com/OpenAgentsInc/openagents/commit/685e31e758a2e1625b8ef02e8cc702fbfeff4841) 2026-02-04
- [crates/rlm/docs/RLM-DASHBOARD.md](https://github.com/OpenAgentsInc/openagents/blob/main/crates/rlm/docs/RLM-DASHBOARD.md)
  - Title: RLM Dashboard: OpenAgents Research Workflow Tool
  - Matches: `RLM`: [44 lines](https://github.com/OpenAgentsInc/openagents/blob/main/crates/rlm/docs/RLM-DASHBOARD.md#L1)
  - Oldest: [bb14252529](https://github.com/OpenAgentsInc/openagents/commit/bb14252529a0763dfd62f2d427bd66c31cb4eabf) 2026-01-08
  - Latest: [3c1256adf5](https://github.com/OpenAgentsInc/openagents/commit/3c1256adf534cf7ee96ba5790dda73374308cbf9) 2026-01-12
- [crates/rlm/docs/RUNNING_EXPERIMENTS.md](https://github.com/OpenAgentsInc/openagents/blob/main/crates/rlm/docs/RUNNING_EXPERIMENTS.md)
  - Title: Running Experiments
  - Matches: `RLM`: [4 lines](https://github.com/OpenAgentsInc/openagents/blob/main/crates/rlm/docs/RUNNING_EXPERIMENTS.md#L4)
  - Oldest: [1976ae8644](https://github.com/OpenAgentsInc/openagents/commit/1976ae8644ac50b594eb8fa14b263c8041f31740) 2026-01-05
  - Latest: [3c1256adf5](https://github.com/OpenAgentsInc/openagents/commit/3c1256adf534cf7ee96ba5790dda73374308cbf9) 2026-01-12
- [crates/rlm/docs/TOOLS.md](https://github.com/OpenAgentsInc/openagents/blob/main/crates/rlm/docs/TOOLS.md)
  - Title: RLM Environment Tools
  - Matches: `RLM`: [13 lines](https://github.com/OpenAgentsInc/openagents/blob/main/crates/rlm/docs/TOOLS.md#L1)
  - Oldest: [ee9c53f932](https://github.com/OpenAgentsInc/openagents/commit/ee9c53f932769a8a5503f7c71059b5b61ed6e0ea) 2026-01-06
  - Latest: [ee9c53f932](https://github.com/OpenAgentsInc/openagents/commit/ee9c53f932769a8a5503f7c71059b5b61ed6e0ea) 2026-01-06

### packages/effuse/

- [packages/effuse/docs/MASTER-PLAN-EFFECT-EFFUSE-COMPLETE.md](https://github.com/OpenAgentsInc/openagents/blob/main/packages/effuse/docs/MASTER-PLAN-EFFECT-EFFUSE-COMPLETE.md)
  - Title: Master Plan: Complete Effuse Stack (No React, No TanStack)
  - Matches: `RLM`: [1 lines](https://github.com/OpenAgentsInc/openagents/blob/main/packages/effuse/docs/MASTER-PLAN-EFFECT-EFFUSE-COMPLETE.md#L1105), `GEPA`: [8 lines](https://github.com/OpenAgentsInc/openagents/blob/main/packages/effuse/docs/MASTER-PLAN-EFFECT-EFFUSE-COMPLETE.md#L83)
  - Oldest: [f962acff8c](https://github.com/OpenAgentsInc/openagents/commit/f962acff8c6895d66a988a9b47068b14e6acd706) 2026-02-06
  - Latest: [f7aa8cdfe8](https://github.com/OpenAgentsInc/openagents/commit/f7aa8cdfe816fe1b667b870022983cebe5f8bb53) 2026-02-08

## Git History (Docs Pickaxe: Commits That Changed Term Occurrence)

These are commits selected by `git log -S<TERM> -- '*.md' '*.mdx'` (string count changes in docs).

### RLM

Total commits: 116

- [9a49407bd4](https://github.com/OpenAgentsInc/openagents/commit/9a49407bd4b4f5b981a31346247cecf5034fa9d8) 2026-02-07 docs: Convex-first MVP for Autopilot chat
- [f962acff8c](https://github.com/OpenAgentsInc/openagents/commit/f962acff8c6895d66a988a9b47068b14e6acd706) 2026-02-06 docs: add RLM synergies and Effuse master plan
- [2e2ae422a7](https://github.com/OpenAgentsInc/openagents/commit/2e2ae422a766d83a40823426dc004443bf86462f) 2026-02-06 docs(autopilot): add microcode synergies
- [412fe7793f](https://github.com/OpenAgentsInc/openagents/commit/412fe7793ff79d717a5c9c5eb986af7ff0accfa3) 2026-02-06 docs(autopilot): add unified DSE optimization roadmap
- [076731089a](https://github.com/OpenAgentsInc/openagents/commit/076731089adb69cc46da91f1369aa03cd8af087b) 2026-02-06 docs: Horizons and Monty synergies, full Effect integration delegation
- [52e6c63ae0](https://github.com/OpenAgentsInc/openagents/commit/52e6c63ae0719a22b13feb56181e0369c700ba08) 2026-02-05 Update sandbox plan log
- [220768e7fa](https://github.com/OpenAgentsInc/openagents/commit/220768e7fa0b987e4c98b5f8f1e64c767c6b6e55) 2026-02-04 Remove OpenClaw, indexer, and spark-api; pivot to LiteClaw
- [df5cf3d8c8](https://github.com/OpenAgentsInc/openagents/commit/df5cf3d8c803c894688712bf563f3e63f722b33b) 2026-02-03 docs(web): add pi plugins support plan
- [05bcc9e5f4](https://github.com/OpenAgentsInc/openagents/commit/05bcc9e5f48036ef950e246256f311e9ddeaa564) 2026-02-01 docs(transcripts): add TOPICS.md index for openagents episode transcripts
- [2c6d8cd913](https://github.com/OpenAgentsInc/openagents/commit/2c6d8cd913704e00d63cdab1e9aa06d8b619b64d) 2026-02-01 Revert "docs(PAPER): add TOC sub-bullets for section 12 (12.3.1–12.3.5, 12.6)"
- [a23d481e78](https://github.com/OpenAgentsInc/openagents/commit/a23d481e786928b4c2f35efa0d42b219793526ed) 2026-02-01 docs(PAPER): add TOC sub-bullets for section 12 (12.3.1–12.3.5, 12.6)
- [7c128e9cc2](https://github.com/OpenAgentsInc/openagents/commit/7c128e9cc2fb040212d53de09e9e2059034da077) 2026-01-31 Phase 1 & 2: Open Protocols Launch Plan, Moltbook parity, wallet attach
- [2c9fc73ac3](https://github.com/OpenAgentsInc/openagents/commit/2c9fc73ac33aeed1298d300824a091c9d8833425) 2026-01-28 Complete Storybook layout-engine migration (Phase 3) (#1544)
- [df157ab2aa](https://github.com/OpenAgentsInc/openagents/commit/df157ab2aa6a163f2924661a22244674363ad418) 2026-01-27 docs: add Nostr audit
- [5e011e9457](https://github.com/OpenAgentsInc/openagents/commit/5e011e9457e0a1e8571695929fe5de83e64b1773) 2026-01-27 Remove legacy Tauri desktop app
- [980939b8cb](https://github.com/OpenAgentsInc/openagents/commit/980939b8cb28be6a98d1cb4d0b5370c3acc454ed) 2026-01-25 Add DSPy and Full Auto integration docs
- [e05c58c635](https://github.com/OpenAgentsInc/openagents/commit/e05c58c63562f2675d3951aff5a51a7f4ec2e807) 2026-01-25 initial dspy report
- [311fe17f8d](https://github.com/OpenAgentsInc/openagents/commit/311fe17f8d95ceafc0c3358f62da2c952376d597) 2026-01-25 Move autopilot desktop into monorepo
- [9d0a720987](https://github.com/OpenAgentsInc/openagents/commit/9d0a720987c0c7cdff4685dce82a8d4f1e436b6b) 2026-01-13 Update issue UX docs, compute tests, and RLM patterns
- [6d5fa50b01](https://github.com/OpenAgentsInc/openagents/commit/6d5fa50b01189a4f45ff25ced5d3580eda694e12) 2026-01-13 Add ADR-0006 through ADR-0015 and enhance ADR system
- [447da2f2bc](https://github.com/OpenAgentsInc/openagents/commit/447da2f2bc76865cf209fae51dfb85f2412cfc6c) 2026-01-13 Update AGENTS.md
- [58fdef3544](https://github.com/OpenAgentsInc/openagents/commit/58fdef3544a975eae70b84d37b4e014e9d54a33c) 2026-01-13 agent foundations doc
- [04dcb7b0c9](https://github.com/OpenAgentsInc/openagents/commit/04dcb7b0c979d48b17f21722005625eaa158f5d0) 2026-01-13 Consolidate root docs: remove "two front doors" problem
- [556e068b84](https://github.com/OpenAgentsInc/openagents/commit/556e068b84a8e1421956e8002f202b6a686c2d31) 2026-01-13 Add MVP priorities and outcome-coupled metrics to DSPy docs
- [dff775ddd8](https://github.com/OpenAgentsInc/openagents/commit/dff775ddd86263a7393981185ccd7f7f7a08109f) 2026-01-13 Add comprehensive DSPy primitives documentation
- [9f7598aaef](https://github.com/OpenAgentsInc/openagents/commit/9f7598aaef408a207acff1437e2e1617a26a4552) 2026-01-13 Add comprehensive DSPy signatures documentation
- [ca7aef3b31](https://github.com/OpenAgentsInc/openagents/commit/ca7aef3b31e10d7e8ce3298a99717c9e8ce9f664) 2026-01-12 Update roadmap with phase-2 implementation
- [04128eb482](https://github.com/OpenAgentsInc/openagents/commit/04128eb4822f47f5ef074394d094765aabcc3602) 2026-01-12 Expand paper and roadmap with OS details
- [0609a309da](https://github.com/OpenAgentsInc/openagents/commit/0609a309da4ca84ea3863ef88a6daa69d4794766) 2026-01-12 roadmap
- [93d4043315](https://github.com/OpenAgentsInc/openagents/commit/93d4043315d98789a2c8b27051bfcf6830e3c873) 2026-01-12 paper feedback and new intro/conclusion
- [224b9bdae0](https://github.com/OpenAgentsInc/openagents/commit/224b9bdae014edce6b41bb1a2b878a2ac84787f9) 2026-01-12 initial paper
- [56b57b8b23](https://github.com/OpenAgentsInc/openagents/commit/56b57b8b2365214258e6fa94fec403c3cf667236) 2026-01-12 agent foundations and clear githooks
- [687510c96d](https://github.com/OpenAgentsInc/openagents/commit/687510c96de3a326e6c23aaedc2ce6658dd2669e) 2026-01-12 Adjust Autopilot UI chrome and docs
- [5ad60ffc72](https://github.com/OpenAgentsInc/openagents/commit/5ad60ffc729bc80c1636c76e494205e69e856b16) 2026-01-12 Add comprehensive Autopilot MVP documentation
- [41b10e955a](https://github.com/OpenAgentsInc/openagents/commit/41b10e955ac22ccd4a4855f840a0003754d27b80) 2026-01-12 Refocus README on Autopilot and expand docs
- [ddf6d1c9d9](https://github.com/OpenAgentsInc/openagents/commit/ddf6d1c9d99195c415d71954757d752aea220c17) 2026-01-12 Wire DSPy situation pipeline and refresh DSPy docs
- [3c1256adf5](https://github.com/OpenAgentsInc/openagents/commit/3c1256adf534cf7ee96ba5790dda73374308cbf9) 2026-01-12 Refresh crates docs after workspace cleanup
- [8606a85188](https://github.com/OpenAgentsInc/openagents/commit/8606a851882c3bd7f4b0d29c7d8aa1a82468929d) 2026-01-12 Prune non-MVP assets and add Codex events
- [2c8e5a9815](https://github.com/OpenAgentsInc/openagents/commit/2c8e5a98159a49f5f09b56693eee74b00b7caab2) 2026-01-12 Prune autopilot MVP workspace
- [4341566f53](https://github.com/OpenAgentsInc/openagents/commit/4341566f53984225877d2bfd18fb200c23fdee63) 2026-01-11 docs: add Codex gap analysis
- [6eb1cf5d1b](https://github.com/OpenAgentsInc/openagents/commit/6eb1cf5d1b6a92d0b3083cad9998bae5f790a997) 2026-01-11 docs(autopilot): snapshot MVP state
- [7df96b6e7a](https://github.com/OpenAgentsInc/openagents/commit/7df96b6e7a918fb546b77e8d6175c76ed884626a) 2026-01-11 Remove Claude/Anthropic references
- [629e1bff59](https://github.com/OpenAgentsInc/openagents/commit/629e1bff5943535adf1e815fdd9a3dcafd57e581) 2026-01-11 Delete claude references
- [26f0162d8f](https://github.com/OpenAgentsInc/openagents/commit/26f0162d8f62ba4fe9884055ada53096286aa514) 2026-01-10 feat(coder): add rlm trace pane
- [5fc192895d](https://github.com/OpenAgentsInc/openagents/commit/5fc192895d0e35bd790c1ba6ce3d8299d8a5da71) 2026-01-10 feat(coder): add rlm runs pane
- [b6ebeb8468](https://github.com/OpenAgentsInc/openagents/commit/b6ebeb8468315a56f15f54987ff6bafe295c144b) 2026-01-10 docs(dspy): update roadmap with Wave 15-16 progress
- [7c9cecfa8d](https://github.com/OpenAgentsInc/openagents/commit/7c9cecfa8da8ada89130880d7b0d4fc1489dec78) 2026-01-10 feat(dspy): implement Waves 15-16 for full DSPy integration
- [0d92ccd54e](https://github.com/OpenAgentsInc/openagents/commit/0d92ccd54e309e7ac8c4a227897b13e9a38f6982) 2026-01-10 docs: update AGENTS.md and README.md with current status
- [104261fc15](https://github.com/OpenAgentsInc/openagents/commit/104261fc15d1d84897f5c81744bd641071e991ad) 2026-01-10 docs(dspy): add comprehensive strategy document
- [e3c0c31cad](https://github.com/OpenAgentsInc/openagents/commit/e3c0c31caddff0bbc912348e7d0f047b55dbc6f0) 2026-01-10 docs(synthesis): update with fleshed-out concepts
- [57aa591599](https://github.com/OpenAgentsInc/openagents/commit/57aa5915995f0ef114c24911c256e3d5ec5bc88c) 2026-01-10 docs: add longform explanations to all SYNTHESIS_EXECUTION.md sections
- [80fdd5eb30](https://github.com/OpenAgentsInc/openagents/commit/80fdd5eb30abe69f314ac0e3633ab4e18516d141) 2026-01-10 docs: comprehensive rewrite of SYNTHESIS_EXECUTION.md
- [45c11d5819](https://github.com/OpenAgentsInc/openagents/commit/45c11d58192809937d4c996efcf1babd6f54cfa6) 2026-01-10 docs(crates): add missing crate descriptions and expand adjutant
- [0a3e6eaced](https://github.com/OpenAgentsInc/openagents/commit/0a3e6eaced04b243b702ff3530a905cfac3cfa65) 2026-01-10 feat(adjutant): add self-improving autopilot system (Wave 14)
- [61fb3a9981](https://github.com/OpenAgentsInc/openagents/commit/61fb3a9981d8ef633157692e3ea378c4a4c93df7) 2026-01-09 docs: comprehensive DSPy integration documentation update
- [c7a604465d](https://github.com/OpenAgentsInc/openagents/commit/c7a604465d38246eba0e9c73bc33fb176a0e12a8) 2026-01-09 feat(adjutant): wire DSPy decision pipelines for intelligent routing
- [cf36426ff2](https://github.com/OpenAgentsInc/openagents/commit/cf36426ff2cf79c40c8fa4323f7adab9f95a011f) 2026-01-09 feat(dsrs): add retrieval policy DSPy pipelines
- [28eb02c2bb](https://github.com/OpenAgentsInc/openagents/commit/28eb02c2bb04ee1d94072d6939c305f9e0cc7330) 2026-01-09 docs(frlm): add DSPy signatures to architecture docs
- [931631fe23](https://github.com/OpenAgentsInc/openagents/commit/931631fe232c76682ae0f947c4bba515f7da8c77) 2026-01-09 feat(frlm): add Wave 12 FRLM DSPy signatures
- [d4dc0c3b8f](https://github.com/OpenAgentsInc/openagents/commit/d4dc0c3b8fa46cc56055ec1b0d1ea6649c8c14e2) 2026-01-09 docs: update DSPy roadmap with Wave 7-9 completion status
- [5550e1c107](https://github.com/OpenAgentsInc/openagents/commit/5550e1c10729eaa6320adb731f3c8b064a3f26a9) 2026-01-09 docs: add comprehensive dsrs documentation and update Wave 3 status
- [ca3abdae22](https://github.com/OpenAgentsInc/openagents/commit/ca3abdae22397122e39c3cbf32368a0927554edf) 2026-01-08 docs: update dspy/ docs to reflect current architecture
- [bbc5226c0b](https://github.com/OpenAgentsInc/openagents/commit/bbc5226c0ba25245e5e3b12ffdc184f5eff09dc8) 2026-01-08 docs: update SYNTHESIS_EXECUTION.md with roadmap changes
- [c3aab56fdc](https://github.com/OpenAgentsInc/openagents/commit/c3aab56fdceeb0bdd44831ec619518231fce17ce) 2026-01-08 docs: deeply update DSPY_ROADMAP.md with agent architecture
- [7deccfca14](https://github.com/OpenAgentsInc/openagents/commit/7deccfca147982972c30e080faa8443ffb6ce1a2) 2026-01-08 docs: add DSPy compiler layer planning notes
- [4b8533a13e](https://github.com/OpenAgentsInc/openagents/commit/4b8533a13e3859dc44f44ee70d1cedd9950d4796) 2026-01-08 docs: add DSPy integration section and roadmap
- [c80a848cb8](https://github.com/OpenAgentsInc/openagents/commit/c80a848cb871fe1814ee9482a81df1b1745b2a3c) 2026-01-08 dspy autopilot plan
- [82ae99e663](https://github.com/OpenAgentsInc/openagents/commit/82ae99e66351906be31b6f4f8b15984e9fa975b2) 2026-01-08 move docs
- [c48ba25959](https://github.com/OpenAgentsInc/openagents/commit/c48ba25959dcc51a274ef943ef7b2f4d6fcf3584) 2026-01-08 feat(frlm): add Claude backend for RLM dashboard integration
- [bf73880d9b](https://github.com/OpenAgentsInc/openagents/commit/bf73880d9bba40f379eb187ba86bf69e8ccc8c9f) 2026-01-08 feat(rlm): Add structured outputs and backend selection to Claude integration
- [0ac34688f2](https://github.com/OpenAgentsInc/openagents/commit/0ac34688f251fd92f1199df80ab89c0c6905e969) 2026-01-08 feat(rlm): Add Claude + RLM integration with MCP server
- [9874eed2a2](https://github.com/OpenAgentsInc/openagents/commit/9874eed2a2d848531581968f2812aa61b6f6382c) 2026-01-08 claude rlm plan
- [e30475cdb4](https://github.com/OpenAgentsInc/openagents/commit/e30475cdb4c93f5b336b3a58f567d5681dbdae59) 2026-01-08 Add Gateway crate README with architecture spec
- [535838987e](https://github.com/OpenAgentsInc/openagents/commit/535838987e0ce3ddd8f5cb2be5983b7374c208a7) 2026-01-08 Fix web demo input and worker template
- [ebc849a540](https://github.com/OpenAgentsInc/openagents/commit/ebc849a5404ec22ced915162cca170574e00e86a) 2026-01-08 Add Adjutant crate and autopilot CLI
- [73418c6977](https://github.com/OpenAgentsInc/openagents/commit/73418c6977853d1b8c6860492921669c0d789499) 2026-01-08 replace cc cli plan
- [667e0d408b](https://github.com/OpenAgentsInc/openagents/commit/667e0d408b5e833f1009db5d50253f26209a46c1) 2026-01-08 RLM dashboard list/detail UI and runtime
- [edaf26f9da](https://github.com/OpenAgentsInc/openagents/commit/edaf26f9da05dfa3316fc52245b2257937c22de8) 2026-01-08 Add RLM sync APIs and Pylon sync
- [d9913db25a](https://github.com/OpenAgentsInc/openagents/commit/d9913db25a5f19a868152c96257820460edba96b) 2026-01-08 Implement Phase 1 RLM local storage
- [15fcc6b6dc](https://github.com/OpenAgentsInc/openagents/commit/15fcc6b6dcf5c56de8e2714f2c9e5d036052959a) 2026-01-08 Add RLM Dashboard implementation plan (W&B-style)
- [1de6ffdf91](https://github.com/OpenAgentsInc/openagents/commit/1de6ffdf9121a0f94cb411bedace7ef03acc572b) 2026-01-08 docs: document missing crates
- [bb14252529](https://github.com/OpenAgentsInc/openagents/commit/bb14252529a0763dfd62f2d427bd66c31cb4eabf) 2026-01-08 Add RLM Dashboard spec for W&B-style experiment tracking
- [3f733583db](https://github.com/OpenAgentsInc/openagents/commit/3f733583db41a7684f5dd64f7c03a46105f83a13) 2026-01-08 Add SYNTHESIS_EXECUTION.md and update agent docs to reference it
- [662fa224fc](https://github.com/OpenAgentsInc/openagents/commit/662fa224fc3cc539a68b63bfebdf9c3b1422b689) 2026-01-07 Add RLM local test log and show both wallet addresses
- [08da271d58](https://github.com/OpenAgentsInc/openagents/commit/08da271d58cb14d953f332f5f1b3041eca81f804) 2026-01-07 ep 203 transcript
- [c267d43162](https://github.com/OpenAgentsInc/openagents/commit/c267d4316209b579a6764a0dc2c9ca74f0dfbc1d) 2026-01-07 Reorganize AGENTS.md for users, move dev instructions to docs/
- [acbd4b3441](https://github.com/OpenAgentsInc/openagents/commit/acbd4b3441a043f65c02aa720d7b1c5f137a2108) 2026-01-07 Fix RLM end-to-end and add comprehensive docs
- [3c0d32a11d](https://github.com/OpenAgentsInc/openagents/commit/3c0d32a11d0864dc70091ae2382f6ef3ab8bccbb) 2026-01-07 Update RLM E2E test log with results
- [610390372d](https://github.com/OpenAgentsInc/openagents/commit/610390372d2ff5e57067b78afaaeea065f1a8dfc) 2026-01-07 Add kind:5940 RLM to relay subscription filter
- [ab3ab1cf51](https://github.com/OpenAgentsInc/openagents/commit/ab3ab1cf51c344f7ccee996127fac348574fcd00) 2026-01-07 Add pylon rlm command for recursive language model queries
- [4a76d5f4e1](https://github.com/OpenAgentsInc/openagents/commit/4a76d5f4e10f76cb6ed7c5ebd133eb9f23c8a768) 2026-01-07 docs: add Pylon v0.1 development plans and NIP-90 job example
- [61de3191bd](https://github.com/OpenAgentsInc/openagents/commit/61de3191bd7590d2bcb9b17e11e3c0bf8b36fa19) 2026-01-06 tomorrow
- [abf1719478](https://github.com/OpenAgentsInc/openagents/commit/abf1719478d1aec129517af2b514f3f6195db5ba) 2026-01-06 202 transcript
- [49fd60ca49](https://github.com/OpenAgentsInc/openagents/commit/49fd60ca4911feb21a40c84b55f9b16f492340ff) 2026-01-06 feat(rlm): implement RLM Visualizer v2 UI redesign
- [ee9c53f932](https://github.com/OpenAgentsInc/openagents/commit/ee9c53f932769a8a5503f7c71059b5b61ed6e0ea) 2026-01-06 docs(rlm): comprehensive documentation for DSPy integration
- [e4bd7f7cb0](https://github.com/OpenAgentsInc/openagents/commit/e4bd7f7cb0cb27037ec38b3e4299b3f7402cea4a) 2026-01-06 feat(rlm): replace fake demo with real DSPy trace playback
- [8cda787387](https://github.com/OpenAgentsInc/openagents/commit/8cda7873877c0b823c900665f8990ce40577364e) 2026-01-06 docs(rlm): add comprehensive DSPy integration documentation
- [a9bcddad0f](https://github.com/OpenAgentsInc/openagents/commit/a9bcddad0fe76a913525b954207ac57a3c48ec9b) 2026-01-06 docs: rewrite RLM visualization docs with current state and next steps
- [bcc15ec612](https://github.com/OpenAgentsInc/openagents/commit/bcc15ec6128a583cfca588b04f16ffa67aab51a5) 2026-01-06 dspy
- [366a35265c](https://github.com/OpenAgentsInc/openagents/commit/366a35265c164bd9a1b6bb9db788c2a803dcdd67) 2026-01-06 fix(rlm): wire up text input event handling
- [bd7cf2bb4e](https://github.com/OpenAgentsInc/openagents/commit/bd7cf2bb4efc05563922b534179387d64200c224) 2026-01-06 docs: add RLM visualization page documentation
- [1976ae8644](https://github.com/OpenAgentsInc/openagents/commit/1976ae8644ac50b594eb8fa14b263c8041f31740) 2026-01-05 Add RLM paper replication infrastructure
- [3bba940b61](https://github.com/OpenAgentsInc/openagents/commit/3bba940b61d3b6b3d6f26a6a2d57af07fcdaf4b3) 2026-01-05 Add comprehensive improvement thoughts to RLM implementation log
- [293c70771f](https://github.com/OpenAgentsInc/openagents/commit/293c70771f2c69d1240386f19fdaf9890253d53f) 2026-01-05 Update FRLM paper with engine-orchestrated analysis
- [eafab354c0](https://github.com/OpenAgentsInc/openagents/commit/eafab354c0718e5ed209e964a00dbcb70a84480e) 2026-01-05 Document comprehensive orchestrated mode test results
- [c303b80364](https://github.com/OpenAgentsInc/openagents/commit/c303b8036467913af9774f756bc72925faee6b60) 2026-01-04 Add tiered prompt system and stuck detection for Apple FM
- [bdddc9c062](https://github.com/OpenAgentsInc/openagents/commit/bdddc9c0622a26d0fcac94462e704914939ed1c4) 2026-01-04 Document context loading test results with Apple FM
- [b352d94501](https://github.com/OpenAgentsInc/openagents/commit/b352d945014dfa451c988e435a12edf188d24f48) 2026-01-04 Update RLM log with context loading and sub-query implementation
- [76a78cedd1](https://github.com/OpenAgentsInc/openagents/commit/76a78cedd14ff7da3494ebb76fe8fa2afce4a63b) 2026-01-04 Add detailed analysis to RLM implementation log
- [36d0a8098f](https://github.com/OpenAgentsInc/openagents/commit/36d0a8098f60ad2b735e8e29377c17b3168d0349) 2026-01-04 Add RLM CLI implementation log with test results
- [cf519d453f](https://github.com/OpenAgentsInc/openagents/commit/cf519d453f614c48707f44b1dce387a201e3070f) 2026-01-04 Integrate FRLM/RLM/Apple FM visualization in Pylon desktop
- [090bc1c15e](https://github.com/OpenAgentsInc/openagents/commit/090bc1c15e6a749948d1e0396327559c1ac8ac5c) 2026-01-04 Implement RLM as Apple FM tools via FRLM conductor
- [0e5cba6ad2](https://github.com/OpenAgentsInc/openagents/commit/0e5cba6ad2d984a488c25a60f5d773fab05ed54e) 2026-01-04 194
- [84273d5a42](https://github.com/OpenAgentsInc/openagents/commit/84273d5a42eae80748bcf6b4d3cc388ea50e8843) 2026-01-04 Implement FRLM (Federated Recursive Language Models) buyside in Pylon
- [ae6f35d81a](https://github.com/OpenAgentsInc/openagents/commit/ae6f35d81aa14ee86b0d8caadc454292e1b0db47) 2026-01-04 FRLM
- [9943b1e958](https://github.com/OpenAgentsInc/openagents/commit/9943b1e9587348ce6825c20e66821523452fb468) 2026-01-04 Convo and paper initial

### GEPA

Total commits: 42

- [f962acff8c](https://github.com/OpenAgentsInc/openagents/commit/f962acff8c6895d66a988a9b47068b14e6acd706) 2026-02-06 docs: add RLM synergies and Effuse master plan
- [a3cddefef2](https://github.com/OpenAgentsInc/openagents/commit/a3cddefef2bc902f9f3e61336b06c262f23d5c8c) 2026-02-06 feat(dse): scaffold Effect-native signatures and predict
- [30d8ff697e](https://github.com/OpenAgentsInc/openagents/commit/30d8ff697eb0822afb18937e643420aa902a09a7) 2026-02-06 docs(autopilot): define prompt IR transforms and hashing
- [1447cfb0ed](https://github.com/OpenAgentsInc/openagents/commit/1447cfb0edc493af9944843109e484d6788fdbe9) 2026-02-06 docs(autopilot): expand ds-effect spec
- [1e2a04238c](https://github.com/OpenAgentsInc/openagents/commit/1e2a04238c473b40142b9e24e61a8a1482f6a61e) 2026-02-06 ds effect spec innitial
- [05bcc9e5f4](https://github.com/OpenAgentsInc/openagents/commit/05bcc9e5f48036ef950e246256f311e9ddeaa564) 2026-02-01 docs(transcripts): add TOPICS.md index for openagents episode transcripts
- [5e011e9457](https://github.com/OpenAgentsInc/openagents/commit/5e011e9457e0a1e8571695929fe5de83e64b1773) 2026-01-27 Remove legacy Tauri desktop app
- [5966837007](https://github.com/OpenAgentsInc/openagents/commit/596683700708b375f4fea32b57e2030016096e29) 2026-01-26 Add big picture market framing for guidance module
- [70a35b5e2c](https://github.com/OpenAgentsInc/openagents/commit/70a35b5e2c0daff0c99cc2e1bab35951405e1baf) 2026-01-26 docs: add autopilot desktop self-improver plan
- [2166ab0c18](https://github.com/OpenAgentsInc/openagents/commit/2166ab0c1801dbcac2354360e95ff64a6fb01faf) 2026-01-26 Update plan mode documentation
- [0de3a34742](https://github.com/OpenAgentsInc/openagents/commit/0de3a347429b69bfa29a0dabf3a2fb5e42855d63) 2026-01-26 Add release mode to autopilot runner
- [980939b8cb](https://github.com/OpenAgentsInc/openagents/commit/980939b8cb28be6a98d1cb4d0b5370c3acc454ed) 2026-01-25 Add DSPy and Full Auto integration docs
- [e05c58c635](https://github.com/OpenAgentsInc/openagents/commit/e05c58c63562f2675d3951aff5a51a7f4ec2e807) 2026-01-25 initial dspy report
- [58fdef3544](https://github.com/OpenAgentsInc/openagents/commit/58fdef3544a975eae70b84d37b4e014e9d54a33c) 2026-01-13 agent foundations doc
- [6f9718b0b2](https://github.com/OpenAgentsInc/openagents/commit/6f9718b0b23c107bb4e6229a52585232e5d7c08b) 2026-01-13 Synchronize docs with canonical vocabulary and protocol surface
- [766aacf409](https://github.com/OpenAgentsInc/openagents/commit/766aacf4093e6a935bfc407910013582f8244a77) 2026-01-13 Final doc coherence fixes + GLOSSARY.md
- [dff775ddd8](https://github.com/OpenAgentsInc/openagents/commit/dff775ddd86263a7393981185ccd7f7f7a08109f) 2026-01-13 Add comprehensive DSPy primitives documentation
- [664ed0f578](https://github.com/OpenAgentsInc/openagents/commit/664ed0f57800e0cfe66ca663f2597dc0afd38819) 2026-01-12 Add DSPy chain visualizer UI to manatap
- [41b10e955a](https://github.com/OpenAgentsInc/openagents/commit/41b10e955ac22ccd4a4855f840a0003754d27b80) 2026-01-12 Refocus README on Autopilot and expand docs
- [3c1256adf5](https://github.com/OpenAgentsInc/openagents/commit/3c1256adf534cf7ee96ba5790dda73374308cbf9) 2026-01-12 Refresh crates docs after workspace cleanup
- [8606a85188](https://github.com/OpenAgentsInc/openagents/commit/8606a851882c3bd7f4b0d29c7d8aa1a82468929d) 2026-01-12 Prune non-MVP assets and add Codex events
- [7df96b6e7a](https://github.com/OpenAgentsInc/openagents/commit/7df96b6e7a918fb546b77e8d6175c76ed884626a) 2026-01-11 Remove Claude/Anthropic references
- [7c9cecfa8d](https://github.com/OpenAgentsInc/openagents/commit/7c9cecfa8da8ada89130880d7b0d4fc1489dec78) 2026-01-10 feat(dspy): implement Waves 15-16 for full DSPy integration
- [0d92ccd54e](https://github.com/OpenAgentsInc/openagents/commit/0d92ccd54e309e7ac8c4a227897b13e9a38f6982) 2026-01-10 docs: update AGENTS.md and README.md with current status
- [104261fc15](https://github.com/OpenAgentsInc/openagents/commit/104261fc15d1d84897f5c81744bd641071e991ad) 2026-01-10 docs(dspy): add comprehensive strategy document
- [80fdd5eb30](https://github.com/OpenAgentsInc/openagents/commit/80fdd5eb30abe69f314ac0e3633ab4e18516d141) 2026-01-10 docs: comprehensive rewrite of SYNTHESIS_EXECUTION.md
- [45c11d5819](https://github.com/OpenAgentsInc/openagents/commit/45c11d58192809937d4c996efcf1babd6f54cfa6) 2026-01-10 docs(crates): add missing crate descriptions and expand adjutant
- [61fb3a9981](https://github.com/OpenAgentsInc/openagents/commit/61fb3a9981d8ef633157692e3ea378c4a4c93df7) 2026-01-09 docs: comprehensive DSPy integration documentation update
- [28eb02c2bb](https://github.com/OpenAgentsInc/openagents/commit/28eb02c2bb04ee1d94072d6939c305f9e0cc7330) 2026-01-09 docs(frlm): add DSPy signatures to architecture docs
- [2d25cb28cf](https://github.com/OpenAgentsInc/openagents/commit/2d25cb28cf51d99b72b430e241d9d8bd6b1ad429) 2026-01-09 feat(dsrs): implement Wave 4 Retrieval & Swarm Integration
- [4fc5484d00](https://github.com/OpenAgentsInc/openagents/commit/4fc5484d00af6b6b916cb11595aba1c4be51f20c) 2026-01-09 docs: add Wave 3 compiler contract planning log
- [5550e1c107](https://github.com/OpenAgentsInc/openagents/commit/5550e1c10729eaa6320adb731f3c8b064a3f26a9) 2026-01-09 docs: add comprehensive dsrs documentation and update Wave 3 status
- [ca3abdae22](https://github.com/OpenAgentsInc/openagents/commit/ca3abdae22397122e39c3cbf32368a0927554edf) 2026-01-08 docs: update dspy/ docs to reflect current architecture
- [42b102b87d](https://github.com/OpenAgentsInc/openagents/commit/42b102b87d5feccb0cc5482852e3229a5b1a7fd4) 2026-01-08 docs: add DSPy compiler layer section to SYNTHESIS.md
- [c3aab56fdc](https://github.com/OpenAgentsInc/openagents/commit/c3aab56fdceeb0bdd44831ec619518231fce17ce) 2026-01-08 docs: deeply update DSPY_ROADMAP.md with agent architecture
- [1fe13c565a](https://github.com/OpenAgentsInc/openagents/commit/1fe13c565afc4ab53927bd6109b6e5119272e5a3) 2026-01-08 docs: update DSPy roadmap and synthesis for dsrs integration
- [7deccfca14](https://github.com/OpenAgentsInc/openagents/commit/7deccfca147982972c30e080faa8443ffb6ce1a2) 2026-01-08 docs: add DSPy compiler layer planning notes
- [4b8533a13e](https://github.com/OpenAgentsInc/openagents/commit/4b8533a13e3859dc44f44ee70d1cedd9950d4796) 2026-01-08 docs: add DSPy integration section and roadmap
- [c80a848cb8](https://github.com/OpenAgentsInc/openagents/commit/c80a848cb871fe1814ee9482a81df1b1745b2a3c) 2026-01-08 dspy autopilot plan
- [82ae99e663](https://github.com/OpenAgentsInc/openagents/commit/82ae99e66351906be31b6f4f8b15984e9fa975b2) 2026-01-08 move docs
- [bdde6f8798](https://github.com/OpenAgentsInc/openagents/commit/bdde6f879827bf9882576bd285abe8199102cbba) 2026-01-08 feat(autopilot): add bidirectional UI and DSPy verification pipeline
- [bcc15ec612](https://github.com/OpenAgentsInc/openagents/commit/bcc15ec6128a583cfca588b04f16ffa67aab51a5) 2026-01-06 dspy

### MIPRO

Total commits: 62

- [a3cddefef2](https://github.com/OpenAgentsInc/openagents/commit/a3cddefef2bc902f9f3e61336b06c262f23d5c8c) 2026-02-06 feat(dse): scaffold Effect-native signatures and predict
- [30d8ff697e](https://github.com/OpenAgentsInc/openagents/commit/30d8ff697eb0822afb18937e643420aa902a09a7) 2026-02-06 docs(autopilot): define prompt IR transforms and hashing
- [1447cfb0ed](https://github.com/OpenAgentsInc/openagents/commit/1447cfb0edc493af9944843109e484d6788fdbe9) 2026-02-06 docs(autopilot): expand ds-effect spec
- [1e2a04238c](https://github.com/OpenAgentsInc/openagents/commit/1e2a04238c473b40142b9e24e61a8a1482f6a61e) 2026-02-06 ds effect spec innitial
- [05bcc9e5f4](https://github.com/OpenAgentsInc/openagents/commit/05bcc9e5f48036ef950e246256f311e9ddeaa564) 2026-02-01 docs(transcripts): add TOPICS.md index for openagents episode transcripts
- [5e011e9457](https://github.com/OpenAgentsInc/openagents/commit/5e011e9457e0a1e8571695929fe5de83e64b1773) 2026-01-27 Remove legacy Tauri desktop app
- [5966837007](https://github.com/OpenAgentsInc/openagents/commit/596683700708b375f4fea32b57e2030016096e29) 2026-01-26 Add big picture market framing for guidance module
- [70a35b5e2c](https://github.com/OpenAgentsInc/openagents/commit/70a35b5e2c0daff0c99cc2e1bab35951405e1baf) 2026-01-26 docs: add autopilot desktop self-improver plan
- [2166ab0c18](https://github.com/OpenAgentsInc/openagents/commit/2166ab0c1801dbcac2354360e95ff64a6fb01faf) 2026-01-26 Update plan mode documentation
- [980939b8cb](https://github.com/OpenAgentsInc/openagents/commit/980939b8cb28be6a98d1cb4d0b5370c3acc454ed) 2026-01-25 Add DSPy and Full Auto integration docs
- [e05c58c635](https://github.com/OpenAgentsInc/openagents/commit/e05c58c63562f2675d3951aff5a51a7f4ec2e807) 2026-01-25 initial dspy report
- [311fe17f8d](https://github.com/OpenAgentsInc/openagents/commit/311fe17f8d95ceafc0c3358f62da2c952376d597) 2026-01-25 Move autopilot desktop into monorepo
- [6f9718b0b2](https://github.com/OpenAgentsInc/openagents/commit/6f9718b0b23c107bb4e6229a52585232e5d7c08b) 2026-01-13 Synchronize docs with canonical vocabulary and protocol surface
- [766aacf409](https://github.com/OpenAgentsInc/openagents/commit/766aacf4093e6a935bfc407910013582f8244a77) 2026-01-13 Final doc coherence fixes + GLOSSARY.md
- [634f5b6277](https://github.com/OpenAgentsInc/openagents/commit/634f5b62774f588c9d0ea95a75771839a0af0ff5) 2026-01-13 Doc hygiene: add headers, split REPLAY/ARTIFACTS, reduce duplication
- [556e068b84](https://github.com/OpenAgentsInc/openagents/commit/556e068b84a8e1421956e8002f202b6a686c2d31) 2026-01-13 Add MVP priorities and outcome-coupled metrics to DSPy docs
- [dff775ddd8](https://github.com/OpenAgentsInc/openagents/commit/dff775ddd86263a7393981185ccd7f7f7a08109f) 2026-01-13 Add comprehensive DSPy primitives documentation
- [04128eb482](https://github.com/OpenAgentsInc/openagents/commit/04128eb4822f47f5ef074394d094765aabcc3602) 2026-01-12 Expand paper and roadmap with OS details
- [0609a309da](https://github.com/OpenAgentsInc/openagents/commit/0609a309da4ca84ea3863ef88a6daa69d4794766) 2026-01-12 roadmap
- [93d4043315](https://github.com/OpenAgentsInc/openagents/commit/93d4043315d98789a2c8b27051bfcf6830e3c873) 2026-01-12 paper feedback and new intro/conclusion
- [224b9bdae0](https://github.com/OpenAgentsInc/openagents/commit/224b9bdae014edce6b41bb1a2b878a2ac84787f9) 2026-01-12 initial paper
- [56b57b8b23](https://github.com/OpenAgentsInc/openagents/commit/56b57b8b2365214258e6fa94fec403c3cf667236) 2026-01-12 agent foundations and clear githooks
- [664ed0f578](https://github.com/OpenAgentsInc/openagents/commit/664ed0f57800e0cfe66ca663f2597dc0afd38819) 2026-01-12 Add DSPy chain visualizer UI to manatap
- [41b10e955a](https://github.com/OpenAgentsInc/openagents/commit/41b10e955ac22ccd4a4855f840a0003754d27b80) 2026-01-12 Refocus README on Autopilot and expand docs
- [3c1256adf5](https://github.com/OpenAgentsInc/openagents/commit/3c1256adf534cf7ee96ba5790dda73374308cbf9) 2026-01-12 Refresh crates docs after workspace cleanup
- [8606a85188](https://github.com/OpenAgentsInc/openagents/commit/8606a851882c3bd7f4b0d29c7d8aa1a82468929d) 2026-01-12 Prune non-MVP assets and add Codex events
- [bc0e35b316](https://github.com/OpenAgentsInc/openagents/commit/bc0e35b316e44f3540ccbb0b6c5a45c5f3df373e) 2026-01-11 Remove codex-agent-sdk and route Codex via app-server
- [7df96b6e7a](https://github.com/OpenAgentsInc/openagents/commit/7df96b6e7a918fb546b77e8d6175c76ed884626a) 2026-01-11 Remove Claude/Anthropic references
- [629e1bff59](https://github.com/OpenAgentsInc/openagents/commit/629e1bff5943535adf1e815fdd9a3dcafd57e581) 2026-01-11 Delete claude references
- [a89e249f5e](https://github.com/OpenAgentsInc/openagents/commit/a89e249f5e40910008319fdcb2a1b4eaa01ce991) 2026-01-10 docs: add DSPy integration, execution flow, and signature docs
- [dd9b37633e](https://github.com/OpenAgentsInc/openagents/commit/dd9b37633e1117a66f72a0be38edaab3d07250f7) 2026-01-10 docs(dspy): rename and expand agent SDK integration guide
- [12d89c8a47](https://github.com/OpenAgentsInc/openagents/commit/12d89c8a47cb207bb8c30bf8170b1258c8992694) 2026-01-10 docs(dspy): add Claude Agent SDK integration strategy
- [c087f16d25](https://github.com/OpenAgentsInc/openagents/commit/c087f16d2593702db3a85be3f144c654040bb421) 2026-01-10 autopilot example
- [b6ebeb8468](https://github.com/OpenAgentsInc/openagents/commit/b6ebeb8468315a56f15f54987ff6bafe295c144b) 2026-01-10 docs(dspy): update roadmap with Wave 15-16 progress
- [7c9cecfa8d](https://github.com/OpenAgentsInc/openagents/commit/7c9cecfa8da8ada89130880d7b0d4fc1489dec78) 2026-01-10 feat(dspy): implement Waves 15-16 for full DSPy integration
- [0d92ccd54e](https://github.com/OpenAgentsInc/openagents/commit/0d92ccd54e309e7ac8c4a227897b13e9a38f6982) 2026-01-10 docs: update AGENTS.md and README.md with current status
- [104261fc15](https://github.com/OpenAgentsInc/openagents/commit/104261fc15d1d84897f5c81744bd641071e991ad) 2026-01-10 docs(dspy): add comprehensive strategy document
- [e3c0c31cad](https://github.com/OpenAgentsInc/openagents/commit/e3c0c31caddff0bbc912348e7d0f047b55dbc6f0) 2026-01-10 docs(synthesis): update with fleshed-out concepts
- [80fdd5eb30](https://github.com/OpenAgentsInc/openagents/commit/80fdd5eb30abe69f314ac0e3633ab4e18516d141) 2026-01-10 docs: comprehensive rewrite of SYNTHESIS_EXECUTION.md
- [45c11d5819](https://github.com/OpenAgentsInc/openagents/commit/45c11d58192809937d4c996efcf1babd6f54cfa6) 2026-01-10 docs(crates): add missing crate descriptions and expand adjutant
- [0a3e6eaced](https://github.com/OpenAgentsInc/openagents/commit/0a3e6eaced04b243b702ff3530a905cfac3cfa65) 2026-01-10 feat(adjutant): add self-improving autopilot system (Wave 14)
- [8dc777dc1d](https://github.com/OpenAgentsInc/openagents/commit/8dc777dc1db731a70dc89c5c0a1d33ff545f394a) 2026-01-09 docs: add plain-language DSPy training/optimization workflow guide
- [61fb3a9981](https://github.com/OpenAgentsInc/openagents/commit/61fb3a9981d8ef633157692e3ea378c4a4c93df7) 2026-01-09 docs: comprehensive DSPy integration documentation update
- [28eb02c2bb](https://github.com/OpenAgentsInc/openagents/commit/28eb02c2bb04ee1d94072d6939c305f9e0cc7330) 2026-01-09 docs(frlm): add DSPy signatures to architecture docs
- [f4e9b936a2](https://github.com/OpenAgentsInc/openagents/commit/f4e9b936a2f8d0863f68cde5f2c7bc1537e7223e) 2026-01-09 feat(dsrs): implement Wave 6 SwarmCompiler
- [b90c789487](https://github.com/OpenAgentsInc/openagents/commit/b90c789487675852d991922db3d07118c8bdfe99) 2026-01-09 feat(dsrs): implement Wave 5 Eval Harness & Promotion Gates
- [2d25cb28cf](https://github.com/OpenAgentsInc/openagents/commit/2d25cb28cf51d99b72b430e241d9d8bd6b1ad429) 2026-01-09 feat(dsrs): implement Wave 4 Retrieval & Swarm Integration
- [4fc5484d00](https://github.com/OpenAgentsInc/openagents/commit/4fc5484d00af6b6b916cb11595aba1c4be51f20c) 2026-01-09 docs: add Wave 3 compiler contract planning log
- [5550e1c107](https://github.com/OpenAgentsInc/openagents/commit/5550e1c10729eaa6320adb731f3c8b064a3f26a9) 2026-01-09 docs: add comprehensive dsrs documentation and update Wave 3 status
- [ca3abdae22](https://github.com/OpenAgentsInc/openagents/commit/ca3abdae22397122e39c3cbf32368a0927554edf) 2026-01-08 docs: update dspy/ docs to reflect current architecture
- [42b102b87d](https://github.com/OpenAgentsInc/openagents/commit/42b102b87d5feccb0cc5482852e3229a5b1a7fd4) 2026-01-08 docs: add DSPy compiler layer section to SYNTHESIS.md
- [c3aab56fdc](https://github.com/OpenAgentsInc/openagents/commit/c3aab56fdceeb0bdd44831ec619518231fce17ce) 2026-01-08 docs: deeply update DSPY_ROADMAP.md with agent architecture
- [b4e14fd865](https://github.com/OpenAgentsInc/openagents/commit/b4e14fd8657510f19288d536ba5c804dfd33b76b) 2026-01-08 feat(adjutant): integrate dsrs for optimizable DSPy signatures
- [1fe13c565a](https://github.com/OpenAgentsInc/openagents/commit/1fe13c565afc4ab53927bd6109b6e5119272e5a3) 2026-01-08 docs: update DSPy roadmap and synthesis for dsrs integration
- [7deccfca14](https://github.com/OpenAgentsInc/openagents/commit/7deccfca147982972c30e080faa8443ffb6ce1a2) 2026-01-08 docs: add DSPy compiler layer planning notes
- [4b8533a13e](https://github.com/OpenAgentsInc/openagents/commit/4b8533a13e3859dc44f44ee70d1cedd9950d4796) 2026-01-08 docs: add DSPy integration section and roadmap
- [c80a848cb8](https://github.com/OpenAgentsInc/openagents/commit/c80a848cb871fe1814ee9482a81df1b1745b2a3c) 2026-01-08 dspy autopilot plan
- [82ae99e663](https://github.com/OpenAgentsInc/openagents/commit/82ae99e66351906be31b6f4f8b15984e9fa975b2) 2026-01-08 move docs
- [bdde6f8798](https://github.com/OpenAgentsInc/openagents/commit/bdde6f879827bf9882576bd285abe8199102cbba) 2026-01-08 feat(autopilot): add bidirectional UI and DSPy verification pipeline
- [ee9c53f932](https://github.com/OpenAgentsInc/openagents/commit/ee9c53f932769a8a5503f7c71059b5b61ed6e0ea) 2026-01-06 docs(rlm): comprehensive documentation for DSPy integration
- [8cda787387](https://github.com/OpenAgentsInc/openagents/commit/8cda7873877c0b823c900665f8990ce40577364e) 2026-01-06 docs(rlm): add comprehensive DSPy integration documentation
- [bcc15ec612](https://github.com/OpenAgentsInc/openagents/commit/bcc15ec6128a583cfca588b04f16ffa67aab51a5) 2026-01-06 dspy

## Reproduction

Current-tree search (docs only):
```bash
git grep -n -i -E 'RLM|GEPA|MIPRO' -- '*.md' '*.mdx'
```

History pickaxe (docs only):
```bash
git log --all -S'RLM'   --oneline -- '*.md' '*.mdx'
git log --all -S'GEPA'  --oneline -- '*.md' '*.mdx'
git log --all -S'MIPRO' --oneline -- '*.md' '*.mdx'
```
