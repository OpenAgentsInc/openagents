# AgentCL Paper Summary

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


## Core Claim

AgentCL is a benchmark framework for continual learning in language agents. Its
central claim is simple and useful: if a benchmark streams tasks without
controlling how those tasks relate, the resulting numbers cannot tell whether an
agent learned reusable experience, merely saw adjacent domains, or was harmed by
irrelevant memory.

The paper proposes two fixes:

- Build task streams with explicit cross-task relationships.
- Measure transfer properties directly instead of only reporting average task
  scores.

## Problem Setting

The agent sees a sequence of tasks and maintains persistent memory across that
sequence. Before each task, it can retrieve from memory; after each task, it can
write the interaction, summary, skill, or other derived experience into memory.

The benchmark assumes sparse online feedback: memory is synthesized by the
agent or memory system, not by ground-truth task annotations provided at solve
time.

## Stream Types

AgentCL compares two stream families.

Naive streams:

- Tasks come from the same environment or broad domain.
- No reuse relationship is guaranteed.
- These streams are still useful because accumulated memory should remain
  harmless even when it is not obviously useful.

Compositional streams:

- Earlier tasks are designed to expose reusable sub-solutions, supporting
  evidence, or workflows.
- Later tasks can profitably compose those earlier pieces.
- These streams make plasticity visible because there is a known opportunity to
  transfer.

The important distinction is that a compositional relationship is not just
"same topic." It is a controlled source-target relation where the later task can
reuse earlier work.

## Metrics

AgentCL uses a memoryless baseline plus a two-pass protocol.

For task `i`:

- `Bi`: memoryless baseline performance.
- `Fi`: first-pass performance after the agent has processed earlier tasks.
- `Si`: second-pass performance on the same task after memory is frozen.
- `Hj`: held-out performance after memory construction, on an unseen task.

The paper defines:

- Plasticity Gain: `PGi = Fi - Bi`
- Stability Gain: `SGi = Si - Fi`
- Generalization Gain: `GGj = Hj - Bj`

Interpretation:

- Plasticity Gain asks whether earlier experience helps later in-stream tasks.
- Stability Gain asks whether a solved task becomes persistently reusable after
  later memory updates.
- Generalization Gain asks whether the constructed memory helps on unseen tasks
  outside the construction stream.

This is the most reusable part for OpenAgents: a memory system can improve
plasticity while still failing stability or held-out generalization.

## Benchmark Construction

Coding:

- Built from CodeEval-Pro / BigCodeBench-Lite-Pro self-invoking problem pairs.
- Uses 48 base subtasks and 48 complex tasks.
- The compositional stream places subtasks before complex tasks.
- The naive stream randomly orders complex tasks without corresponding
  subtasks.
- Held-out coding evaluation uses HumanEval-Pro tasks.

Deep research:

- Built from BrowseComp+.
- Uses 100 complex tasks and 308 synthesized subtasks.
- Subtasks share evidence documents and localized reasoning chains with parent
  tasks without directly exposing final answers.
- The paper reports verifier checks for entity specificity, evidence support,
  determinism, and answer completeness.

Language understanding and reasoning:

- MMLU-Pro is used as a naive stream across economics, engineering, and
  philosophy.
- AgentBoard BabyAI has both naive and compositional streams.
- AgentBoard ScienceWorld is used as a naive stream, with a block-stream
  appendix test grouping near-duplicate goal families.

## Methods Evaluated

The paper evaluates non-parametric memory methods:

- ReAct as the memoryless reference.
- LangMem and Mem0 for adaptive semantic or episodic memory.
- Agent Workflow Memory and Dynamic Cheatsheet for procedural memory.
- ExpRAG, ReMem, and MemProbe for self-evolving stream memory.

MemProbe is introduced as a diagnostic method. It stores three views of
experience:

- Interaction memory: concrete trajectory and final response.
- Insight memory: distilled pattern, failure mode, and reusable takeaway.
- Skill memory: procedure-level abstraction or short reusable snippet.

It retrieves semantically similar task-level entries, solves with that bounded
context, then consolidates with validity checks and an LLM quality judgment. The
retrieved memory is treated as reference context, not authority.

## Main Results

Controlled streams increase discriminative power.

- On CodeEval-Pro, method accuracy dispersion on complex tasks is much higher
  in compositional streams than naive streams: 9.4 and 8.8 across the two
  compositional passes, compared with 3.0 and 1.9 in the naive stream.
- On BrowseComp+, the same pattern is stronger: 14.9 and 16.0 in
  compositional passes, compared with 2.3 and 5.7 in the naive stream.
- MMLU-Pro, used as a naive stream, shows only modest separation: 1.7 and 1.8
  average-accuracy standard deviation across passes.

Plasticity is real in controlled reuse settings.

- On CodeEval-Pro compositional streams, ExpRAG, ReMem, and MemProbe show large
  Plasticity Gains of +17.7, +13.5, and +21.9 points.
- On BrowseComp+ compositional streams, MemProbe reaches +40.0 points
  Plasticity Gain on complex questions.

Stability and generalization remain unsolved.

- CodeEval-Pro compositional Stability Gain is flat or negative for several
  high-plasticity methods: ExpRAG +0.0, ReMem -2.0, MemProbe -2.1.
- Held-out HumanEval-Pro gains are limited, and memoryless ReAct remains
  strongest overall at 72.5. MemProbe is closest among memory methods at 71.7
  after the compositional stream.
- BrowseComp+ exposes large naive-stream Stability Gain swings, including
  Mem0 at -11.0 and MemProbe at +11.0.

Naive streams still matter.

- They are weak at proving useful reuse because transfer opportunities are not
  controlled.
- They are good at detecting whether memory becomes harmful when the task
  stream offers no guaranteed reuse.

## Case-Study Lesson

The paper includes coding cases where memory helps when a retrieved task is a
true sub-solution and hurts when a retrieved task is only superficially similar.

The engineering lesson is that retrieval hit rate is not enough. A memory system
also needs:

- Relationship provenance between memory and current task.
- A way to mark memory as reference rather than instruction.
- A mechanism to ignore misleading but semantically similar memories.

## Limitations

The paper intentionally focuses on non-parametric memory. It does not
systematically evaluate parameter-update or policy-training continual-learning
methods.

Other boundaries to remember:

- BrowseComp+ subtasks are synthesized with privileged construction-time
  access to parent answers and evidence, though this information is not exposed
  to evaluated agents.
- All memory methods retrieve top-2 memory chunks in the shared setup, which is
  useful for fair comparison but not necessarily optimal.
- The strongest claims are about evaluation methodology and non-parametric
  memory diagnosis, not a solved memory architecture.
