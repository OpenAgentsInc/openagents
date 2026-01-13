# DSPy Roadmap (OpenAgents)

- **Status:** Needs audit
- **Last verified:** d44f9cd3f
- **Source of truth:** N/A (planning doc)
- **Doc owner:** dsrs
- **If this doc conflicts with code, code wins.**

**Reality check:** "Wave complete" means structs exist + unit tests. "MVP ready" means wired into production path.

## North Star: DSPy as the Compiler Layer

DSPy is the compiler layer for agent behavior in OpenAgents. It turns a task
description and typed inputs into an executable program whose structure
(prompts, tools, examples, routing) can be optimized without rewriting business
logic. The runtime and marketplace decide where and how execution happens; DSPy
decides what to do and how to structure it. This separation keeps agent logic
portable across models and execution targets.

The goal is declarative AI programming: signatures describe intent, not hand
tuned prompts. Optimizers search for better prompt structure and few-shot
examples, while we keep the program shape stable. This is the part that lets us
swap models, change routing, and accumulate learning signals without
re-architecting the agent.

## Coverage Map

The dsrs crate is the compiler layer itself. It hosts the core signature system,
predictors, optimizers, tracing, caching, and LM routing so that every other
component can define intent without worrying about model wiring.

OANIX provides DSPy signatures for situation assessment and lifecycle decisions.
It turns the discovered system manifest into a typed priority signal so the
agent can reason about whether to wait, work issues, or acquire resources.

Autopilot core owns the planning, execution, verification, and optimization
signatures that turn a user prompt into a plan, a sequence of actions, and a
verification verdict. This is the core decision surface for the autonomous
coding workflow.

Adjutant wires those signatures into planning orchestration for both the UI and
CLI. It owns the self-improvement loop, so it is responsible for turning
decisions and outcomes into training signals and optimization runs.

Runtime holds the tool selection and tool result interpretation signatures. It
bridges the typed DSPy decisions into concrete tool calls and helps normalize
tool results so they can become learning data.

RLM and FRLM provide DSPy signatures for recursive or long-context workflows.
These modules are used when a single model call is insufficient and the agent
must orchestrate deeper analysis or multi-step inference.

Each component owns signatures for its decision surface, while dsrs provides the
optimizer and runtime machinery that keep those signatures portable and
optimizable.

## Autopilot DSPy Flow

Autopilot now runs its planning stages through DSPy signatures end to end.
Environment assessment is produced by the OANIX SituationAssessmentSignature
via SituationPipeline, using the discovered manifest (hardware, compute,
network, identity, workspace). This gives a typed, optimizable signal for
priority and urgency rather than a fixed heuristic, with a fallback when no LM
is configured.

Planning uses PlanningSignature and DeepPlanningSignature to generate a
structured plan and actionable steps. Execution uses ExecutionStrategySignature
and ToolSelectionSignature to decide the next action and choose tools, while
VerificationPipeline (build, test, requirement checks, and
SolutionVerifierSignature) evaluates completion. The verification retry loop now
appends next_action into plan_steps, which keeps retries in DSPy space even
though the triage logic is still basic.

The self improvement loop is wired through Adjutant session tracking, labeled
examples, performance tracking, and auto optimization. This closes the loop:
DSPy signatures generate decisions, outcomes label them, and optimizers refine
them.

## Wave Status

- **Note:** Wave status tracks component readiness (structs + unit tests); MVP readiness depends on wiring. See ROADMAP.md "NOW" section for true MVP gates.

| Wave | Status | Description |
| --- | --- | --- |
| 0 | Complete | Protocol and schema registry |
| 1-2 | Complete | RLM and Autopilot signatures |
| 2.5 | Complete | Multi-provider LM routing |
| 3 | Complete | Compiler contract, callbacks, trace |
| 4 | Complete | Retrieval signatures and pipelines |
| 5 | Complete | Eval harness and promotion gates |
| 6 | Complete | SwarmCompiler |
| 7 | Complete | Privacy module |
| 8 | Complete | OANIX DSPy signatures |
| 9 | Archived | Agent-orchestrator signatures (backroom) |
| 10 | Complete | Runtime tool signatures |
| 11 | Complete | Autopilot optimization infra |
| 12 | Complete | FRLM signatures |
| 13 | Complete | Pipeline wiring (adjutant, runtime, oanix, autopilot-core) |
| 14 | Complete | Self-improving Autopilot loop |

### MVP Gates (Beyond Wave Completion)

| Gate | Status | Notes |
|------|--------|-------|
| Verified PR Bundle emission | 🔄 Partial | ReplayBundle exists; REPLAY.jsonl exporter pending |
| ToolCallSignature wired | ⏳ Spec only | Defined in SIGNATURES.md |
| ToolResultSignature wired | ⏳ Spec only | Defined in SIGNATURES.md |
| VerificationSignature wired | ⏳ Spec only | Defined in SIGNATURES.md |
| Policy pin/rollback basic | ⏳ Not started | CLI commands pending |

The wave table captures the current implementation status, but it is also a
dependency map. Earlier waves establish the data structures and runtime
contracts, while later waves focus on wiring, self-improvement, and full
automation of the decision loop.

## Archived DSPy Work

Older DSPy experiments for agent orchestration, marketplace security, and relay
logic were moved to backroom during the MVP prune. The code remains a reference
for signature design, but the live pipeline is now centered on Adjutant,
Autopilot, and OANIX.

## Near-term Gaps

Autopilot still has two per-step DSPy calls (execution strategy and tool
selection), and it lacks a typed tool result interpretation stage with a
learning signal. The long-term direction is to merge tool selection into a
single ToolCall signature and add a ToolResult signature to label step utility.
The Plan IR is also duplicated across Adjutant and Autopilot, and should be
unified for better training data.

These gaps are not just architectural debt; they directly affect the quality of
learning signals and the cost of execution. Consolidating per-step decisions and
adding structured tool result interpretation will reduce inference overhead and
produce cleaner training examples for optimization.
