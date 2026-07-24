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

The supported Codex lane is Omega's registered `codex-acp` external agent. Omega starts that agent through its native Zed-derived ACP host, and Codex remains the single owner of its local configuration, authentication, and token rotation. A user who is already logged into Codex does not need to import or copy that session into an Omega language-model provider.

Omega commit `31efeaffbc` briefly implemented a token-copying import and was rejected during independent review because it would create two competing refresh-token stores. The replacement candidate reverts that import and keeps the owner-real journey on the existing ACP authority. This path does not depend on the legacy OpenAgents/Pylon account bridge.

## Owner blockers (smallest irreducible)

1. Produce and install a signed Omega RC candidate using the native `codex-acp` authority (`script/bundle-omega-rc` then install).
2. Run one owner-real multi-turn Full Auto journey on that candidate.
3. Capture one live cross-provider handoff with sidebar/report evidence.
4. Independent reviewer admits `specs/omega/full-auto.assurance-spec.md` against the exact candidate digests.

Until those four land, `OMEGA-OA-05` stays open and Electron Full Auto remains the rollback surface.

## Next

- Keep Electron Full Auto as dogfood/rollback
- Continue Omega Agent Computer sequence (`OMEGA-AC-00`…) without claiming Full Auto primary cutover
