# 0552 Work Log

- Updated HUD protocol with tb_run_history push message and type guard; added server support to broadcast run history on completion (no polling) and seed initial data.
- Mainview now listens for tb_run_history events, prunes stale details, removes TB polling interval, and relies on push updates.
- Documented new message in docs/hud/TERMINAL-BENCH.md; added type guard test coverage.
