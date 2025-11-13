# Local‑First “Cursor” Design: Bringing Cloud Dependence Near Zero

Goal: Design a Cursor‑class coding assistant that maximizes local compute and minimizes cloud reliance (ideally fully offline). This summarizes architecture, components, trade‑offs, and a phased path to an MVP.

## Principles
- Local by default: No network required after initial install (models, extensions, indexes packaged or fetched once and cached).
- Privacy and control: Never transmit source, prompts, or telemetry without explicit opt‑in.
- Practical performance: Sub‑second tab completions, interactive chat/edits, and background indexing on commodity Apple Silicon.
- Modular: Swap model backends, embeddings, and vector stores without changing the editor.

## Core Stack Options
- Shell app
  - VS Code fork (keeps extension ecosystem) or
  - Tauri + Monaco editor (smaller footprint, first‑party control) with Rust backend.
- Local inference backends
  - llama.cpp (GGUF; Metal backend on macOS) for Llama/Qwen/Mistral/DeepSeek‑Coder variants.
  - MLX (Apple) or MLC LLM as alternative Apple‑Silicon‑optimized runtimes.
  - Ollama or LocalAI as a local multi‑model orchestrator (simple lifecycle, model mgmt, HTTP API).
- Language servers (LSPs)
  - Reuse upstream LSPs for diagnostics, hover, go‑to‑def, and as validators in a shadow workspace.

## Models: Roles and Feasible Sizes
- Realtime completions (Tab)
  - 1–3B “tiny code” models for latency; quantized Q4/Q5; draft/speculative decoding to hit <50–120 ms tokens.
- Edits + multi‑file refactors
  - 7–8B code models for quality, Q4_K_M quantization; allow async “fast apply” planning then precise patching.
- Chat/Reasoning
  - 7–13B general or code‑tuned models for reliable chain‑of‑thought/tool use; optional 32k context builds.
- Embeddings
  - Small ONNX embedding models (e5‑small‑v2, bge‑small) for speed; optional larger bge‑large reranker.

Notes: 70B‑class is unrealistic fully local on typical laptops. Prefer two‑tier strategy (tiny for realtime, mid‑size for quality) with speculative decoding (tiny drafts → mid verifies).

## Inference Runtimes and Acceleration
- llama.cpp (recommended baseline): Metal backend; draft spec decoding; KV cache reuse; continuous batching for completions.
- MLX (Apple): Swift/Python APIs; very strong M‑series perf; packaged models via mlx‑community.
- MLC LLM: WebGPU/native backends; portable deployments, good for cross‑platform.
- Whisper.cpp (optional): local speech‑to‑text.

Packaging: ship curated GGUF/MLX models with license checks and per‑platform builds.

## Retrieval, Indexing, and Local Stores
- File scanning + hashing
  - Rust notify + content hashing (BLAKE3) for incremental updates. Merkle‑style index state to avoid rescans.
- Embeddings
  - ONNX Runtime (with Metal EP) for e5/bge; batch inference; memory‑mapped models.
- Vector store
  - sqlite‑vec for “just SQLite” deployments, or Qdrant embedded for higher throughput; both fully local.
- Reranking
  - ONNX cross‑encoders (bge‑reranker) for better top‑k quality with small batch sizes.
- Full‑text fallback
  - SQLite FTS5 or Tantivy for fast literal/regex search; integrate ripgrep for CLI parity.

## Shadow Workspace (Fully Local)
- Snapshot strategy
  - APFS clonefile (copy‑on‑write) or per‑OS reflinks to duplicate workspace fast.
  - Alternate: overlay/union FS or per‑project temp dir with sparse copy.
- Validation
  - Run LSPs against shadow; collect diagnostics; compute diff risk score.
- Test hooks (optional)
  - Run selected tests/linters in sandbox, capture failures, feed back into prompts.

## Sandboxing and Safety
- macOS: run tasks under a restricted child process (seatbelt entitlements with hardened runtime if shipping; or launchd job with limited permissions). Avoid deprecated sandbox‑exec.
- Linux: bubblewrap/Firejail; Windows: Defender Application Control / AppContainer where possible.
- Policy engine: all tools declare access (FS/network/process); default‑deny with user prompts.

