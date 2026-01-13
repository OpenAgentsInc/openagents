# Glossary

> **Status:** Accurate
> **Last verified:** (see commit)
> **Source of truth:** This document is the canonical vocabulary reference
> **Doc owner:** dsrs
> **If this doc conflicts with code, code wins.**

Canonical definitions for DSPy and OpenAgents terminology.

---

## Core DSPy Primitives

| Term | Definition |
|------|------------|
| **Signature** | Typed input/output contract for an LLM call. Declares what fields go in, what fields come out. |
| **Module** | Composable unit that implements `forward()` and optionally `batch()`. Wraps signatures with execution logic. |
| **Predictor** | A module that runs inference (e.g., `Predict`, `ChainOfThought`, `Refine`). |
| **Optimizer** | Algorithm that improves signatures (MIPROv2, COPRO, GEPA, Pareto). |
| **Metric** | Scoring function that measures prediction quality. Drives optimization. |
| **Adapter** | Serializes requests and parses responses for a specific LLM format. Does NOT validate or retry. |
| **Provider** | Backend that actually runs inference (Ollama, llama.cpp, Codex, etc.). |

---

## Execution Concepts

| Term | Definition |
|------|------------|
| **Execution Runtime** | The layer that validates tool params, enforces retries, and runs tools. Distinct from adapters. |
| **Lane** | Routing category for inference: Local (free), Swarm (NIP-90), Datacenter (API). |
| **Dispatcher** | Component that sends NIP-90 jobs to the swarm (e.g., SwarmDispatcher). |
| **step_utility** | Learning signal for a tool call. Canonical range: **-1.0..+1.0** (from `ToolResultSignature`). |
| **verification_delta** | Change in failing tests: `tests_before - tests_after`. Positive = improvement. |
| **PlanIR** | Canonical intermediate representation for plans. Emitted by both Adjutant and Autopilot planners. |

---

## Artifacts

| Term | Definition |
|------|------------|
| **Verified PR Bundle** | The three artifacts emitted by every session: PR_SUMMARY.md, RECEIPT.json, REPLAY.jsonl. |
| **PR_SUMMARY.md** | Human-readable summary of what changed, verification results, and confidence. |
| **RECEIPT.json** | Machine-readable receipt with hashes, tool calls, verification, and policy_bundle_id. |
| **REPLAY.jsonl** | Canonical event stream for replay and debugging. Spec in REPLAY.md. |
| **ReplayBundle** | Current implementation format (different from REPLAY.jsonl v1). See REPLAY.md "Compatibility Plan". |
| **policy_bundle_id** | Identifier for the policy bundle (instructions + demos + optimizer config) used in a session. |

---

## Metrics & Scoring

| Term | Definition |
|------|------------|
| **Proxy Metric** | Cheap, fast metric (format, syntax, length). Runs first. |
| **Truth Metric** | Expensive, accurate metric (LLM judge, sandbox). Runs only if proxy passes. |
| **ToolParamsSchemaMetric** | Proxy metric scoring whether tool params match JSON schema. Does NOT block execution. |
| **OutcomeCoupledScorer** | Composite scorer using step_utility, verification_delta, repetition, and schema validity. |

---

## Replay Events

| Term | Definition |
|------|------------|
| **ReplayHeader** | First line of REPLAY.jsonl. Contains replay_version, producer, created_at. |
| **SessionStart** | Event marking session start. Contains session_id, issue_number, policy_bundle_id. |
| **ToolCall** | Event when a tool is invoked. Contains id, tool, params, params_hash, step_id. |
| **ToolResult** | Event when a tool returns. Contains id, output_hash, step_utility, latency_ms. |
| **Verification** | Event when verification runs. Contains commands, exit_codes, verification_delta. |
| **SessionEnd** | Final event. Contains status, confidence, total_tool_calls, total_latency_ms. |

---

## NIP-90 / Nostr

| Term | Definition |
|------|------------|
| **NIP-90** | Data Vending Machine protocol. Defines job request/result flow. |
| **kind:5050** | Job request event. |
| **kind:6050** | Job result event. |
| **kind:7000** | Job invoice/feedback event. |

---

## Status Terms

| Term | Definition |
|------|------------|
| **Implemented** | Signature/module exists in code AND is wired to production paths. |
| **In code** | Struct exists but not yet wired to production. |
| **Spec only** | Documented specification, code not yet written. |
| **Wave** | Implementation phase. Wave status tracks struct existence + unit tests. |
| **MVP ready** | Wired into production path, not just struct existence. |

---

## Naming Collisions

| Collision | Resolution |
|-----------|------------|
| `step_utility` (0..1 vs -1..+1) | Canonical is **-1.0..+1.0** from `ToolResultSignature`. `ToolStepUtilitySignature` outputs 0..1 and should be normalized or renamed. |
| `policy_version` vs `policy_bundle_id` | Canonical is **policy_bundle_id**. All docs should use this term. |

---

## See Also

- [ARCHITECTURE.md](ARCHITECTURE.md) - Core traits and runtime layers
- [SIGNATURES.md](SIGNATURES.md) - Signature specifications
- [REPLAY.md](REPLAY.md) - REPLAY.jsonl format
- [ARTIFACTS.md](ARTIFACTS.md) - MVP artifact schemas
