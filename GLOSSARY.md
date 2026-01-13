# Glossary

- **Status:** Accurate
- **Last verified:** (see commit)
- **Source of truth:** This document is the canonical vocabulary reference for all OpenAgents documentation
- **Doc owner:** root
- **If this doc conflicts with code, code wins.**

Canonical definitions for OpenAgents terminology. All docs should use these terms consistently.

---

## Cryptographic Identity

| Term | Definition |
|------|------------|
| **FROST** | Flexible Round-Optimized Schnorr Threshold signatures. Threshold signing where no party ever holds the full key. |
| **FROSTR** | FROST for Nostr. Our implementation adapted for Nostr's event signing requirements. |
| **Bifrost** | Coordination protocol for threshold operations over Nostr relays (peer discovery, message routing, share aggregation). |
| **Threshold Config** | Specifies participant public keys, signing threshold, and cryptographic parameters for distributed key generation. |

---

## Payment Rails

| Term | Definition |
|------|------------|
| **Spark** | Breez's nodeless Lightning solution combining LN channels + L2 transfers + on-chain settlement. |
| **Rail** | A payment network + settlement mechanism (Lightning, Cashu mint, Taproot Assets, on-chain). Each rail has distinct trust properties. |
| **AssetId** | Specific asset on a specific rail. `BTC_LN` ≠ `BTC_CASHU(mint_url)` ≠ `USD_CASHU(mint_url)`. |
| **Quote** | Prepared payment intent with reserved funds, expiry timestamp, and idempotency key. States: CREATED → UNPAID → PENDING → PAID/FAILED/EXPIRED. |
| **Reconciliation** | Background process resolving pending quotes, expiring reservations, and repairing state after crashes. |

---

## Treasury & Exchange

| Term | Definition |
|------|------------|
| **Neobank** | Programmable treasury layer for agents. Not a bank but a payments router with budget enforcement, multi-rail support, and audit trails. |
| **TreasuryRouter** | Policy engine deciding payment routing: which rail, which asset, which limits, which approvals. |
| **Exchange** | Agent-to-agent marketplace for FX (BTC↔USD), liquidity swaps, and payment routing services. |
| **Treasury Agent** | Specialized agent providing financial services to the network. |

---

## Nostr Protocols

| Term | Definition |
|------|------------|
| **NIP-90** | Data Vending Machine protocol. Defines job request/result flow. |
| **kind:5050** | Job request event (NIP-90). |
| **kind:6050** | Job result event (NIP-90). |
| **kind:7000** | Job invoice/feedback event (NIP-90). |
| **NIP-SA** | Sovereign Agent Protocol (proposed). Defines agent lifecycle events: profile, state, schedule, ticks, trajectories. |
| **NIP-57** | Zaps. Lightning payments attached to Nostr events. |
| **NIP-42** | Authentication. Required for agent relay access. |
| **NIP-44** | Encryption. End-to-end encryption for agent state. |
| **NIP-34** | Git primitives for Nostr. Repositories, issues, patches as events. |
| **NIP-89** | Handler discovery. Providers register capabilities via kind 31990 announcements. |
| **DVM** | Data Vending Machine. A compute provider responding to NIP-90 job requests. |

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
| **Policy Bundle** | Versioned compilation of signatures/modules/routing thresholds. Contains instructions, demos, optimizer config. |

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
| **Autonomy Level** | Supervision mode: Supervised (human approves each action), Semi-autonomous (human approves high-risk), Autonomous (agent decides). |

---

## Artifacts & Trajectories

| Term | Definition |
|------|------------|
| **Verified PR Bundle** | The three artifacts emitted by every session: PR_SUMMARY.md, RECEIPT.json, REPLAY.jsonl. |
| **PR_SUMMARY.md** | Human-readable summary of what changed, verification results, and confidence. |
| **RECEIPT.json** | Machine-readable receipt with hashes, tool calls, verification, and policy_bundle_id. |
| **REPLAY.jsonl** | Target canonical event stream format for replay and debugging. Spec in `crates/dsrs/docs/REPLAY.md`. |
| **ReplayBundle** | Current implementation format in `autopilot-core/src/replay.rs`. Different from REPLAY.jsonl v1. |
| **rlog** | Session recording format. Structured logs capturing agent trajectories (messages, tool calls, thinking, errors). Predecessor to REPLAY.jsonl. |
| **Trajectory** | Full record of decisions, tool calls, intermediate outputs, and verification steps for a session. |
| **policy_bundle_id** | Identifier for the policy bundle used in a session. Canonical term (not `policy_version`). |

