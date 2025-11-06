# ADR 0006 — Use Apple Foundation Models for On‑Device Intelligence

- Date: 2025-11-04
- Status: Accepted — Standardization (Phase 1)

## Context

Building on ADR‑0003 (Swift cross‑platform app) and ADR‑0004 (Liquid Glass adoption), we want to leverage Apple's on‑device Foundation Models framework to enhance the native Swift app with fast, private intelligence tasks. These models power Apple Intelligence on supported devices (iOS/iPadOS 26, macOS 15+) and enable low‑latency language understanding and generation without network calls.

Initial target tasks:
- Conversation titles: generate concise 3–5 word summaries for threads shown in the sidebar.
- Short summaries/snippets for previews (e.g., first message or latest answer in a thread).
- Lightweight classifications and tags (e.g., work/personal, provider source, topical tags) for local organization.

Constraints and guardrails:
- Availability varies by device and settings; users can disable Apple Intelligence or the model may be downloading. We must always provide a graceful fallback.
- Acceptable‑use requirements must be followed (see Apple’s policy). On‑device usage only; no sensitive logging of raw prompts/outputs beyond what’s needed for product diagnostics.
- Maintain ACP alignment: results are decorations on top of ACP/Tinyvex data, not a new contract.

## Decision

Adopt Foundation Models in the Swift app for on‑device, best‑effort intelligence with strict fallbacks:

- Availability
  - Probe `SystemLanguageModel.default.availability` on startup and log one line for diagnostics.
  - Gated by OS (26+) and Apple Intelligence settings; treat `.modelNotReady` as temporary.

- Sessions and prompts
  - Use `LanguageModelSession(model:tools:instructions:)` with concise instructions tailored to the task (e.g., “Return 3–5 plain words, no punctuation”).
  - Keep temperature low and prompts short; split large contexts if needed.
  - Prewarm sessions when convenient to reduce latency.

- Fallbacks
  - If unavailable, task fails, or safety guardrails trigger, fall back to deterministic local logic (e.g., take first real user message ~5 words) so UX remains responsive.
  - Do not block UI on model readiness.

- Privacy and safety
  - Never send content off device via FM usage.
  - Respect guardrails; if tripped, drop back to local logic (no retries with different wording unless UX requires it).
  - Provide a feedback path by logging a compact marker and optionally exposing Apple’s feedback attachment API in a diagnostics pathway (not user‑facing by default).

- Persistence
  - Cache derived artifacts (e.g., generated titles) keyed by source path + mtime to avoid recomputation and keep results stable across launches.
  - Keep caches local to the app sandbox; allow invalidation on file changes.

## Rationale

- Latency and privacy: on‑device models avoid network, providing quick, private summaries/tags suitable for background decoration.
- Quality: for short summaries and tags, the on‑device model provides better phrasing than simple heuristics.
- Robustness: deterministic local fallbacks keep the UI responsive on unsupported devices or when models are not ready.

## Scope

- In‑scope (Phase 1): conversation titles, short preview snippets, lightweight tags; developer diagnostics (availability log).
- Out‑of‑scope (Phase 1): tool calling, structured generation with @Generable (evaluate later), and invasive UI depending on FM output.

## Implementation Plan

1) Titles (done)
   - Implement `FoundationModelSummarizer` with availability checks, prewarm, low‑temperature options, and strict output constraints.
   - Add safe parsing of `LanguageModelSession.Response` (no KVC); treat guardrail responses as failure; fall back to first‑user‑five‑words.
   - Persist titles by (file path + mtime). Log `[FM] availability`, `[Summary] used=…`, and `[Titles] …` in development.

2) Previews and tags (next)
   - Add preview generation (one‑line snippet) with the same availability + fallback pattern.
   - Add lightweight classification/tags; store alongside titles in a per‑file cache; expose non‑intrusive badges in the UI.

3) Structured output (later)
   - Evaluate `@Generable` + `GenerationOptions` for structured tags or categorization.
   - Add unit tests for schema‑guided responses.

4) Safety + diagnostics
   - Gate any extended logging or attachment export behind a developer setting. Default to minimal logs only.
   - Add an optional diagnostics screen to show availability and last FM results (development builds only).

## Consequences

- Positive: better titles/labels with low overhead; consistent local fallback keeps UI snappy.
- Neutral: additional code paths and tests; minor storage for caches.
- Negative: FM availability variability may cause different results across devices; we mitigate with fallbacks and caching.

## Acceptance

- Titles are generated on supported devices using FM, with deterministic fallback otherwise; no crashes when FM is unavailable or when guardrails trigger.
- A local cache persists titles keyed by file path + mtime.
- Unit tests cover FM response parsing and fallback selection; development logs indicate availability and path used.
- No changes to ACP/Tinyvex contracts or bridge behavior.

## References

- ADR‑0003 — Swift Cross‑Platform App (macOS + iOS)
- ADR‑0004 — Adopt Liquid Glass for Apple Platforms
- Apple Docs: Foundation Models, SystemLanguageModel, LanguageModelSession, GenerationOptions, Instructions
- Acceptable use requirements for the Foundation Models framework
