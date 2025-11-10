# GPT‑OSS Next Steps (2025‑11‑10)

This note synthesizes research.md and claudeplan.md into a concrete, short list to finish GPT‑OSS Phase 1–2 and de‑risk user‑visible issues observed today.

What we just fixed
- Byte‑based download progress with explicit link + file list logging.
- Pause no longer yields false “Ready”; token‑gated completion.
- Detect installed model on app appear; compute size recursively.
- Prefer GPT‑OSS as default provider when installed (server‑side fallback).

Immediate next work (Phase 2)
- Streaming to ACP (Issue 004):
  - Use `ChatSession.streamResponse` → map to `agentMessageChunk`.
  - Harmony: suppress `analysis` channel, pass through `commentary` preambles.
  - Wire cancel to terminate the stream immediately.
- Provider status + settings:
  - Expose model state (notLoaded/loading/ready/error) via a lightweight RPC for the UI card.
  - Add unload button; display memory and disk info.
- Routing from FM:
  - Add `gptoss.generate` tool in OpenAgentsLocalProvider.
  - Heuristics: prefer GPT‑OSS when token budget > 140 or when code generation requested; log routing decision.

Stability and UX hardening
- Resume downloads across app restarts (Hub already resumable; persist progress token/id).
- Surface offline‑mode errors clearly (log HF_HUB_OFFLINE/TRANSFORMERS_OFFLINE states to UI).
- Coalesce duplicate progress logs; avoid 0% flicker on cache hit by setting 100% immediately.

Docs and tests
- Golden test: Assert Harmony template prefix tokens for a trivial chat.
- Streaming test: Ensure only `final` is surfaced; `analysis` hidden.
- Update gptoss‑integration‑spec with the default‑provider behavior and byte‑logging expectations.

Nice‑to‑have (post‑MVP)
- Multi‑model picker (20B MXFP4 vs smaller variants) with memory gating.
- Background warmup and auto‑unload on idle.
- Vector‑aware RAG path once SearchKit is wired to EmbeddingService.

Files touched today
- ios/OpenAgentsCore/Sources/OpenAgentsCore/GPTOSS/GPTOSSModelManager.swift
- ios/OpenAgents/Views/macOS/GPTOSS/GPTOSSDownloadViewModel.swift
- ios/OpenAgents/Views/macOS/GPTOSS/GPTOSSStatusCard.swift
- ios/OpenAgentsCore/Sources/OpenAgentsCore/DesktopBridge/DesktopWebSocketServer.swift
- ios/OpenAgentsCore/Sources/OpenAgentsCore/DesktopBridge/DesktopWebSocketServer+Session.swift

