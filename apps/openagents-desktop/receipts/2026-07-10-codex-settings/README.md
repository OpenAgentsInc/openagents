# 2026-07-10 — Settings / Codex reconnect pixel receipts (#8574, #8640 unblock)

Captured by the real-Electron smoke (`bun run smoke` with
`OPENAGENTS_DESKTOP_SMOKE_SHOTS`).

- `04-settings-accounts.png` — Settings screen: Codex accounts list with
  `credentials_revoked` warning chips + the primary "Connect Codex account"
  button.
- `05-settings-awaiting-browser-fixture.png` — the awaiting_browser state:
  clickable verification link + large user code.

HONESTY NOTE: a headless smoke cannot complete a real browser device-auth, so
smoke mode runs the codex-connect service against a scripted FIXTURE spawn
(`makeFixtureSpawnPylon` in `src/codex-connect.ts`; main logs
"SMOKE FIXTURE mode"). The account refs, revoked states, URL, and code in
these two shots are fixture data in the real rendered UI — the live path
spawns `bun apps/pylon/src/index.ts auth codex` (isolated per-account
`CODEX_HOME`; never `~/.codex`).
