# FA-REL-01 Full Auto release admission residual

- Class: receipt
- Issue: [#8979](https://github.com/OpenAgentsInc/openagents/issues/8979)
- Parent: [#8967](https://github.com/OpenAgentsInc/openagents/issues/8967)
- Date: 2026-07-21
- Reviewer: authority-delegated independent reviewer (AUTHORITY.md rev 6)
- Review base: `fe89a057cb6a9a8621fcc50c3b6c46f5d6d4a95d`
- Disposition: **release-green incomplete** — issue stays open
- Promise flip: **no** (`autopilot.desktop_full_auto_guidance.v1` stays red)
- New stable release: **none** (0.1.0 already shipped; `reserved.stable_release_without_direction` honored)

## Purpose

Reconcile `condition.release_green` for Full Auto after independent
AssuranceSpec admission. Record each gate as green or red with exact
evidence. Do not round a residual up to release or public-claim authority.

## Candidate identity (already shipped)

| Field | Value |
| --- | --- |
| Desktop tag | `openagents-desktop-v0.1.0` |
| Source revision | `26d1627722ee1130a9fc3060b0be3b0b9b91cb6a` |
| Version | `0.1.0` (stable) |
| Local signed app | `/Applications/OpenAgents.app` |
| Codesign | Developer ID Application: OpenAgents, Inc. (`HQWSG26L43`) |
| Gatekeeper | accepted; source = Notarized Developer ID; notary ticket stapled |
| DIST receipt | [`docs/deploy/receipts/2026-07-21-desktop-0.1.0-stable-release.md`](../../deploy/receipts/2026-07-21-desktop-0.1.0-stable-release.md) |
| Release set | stable channel; generation prefix `85ad830fdfb8c5e8` (see DIST receipt) |

Full Auto commits required by the issue are ancestors of this tag (includes
owner-real matrix `3123d926a3` and the thread-pressure fix).

## Gate matrix

| Gate | Result | Evidence |
| --- | --- | --- |
| Admitted AssuranceSpec | **green** | Lifecycle admitted on `main` at `fe89a057cb`. Receipt [`docs/assurance/receipts/authority.decision.de1e10314822b99f8d96dc46bb5302cd.json`](../../assurance/receipts/authority.decision.de1e10314822b99f8d96dc46bb5302cd.json). Counts: 61 executable green; 8 designed-only; 2 smoke-gated; 5 receipt-backed. Admission is not release authority (Law 6 / receipt scope notes). |
| Signed 0.1.0 candidate contains Full Auto | **green** | Tag `openagents-desktop-v0.1.0` @ `26d1627722`; FAV/HARN Full Auto loop and restart probe are in the tag. Installed app reports version `0.1.0`. |
| DIST release-set / stable feed / exact artifacts | **green** | DIST-12 #8925 closed path; stable feed and ten signed artifacts listed in the 0.1.0 stable release receipt. No alternate publication path used for this review. |
| Automatable unit/e2e Full Auto suites (source) | **green (dev tier)** | Assurance admission reproduced 516 Desktop + 11 assurance-spec tests. Owner-real development matrix six-of-six at `3123d926a3` ([`2026-07-18-full-auto-real-owner-acceptance.md`](./2026-07-18-full-auto-real-owner-acceptance.md)). Not a signed-package claim. |
| Two-OS-process `smoke:full-auto-restart` against **signed packaged** 0.1.0 | **red** | See section "Packaged restart smoke". Oracle `ok:false`. |
| Partial packaged resume dispatch (informational) | **partial** | Seed and resume both ran as real OS processes of `/Applications/OpenAgents.app`. Resume dispatched one fixture continuation on `codex-local` with journal text `Codex local **fixture** proof.` Duplicate-dispatch was not observed in this single-pair run. Cap-terminal and full oracle still fail. |
| Owner-observed packaged six-test / Test 05 quit-relaunch | **red (unobserved)** | Only development packaging mode receipt exists. No signed 0.1.0 owner-real sidebar batch. |
| Packaged Pause / Resume / Stop / provider transition UI | **red (unobserved)** | No packaged UI observation receipt for 0.1.0. |
| Telemetry-off zero-outbound in packaged candidate | **unobserved** | Not re-measured in this session; default-off remains design intent only for this residual. |
| `condition.public_claim_evidence` / promise transition | **red** | Named gates above are not all green. Promise must stay red. |
| `reserved.stable_release_without_direction` | **honored** | No new stable version or tag was cut. |

## Packaged restart smoke

Command (worktree at review base; packaged binary forced to the notarized
0.1.0 app):

```text
ln -sfn /Applications/OpenAgents.app \
  apps/openagents-desktop/out/OpenAgents-darwin-arm64/OpenAgents.app
pnpm --dir apps/openagents-desktop run smoke:full-auto-restart
```

Public-safe phase-b receipt (happy path only; mismatch and Claude phases did
not run after resume exit 1):

```json
{
  "variant": "resume",
  "seeded": true,
  "resumed": true,
  "dispatchedTurnRefPresent": true,
  "dispatchedLane": "codex-local",
  "continuationCount": 20,
  "blockedReason": null,
  "ok": false
}
```

Isolated diagnostic (temp `OPENAGENTS_DESKTOP_USER_DATA`, no Keychain probe):

1. Seed wrote legacy `full-auto/registry.json` with `enabled:true` and
   `continuationCount:19`.
2. Resume logged `legacy registry migration: migrated=1`.
3. Migration created `full-auto/runs.json` with
   `successfulAttempts:0`, `turnCap:20`, `state:"running"`,
   `objectiveSource:"legacy_migration"`.
4. One fixture continuation completed on `codex-local`.
5. Post-resume legacy row stayed `enabled:true`,
   `continuationCount:20`, no `blockedReason`.
6. Post-resume run row stayed `state:"running"` with
   `successfulAttempts:1` (not cap-terminal).

### Residual mechanism (public-safe)

The restart probe seeds the **legacy** Full Auto registry near the
continuation cap. On resume, 0.1.0 migrates that row into the **FullAutoRun**
registry with `successfulAttempts:0` and the default `turnCap:20`. The probe
oracle still waits for the legacy row to disable with
`blockedReason:"continuation_cap_reached"`. After one successful continuation
the run is not cap-terminal, the legacy row remains enabled, and the oracle
fails with `blockedReason:null` and `ok:false`.

This residual blocks FA-AC-07 / FA-AC-29 at the **signed packaged** rung. It
also means a high legacy continuation count does not map into the migrated
run turn budget on this candidate.

No Keychain or secret material was inspected.

## Promise reconciliation

Promise id: `autopilot.desktop_full_auto_guidance.v1`

- Current registry state: **red**
- Transition decision: **do not flip**
- Reason: `condition.public_claim_evidence` requires every named verification
  gate green. Packaged restart oracle is red. Packaged owner six-test and
  packaged control UI observations are unobserved. Assurance admission
  explicitly withholds release and public-claim authority.

Keep public copy experimental or not-live for Full Auto unattended success.

## Issue and epic status

| Item | Status after this review |
| --- | --- |
| #8978 FA-AS-01 | Closed (prior independent admission) |
| #8979 FA-REL-01 | **Open** — release-green incomplete |
| #8967 Full Auto epic | **Open** — child #8979 not terminal |

## Exact residual list (do not round up)

1. Make signed-packaged `pnpm run smoke:full-auto-restart` pass against the
   exact 0.1.0 (or a later already-shipped signed candidate that still claims
   the same evidence) with `ok:true` for happy path, workspace mismatch, and
   Claude lane pairs.
2. Repair or re-prove legacy→FullAutoRun migration so prior continuation
   budget and cap-terminal disable semantics survive restart, **or** change
   the probe to assert the run-registry terminal that product authority now
   owns — then re-run the packaged smoke.
3. Produce owner-observed packaged 0.1.0 evidence for the six named sidebar
   tests (especially Test 05 quit/relaunch) without raw private transcripts.
4. Produce packaged UI checks for Pause, Resume, Stop, and provider transition.
5. Only after (1)–(4) are green for the same candidate identity, run the typed
   product-promise transition for `autopilot.desktop_full_auto_guidance.v1`.

## Authority notes

- Grant used: `grant.local_provider_and_device_operation` for local signed-app
  smoke with isolated temporary user-data.
- Not used: Keychain dump, new stable release, promise self-flip, assurance
  self-admission (already independent on #8978).
- `condition.release_green` remains unsatisfied while packaged restart and
  packaged dogfood gates are red or unobserved.
