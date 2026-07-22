# Conversation complexity rubric

- Date: 2026-07-22
- Status: active analysis method (metric `coherence-screen-v2`)
- Audience: agents, product reviewers, and test authors
- Related methods: [coherence rubric](./conversation-thread-coherence-rubric.md),
  [deterministic screening](./deterministic-coherence-screening.md),
  [flywheel](./coherence-flywheel.md)
- Tracking issue: [#9161](https://github.com/OpenAgentsInc/openagents/issues/9161)
- Result authority: analysis only

## Why complexity exists

A coherence score alone is not evidence. A single question with one direct
answer scores 100 and proves almost nothing about the implementation. The
purpose of grading is to prove that the full system works: routing,
delegation, sub-agent orchestration, handoffs between agents and models,
tool execution, and long multi-step work. A coherence score therefore
carries evidence weight proportional to how much of that machinery the
conversation actually exercised.

The target of the hill-climb is not "high coherence." It is **high
coherence at high complexity**: conversations that spawn sub-agents, pass
work between them, run many tools across many turns — and still stay
completely coherent with what the user asked.

## The deterministic complexity score

`computeComplexity` in `scripts/coherence-core.ts` derives a 0-100 score
from transcript features. Components, each capped:

| Component | Feature | Points | Cap |
| --- | --- | --- | --- |
| Continuations | Assistant turns beyond the first | 2 per turn | 10 |
| Dialogue | User turns beyond the first | 1 per turn | 5 |
| Tool volume | Tool calls (log-scaled: `8·log2(1+n)`) | — | 24 |
| Tool diversity | Distinct tool or item kinds | 3 each | 12 |
| Mutations | File changes | 2 each | 10 |
| Sub-agent activity | Spawn plus communication events (log-scaled: `7·log2(1+n)`) | — | 21 |
| Sub-agent breadth | Distinct sub-agent identities | 4 each | 12 |
| Multi-model | Distinct models beyond the first | 5 each | 10 |

Feature sources: Codex rollouts supply `sub_agent_activity`
(`agent_thread_id`, `kind: started` and interaction kinds), `turn_context.model`,
item and tool events. Claude Code transcripts supply `Agent`, `Task`, and
`Workflow` tool calls (spawns), `SendMessage` (communication),
`message.model`, and typed tool names.

## Tiers

| Tier | Score | Meaning |
| --- | --- | --- |
| C0 | 0-9 | Trivial. One question, direct answer. Proves routing exists, nothing more. |
| C1 | 10-24 | Simple. A few turns or a few tools. |
| C2 | 25-49 | Moderate. Multi-step tool work with mutations. |
| C3 | 50-74 | Complex. Sub-agents or multiple models plus sustained tool work. |
| C4 | 75-100 | Heavy orchestration. Many sub-agents communicating, multiple models, long horizon. |

## How to read the pair

Report every conversation as `coherence @ complexity`, for example
`100/A @ C0(0)` or `92/A @ C3(61)`.

- `100/A @ C0` is a smoke result. It proves the path runs. It is not
  release-grade evidence for anything else.
- A high coherence score at C3 or C4 is the evidence the system needs.
- A coherence drop that appears only at higher tiers localizes the defect
  to the orchestration machinery: handoff, sub-agent communication, or
  long-horizon state.

Aggregates therefore include **complexity-weighted coherence**: the mean
coherence weighted by each conversation's complexity score. A corpus of
trivial clean chats cannot mask incoherent complex ones.

## Ratchet rules

- Never optimize the complexity score itself. Complexity is a weight, not
  a target: manufacturing busywork to raise the tier corrupts the metric.
  Complexity must come from honest test scenarios (the flywheel scenario
  matrix) and real usage.
- The flywheel target is stated as a pair: coherence at or above the prior
  entry, at a complexity tier at or above the prior entry.
- Tier thresholds and component weights are metric changes: land them with
  tests and a ledger note, and reset trend comparison at that entry.

## Known limits

- The score measures observed activity, not task difficulty. A hard
  reasoning question with no tools scores C0.
- Exec-mode transcripts under-report streamed detail compared to
  app-server transcripts. Compare tiers within one transport when judging
  trends.
- Sub-agent detection depends on the toolchain's event vocabulary. A
  toolchain without sub-agent events caps at the tool-and-turn components.
