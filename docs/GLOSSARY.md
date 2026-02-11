# Glossary

- **Status:** Accurate
- **Last verified:** (see commit)
- **Source of truth:** This document is the canonical vocabulary reference for all OpenAgents documentation
- **Doc owner:** root
- **Conflict rules:**
  - If *terminology* conflicts across docs, **GLOSSARY wins**.
  - If *behavior/implementation details* conflict with code, **code wins**.

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
| **Cashu Proof** | A blind-signed token ("coin") redeemable at a Cashu mint. Avoid generic "proof" which collides with other meanings. |
| **Reconciliation** | Background process resolving pending quotes, expiring reservations, and repairing state after crashes. |

---

## Treasury & Exchange

| Term | Definition |
|------|------------|
| **Neobank** | Programmable treasury layer for agents. Not a bank but a payments router with budget enforcement, multi-rail support, and audit trails. |
| **TreasuryRouter** | Policy engine deciding payment routing: which rail, which asset, which limits, which approvals. |
| **Exchange** | Agent-to-agent marketplace for FX (BTC↔USD), liquidity swaps, and payment routing services. |
| **FX Quote** | Price + expiry + settlement instructions for an asset swap. Distinct from payment **Quote** (which is a payment intent state machine). |
| **Treasury Agent** | Specialized agent providing financial services to the network. |

---

## Nostr Protocols

> **Note:** Kind numbers are illustrative examples. Schema IDs (e.g., `oa.code_chunk_analysis.v1`) are canonical identifiers for job types. Rust-era protocol mapping docs were archived to backroom during the 2026-02-11 deprecation.

| Term | Definition |
|------|------------|
| **NIP-90** | Data Vending Machine protocol. Defines job request/result flow. |
| **NIP-90 Job Request** | Job submission event (e.g., kind:5050 for text generation). |
| **NIP-90 Job Result** | Job completion event (e.g., kind:6050 for text generation). |
| **NIP-90 Job Feedback** | Invoice/status event (kind:7000). Used for all job types. |
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
| **Provider** | Backend that runs inference or compute. See subtypes below. |
| **Inference Provider** | Model backend that runs LLM inference (Ollama, llama.cpp, Codex, GPT-OSS, etc.). |
| **Compute Provider (DVM)** | NIP-90 responder that fulfills jobs (sandbox execution, embeddings, etc.). |
| **Policy Bundle** | Versioned compilation of signatures/modules/routing thresholds. Contains instructions, demos, optimizer config. |
| **Forge Adapter** | Integration layer that maps Verified Patch Bundles to a target collaboration surface (GitAfter, GitHub, git, NIP-34). Handles PR creation, branch management, and platform-specific operations. *(Not a DSPy Adapter; this is a forge integration surface.)* |

---

## Execution Concepts

| Term | Definition |
|------|------------|
| **Execution Runtime** | The layer that validates tool params, enforces retries, and runs tools. Distinct from adapters. |
| **Lane** | Named routing bucket for inference/execution. Standard *classes* are **Local** (free, on-device), **Cloud** (hosted API), **Swarm** (NIP-90 marketplace). Implementations may expose additional lane names (e.g., `cheap`, `fast`, `premium`) that map onto these classes. Note: "Datacenter" in supply class docs refers to Cloud lanes. |
| **Dispatcher** | Component that sends NIP-90 jobs to the swarm (e.g., SwarmDispatcher). |
| **DelegationTarget** | Where to route a task. Canonical enum: `local_tools` (simple edits), `rlm` (recursive analysis), `codex` (complex multi-file), `swarm_fanout` (parallel provider queries), `objective_job` (sandboxed verifiable jobs like tests/builds). |
| **step_utility** | Learning signal for a tool call. Canonical range: **-1.0..+1.0** (from `ToolResultSignature`). |
| **verification_delta** | Change in failing tests: `tests_before - tests_after`. Positive = improvement. |
| **PlanIR** | Canonical intermediate representation for plans. Emitted by both Adjutant and Autopilot planners. |
| **Autonomy Level** | Supervision mode: Supervised (human approves each action), Semi-autonomous (human approves high-risk), Autonomous (agent decides). |

---

## Context & Long-Context