## Completion and Edit UX
- VS Code fork path
  - Implement CompletionItemProvider hooking Monaco/VS Code suggest; stream tokens; fallback to LSP ghosts.
  - Inline diff UI for “accept/inspect/apply” with tree‑sitter‑aware patching.
- Tauri path
  - Monaco editor; same providers; assistant sidecar UI for chat and “fast apply”; stream via SSE from local runtime.
- Speculative decoding pipeline
  - Tiny draft model generates; mid‑size model verifies/corrects; adopt verified prefix policy.

## Tooling (Offline)
- Built‑in tools
  - Search, summarize selection, explain diagnostics, generate tests, run tests, run script (guarded).
- Code execution
  - Project runners via adapters (Node, Python, Go, Rust) in sandbox; cap CPU/mem; collect logs.
- Data privacy
  - Tools cannot network by default; require explicit “allow network” setting per project.

## Telemetry and Metrics (Local)
- Off by default. If enabled, write to local files only (human‑readable JSONL). Optional export for bug reports.
- Dev profiling: OTel SDK with a local file exporter for spans; ASCII flamegraphs; no external collectors.

## Updates and Marketplace (Offline)
- Extensions
  - Offline gallery mirror: allow importing VSIXs from disk; optional LAN mirror.
- Models
  - Local model registry (folder); allow manual drop‑in; verify checksums/signatures; never auto‑fetch without consent.

## Packaging & Distribution
- Tauri bundles: very small footprints; native perf; auto‑updates disabled by default; signed installers.
- VS Code fork: heavier bundle but keeps ecosystem; ship with updates disabled unless opted‑in.

## Performance Targets (Apple Silicon guidance)
- M2 (16 GB):
  - Completions (1–3B Q4): <100 ms avg token at 128 ctx; ~1.5–2.5 tok/ms with draft.
  - Edits/Chat (7–8B Q4_K_M): interactive at 10–25 tok/s; context 8k–16k.
- Indexing: 100k LOC initial in <60s; steady‑state <500ms per changed file.

## Phased Implementation Plan
1) MVP (4–6 weeks)
- Editor integration (Monaco or VS Code fork); local runtime process.
- Local completions via llama.cpp tiny model; basic settings UI.
- Local embeddings + sqlite‑vec; background indexer; simple RAG for chat.
- Inline edit/apply with diff preview; no shadow workspace yet.

2) Quality + Reliability (6–10 weeks)
- Add mid‑size model and speculative decoding path.
- Shadow workspace with APFS clones + LSP validation loop.
- ONNX reranker; improved chunking; tree‑sitter‑aware edits.
- Sandboxed test/lint runner with caps.

3) Enterprise & Polishing (ongoing)
- Model registry UI (manual add, checksum verify).
- Offline VSIX management (import/export); optional LAN mirror.
- Local metrics opt‑in; redaction & secure export.

## Risks and Mitigations
- Quality gap vs cloud frontier models
  - Mitigate with multi‑model cascade (tiny+mid), reranking, and domain‑specific prompts. Offer optional “bring your own LAN server” for high‑end GPUs.
- Memory/thermal pressure
  - Aggressive quantization; unload idle models; shared KV cache; user‑selectable model sizes.
- Licensing constraints
  - Prefer Apache‑2.0/BSD/MIT models; if using Llama/Qwen/Mistral, honor their licenses and distribution rules.
- Model updates
  - Explicit user pull; signed artifacts; rollbacks stored locally.

## Suggested Components (by task)
- Inference: llama.cpp (Metal), MLX, or Ollama orchestrator.
- Embeddings: ONNX Runtime + e5/bge; Metal EP on macOS.
- Vector DB: sqlite‑vec (simple) or embedded Qdrant (faster ANN).
- FTS: SQLite FTS5 or Tantivy.
- Parsing: tree‑sitter for structural edits.
- Editor: Monaco (Tauri) or VS Code fork.
- FS Watch: notify (Rust) or chokidar (JS) with hashing.
- Sandboxes: bubblewrap/Firejail (Linux), seatbelt/hardened child processes (macOS), WDAC/AppContainer (Windows).

Outcome: A private, offline‑capable coding assistant with competitive UX for completions/edits and robust local indexing, that can scale up quality with a mid‑size code model while keeping cloud usage at zero by default.

