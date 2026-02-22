# Web Parity Reversal Plan: Reintroduce Vercel Streaming API Using Laravel AI Patterns

Date: 2026-02-22
Status: Proposed (governance change required before implementation)
Owner lane: openagents.com platform + runtime integration + contracts/docs

## 1) Objective

Reintroduce a production `Vercel AI SDK` compatible streaming API for web chat (`/api/chat/stream`, `/api/chats/{conversationId}/stream`, and related `/api/chats*` thread APIs) while keeping Rust/Codex/Khala authority boundaries intact.

This reverses the current codex-only/no-Vercel-lane parity direction and defines the safe path to do it.

## 2) Sources Reviewed (deep read)

- `docs/adr/ADR-0001-rust-only-architecture-baseline.md`
- `docs/adr/ADR-0002-proto-first-contract-governance.md`
- `docs/adr/ADR-0003-khala-ws-only-replay-transport.md`
- `docs/adr/ADR-0005-compatibility-negotiation-and-support-window-policy.md`
- `docs/plans/active/rust-migration-invariant-gates.md`
- `apps/openagents.com/docs/20260222-laravel-rust-wgpui-full-parity-master-plan.md`
- `apps/openagents.com/docs/20260222-web-parity-charter-checklist.md`
- `docs/audits/2026-02-22-codex-app-server-parity-audit.md`
- `docs/audits/2026-02-22-laravel-ai-codebase-audit.md`
- `apps/openagents.com/routes/api.php`
- `apps/openagents.com/routes/web.php`
- `apps/openagents.com/service/src/lib.rs`
- `apps/openagents.com/app/AI/RunOrchestrator.php`
- `apps/openagents.com/app/Http/Controllers/ChatApiController.php`
- `apps/openagents.com/tests/Feature/ChatStreamingTest.php`
- `/Users/christopherdavid/code/laravel-ai/src/Responses/Concerns/CanStreamUsingVercelProtocol.php`
- `/Users/christopherdavid/code/laravel-ai/src/Responses/StreamableAgentResponse.php`
- `/Users/christopherdavid/code/laravel-ai/src/Streaming/Events/*`

## 3) Current State (what exists now)

### 3.1 Governance and parity docs currently reject this lane

- Master plan states no parallel Vercel chat authority and codex-only end state.
- Charter pass/fail includes "No production Vercel-protocol chat authority remains."
- OA-WEBPARITY issues 019/020/021/023/024 explicitly remove Vercel + Laravel-AI protocol behavior.

### 3.2 Runtime/transport constraints

- `ADR-0003` + `INV-03` enforce WS-only live transport for Khala topics and ban new Khala SSE lanes.
- `ADR-0001` + `ADR-0002` enforce Rust/proto-first authority boundaries.

### 3.3 Code state in Rust control service

- Rust service still exposes `/api/chat/stream` and `/api/chats*` aliases.
- Current behavior is intentionally retired/bridge mode:
  - returns JSON (not SSE data stream)
  - `stream_protocol: "disabled"`
  - `x-oa-legacy-chat-retired: true`
  - bridges command to codex worker `turn/start`
- Legacy compatibility tests in `apps/openagents.com/service/src/lib.rs` assert the retired behavior.

### 3.4 Legacy Laravel behavior still available as reference

- Laravel controllers + `RunOrchestrator` implement Vercel AI data stream SSE semantics.
- Existing tests define expected stream shape (`start`, `start-step`, `text-delta`, `finish-step`, `finish`, `[DONE]`) and guest/auth edge cases.

## 4) Decision Boundary and Recommended Direction

Implement **authority-preserving Vercel compatibility** (recommended), not authority fork.

- Keep canonical command authority in Codex worker control APIs.
- Keep Khala WS as canonical live/replay event fabric.
- Add Vercel SSE as a compatibility output adapter for web clients that need AI SDK protocol.
- Do not reintroduce Laravel/PHP as authority path.
- Do not reintroduce separate Vercel conversation DB authority.

This satisfies user intent (bring Vercel streaming API back) without reintroducing split-brain authority.

## 5) Required Governance Changes First

No implementation should start before this doc-level sequence lands.

