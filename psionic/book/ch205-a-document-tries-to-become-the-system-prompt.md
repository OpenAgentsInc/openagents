# 205. A document tries to become the system prompt

> **Status: unimplemented — design draft.** Nothing here runs yet.

Retrieved passages and tool results are data. A hostile document can still
contain text that looks like instructions. This chapter builds a replayable
lab that plants such text and checks that it cannot grant authority, while the
harness still completes its legitimate task.

## What this chapter covers

- Attack fixtures in retrieved passages and tool results:
  - forged role markers;
  - requests to reveal a synthetic sentinel;
  - attempts to widen a tool grant.
- A closed environment: local simulated tools, disposable files and a captured
  outbound sink, so every effect is inspectable.
- Deterministic checks that send unauthorized calls straight through the
  capability gate and require zero unauthorized effects.
- Model runs that report four things separately: attempted calls, admitted
  effects, answer leakage and legitimate-task completion.
- Paired benign and hostile runs. Grants, budgets, model and build identities,
  seeds, acceptance bounds and held-out attack variants are frozen before the
  comparison.

## Dependencies

- [Chapter 57](ch057-answer-from-current-sources.md): the documentation assistant whose retrieved passages are attacked.
- [Chapter 200](ch200-the-simplest-loop.md), [Chapter 201](ch201-model-visible-means-logged.md), [Chapter 202](ch202-keep-the-prefix-stable.md), [Chapter 203](ch203-tools-as-typed-capabilities.md) and [Chapter 204](ch204-a-sandbox-and-approvals.md): the loop, log,
  prompt assembly, tools and sandbox.
- A pinned Psionic-served model for the behavioral runs.

## Done means

- Deterministic gate checks show zero unauthorized effects for every planted
  call.
- Model runs on held-out attack variants report attempted calls, admitted
  effects, leakage and task completion as separate numbers.
- A harness that refuses every task fails the utility gate.
- Any policy change is checked in Coax before the attacks run again.
- A clean run states its measured scope. It does not claim immunity to future
  instructions.

## Sources

- [Chapter plan](PLAN.md), Part XXI.
- [Harness notes from DeepSeek Harness](../docs/harness.md).
