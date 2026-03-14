# Structured Agent Weather Pilot

> Status: canonical `PSI-258` / `#3563` pilot record, updated 2026-03-14 after
> landing the runnable pilot harness in
> `scripts/release/check-psionic-weather-agent-pilot.sh`.

This document records the first end-to-end structured-agent workload pilot for
Psionic.

The chosen workload is a tiny weather agent that combines:

- structured JSON output
- tool calls
- router-owned tool-loop execution
- response-state continuation
- route and cache truth on the generic Psionic server

## Why This Workload

This is the workload class that makes the `SGLang` lessons relevant to
OpenAgents:

- not raw token generation alone
- not stateless chat alone
- not a product demo that bypasses the router

The workload is intentionally small and deterministic, but it still crosses the
same Psionic-owned serving layers that a real OpenAgents agent workload needs.

## Canonical Runner

Run the pilot from the repo root:

```bash
scripts/release/check-psionic-weather-agent-pilot.sh
```

## Workload Shape

The pilot uses one generic Psionic server with two tiny decoder models:

- a tool-call model that emits the weather-tool request on `/v1/responses`
- a structured-output model that emits JSON-schema-constrained output on
  `/v1/chat/completions`

The pilot workload then performs four connected stages:

1. structured summary request with prefix-cache controls
2. repeated structured summary request that proves cache reuse and stable route
   reporting
3. `/v1/responses` request that stores response state and surfaces a tool call
4. router-owned tool loop and response continuation over the same server/router
   stack

## Pass Criteria

The pilot is green only if all of the following are true:

- the structured summary model emits machine-checkable JSON-schema output
- repeated structured summary requests surface:
  - `x-psionic-route-worker`
  - `x-psionic-route-strategy`
  - `x-psionic-prefix-cache-state`
- the second structured request reports a cache hit instead of silently acting
  as a cold path
- the `/v1/responses` request stores response state and surfaces the requested
  weather tool call
- the router-owned tool loop executes a tool call and returns the final
  assistant message without app-local orchestration
- the continuation request replays response state and advances the conversation
  revision explicitly

## Expected Signals

The current tiny weather pilot should produce these signals:

- structured summary request:
  - `x-psionic-structured-output-mode = fallback_json_schema`
  - `x-psionic-route-worker = openai-compat`
  - first request `x-psionic-prefix-cache-state = none`
  - repeated request `x-psionic-prefix-cache-state = hit`
- tool-call response:
  - `psionic_tool_calls[0].name = "get_weather"`
  - `psionic_response_state.stored = true`
- router tool loop:
  - first step emits the `get_weather` tool call
  - second step returns the final assistant message
- continuation:
  - `previous_response_id` points at the stored response
  - `conversation.id` remains stable
  - `conversation.revision` advances
  - `psionic_response_state.replayed_prompt_messages` is greater than zero

## Current Limitations

This pilot is intentionally bounded:

- tiny deterministic local models, not a production agent benchmark
- one built-in weather tool executor, not an external marketplace tool provider
- one-worker route truth, not a multi-worker latency or failover pilot
- no human review or release-governance workflow

Those are acceptable limits for this issue because the pilot's purpose is to
prove integrated workload ownership, not to claim full productization.

## Claim Rule

This pilot is sufficient to prove that Psionic now has one real structured or
agentic workload that runs through Psionic-owned structured-serving and router
code paths end to end.

It is not sufficient to claim:

- every agent workload is productized
- multi-worker routed agent workloads are already benchmarked
- every tool-provider interface is fully operational outside this pilot
- broad marketplace or billing readiness