1. Create new ADR (`ADR-0008`) to allow a bounded Vercel SSE compatibility lane.
2. Mark current anti-Vercel web parity language as superseded where necessary.
3. Update invariant wording:
- `INV-03` remains WS-only for Khala live transport.
- Add explicit clause: SSE compatibility adapters are allowed only if they are derived views over codex/Khala authority and are not a separate live-sync authority lane.
4. Update OA-WEBPARITY scope docs:
- Replace "retire Vercel protocol" with "reintroduce bounded compatibility lane".
- Keep codex authority requirement explicit.

## 6) Target Architecture

### 6.1 Control/API contract

- Re-enable full behavior for:
  - `POST /api/chat/stream`
  - `POST /api/chats/{conversationId}/stream`
  - `GET/POST /api/chats`
  - `GET /api/chats/{conversationId}`
  - `GET /api/chats/{conversationId}/messages`
  - `GET /api/chats/{conversationId}/runs`
  - `GET /api/chats/{conversationId}/runs/{runId}/events`
- Preserve current codex worker request path as canonical command ingress.

### 6.2 Streaming pipeline

- Input: legacy AI SDK payloads (`messages`, content parts, alias fields).
- Command bridge: emit `turn/start` through runtime codex worker control.
- Live output: stream Vercel protocol SSE frames from canonical event state.
- Completion: emit `finish-step`, final `finish`, then `[DONE]`.

### 6.3 Adapter implementation style (from laravel-ai patterns)

Adopt pattern, not direct transplant:

- `StreamEvent`-like internal enum in Rust for normalized event types.
- Vercel adapter serializer layer (`to_vercel_event()` style).
- Stateful stream writer logic:
  - one start per stream
  - tool-result only after tool-call correlation
  - hold terminal finish event until end
  - optional synthetic `text-start` injection when deltas arrive first

### 6.4 Data ownership

- Keep thread/message/run persistence in Rust-owned stores.
- Do not revive standalone legacy `threads/runs/messages/run_events` authority tables as independent source of truth.
- If compatibility projections are needed, generate them from canonical runtime/control projections.

## 7) Detailed Execution Plan

### Phase A: Governance and docs (blocking)

1. `OA-WEBPARITY-069` Draft and accept `ADR-0008` for Vercel compatibility lane.
2. `OA-WEBPARITY-070` Update parity master plan and charter checklist to reflect new scope.
3. `OA-WEBPARITY-071` Update invariant gates text (`INV-03`) with bounded compatibility exception language.

### Phase B: Contract and fixtures

4. `OA-WEBPARITY-072` Define canonical mapping spec: codex/Khala events -> Vercel stream events.
5. `OA-WEBPARITY-073` Capture/normalize fixture corpus from:
- legacy Laravel `ChatStreamingTest`
- existing parity fixture manifests
- codex worker event fixtures
6. `OA-WEBPARITY-074` Add compatibility negotiation expectations for SSE lane (build/schema/protocol window behavior on stream endpoints).

### Phase C: Rust compatibility adapter

7. `OA-WEBPARITY-075` Add Rust stream adapter module in `apps/openagents.com/service`:
- request payload normalization
- conversation/thread resolution
- event normalization layer
- Vercel SSE writer
8. `OA-WEBPARITY-076` Replace current retired responses for `/api/chat*` and `/api/chats*` with real compatibility behavior as the default production behavior.

### Phase D: Web client path integration

9. `OA-WEBPARITY-077` Switch web-shell chat transport to the Vercel SSE-compatible endpoints as the single production mode while preserving route-shell behavior and auth/session gates.
10. `OA-WEBPARITY-078` Run full-traffic pre-prod dual-run/shadow comparison between codex-native and Vercel-compatible outputs before production cutover.

### Phase E: Test + rollout + cleanup

11. `OA-WEBPARITY-079` Port legacy Laravel chat streaming edge-case tests into Rust service integration tests.
12. `OA-WEBPARITY-080` Execute a one-shot production cutover with explicit rollback drills and hard SLO/error-budget gates.
13. `OA-WEBPARITY-081` Validate post-cutover steady-state and lock the one-shot cutover behavior as the standard lane.
14. `OA-WEBPARITY-082` Remove retired-header semantics and old retirement assertions once stable.

