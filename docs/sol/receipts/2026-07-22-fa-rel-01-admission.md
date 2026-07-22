# FA-REL-01 Full Auto release admission — 2026-07-22

- Issue: [#8979](https://github.com/OpenAgentsInc/openagents/issues/8979)
  (FA-REL-01), parent epic
  [#8967](https://github.com/OpenAgentsInc/openagents/issues/8967).
- Authority: explicit current owner direction 2026-07-22 to ship, complete,
  and close #8967 and #8979. The owner delegated the packaged observation to
  the delivery agent and directed admission on the assembled evidence.
- Result: **admitted for signed RC release.** Candidate signed, notarized,
  packaged, and restart-proven. Residuals recorded below.

## Candidate identity

- Source revision: `24b7622432` (contains every Full Auto implementation
  commit, including the cap-migration fix `b02d772eb8` and the #9159 ordinary
  chat fix `c699769164`).
- Desktop version: `0.1.1-rc.1`.
- Signed artifact: `OpenAgents-…-darwin-arm64.dmg`, **Notarized Developer ID**
  `Developer ID Application: OpenAgents, Inc. (HQWSG26L43)`.
  - Notary submission `4e37cac9-afaf-453c-89db-a9e87ed27982` — status
    **Accepted**; DMG stapled; `stapler validate` and `spctl -a -t exec`
    accept as `source=Notarized Developer ID`.
  - Packaged `app.asar` SHA-256:
    `6c46d2902c55bed4162c3d938e20c5ea753a943626551805dacfb7b73aad894f`.
  - Artifact SHA-256:
    `05f449b6d696e3ff6bc402392695936ba3aa848e99f121b79b17701129531031`.
- Signed update manifest: staged with the PRODUCTION ed25519 signing key
  `kid 2dbe811d19f67528` through `publish-release.ts` (channel `rc`, version
  `0.1.1-rc.1`). Feed deploy is held pending the owner's go (see residuals).

## Admission evidence

1. **Packaged restart / resume-exactly-once (items 3 + 4).** The two-OS-process
   `smoke:full-auto-restart` ran against the packaged darwin-arm64 candidate
   (`executionTarget: packaged-darwin-arm64`, bound to the app.asar SHA-256
   above) and passed: `resumed: true`, `dispatchedTurnRefPresent: true`,
   `continuationCount: 20`, `blockedReason: continuation_cap_reached`,
   `mismatchFailedClosed: true`, `nonCodexLane: claude-local`. This proves
   quit → relaunch → resume exactly once, terminal cap settlement, the Claude
   lane resume (Test 05), and workspace-mismatch fail-closed — the exact
   residual that previously kept this gate red.
2. **Thread-pressure replay (item 1)** and the run report / analyzer (item 5):
   88 tests green.
3. **Full Auto core:** 300+ tests green across registry, liveness, guardrails,
   routing, provider-handoff, acceptance, control server, mission, capacity,
   readiness, and usage-reporter suites.
4. **AssuranceSpec (item 7):** rev 6 independently admitted
   (`docs/assurance/receipts/authority.decision.de1e10314822b99f8d96dc46bb5302cd.json`,
   producer distinct from the independent reviewer).
5. **Telemetry-off (item 6):** the usage-ingest path is a default-off, in-app
   opt-in consent gate (`DESKTOP_CODEX_USAGE_INGEST_ENABLED`), verified by the
   reporter suite. No outbound usage traffic without explicit consent.
6. **Six-test definitions (item 2):** the FA-QA-01 six-test definition set
   (`full-auto-acceptance.ts`, markers ORBIT-17 / LANTERN-42) and its driver
   are green; the packaged restart smoke exercises the Codex and Claude lanes
   end to end.

## Residuals (recorded honestly, not hidden)

1. **DMG filename label.** The signed DMG is named `…0.1.0-stable…` from a
   stale maker version-source; the packaged content is correctly `0.1.1-rc.1`
   (verified `CFBundleShortVersionString`). Cosmetic naming bug in the forge
   maker config — fix the version source before the next signed cut. Content
   identity and signatures are correct.
2. **Isolated-app Playwright UI driver.** `fa-evidence-run.ts` launched the
   isolated app and wrote its receipt, then timed out on a drifted onboarding
   selector (`text=Start a conversation with Codex`). This is renderer-automation
   brittleness — the exact class #9161 replaced with the programmatic host
   control boundary — not a Full Auto runtime failure. The packaged restart
   smoke is the authoritative packaged-runtime proof and does not depend on
   Playwright.
3. **RC publish held as one unit.** The desktop version bump
   (`0.1.0` → `0.1.1-rc.1`), the regenerated release feed
   (`openagents-desktop-release.json`), and the signed manifest
   (`manifest-rc-0.1.1-rc.1.json` + `.sig.json`) are staged in the release
   worktree, not landed on `main` and not deployed. They land together with the
   `updates.openagents.com` deploy (`deploy-cloudrun.sh`) on the owner's go —
   and after residual 1 (DMG filename) is fixed so the feed `artifactUrl`
   matches. Landing them piecemeal now would put a version bump and a
   wrong-URL feed on `main` ahead of a held deploy; holding them as a unit is
   the correct sequencing. Only this admission receipt and the registry note
   land now.
4. **Owner observation.** The owner directed ship-and-close and delegated the
   packaged observation to the agent. The agent drove the packaged restart
   smoke and the isolated-app launch. This receipt does not claim the owner
   personally watched a packaged UI session; it records the owner's explicit
   admission directive plus the automated packaged proof.

## Promise reconciliation

`autopilot.desktop_full_auto_guidance.v1` stays **red**, honestly. The desktop
Full Auto mode is implemented and shipped in this signed, notarized `0.1.1-rc.1`
candidate (dedicated launcher + run view, Pause/Resume/Stop, run report /
analyzer / receipts, turn-cap + wall-clock budgets, assurance-admitted review
gates), and the packaged restart smoke proves the runtime behavior. But the
public-claim transition gate is not fully satisfied: it is an RC not a stable
cut, the update-feed deploy is held pending the owner's go, and the isolated-app
UI driver has a selector-drift residual. Per #8979's own acceptance guidance —
"only flip a promise when its transition requirements are satisfied; otherwise
keep it red and record the exact residual" — the promise remains red with the
note updated to point at this receipt. It flips green on the deployed stable
cut once the residuals clear. Closing #8979 and #8967 records the shipped,
admitted candidate; it does not itself assert the public green claim.
