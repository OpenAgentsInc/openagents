# OpenAgents Agent Traces (shareable sessions) — spec

> **Status: SPEC / proposal (2026-06-24). Not built.** This describes a primitive
> we want, grounds it in a standard format (ATIF), and records what exists vs what
> is needed. The first piece — the QA process emitting one ATIF trace + a beautiful
> render — is being built now; the shareable `/trace/{uuid}` surface is not.

## The idea

When an agent does work anywhere in OpenAgents — a Khala chat session, an
Autopilot work order, a QA run, a Pylon assignment — that session is the thing
people actually want to look at and share. So every shareable agent session should
have **one stable, public URL**:

```
https://openagents.com/trace/{uuid}
```

A `/trace/{uuid}` page renders the full session: the goal, each agent step (its
narration, reasoning, the tool call it made, the observation/result that came
back), any screenshots/video, and the run metrics. You verify an agent's work by
reading the trace — no local setup, just a link you can drop in a PR, a Forum
thread, a DM, or a customer report.

### Naming: it's a *trace*, not an "eval"

What the QA flow first shipped was framed as an "eval" at `/pro/evals/<id>`. Both
are wrong:

- **"eval"** in the AI community means a *scored benchmark* (accuracy on a suite).
  What we have is a **record of an agent session** — closer to a chat transcript /
  agent trajectory. Call it a **trace**.
- **`/pro/`** is the logged-in operator console. A *shareable* artifact should not
  live under it. The shareable surface is `/trace/{uuid}`.

A comparison ("how do agents perform with these MCP changes?") is then a **view
over multiple traces**, not a primitive of its own.

## Format: ATIF

Traces use **ATIF — the Agent Trajectory Interchange Format** (a JSON standard for
logging the full interaction history of LLM agents). Spec (vendored reference):
`projects/repos/harbor/rfcs/0001-trajectory-format.md` (ATIF-v1.7);
docs: <https://www.harborframework.com/docs/agents/trajectory-format>.

Why ATIF instead of a bespoke shape:

- It already models exactly what a trace is: `Trajectory { schema_version,
  session_id, trajectory_id, agent{name,version,model_name,tool_definitions},
  steps[ StepObject{ step_id, source:"user"|"agent"|"system", message,
  reasoning_content, tool_calls[{tool_call_id, function_name, arguments}],
  observation{results[{source_call_id, content}]}, metrics{prompt_tokens,
  completion_tokens, cost_usd, ...} } ], final_metrics, subagent_trajectories }`.
- It covers tool calls + observations, per-step + aggregate metrics, multimodal
  content (images), and **multi-agent / subagent** trajectories — all things our
  agents produce.
- It has existing tooling and adoption: a validator, a viewer, and exporters for
  OpenHands, Claude Code, Codex, Gemini CLI, mini-swe-agent. Interop is free.

We store and serve a **public-safe projection** of an ATIF trajectory (never raw
secrets, tokens, wallet material, PII, or raw/split provider model ids — only
`openagents/khala`-class public ids).

## Surface + storage (proposed)

- **Page:** `GET /trace/{uuid}` — public-safe render of the ATIF trajectory: a step
  timeline, tool calls + observations, inline screenshots, embedded video, and
  final metrics. (`/pro` stays the operator console; sharing happens at `/trace`.)
- **Store:** the ATIF trajectory projection in D1; video/screenshots in R2,
  referenced from the trajectory.
- **Ingest:** `POST /api/traces` — agent-bearer auth + `Idempotency-Key`, a
  public-safe tripwire that rejects secrets/raw ids, returns the `{uuid}`.
- **Visibility (open question):** public vs unlisted-by-uuid vs owner-only share.

## How existing work maps

- A **QA / Khala run** is a trace: the qa-runner emits an ATIF trajectory + video,
  which becomes `/trace/{uuid}`. (The ATIF emitter + a sample beautiful render are
  the first build.)
- **"Compare agents across MCP changes"** = a comparison view over several trace
  uuids (the thing previously mis-framed as `/pro/evals`).
- **Khala chat**, **Autopilot work orders**, **Pylon assignments** should each be
  expressible as ATIF traces over time.
- A **receipt** (`docs` / issue #6188) can reference the trace uuid as its
  execution-trace evidence.

## Status — what exists vs what's needed

**Exists today:** the qa-runner produces `result.json` + `session-trace.json`
(`KhalaSessionTrace`) + a video per run. An **ATIF emitter** (run → ATIF
trajectory) and a **beautiful standalone trace render** (one real Khala `/login`
trace) are being built now as the first concrete artifact.

**Not built yet:**
- The `/trace/{uuid}` route, page, D1/R2 store, and `POST /api/traces` ingest.
- Cross-app trace emission (Khala chat, Autopilot, Pylon).
- The comparison/trace-set view (the real "chill-evals" surface).
- A pinned public-safe ATIF subset + a TS/Effect-Schema validator in-repo.
- Visibility/sharing model + retention.

## Open questions

- Pin a specific ATIF version + a public-safe required subset; in-repo Effect
  Schema vs adopting harbor's types.
- Visibility/auth (public, unlisted-by-uuid, owner-only) and abuse/retention.
- Video/artifact storage + size limits in R2; multimodal `ContentPart` images.
- The comparison view contract (compare N trace uuids on pass/latency/behavior).
- Relationship to receipts (#6188), product-promises, and the operator `/pro`
  console (which links out to `/trace/{uuid}`).
