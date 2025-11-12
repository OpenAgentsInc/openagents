# 2025-11-12 — ACP streaming + runtime refactor log (09:19)

Context
- Frontend logs showed tinyvex WS connected but no visible streaming in assistant-ui. Server logs confirmed ACP updates and tinyvex broadcasts. Prior change hinted at fixes but UI still didn’t stream new ACP events.
- Goal: make ACP updates flow live into the assistant-ui thread and reduce App.tsx code smell by extracting adapters/providers.

Root cause analysis (Issue #1475)
- No fetch on update: `tinyvex.update` does not include text. Client must issue a `messages.list` query after updates. Our hook only queried on subscribe/finalize.
- Partial rows ignored: Streaming rows set `partial=1` but contain the full accumulated text. The hook filtered to `partial === 0`, dropping all streaming updates.
- Wrong aggregation: Concatenated text across multiple rows, not the latest assistant turn’s text.

Changes made
1) Streaming hook fixes
   - File: `tauri/src/lib/useAcpSessionUpdates.ts`
   - On `tinyvex.update` (messages): debounce `tvx.query messages.list` (75ms) to fetch fresh rows.
   - Compute latest assistant/reason text from the newest row (ignore partial flag). Stop concatenating.
   - Mark `finalizedRef` true on `tinyvex.finalize` and set `isStreaming=false`.
   - WS URL now configurable via `VITE_TINYVEX_WS_URL`.

2) Adapters extracted
   - ACP adapter: `tauri/src/runtime/adapters/acp-adapter.ts`
     - Streams by polling `session.liveTextRef` until finalize (with idle fallback). Finalizes on `finalizedRef`.
   - Ollama adapter: `tauri/src/runtime/adapters/ollama-adapter.ts`

3) Provider extracted
   - `tauri/src/runtime/MyRuntimeProvider.tsx`: Chooses adapter (codex vs ollama), hosts `useAcpSessionUpdates`, registers minimal attachment adapter, wraps `AssistantRuntimeProvider`.
   - `tauri/src/App.tsx`: Now only composes UI inside `MyRuntimeProvider` and applies dark mode.

4) Config extraction
   - `tauri/src/config/ollama.ts`: `OLLAMA_BASE_URL`, `OLLAMA_MODEL`.
   - `tauri/src/config/acp.ts`: `TINYVEX_WS_URL`.

5) ACP runtime prototype (external-store)
   - `tauri/src/runtime/useAcpRuntime.tsx` (not wired yet):
     - Builds an `ExternalStoreAdapter` that mirrors finalized message rows from `messages.list` and drives assistant-ui without SSE.
     - `onNew` uses Tauri commands `create_session` / `send_prompt`; tinyvex WS drives updates.
     - This is the basis for a full “assistant-ui speaks ACP over WebSocket” runtime.

Build
- Ran `bun run build` in `tauri/`: success. TypeScript passes, vite bundles complete.

Rationale
- Keep App.tsx focused on composition; push transport/orchestration to adapters/provider.
- Fix streaming at the source (debounced query-on-update + latest-row selection) to feed adapters cleanly.
- Provide a migration path away from ChatModelAdapter polling by scaffolding an ACP-native runtime (`useAcpRuntime`).

Next steps
1) Replace ACP adapter polling with push updates (wire `useAcpRuntime` into provider behind a flag and verify parity).
2) Extend `useAcpRuntime` to include reasoning/tool call visualization.
3) Unify completion: Prefer finalize notifications over idle timeouts everywhere.
4) Optional: Introduce assistant-transport-compatible facade if we ever add an HTTP SSE endpoint; otherwise keep WS-first ACP.

References
- Backend WS: `tauri/src-tauri/src/tinyvex_ws.rs`
- Tinyvex writer & DB: `crates/tinyvex/src/{lib.rs,writer.rs}`
- ACP client/manager: `tauri/src-tauri/src/oa_acp/{client.rs,session_manager.rs}`
- Issue: #1475 — Connect ACP streaming updates to UI (Phase 2)

