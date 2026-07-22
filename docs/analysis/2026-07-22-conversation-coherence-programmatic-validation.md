# Conversation coherence programmatic validation

- Date: 2026-07-22
- Status: producer self-analysis
- Scope: GitHub issue #9159
- Result authority: analysis only

## Purpose

This record validates the conversation correction for issue #9159.
The validation uses the production Desktop turn kernel and its durable stores.
It does not launch Electron.
It does not use Playwright or a renderer control.

The [conversation coherence rubric](./conversation-thread-coherence-rubric.md)
is the score authority for this record.
This self-analysis is not independent assurance.
It does not give release authority.

## Programmatic method

The tests call `installDesktopTurnKernel` with typed provider ports.
They submit turns through the registered `turn:submit` handler.
They read ordered turn frames and the main-owned thread store.
They open the thread store again to check durable state.

Provider results are deterministic test data.
The tests do not claim real-provider quality or availability.
GitHub issue #9161 tracks a complete headless control surface for real-provider tests.

## Baseline result

The reported thread starts with `hey who are you`.
It starts Full Auto without an owner trigger.
It performs unrelated repository work.
It returns an unrelated completion report.

The rubric gives this source thread a score of 9 and grade `F`.
Gates G1, G2, G3, and G5 fail in the deterministic test fixture.

## Corrected identity result

The corrected test makes Codex, Claude, and Grok available.
It submits the exact message `hey who are you`.
The result is a direct OpenAgents answer.
The result has provider `apple_fm`.
The test finds no delegate request and no child runtime note.
It also finds no material action and no mode start.

The test opens the thread store again.
The stored answer and provider agree with the terminal result.
The rubric gives this result a score of 100 and grade `A`.
No hard-fail gate applies.

## Corrected delegation result

The delegation fixture contains an explicit delegation request.
The projected timeline puts the route record before the answer record.
The route record states `OpenAgents routed to Claude subagent`.
The answer record states `via Claude subagent`.
The repeated projection has the same order, route, answer, and attribution.

The rubric gives this result a score of 100 and grade `A`.
No hard-fail gate applies.

## Authority checks

The ordinary delegation constructor sets `background` to `true`.
It sets `fullAuto` to `false`.
The ordinary message does not contain the Full Auto instruction.

The explicit Full Auto prompt still contains the Full Auto instruction.
Thus, background execution and Full Auto authority have separate typed facts.

A second fixture returns an answer about unrelated release work.
The relevance gate changes the completed delegate projection to `failed`.
The timeline does not promote the unrelated answer.

## Verification

The focused command is source data:

```sh
pnpm exec vp test \
  apps/openagents-desktop/src/turn/apple-fm-prompt.test.ts \
  apps/openagents-desktop/src/turn/desktop-apple-fm-provider.test.ts \
  apps/openagents-desktop/src/turn/desktop-delegation.test.ts \
  apps/openagents-desktop/src/turn/desktop-delegate-execution.test.ts \
  apps/openagents-desktop/src/turn/delegated-answer-relevance.test.ts \
  apps/openagents-desktop/src/turn/desktop-turn-main.test.ts \
  apps/openagents-desktop/src/turn/conversation-coherence-grade.test.ts \
  apps/openagents-desktop/src/renderer/react-timeline.test.tsx
```

Result: 8 test files passed and 126 tests passed.

The type check is source data:

```sh
pnpm --dir apps/openagents-desktop run typecheck
```

## Limits

This record does not validate visual layout or accessibility.
It does not validate real-provider output quality.
It validates the typed route, mode, answer, event order, and durable result.