## 8) Proposed Contract Semantics

### 8.1 Request compatibility

Accept legacy request variants currently seen in code/tests:

- `conversationId`, `conversation_id`, `threadId`, `thread_id`, `id`
- `messages[].content` as string/object/parts array
- optional `workerId`/`worker_id`

### 8.2 Response/event compatibility

Emit Vercel-compatible SSE frames with deterministic order:

1. `start`
2. `start-step`
3. `text-start` when needed
4. `text-delta` chunks
5. `tool-input-available` / `tool-output-available` where applicable
6. `finish-step`
7. `finish`
8. `[DONE]`

Required headers:

- `Content-Type: text/event-stream`
- `Cache-Control: no-cache, no-transform`
- `x-vercel-ai-ui-message-stream: v1`
- `x-accel-buffering: no`

### 8.3 Error semantics

- Keep structured 4xx/5xx JSON for pre-stream validation/auth failures.
- For in-stream failures after headers sent, emit deterministic error event then terminate with `finish-step` + `[DONE]`.

## 9) Testing and Verification Gates

Required before any production cutover:

1. Rust service integration tests for all `/api/chat*` and `/api/chats*` paths.
2. Fixture replay tests comparing adapter output to approved golden streams.
3. Cross-surface contract harness for chat/stream changes.
4. Compatibility-window tests on stream endpoints (`unsupported_protocol_version`, `unsupported_schema_version`, etc.).
5. Staging shadow diff reports comparing codex-native and Vercel-compat outputs.
6. Load tests on stream concurrency, flush behavior, and disconnect handling.

## 10) Rollout and Rollback

### 10.1 One-shot cutover plan

1. Complete all contract/test/load gates in staging with full-traffic replay/shadow diff evidence.
2. Schedule a production cutover window and deploy server + web-shell together.
3. Switch `/api/chat*` and `/api/chats*` to Vercel-compatible streaming behavior in one release.
4. Run immediate post-deploy smoke + auth + stream-shape checks.
5. Hold release only if SLOs stay within thresholds and no critical contract drift appears.

### 10.2 Rollback plan

1. Revert to previous release artifacts (service + web-shell) if cutover SLO gates fail.
2. Restore previous retired/bridge behavior for `/api/chat*` and `/api/chats*` via release rollback.
3. Keep command authority unchanged, so rollback remains transport/adapter-only.
4. Re-run smoke + parity fixtures before reopening production cutover.

## 11) Risks and Mitigations

1. Risk: protocol drift between codex events and Vercel stream frames.
- Mitigation: explicit mapping spec + golden fixtures + CI drift gate.

2. Risk: accidental second authority lane.
- Mitigation: ADR language requiring adapter-only behavior, no separate write path.

3. Risk: regression in reconnect/live behavior.
- Mitigation: keep Khala WS semantics untouched; treat SSE lane as compatibility presentation.

4. Risk: duplicate or malformed terminal events.
- Mitigation: stateful writer rules copied from proven laravel-ai/RunOrchestrator patterns and test for ordering invariants.

5. Risk: guest/session edge-case regressions.
- Mitigation: port Laravel guest-chat tests into Rust service suite before rollout.

## 12) Explicit Scope Changes vs Existing OA-WEBPARITY

The following prior assumptions are superseded by this plan once approved:

- OA-WEBPARITY-019/020/021/023/024 wording that requires removing Vercel protocol behavior entirely.
- Charter criterion "No production Vercel-protocol chat authority remains" becomes:
  - "No separate Vercel authority lane remains; Vercel protocol may exist as a compatibility adapter over codex authority."

## 13) Definition of Done

This reintroduction is complete when all are true:

1. Governance docs are updated and accepted (`ADR-0008` + parity docs + invariant text).
2. Rust service streams Vercel protocol on `/api/chat*` and `/api/chats*` with fixture parity.
3. Canonical command authority remains codex worker control endpoints.
4. Web-shell uses the Vercel-compatible streaming lane in production with no critical regressions.
5. One-shot cutover and rollback evidence are documented.
6. Old retired-response behavior and related tests are removed or marked historical.
