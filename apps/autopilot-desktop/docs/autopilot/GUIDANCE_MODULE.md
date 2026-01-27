# Codex Turn-to-Turn Guidance Module

- Status: Draft
- Source of truth: code + SYNTHESIS_EXECUTION.md
- If docs conflict with code, code wins. If terms conflict, GLOSSARY.md wins.

## Purpose

Describe the intelligence layer that runs between Codex turns inside a Full Auto
run. This replaces the manual "continue" loop with a structured, extensible
Guidance Module that can be evaluated and improved over time.

## Definitions

- Turn: one Codex execution window that ends in `turn/completed` or `turn/error`.
- Run: a multi-turn session (Full Auto) composed of turns.
- Guidance: a soft recommendation for what to do next.
- Guardrails: hard, deterministic constraints that can override guidance.

## The problem to solve

Today, long-running Codex runs are driven by a manual loop:

- You issue a run, then type "continue" or queue a few more prompts.
- Turns are not aware of each other beyond ad hoc file handoff.
- There is no shared model of state, budget, or environment across turns.

This is brittle and does not scale. The system needs a real decision engine
between turns that can:

- See the full context of what happened in the last turn.
- Understand the goal, constraints, environment, and remaining budget.
- Decide the next action with measurable confidence.
- Improve over time via evals and community contribution.

## What changes for the user

Before:
- The human is the scheduler ("continue, continue").
- The "Ralph" loop: a deterministic while-loop that repeatedly feeds a fixed harness prompt
  (often `PROMPT.md`) to a coding agent, where the prompt instructs the agent to maintain/update
  a TODO/plan and keep iterating until done.

After:
- The Guidance Module is the scheduler (it decides what happens next).

This improves autonomy vs manual "continue", but the "state + scheduler" is still mostly
encoded in a text prompt and repo files rather than a typed, inspectable decision system with
explicit budgets, policies, and replayable decision records.

Concrete examples:
- Turn ends with failing tests and low remaining budget -> Guidance returns
  "pause" with a reason and waits for human input.
- Turn ends with clean diff and tests passing -> Guidance returns "continue"
  with a follow-up prompt (docs, refactor, polish).

## The key idea

Replace the "continue" step with a DSPy-powered Guidance Module:

Codex Turn -> Guidance Module -> Codex Turn

This Guidance Module is not one monolithic function. It is a composable stack
of DSPy signatures and modules plus policy gates, state, and optimization.

## Guidance Module contract (conceptual)

A minimal contract makes the system implementable and extensible:

```ts
type GuidanceInputs = {
  goal: { intent: string; success_criteria?: string[] }
  summary: FullAutoTurnSummary
  state: {
    turn_count: number
    no_progress_count: number
    tokens_remaining?: number
    time_remaining_ms?: number
    permissions: { can_exec: boolean; can_write: boolean; network: "none" | "scoped" | "full" }
  }
}

type GuidanceDecision = {
  action: "continue" | "pause" | "stop" | "review"
  next_input?: string
  reason: string
  confidence: number
  tags?: string[] // e.g. ["needs-user-input", "budget-low", "tests-failing"]
}
```

This contract is the surface that guidance packages target.

## DSPy mapping

DSPy vocabulary:

- Signature: typed contract, inputs -> outputs.
- Module: a composable program that uses one or more signatures.
- System (informal): modules + orchestration + state + evaluation.

Guidance is a system. Internally, we still implement a top-level DSPy
composition module (e.g. `GuidanceRoot`) that calls submodules; externally, we
call the whole layer the Guidance Module.

## Why "Guidance Module" is the right name

Guidance Module is the better choice for the product narrative:

- "Decision" implies a single authoritative choice.
- "Guidance" implies a multi-signal, policy-driven recommendation.
- It fits open, extensible, and community-optimized intelligence.

We still use "decision" internally for specific signatures and outputs:

- DecisionSignature
- DecisionPolicy
- NextActionDecision

Guidance is the layer; decisions are its outputs.

## Inputs and outputs (conceptual)

Inputs (examples):

- Turn summary: status, errors, plan, diff, tool calls, approvals.
- Goal and constraints: user intent, deadlines, success criteria.
- Environment: workspace, tool availability, permissions.
- Budget: token/time limits, remaining retries.
- History: recent actions, no-progress streaks.

Outputs (examples):

- Next action: continue, pause, stop, review.
- Next input: what to send to Codex next.
- Reason + confidence.
- Tags like `needs-user-input` or `budget-low`.
- Guardrail outcome if overridden by policy.

## Invariants (hard constraints)

- Guardrails are deterministic and cannot be overridden by models.
- If parsing/validation fails, default to pause (safe failure mode).
- Every decision is logged with inputs, outputs, and versions for replay.

## Guidance Module architecture (conceptual)

Top-level name: Guidance Module (turn-to-turn layer)

Inside the layer (DSPy terms):

- StateSummarizer (signature/module)
- GoalRefiner (signature/module)
- BudgetPolicy (signature/module)
- SafetyPolicy (signature/module)
- NextActionSelector (signature/module)
- Verifier / StopDecider (signature/module)

This is a module stack, not a single module.

## Open source vs forkable (the marketplace story)

We want a system that is not just open source, but extensible and packaged:

- Each policy or selector can be shipped as a package.
- Each package ships with evals and measurable gains.
- The runtime can load, compare, and route between packages.
- Contributors can improve a single piece and get credited or rewarded.

Think NPM, but for agent intelligence (signatures, modules, and policies).

## Guidance package format (conceptual)

A guidance package may provide:

- One module (e.g. BudgetPolicy, NextActionSelector).
- Metadata + compatibility constraints.
- Evals + expected metrics.
- Optional attribution or reward address.

Example shape (conceptual):

```json
{
  "name": "openagents-budget-policy",
  "version": "0.1.0",
  "entry": "BudgetPolicy",
  "compatible_with": ["GuidanceInputs@v1"],
  "evals": ["budget-safety", "cost-overrun"],
  "tags": ["budget", "safety"]
}
```

Routing can be based on eval score, bandit selection, or domain-specific rules.

## Decision record (replay and attribution)

Stored after every turn:

- Inputs hash + full `FullAutoTurnSummary`.
- Decision output + guardrail audit.
- Model id/version and package versions.
- Timestamps, token usage, and outcome labels.

This enables debugging, evaluation, attribution, and payout logic.

## How this maps to Full Auto today

In Autopilot Desktop, Full Auto already uses a DSPy decision step after each
Codex turn completion. That is the first implementation of this Guidance Module.

Key elements already exist:

- Turn summary built from Codex events.
- DSPy signature for next action.
- Guardrail enforcement (stop/pause conditions).
- Decision logging and replayability.

The next step is to grow this into an extensible module stack and open package
surface instead of a single decision signature.

## Current Guidance Module (Full Auto today)

Today the Guidance Module is implemented as four concrete steps, shown in the
canvas graph. These are the actual stages executed between Codex turns.

Pipeline:

events -> summary -> decision -> guardrails -> dispatch -> next turn

### 1) Turn Summary (FullAutoTurnSummary)

Purpose:
- Build a compact, structured snapshot of what just happened in the last Codex turn.

Inputs:
- App-server events emitted during the turn:
  - `turn/plan/updated`, `turn/diff/updated`
  - `thread/tokenUsage/updated`
  - `item/commandExecution/requestApproval`, `item/fileChange/requestApproval`
  - `item/tool/requestUserInput`
  - `turn/error`, `turn/completed`

Outputs:
- A structured summary with:
  - `last_turn_status`, `turn_error`
  - `turn_plan`, `diff_summary`
  - `token_usage`
  - `pending_approvals`, `pending_tool_inputs`
  - `compaction_events`, `recent_actions`
  - `turn_count`, `no_progress_count`

Role in the pipeline:
- This is the canonical, typed input to the DSPy decision signature.
- It is also persisted in decision logs for replayability.

### 2) DSPy Decision (FullAutoDecisionSignature)

Purpose:
- Use DSPy to choose the next action after a completed turn.

Inputs:
- The `FullAutoTurnSummary` for the completed turn.

Outputs:
- `action`: continue | pause | stop | review
- `next_input`: optional next prompt for Codex
- `reason`: short explanation
- `confidence`: numeric confidence score

Role in the pipeline:
- This is the intelligence between turns. It replaces the manual "continue"
  prompt with a structured, optimizable decision.
- It is the component intended to be optimized and extended by the community.

### 3) Guardrails (enforce_guardrails)

Purpose:
- Apply safety, budget, and quality constraints to the DSPy decision.

Inputs:
- The DSPy decision output.
- Current full-auto state (turn count, progress signature, token budget, etc.).

Rules enforced today:
- `turn_failed` -> stop
- `turn_interrupted` -> pause
- `max_turns` / `max_tokens` -> stop
- `no_progress_limit` -> stop
- `low_confidence` or `review` -> pause

Outputs:
- A possibly overridden action with a guardrail audit trail:
  - `triggered`, `rule`, `original_action`, `enforced_action`, and confidences.

Role in the pipeline:
- Guarantees bounded and safe execution regardless of model output.
- Ensures Full Auto can be trusted even when DSPy confidence is low.

### 4) Dispatch (FullAutoAction)

Purpose:
- Execute the chosen action and start the next Codex turn if appropriate.

Inputs:
- Final (guardrail-validated) action.
- `next_input` (if present) or fallback continue prompt.

Behavior:
- continue -> start next turn with `next_input` or fallback prompt
- pause -> stop auto loop and wait for user
- stop -> end run and stop future turns

Role in the pipeline:
- This is the actual handoff back to Codex, completing the turn-to-turn loop.

### How they relate (turn-to-turn loop)

1. Codex completes a turn and emits events.
2. Turn Summary aggregates those events into `FullAutoTurnSummary`.
3. DSPy Decision predicts the next action + rationale + confidence.
4. Guardrails enforce budget/safety constraints and can override the action.
5. Dispatch executes the action and (if continue) triggers the next Codex turn.

These four steps are the current, concrete implementation of the Guidance
Module in Full Auto today.

## Summary

- Guidance Module is the product name for the turn-to-turn intelligence layer.
- DSPy provides the signatures and modules used inside it.
- Guardrails are deterministic and enforce safe execution.
- The system is designed to be extensible and package-friendly.
