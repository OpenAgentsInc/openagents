# Programming and improving agent systems

Status: architecture and implementation plan, 2026-09-21. These documents
extend the [general agent architecture](../agents/README.md) and the
[TypeSafe Coder plan](../coder/design/typesafe-agent-analysis.md). Coding is
the first specialization. No DSPy runtime, GEPA campaign, new measurement,
or automatic promotion ships with this specification.

The durable investment is a clear task definition, representative evaluations,
and code that controls information and effects. Models, prompts, examples,
inference strategies, and useful internal decompositions should be replaceable
when measurements justify it. Define semantic operations whose implementations
can improve without rewriting their purpose or authority boundaries.

Jev, Kev, and Lev are useful typed decision backends. Generation is
useful for open-ended outputs. DSPy provides a model for expressing and
optimizing composed AI software; GEPA provides an optimization approach.
Gym supplies our measurement and acceptance evidence. None replaces the
host's authority or makes an unmeasured design effective.

## Read this set

| Document | What it answers |
| --- | --- |
| [Talk, DSPy, and GEPA](concepts.md) | What the talk and post challenge, and what these tools mean. |
| [Architecture and opportunity map](architecture.md) | What stays fixed, what can vary, and how this changes every TypeSafe opportunity. |
| [Experiments and promotion](experiments.md) | How to reuse Gym, isolate evidence, run actual candidates, and adopt results. |
| [Conceptual references](sources.md) | Primary sources for the concepts used in this design. |
| [Proposed integration issues](proposed-issues.md) | One consolidated, unfiled implementation backlog with dependencies and acceptance criteria. |
| [NIP-OPT](../../nips/openagents/NIP-OPT.md) | The normative v1 wire contracts for semantic signatures, implementations, studies, candidates, trials, and results. |

## The complete loop

1. Define the task's meaning, typed inputs/outputs, and protected constraints.
2. Establish a working baseline and an evaluation capable of detecting a
   useful improvement on the actual workload.
3. Admit a bounded search over named implementation choices.
4. Materialize and execute each exact candidate; record all costs and failures.
5. Select using development evidence, then confirm under a frozen policy.
6. Adopt an eligible immutable version through normal operator policy.
   Retain the previous eligible pin and the evidence for keeping or rejecting it.

This loop also applies to document extraction, research synthesis, record
classification, and other agent domains. Domain adapters define their own
evidence and success criteria. Optimization does not authorize sending a
message, modifying a record, or publishing a package.

Nostr carries portable definitions and attributable records. It does not run
the optimizer, decide which metric matters, or make a model answer true.
Local experiments use the same artifacts without publishing every trial.
