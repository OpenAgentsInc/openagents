# Autopilot Bootstrap Plan

This document defines the **bootstrap state machine** used to onboard a new user/thread in Autopilot.

Purpose:
- Collect a minimal set of user preferences/identity fields.
- Avoid generic assistant greetings during bootstrap.
- Make progress deterministic and replayable (state-driven, not heuristics-driven).

Related:
- `docs/autopilot/spec.md`
- `docs/autopilot/reference/bootstrap-flow-and-greeting-fix.md`

## Blueprint Storage

Bootstrap state is persisted in the thread's **Blueprint** record (Convex-first MVP) and/or Durable Object storage (alternate execution plane).

Canonical requirement:
- Bootstrap behavior is driven from **persisted state** (`bootstrapState`), not inferred from recent messages.

## BootstrapState (Conceptual)

```ts
type BootstrapStatus = "in_progress" | "complete"

type BootstrapStage =
  | "ask_user_handle"
  | "ask_agent_name"
  | "complete"

type BootstrapState = {
  status: BootstrapStatus
  stage: BootstrapStage
}
```

Notes:
- Exact storage fields are implementation-defined; this is the conceptual contract.
- `status: "complete"` implies the system prompt no longer applies bootstrap constraints.

## Stages And Transitions

### 1) ask_user_handle

Goal: determine what to call the user (e.g. `addressAs` / `name`).

Rules:
- Do not emit generic greetings (e.g. "Hello! How can I assist you today?").
- If the user did not provide a name, respond only with a re-ask prompt (e.g. "What shall I call you?") until they do.

Transition condition:
- A non-empty handle is extracted and persisted.

### 2) ask_agent_name

Goal: collect the user's preferred name for the assistant/agent (if applicable to the product surface).

Transition condition:
- Agent name is provided and persisted (or explicitly skipped if the surface supports skipping).

### 3) complete

Goal: normal chat behavior.

Entry condition:
- Required fields are present; bootstrap no longer gates the system prompt.

## Implementation Notes

Convex-first (web):
- Extract handle via DSE signature (`ExtractUserHandle`) and persist via mutation (e.g. `applyBootstrapUserHandle`).

Durable Object execution plane (worker):
- During bootstrap, force tool choice to the bootstrap tool (e.g. `bootstrap_set_user_handle`) so the model cannot skip persistence.

## Verification

Minimum behavior checks:
- Saying "hi" during `ask_user_handle` results in a re-ask for handle (not a generic greeting).
- Providing a handle advances `bootstrapState.stage` to `ask_agent_name`.

