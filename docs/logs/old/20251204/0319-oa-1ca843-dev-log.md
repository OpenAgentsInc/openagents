# 0319 Work Log — oa-1ca843 dev

- Added HUD transport wrapper to centralize HUD client + status stream wiring.
- Refactored emit helpers to route through the transport instead of duplicating send/broadcast logic.
- Drafted new transport tests (env gating, missing token skip, send fan-out) using fake clients/streams.
