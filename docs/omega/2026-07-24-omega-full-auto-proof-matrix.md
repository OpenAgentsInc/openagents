# Omega Full Auto proof matrix (FA-07)

- Date: 2026-07-24
- Packet: `OMEGA-FA-07`
- Omega issue: [OpenAgentsInc/omega#26](https://github.com/OpenAgentsInc/omega/issues/26)
- Package: `@openagentsinc/omega-effectd` `0.1.0`
- Pack SHA-256: `a04dd9aef716504f586f4bb8d18314db1b79a87acb68678cbd28b89138cd1448`
- Freeze: [2026-07-24-full-auto-contract-freeze.md](./2026-07-24-full-auto-contract-freeze.md)
- Assurance: `specs/omega/full-auto.assurance-spec.md` remains `proposed`

## Result

This packet records the FA-07 proof matrix for Omega Full Auto.
It does **not** close `OMEGA-OA-05` release readiness.
It does **not** admit public claims.
Independent assurance is still required.

## Gate matrix (§12.2)

| # | Gate | Status | Evidence |
| --- | --- | --- | --- |
| 1 | Dedicated launcher / no composer toggle | green (code) | `crates/full_auto_ui` product law and `full_auto_is_not_a_composer_mode_flag`. Agent menu opens `OpenLauncher` only. |
| 2 | Non-overridable guardrail immunity | green (automated) | FA-04 framed capacity and guardrail decode tests |
| 3 | Redaction by explicit field lists | green (automated) | FA-05/FA-07 receipt, attention, and `list_runs` redaction tests |
| 4 | 2026-07-17 eviction incident shape | green (automated) | Drop host thread registry yields `stalled` / `host_thread_missing` / `stop_only` in FA-04 and FA-07 matrix |
| 5 | Owner-real multi-turn unattended run on Omega | blocked | Needs installed Omega and live own-capacity lanes |
| 6 | Visible Codex to Claude handoff on Omega | partial | The framed service now exposes paused-only, live-lane-revalidated, durable handoff under #9215. Live sidebar proof is still required on the replacement packaged candidate. |
| 7 | Packaged restart reconciliation | partial | Supervisor restart and native evidence survive in Rust FA-07 test. Packaged RC install is absent (`/Applications/Omega.app` missing). |
| 8 | Mobile pause/resume/stop typed outcomes | green (automated) | FA-05 and FA-07 `apply_control_intent` matrix |
| 9 | No ordinary chat sets Full Auto authority | green (code) | Dedicated panel only. No composer Full Auto start path. |
| 10 | Independent assurance on exact candidate | blocked | Producer cannot admit. AssuranceSpec stays `proposed`. |

## Automated verification

- `pnpm --filter @openagentsinc/omega-effectd test` — includes `server.fa07-proof.test.ts`
- `cargo test -p omega_effectd --lib` — includes `fa07_control_matrix_and_native_join_survive_restart`
- `script/bundle-omega-rc --dry-run` — release-record schema ok (no compile/sign)
- `script/prove-omega-rc-install --harness-check` — harness OK (not installed-app proof)

## Native Codex authentication

Omega commit `31efeaffbc` adds **Use Existing Codex Login** to the ChatGPT Subscription provider. The import runs inside Omega, reads the same local `auth.json` used by Codex (`$CODEX_HOME/auth.json` or `~/.codex/auth.json`), validates its file permissions, size, token shape, and expiry, and then stores the credentials through Omega's own system-keychain provider. The source file is not changed, and token values are not written to settings or logs.

This is the supported Codex lane for the owner-real journey. It uses Omega's native Zed-derived provider stack and does not depend on the legacy OpenAgents/Pylon account bridge. The existing browser OAuth flow remains available when there is no reusable local Codex session.

Focused verification on the implementation commit:

- `cargo test -p language_models codex_cli_credentials` — 2 passed
- `./script/clippy -p language_models` — passed

## Owner blockers (smallest irreducible)

1. Produce and install a signed Omega RC candidate containing the native Codex-login import (`script/bundle-omega-rc` then install).
2. Run one owner-real multi-turn Full Auto journey on that candidate.
3. Capture one live cross-provider handoff with sidebar/report evidence.
4. Independent reviewer admits `specs/omega/full-auto.assurance-spec.md` against the exact candidate digests.

Until those four land, `OMEGA-OA-05` stays open and Electron Full Auto remains the rollback surface.

## Next

- Keep Electron Full Auto as dogfood/rollback
- Continue Omega Agent Computer sequence (`OMEGA-AC-00`…) without claiming Full Auto primary cutover
