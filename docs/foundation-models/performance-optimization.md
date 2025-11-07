#+ Performance Optimization (On‑Device FM)

Practical steps to improve latency and throughput when using Foundation Models.

## Session Lifecycle

- Reuse a persistent `LanguageModelSession` per conversation/workspace
- Avoid recreating sessions unless switching tasks/domains

## Prewarm Strategically

- Call `session.prewarm(promptPrefix:)` when screen appears or typing begins
- Pass a common prefix if you have predictable context to prime tokenization

## Stream When It Helps UX

- Prefer streaming for long outputs to reduce time‑to‑first‑content
- Bind snapshot fields directly to SwiftUI; avoid manual accumulation

## Manage Context

- Summarize older transcript segments; apply a sliding window of recent turns
- Keep instructions concise; move format enforcement into `@Generable` when possible

## Tool Efficiency

- Enforce timeouts and result caps in tools; return compact, model‑friendly outputs
- Cache deterministic tool results within a short horizon (e.g., file lists)

## OpenAgents Guidance

- ExploreOrchestrator should own a single session, serialize calls, prewarm before plan/analysis
- For heavy work, stream and forward partials via ACP; compose a final message at completion

