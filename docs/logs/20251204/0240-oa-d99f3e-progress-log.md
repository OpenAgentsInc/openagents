# 0240 Work Log

- Moved sandbox HUD adapter to shared module (src/sandbox/hud-adapter.ts) and re-exported via sandbox index
- Updated sandbox-runner to consume shared adapter import path (removed local helper)
- Next: ensure sandbox-runner logic clean with new adapter, rerun checks, close task