---

## Metrics & Scoring

| Term | Definition |
|------|------------|
| **Proxy Metric** | Cheap, fast metric (format, syntax, length). Runs first. |
| **Truth Metric** | Expensive, accurate metric (LLM judge, sandbox). Runs only if proxy passes. |
| **ToolParamsSchemaMetric** | Proxy metric scoring whether tool params match JSON schema. Does NOT block execution (that's runtime's job). |
| **OutcomeCoupledScorer** | Composite scorer using step_utility, verification_delta, repetition, and schema validity. |
| **APM** | Actions Per Minute. Velocity metric: (messages + tool calls) / duration. Higher = faster autonomous operation. |

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

## Job Types

| Term | Definition |
|------|------------|
| **oa.code_chunk_analysis.v1** | Subjective job type for parallel file/chunk analysis. |
| **oa.retrieval_rerank.v1** | Subjective job type for LLM-based candidate reranking. |
| **oa.sandbox_run.v1** | Objective job type for build/test/lint in isolated sandbox. |
| **Objective Job** | Verification via exit code and artifact hashes. Deterministic. |
| **Subjective Job** | Requires judgment. Uses redundancy, adjudication, or judge models. |

---

## Compute Supply Classes

| Term | Definition |
|------|------------|
| **SingleNode** | One machine, prosumer. Cheap batch jobs, async tasks. |
| **BundleLAN** | Exo-style: multiple devices on same LAN. Higher throughput, bigger models. |
| **BundleRack** | Datacenter: multi-GPU server or small cluster. Low latency, high reliability. |
| **InstanceMarket** | Vast-style: rentable capacity. SLA-critical, burst capacity. |
| **ReservePool** | OpenAgents-controlled capacity. Guaranteed fills, training wheels. |

---

## Status Terms

| Term | Definition |
|------|------------|
| **Implemented** | Code exists AND is wired to production paths. |
| **In code** | Struct exists but not yet wired to production. |
| **Spec only** | Documented specification, code not yet written. |
| **Wave** | Implementation phase. Wave status tracks struct existence + unit tests. |
| **MVP ready** | Wired into production path, not just struct existence. |

Status legend used in docs:
- 🟢 **Implemented**: Code exists, tests pass
- 🟡 **In Progress**: Active development
- 🔵 **Specified**: Protocol/types defined, not yet wired
- ⚪ **Planned**: Roadmap item, design incomplete

---

## Naming Collisions (Resolved)

| Collision | Resolution |
|-----------|------------|
| `step_utility` (0..1 vs -1..+1) | Canonical is **-1.0..+1.0** from `ToolResultSignature`. `ToolStepUtilitySignature` outputs 0..1 and should be normalized or renamed to `step_utility_norm`. |
| `policy_version` vs `policy_bundle_id` | Canonical is **policy_bundle_id**. `policy_version` may be used as display metadata derived from bundle. |
| `rlog` vs `trajectory` vs `REPLAY.jsonl` | `rlog` and `trajectory` are conceptual terms for session logs. `ReplayBundle` is current implementation. `REPLAY.jsonl v1` is target interoperable format. |

---

## See Also

- [PAPER.md](PAPER.md) - Technical systems paper
- [SYNTHESIS.md](SYNTHESIS.md) - Vision and strategy document
- [SYNTHESIS_EXECUTION.md](SYNTHESIS_EXECUTION.md) - Implementation status and practical guide
- [ROADMAP.md](ROADMAP.md) - Development roadmap with MVP gates
- [crates/dsrs/docs/ARCHITECTURE.md](crates/dsrs/docs/ARCHITECTURE.md) - DSPy core traits and runtime layers
- [crates/dsrs/docs/SIGNATURES.md](crates/dsrs/docs/SIGNATURES.md) - Signature specifications
- [crates/dsrs/docs/REPLAY.md](crates/dsrs/docs/REPLAY.md) - REPLAY.jsonl format specification
- [crates/dsrs/docs/ARTIFACTS.md](crates/dsrs/docs/ARTIFACTS.md) - MVP artifact schemas
- [docs/PROTOCOL_SURFACE.md](docs/PROTOCOL_SURFACE.md) - Canonical protocol details
