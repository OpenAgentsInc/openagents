# AskUserQuestion Desktop pixel receipt

Issue: [#8941](https://github.com/OpenAgentsInc/openagents/issues/8941)

These screenshots come from the production-built OpenAgents Desktop React
renderer with the deterministic smoke providers enabled. No real provider,
account, credential, or user profile participated.

- `01-pending-question.png` proves the Claude question remains visible and
  answerable after the immediate render/focus cycle. It includes the bounded
  question, labeled options and descriptions, Other text input, submit action,
  and the non-working `Waiting for your answer` state.
- `02-answer-round-trip.png` proves the Other answer crossed the schema-decoded
  host bridge, reached the fixture agent, appeared in the agent response, and
  settled back to ordinary Send admission.

Verification command:

```sh
OPENAGENTS_DESKTOP_SMOKE=1 \
OPENAGENTS_DESKTOP_SMOKE_REACT=1 \
OPENAGENTS_DESKTOP_SMOKE_QUESTION_ONLY=1 \
OPENAGENTS_DESKTOP_SMOKE_SHOTS=/tmp/oa-8941-final-shots \
pnpm --dir apps/openagents-desktop exec electron .
```
