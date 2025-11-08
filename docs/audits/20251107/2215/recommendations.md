# Recommendations (Prioritized)

## P0 — ACP Wire Compliance
- Decide and document casing policy exceptions vs ADR‑0002 for `sessionUpdate`, `mimeType`, `lastModified`.
  - Option A (preferred): Keep ACP’s canonical keys; amend ADR‑0002 to list explicit exceptions.
  - Option B: Move to snake_case (`session_update`, `mime_type`, `last_modified`) and add compatibility decoding for one release.
- Implement or defer `session/load`:
  - Add server router + handler for `ACPRPC.sessionLoad` or remove from public API until implemented.
- Add golden JSON fixture tests under `ios/OpenAgentsCore/Tests/Fixtures/acp/`:
  - Round‑trip encode/decode for all supported `session/update` variants and core requests.

## P0 — Bridge Surface Hardening
- Finalize DesktopWebSocketServer extraction:
  - Move remaining domain handlers into dedicated files and reduce server to wiring.
- ACPExt capability gating:
  - Advertise extension capabilities during `initialize`; gate `orchestrate.explore.*` and document in `docs/ios-bridge/`.

## P1 — Developer Experience
- Enforce lint rules that protect ACP contract:
  - No ad‑hoc JSON keys; require CodingKeys to match spec or documented exceptions.
- Logging improvements:
  - Ensure `os.Logger` categories exist for `bridge.server`, `bridge.client`, `acp`, and `ui.timeline` with DEBUG gating.

## P1 — Test Coverage
- Add compliance tests for mixed‑case tolerance (if Option B chosen) and ensure failures are obvious on drift.
- Expand DesktopWebSocketServerComprehensiveTests to include `session/set_mode` and negative cases for `session/prompt` params.

## P2 — Documentation
- Update ADR‑0002 to reflect Swift‑only v0.3 architecture while keeping normative ACP guidance.
- Update `docs/ios-bridge/` with ext capability negotiation and examples for `session/prompt` content array usage.

