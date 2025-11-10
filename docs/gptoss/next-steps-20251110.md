# GPT‑OSS 20B — Next Steps, Gaps, and Revisions

Last updated: 2025‑11‑10

This note synthesizes the GPT‑OSS research plan and the Claude integration plan into a concrete, low‑risk path to ship GPT‑OSS 20B (MXFP4) as a native agent provider on macOS. It flags gaps, proposes revisions, and orders work against current open issues.

## What’s Solid
- Model choice and format: `mlx-community/gpt-oss-20b-MXFP4-Q8` with Harmony chat template (tokenizer‑driven) is correct and realistic for ≥16 GB Macs.
- Loader stack: MLXLLM + MLXLMCommon for inference; Hugging Face `Hub.snapshot` for resumable, verifiable downloads.
- Provider shape: Mirroring the embeddings provider (actor, availability, load/unload) fits our architecture and concurrency model.
- Delegation pattern: FM orchestrator → tool → local server path is aligned with #1469 and existing Codex/Claude flows.

## Gaps To Close
1) Tokenizer + Harmony enforcement
   - Ensure we never bypass the tokenizer’s chat template. If we must construct prompts, add an explicit Harmony template adapter and unit tests.

2) Download/Verification UX
   - We should persist a manifest (files, sizes, checksums) and surface integrity state in Settings. Resume behavior should be visible (percent, ETA).

3) Memory headroom guardrails
   - Add preflight checks (total and available memory) with actionable guidance. Auto‑unload on idle; show memory watermark while loaded.

4) Cancellation + backpressure
   - Streaming must support user cancel mid‑generation and propagate to MLX session cleanly (no dangling tasks). Backpressure on the UI stream to avoid buffer growth.

5) Routing rubric (FM → GPT‑OSS)
   - Today’s plan says “heavy reasoning → GPT‑OSS” but lacks concrete thresholds. Add heuristics (token budget estimate, tool count forecast, file count) and log routing decisions for tuning.

6) Safety/policy surfaces
   - Add a simple policy gate: GPT‑OSS available only on macOS and when user confirms model download (explicit license acknowledgement).

7) Telemetry for dev builds
   - In dev builds, collect anonymous metrics: first‑token latency, tokens/sec, memory peak; write to a local log for profiling, no network.

## Proposed Revisions (Docs/Plan)
- In docs/gptoss/research.md:
  - Add a small “Harmony Compliance” section with a one‑liner: “All chat requests must be created via the tokenizer’s chat template; do not hand‑roll prompts.” Include a test snippet (golden prompt → expected token IDs prefix).
- In docs/gptoss/claudeplan.md:
  - Under “Task Allocation Matrix,” add a routing rubric appendix with concrete heuristics and a telemetry flag to evaluate decisions.
  - Add a brief interface contract for `GPTOSSAgentProvider` (methods, availability states, unload policy), mirroring the embeddings provider table.

## Execution Order (2–3 weeks, macOS only)
1) Dependencies + Loader (P0)
   - Add SPM deps: MLXLLM, MLXLMCommon, Tokenizers; confirm minimal example builds.
   - Create GPTOSSModelManager (actor): download (Hub.snapshot), verify, list, delete.

2) Provider + Streaming (P0)
   - Implement `GPTOSSAgentProvider` (actor): load/unload, `startSession()`, `stream(...)`.
   - Convert `ChatSession.streamResponse(...)` → ACP `agent_message_chunk` updates with cancel support.

3) Registry + Mode (P0)
   - Add `ACPSessionModeId.gptoss_20b`; register provider; `session/set_mode` path.
   - isAvailable(): platform + memory checks.

4) Settings + Install UI (P1)
   - Model panel: “Install GPT‑OSS 20B (12.1 GB)”; show license, disk/memory checks, progress/ETA, verify, ready state, unload button.

5) FM Delegation Tool (P1)
   - Add `gptoss.generate` tool; follow `codex.run` pattern in #1469 (FM model‑initiated). Log routing decisions with brief reason strings.

6) Tests + Profiling (P1)
   - Unit: tokenizer template used; model load/unload; cancel path.
   - Integration: FM tool → provider streaming → ACP UI.
   - Perf: first‑token latency target <2s on M1/M2 (baseline), tokens/sec measurement; memory watermark <= configured bound; unload frees memory.

## Interfaces (sketch)
```swift
public actor GPTOSSAgentProvider: AgentProvider {
    public enum Availability { case unavailable(reason: String), downloading(Double), ready }
    public func load(modelURL: URL?) async throws
    public func unload() async
    public func stream(prompt: String, onChunk: @Sendable (String) -> Void, onDone: @Sendable () -> Void) async throws
    public var availability: Availability { get }
}
```

## Risks & Mitigations
- First‑time download failures → Retry with exponential backoff; persist partial state; surface disk errors clearly.
- Memory pressure on 16 GB Macs → preflight checks; unload on idle; let user opt for lower‑RAM alternatives (turn off reranking/tools during GPT‑OSS runs).
- Tokenizer mismatches → pin tokenizer files in manifest; add golden tests for chat template.

## Alignment with Open Issues
- #1469 (FM codex.run wiring): Implement `gptoss.generate` in parallel using the same FM→server tool path and streaming adapter.
- #1468/#1467 (Embeddings): No hard dependency, but MLX stack and download UX are shared—reuse the Hub snapshot and progress UI patterns.

## Definition of Done (MVP)
- macOS can install, verify, and unload GPT‑OSS 20B.
- User can pick GPT‑OSS mode and stream responses; cancel works.
- FM can delegate via `gptoss.generate` with visible tool_call and streamed agent output.
- Harmony compliance guaranteed; tests pass; basic perf targets documented.