| Term | Definition |
|------|------------|
| **Context rot** | A context failure mode where model quality degrades as the *tokenized prompt* grows past a model's soft limit. Not a hard context window overflow: the model still produces outputs, but accuracy and coherence drop in ways that can be hard to notice in long-running agents. |
| **Soft context limit** | An empirical threshold (in prompt tokens and prompt composition) after which quality begins to degrade. Typically far below a model's advertised maximum context window. Used for routing and safety leashes. |
| **Context pressure** | A runtime estimate of "how risky it is to keep adding tokenized context" (e.g., based on prompt tokens, retrieved snippet size/count, tool log bloat). Used to trigger strategy changes (RLM, retrieval, compaction) before context rot sets in. |
| **Token space** | The model's in-window prompt: the tokenized messages sent to the LLM. Subject to context limits and soft-limit quality effects. |
| **Variable space / programmatic context** | External state not directly in the prompt (repo snapshots, logs, large blobs, indexes) referenced by handles/variables and accessed via operations (read/grep/chunk/peek). The core RLM trick is controlling what moves from variable space into token space. |
| **Context ops** | Operations over external context (peek/read_lines/grep/chunk/symbols/summarize). In OpenAgents these are tool calls: schema-validated, metered/budgeted, and logged with receipts/provenance. |
| **BlobRef** | Content-addressed reference to a large blob stored outside token space (e.g., in a BlobStore). Enables prompts/traces to carry stable handles rather than duplicating large text. |
| **VarSpace** | Named variable store used by RLM strategies. Maps variable names to small JSON values or BlobRefs (and derived artifacts), enabling long-context work without stuffing everything into token space. |
| **Symbolic recursion** | Code-driven recursion/fanout where the executor/kernel generates O(N) sub-queries over chunks/fragments; the model does not need to "write" O(N) recursive calls inside its own output. Key for scaling to very large contexts. |
| **RLM (Recursive Language Model)** | Inference-time execution mode for long contexts: maintain variable space + token space, run an iterative controller loop (REPL/action DSL/kernel ops), and optionally use sub-model calls. The goal is to mitigate context rot by keeping token space small while still operating over huge inputs. |
| **FRLM (Federated RLM)** | RLM that federates sub-queries across multiple venues/backends (local, cloud APIs, swarm/NIP-90), bounded by explicit budgets and logged with receipts. |
| **Trace mining** | Post-hoc analysis of traces (REPLAY events, receipts, per-iteration RLM events) to extract repeating tactics and distill them into explicit signatures/modules/graphs that are faster and more reliable than an exploratory RLM loop. |

---

## Product & Commercial Terms

| Term | Definition |
|------|------------|
| **Autonomy-as-a-Service (AaaS)** | Bounded, auditable delegation sold as a contracted outcome over time. Defined by scope, horizon, constraints, verification, and escalation behavior. |
| **Predictable Autonomy** | The product promise of AaaS: reliable throughput with known failure modes and objective verification, not just “model output.” |
| **Autonomy SLA** | The explicit service contract for predictable autonomy (scope + horizon + constraints + verification + escalation). |
| **Contracted Autonomy** | Synonym for AaaS emphasizing the outcome contract rather than the model. |
| **Guaranteed Delegation** | Informal label for AaaS emphasizing risk transfer and operator responsibility. |
| **Outcome Ops** | Informal label for the operational layer that prices and verifies autonomy by outcomes and receipts. |

---

## Artifacts & Trajectories

| Term | Definition |
|------|------------|
| **Verified Patch Bundle** | The canonical output of an agent session: a human-readable patch summary, a machine-verifiable receipt, and a replay log—independent of any specific forge or workflow. Files: `PR_SUMMARY.md`, `RECEIPT.json`, `REPLAY.jsonl`. |
| **Patch Summary** | Human-readable summary of what changed, verification results, and confidence. File: `PR_SUMMARY.md` (filename kept for tooling stability). |
| **RECEIPT.json** | Machine-readable receipt with hashes, tool calls, verification, and policy_bundle_id. |
| **REPLAY.jsonl** | Canonical event stream format target for replay/debugging in OpenAgents docs. |
| **ReplayBundle** | Historical format used by the deprecated Rust runtime (archived to backroom). |
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
| **sAPM** | Success-adjusted APM. Formula: `APM × 1{verified_success}`. For objective jobs: `verified_success` = verification harness passes (all exit codes 0). For subjective jobs: use separate metric or N/A. Measures productive velocity rather than raw speed. |

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

## UI Runtime

| Term | Definition |
|------|------------|
| **UITree** | Flat UI tree representation with `root` and `elements` used for Effuse dynamic UI. |
| **UiPatch** | JSON patch operation (`add/remove/replace/set`) applied to a UITree using JSON Pointer paths. |
| **Effuse Catalog** | Whitelisted component/action definitions + schemas used to validate UITree output. |

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

ROADMAP.md uses execution-focused emoji (✅/🔄/⏳). Rough mapping:
- ✅ ≈ Implemented (shipped/done)
- 🔄 ≈ In Progress (active work)
- ⏳ ≈ Planned/Not started

---

## Naming Collisions (Resolved)

| Collision | Resolution |
|-----------|------------|
| `step_utility` vs `step_utility_norm` | Canonical `step_utility` is **-1.0..+1.0** from `ToolResultSignature`. `ToolStepUtilitySignature` outputs `step_utility_norm` in range **0.0..1.0** (the `_norm` suffix distinguishes it). |
| `policy_version` vs `policy_bundle_id` | Canonical is **policy_bundle_id**. `policy_version` may be used as display metadata derived from bundle. |
| `RLM` (Recursive Language Models) vs `rlm` (reward-signal evaluation crate in external runtimes) | In OpenAgents docs, `RLM` means **Recursive Language Models** (long-context execution). If referencing a reward-signal evaluator crate named `rlm` (e.g., in Horizons), call it **reward-signal evaluation (`rlm` crate)** to avoid acronym collision. |
| `rlog` vs `trajectory` vs `REPLAY.jsonl` | `rlog` and `trajectory` are conceptual terms for session logs. `ReplayBundle` is current implementation. `REPLAY.jsonl v1` is target interoperable format. |

---

## See Also

- [README.md](README.md) — active docs index
- [PROJECT_OVERVIEW.md](PROJECT_OVERVIEW.md) — current repository map
- [ROADMAP.md](ROADMAP.md) — active roadmap
- [MOLTBOOK.md](MOLTBOOK.md) — social policy and operations
- [RUST_DOCS_ARCHIVE_2026-02-11.md](RUST_DOCS_ARCHIVE_2026-02-11.md) — archive locations for deprecated Rust docs
