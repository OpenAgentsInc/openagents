# Smells & Risks

- Mixed wire casing vs ADR‑0002 guidance
  - `ACP.Client.SessionUpdate` uses `sessionUpdate` (camelCase) as the discriminant key within the `update` object, while other keys use snake_case (e.g., `current_mode_id`).
  - Several content types use `mimeType` and `lastModified` (camelCase). If this matches ACP spec examples, update ADR to record the exceptions; otherwise, consider compatibility decoding with snake_case output.

- Optional method surface
  - `ACPRPC.sessionLoad` exists but server handler is not registered. Either implement (`session/load`) or hide until supported.

- Extension namespace consistency
  - Orchestration methods (`orchestrate.explore.*`) use dotted names, while ACP uses slashes. This is fine for extensions, but document capability gates and ext negotiation (ACPExt) to prevent accidental exposure.

- Large single files remain
  - `DesktopWebSocketServer.swift` improved but still a hotspot; ensure the rest of routing/handlers are moved under dedicated components (JsonRpcRouter, SessionUpdateHub already exist — continue extracting domain handlers).

