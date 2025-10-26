Engineering Audit — 2025-10-26

Scope
- Assesses current codebase quality (Bridge, Desktop/Tauri, Mobile/Expo, Convex functions) with respect to our product goals: reliable live streaming, cross‑device sync, offline‑friendly desktop, and maintainability.
- Informed by recent work (streaming refactor, watchers, Tauri split), observed logs/issues (#1318, #1320), and current repo state.

Top Risks (Prioritized)
- Legacy code lingering in Bridge main.rs
  - Unused helpers and imports in `crates/codex-bridge/src/main.rs` create noise and risk diverging behavior.
  - Action: prune or gate behind feature flags; move any remaining helpers into dedicated modules.

- Partial tool-row coverage and tests
  - We added mapping for `command_execution`, `file_change`, `web_search`, `mcp_tool_call`, `todo_list`, but tests are light.
  - Action: add ws forwarder tests with a fake Convex client trait to assert mutations (cmd/file/search/mcp/todo paths).

- Watchers churn and WS reset logs
  - Rapid re-subscriptions and filesystem bursts log “WebSocket protocol error” and can thrash.
  - Action: add explicit unsubscribe/debounce; consider a subscription manager that reuses connections per thread.

- Streaming semantics consistency
  - Upsert/finalize ordering across assistant/reasoning is working but lacks sequence/ordering tests and backoff in Convex calls.
  - Action: unit tests for sequence monotonicity and finalize fallback; consider server-side debouncing.

- Desktop bootstrap fragility
  - Sidecar discovery and fallback paths are permissive; failures can be silent.
  - Action: centralize sidecar resolution, surface errors in UI (toast/status panel), and cache last-known-good path.

Architectural Improvements
- Bridge module hygiene
  - Good: recent split into `ws.rs`, `codex_runner.rs`, `convex_write.rs`, `watchers.rs`, `controls.rs`, `state.rs`, `bootstrap.rs`, `util.rs`.
  - Improve:
    - Remove legacy functions from main.rs and prefer the modular equivalents.
    - Extract a small trait over `ConvexClient` for easier mocking in unit tests.
    - Gate expensive paths (e.g., history import) behind feature flags or explicit WS controls.

- Data model clarity
  - Messages schema now includes `itemId`, `partial`, `seq`. Ensure all writers set these consistently and that readers tolerate absence.
  - Add indexes that match access patterns (already using `by_thread_ts`, `by_thread_item`). Confirm index presence on deploy.

- Control plane hardening (WS)
  - Validate payloads rigorously (current parser is tolerant). Reject multi-line or mixed payloads with a clear error message broadcast.
  - Document `run.submit` contract in `docs/sync.md` (fields, defaults, how `resume` interacts with `threadDocId`).

- Desktop/Tauri lifecycle
  - Consolidate emitting of `bridge:ready` and `convex:local_status` and render a single status indicator.
  - Add an “Advanced” panel with sidecar path, port, and quick actions (restart, tail logs) for supportability.

- Mobile/Expo parity
  - Ensure component parity with desktop (badges, composer autofocus, tool row renderers); extract shared view logic where feasible.
  - Add a minimal provider boundary to share message hide rules across apps.

Testing & Quality
- Unit
  - Controls: malformed JSON and missing-key coverage (added; extend with type mismatch matrix).
  - WS forwarder: fake Convex client to assert upsert/finalize and tool rows (pending).
  - Watchers: isolate debounce logic and event coalescing for deterministic tests.
  - Mapping: robust tests for numeric coercions (i64/f64) and optional keys.

- Integration
  - Feature-gated tests that feed a temp JSONL file into the sessions tailer and assert Convex mutations.
  - End-to-end smoke: start the bridge with a mock Codex producer that emits a curated JSONL stream; assert Convex rows.

- Coverage
  - Add Cargo Tarpaulin in CI for `crates/*` and `tauri/src-tauri` (exclude wasm/desktop UI).
  - Target 80%+ for newly added modules; focus on `ws.rs`, `convex_write.rs`, `watchers.rs`, and `controls.rs`.

Performance & Resilience
- Convex write volume
  - Streaming deltas can be chatty; we compact logs in the console, but Convex calls can still spike.
  - Action: batch or debounce deltas server-side; optionally coalesce on the client by skipping unchanged seq.

- Backpressure & timeouts
  - Logs show occasional “Query execution time out: 16s”. Debounce subscriptions and avoid creating multiple concurrent clients per thread.
  - Action: single subscription owner per thread; reuse client; add retry with jitter.

- Watchers
  - Debounce filesystem bursts more explicitly (distinct from `try_recv` drains). Consider a timer wheel or a small async delay.
  - Consider a persisted offset for sessions tailer to avoid re-parsing large files.

Security & Permissions
- Bridge spawns Codex with full access by default (developer-friendly). Document and enforce safer defaults for production/distribution.
  - Action: expose a minimal “safe mode” profile and a UI toggle; validate control messages origin when remote access is enabled.
  - Confirm no secrets are logged. Review logs for accidental inclusion of auth tokens.

DX & Tooling
- Lints & formatting
  - Add Clippy (with `-D warnings` for CI) and rustfmt checks.
  - Add ESLint/Prettier enforcement in the Expo app; ensure consistent imports and TS strictness across providers/components.

- CI pipelines
  - GitHub Actions to build bridge and tauri backend, run tests, and (optionally) generate OTA bundles for Expo.
  - Cache Convex CLI and sidecar binaries in CI to speed setup.

Observability
- Logging
  - Standardize tracing targets and levels across modules; add a concise request id/session id to tie WS control and stream lines.
  - Add a minimal log panel in the desktop UI to surface key transitions (sidecar ready, bridge ready, subscribe events, errors).

- Metrics (optional)
  - Add counters for messages per second, streaming patch rate, debounce drops. Keep disabled by default, feature-gated.

UX & Parity Improvements
- Sidebar counts & badges
  - Ensure counts update instantly on first message; hide zero-message threads by default (implemented) but allow toggling.

- Composer
  - Maintain focus on thread switches; allow Shift+Enter for newline; add “sending…” affordance on enqueue.

Documentation
- Align and cross-link:
  - docs/ARCHITECTURE.md — architecture and data flow (added).
  - docs/sync.md — up-to-date module references (`ws.rs`, `watchers.rs`, `convex_write.rs`).
  - docs/test-coverage.md — reflect new tests and target gaps.
  - docs/permissions.md — call out dev vs. safe profiles.

Issue Alignment
- #1320: “Real-time Codex chat streaming + refactor”
  - Status: bridge split; streaming partials + finalization; sessions tailer; Tauri split; partial tests done.
  - Next: complete tool-row tests, finalize ordering tests, watchers debounce, main.rs pruning.

- #1318: “Bundle Convex local backend as Tauri sidecar”
  - Status: sidecar wiring present; ensure discovery is robust and errors surface.
  - Next: add UI-facing status and restart controls; ensure Windows/macOS/Linux binaries handled via resources.

Concrete Backlog (Actionable)
- Bridge
  - [ ] Remove unused helpers/imports in main.rs; keep only routing and bootstrap.
  - [ ] Add fake Convex client trait + tests in ws.rs for tool rows and assistant/reason finalize paths.
  - [ ] Add debounce utilities for watchers (projects/skills/sessions); unit-test coalescing.
  - [ ] Add offset tracking for sessions tailer and backfill tool.

- Tauri
  - [ ] Sidecar discovery rework with structured error reporting; UI status panel.
  - [ ] Single source for `bridge:ready` and `convex:local_status` emissions; consolidate dot indicator.
  - [ ] Jest-like tests for sidebar badge and composer autofocus (if we introduce a testing strategy for the webview layer).

- Expo
  - [ ] Extract shared hide rules with desktop; ensure parity in badge logic and thread subscription reuse.
  - [ ] Add a Library screen that matches the Tauri component library for render parity checks.

- Convex
  - [ ] Add guards in mutations to dedupe repeated streamed writes (optional).
  - [ ] Verify indexes exist for hot paths pre-deploy; fail fast if missing.

Appendix: References
- Architecture: docs/ARCHITECTURE.md
- Sync model and module mapping: docs/sync.md
- Test coverage audit: docs/test-coverage.md
- Permissions: docs/permissions.md
- Projects/Skills schema: docs/projects-and-skills-schema.md
Updates in this pass (2025‑10‑26)
- Bridge entrypoint docs clarified to reflect current model (WS control + JSONL broadcast for legacy; Convex as the persistence layer).
- Added unit tests:
  - ws.rs kind mapping for tool rows (cmd/file/search/mcp/todo).
  - watchers.rs `sessions_base_dir` env override and HOME fallback.
- Tauri/desktop UI is already wired with counts and composer autofocus; badges render and update.
- Leftover legacy helpers in `crates/codex-bridge/src/main.rs` are still present (now gated by imports to keep build/tests green). They are slated for removal as part of #1320 cleanup.
