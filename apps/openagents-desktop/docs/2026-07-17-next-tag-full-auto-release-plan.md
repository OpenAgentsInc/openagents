# Next tag release plan — first tag containing Full Auto (REL-FEED-01, #8993)

Date prepared: 2026-07-17 (worker lane oa-w0-f). Part of EPIC #8913.
This is the NEEDS_OWNER-compatible ceremony record required by #8993. It
PREPARES the release; it does not sign, tag, publish, or promote anything.
Every owner-gated step below stays owner-gated.

The operator sequence authority is
[`docs/deploy/openagents-desktop-production-release.md`](../../../docs/deploy/openagents-desktop-production-release.md)
(§10 is the currently sanctioned macOS arm64 RC compatibility lane). This file
instantiates that runbook for the specific next tag with verified current
facts.

## 1. Current tag state (verified 2026-07-17)

| Fact | Value | Verification |
| --- | --- | --- |
| Newest Desktop tag | `openagents-desktop-v0.1.0-rc.17` | `git tag --list 'openagents-desktop-*' \| sort -V \| tail -1` |
| rc.17 tag commit | `c2f52da919` (2026-07-17 14:23:21 -0500) | `git show openagents-desktop-v0.1.0-rc.17 --no-patch` |
| rc.17 is an ancestor of `main` | yes | `git merge-base --is-ancestor openagents-desktop-v0.1.0-rc.17 origin/main` → exit 0 |
| FullAutoRun model commit | `eb15ce99c5` (FA-RUN-01 #8969, 2026-07-17 15:39:30 -0500) | introduced `src/full-auto-run-registry.ts` |
| **rc.17 contains the FullAutoRun model** | **NO** | `git merge-base --is-ancestor eb15ce99c5 openagents-desktop-v0.1.0-rc.17` → exit 1 |
| `main` at preparation time | `88b126733b` | `git rev-parse origin/main` |
| Commits rc.17 → main | 117 total; 36 touch `apps/openagents-desktop` | `git rev-list --count openagents-desktop-v0.1.0-rc.17..origin/main` |
| `package.json` version already staged | `0.1.0-rc.18` | `apps/openagents-desktop/package.json` |

rc.17 does carry the earlier Full Auto lane/registry/control-server files, but
NOT the durable FullAutoRun objective/lifecycle/control model or anything
after it. The Full Auto loop the GUARANTEES page describes has therefore never
shipped in any tag — this is the #1 gap named by the Full Auto audit and issue
#8993.

## 2. What the next tag must include

Tag name: `openagents-desktop-v0.1.0-rc.18` · channel `rc` · version
`0.1.0-rc.18` (already in `package.json`; no bump commit needed unless further
rc.18-scoped work lands first).

The tag commit is the exact `origin/main` HEAD frozen at ceremony start. It
MUST be `88b126733b` or newer, and MUST satisfy (re-verify at freeze):

```sh
git merge-base --is-ancestor eb15ce99c54af497874a998192b1afbb2fa8268b <tag-commit>   # exit 0
```

Full Auto commits that enter a Desktop release for the first time
(rc.17..main, `full-auto-*` sources):

- `eb15ce99c5` FA-RUN-01 durable FullAutoRun objective/lifecycle/migration/control model (#8969)
- `d9b3b8ed6f` FA-RUN-03 main-owned liveness/stall detection (#8971)
- `e82f3a44a5` FA-RUN-04 bounded private FullAutoRunReport + public-safe receipt (#8972)
- `0314f4d4cd` FA-RUN-05 offline/private dogfood run analyzer (#8973)
- `0dae0911bf` FA-RUN-05 live FullAutoRun projection for mobile (#8981)
- `26f4cd3206` + `20f352f332` FA-HO-01 receipted cross-provider handoff (#8975)
- `9e0314e00a` FA-QA-01 typed handoff acceptance harness (#8976)
- `84cbe56589` FA-RPT-01 derived run report + metrics default-on (#8988)
- `c3dd17313e` FA-RT-01 multi-lane routing policy (#8987)

Also new since rc.17 (this change): the REL-FEED-01 update-feed configuration
seam (`src/update-feed-config.ts`) — production default unchanged, staging
override fail-closed — plus the staging-channel end-to-end feed proof.

## 3. Preflight check-mode record (run 2026-07-17, prep worktree)

`node --import tsx scripts/release-preflight.ts --channel rc
--latest-released 0.1.0-rc.17 --json` after `pnpm run build`:

- PASS `version_monotonic` — 0.1.0-rc.17 → 0.1.0-rc.18 strict rc upgrade
- PASS `attribution_intact`, `app_identity_stable`, `artifact_set_complete`
  (all 11), `no_upstream_updater_remnants`, `no_legacy_ui_entrypoints`,
  `no_source_checkout_paths`
- FAIL `clean_origin_main` — expected here: the prep worktree carried the
  uncommitted REL-FEED-01 change. At ceremony time the owner runs from a
  clean `origin/main`; this row must be green.
- FAIL `signing_credentials_present` — expected and CORRECT: this agent
  session holds no Developer ID / notary credentials and there is no unsigned
  fallback. Green only in the owner's credentialed shell.

No other row is permitted to be red at ceremony time.

## 4. Staging-channel rehearsal (no owner secrets)

Before the production ceremony, the update cycle is provable end to end
against a staging feed with throwaway keys:

```sh
pnpm --dir apps/oa-updates exec vp test --run src/desktop-staging-feed-e2e.test.ts
```

This drives a live local instance of the oa-updates ReleaseSet v2 feed
(admission → CAS promotion → HTTP serving) and the real Desktop update host
through discover → pinned-verify → stage → apply → first-launch receipt →
retained-slot rollback → feed pointer rollback, and proves a
production-pinned client refuses the staging feed. For a packaged-build
rehearsal against a deployed staging Cloud Run instance, launch the packaged
app with:

```sh
OPENAGENTS_DESKTOP_UPDATE_FEED_BASE_URL=<staging-feed-origin> \
OPENAGENTS_DESKTOP_UPDATE_FEED_STAGING_PIN='<staging PUBLIC pin JSON>' \
  open -a OpenAgents   # or the packaged binary directly
```

Both variables default to off; unset means the production feed + production
pin, byte-identical to prior behavior. A staging pin never applies to the
production host, and any invalid override disables update checks entirely.

## 5. Ordered owner ceremony (owner-gated; do not delegate)

Per `docs/deploy/openagents-desktop-production-release.md` §10. Steps marked
**[OWNER]** need the owner's credentials/custody and must be run by the owner.

1. Freeze: fresh worktree at exact `origin/main`; verify clean tree, record
   the tag commit SHA, and re-run the §1 `merge-base --is-ancestor` check for
   `eb15ce99c5`.
2. Gates: run the §10 focused contract gate set, `typecheck`, `build`, then
   check-mode preflight (`--channel rc --latest-released 0.1.0-rc.17`).
   Everything except `signing_credentials_present` must be green in a
   non-credentialed shell.
3. **[OWNER] Sign + notarize:** in the credentialed shell (Developer ID
   `OpenAgents, Inc. (HQWSG26L43)`, `ASC_API_*`, per the release-signing
   runbook): `pnpm --dir apps/openagents-desktop run make:mac`. Wait for the
   Apple notarization ticket; the make fails closed unless every Gatekeeper
   oracle (codesign/spctl/stapler on BOTH `.app` and `.dmg`) is green.
4. **[OWNER] Post-staple preflight:** re-run preflight with `--dmg`/`--app`
   against the final stapled bytes; record sha256 + byte length of the DMG.
5. **[OWNER] Sign the update manifest:** stage v1 via
   `scripts/publish-release.ts --channel rc --version 0.1.0-rc.18` with
   `OPENAGENTS_RELEASE_SECRETS_PATH` (production Ed25519 key, kid
   `2dbe811d19f67528`; never printed). The script self-verifies through the
   client seam and prints the exact artifact upload command.
6. **[OWNER] Upload + candidate deploy:** upload the immutable DMG bytes to
   the printed GCS URL; deploy oa-updates via the incremental Cloud Build
   path (`OA_UPDATES_DEPLOY_MODE` auto-selects; never a metadata-only
   `--source .` deploy). Verify the three rc documents and the mobile
   manifest through the tagged candidate BEFORE any traffic moves (§10 curl
   block).
7. **[OWNER] Tag:** only after the candidate verifies:
   `git tag openagents-desktop-v0.1.0-rc.18 <frozen-commit> && git push origin openagents-desktop-v0.1.0-rc.18`.
8. **[OWNER] Promote traffic** to the candidate revision, then run the §10
   post-promotion acceptance: download the public DMG fresh, verify signed
   sha256/length, mount, packaged smoke (`[openagents-desktop smoke] OK`),
   and the bundled Codex artifact oracle (`state:"ready"`,
   `signatureVerified:true`).
9. Update-path acceptance: on a machine running rc.17, let the packaged app
   check → stage → apply rc.18; confirm the first-launch receipt lands and
   the retained rc.17 slot exists; record the receipt per §16.

## 6. Verification oracles that must pass

- `apps/oa-updates/src/desktop-staging-feed-e2e.test.ts` (staging cycle e2e)
- `apps/openagents-desktop/src/update-feed-config.test.ts` (feed override
  fail-closed rules)
- The §10 focused gate set: `release-preflight`, `update-contract`,
  `publish-release`, `package-macos`, `macos-gatekeeper`, `launch-receipt`,
  `update-rollback` test files
- `release-preflight.ts` all-green in the owner shell (steps 2 and 4)
- Gatekeeper oracles inside `make:mac` (notarized Developer ID acceptance)
- Candidate-URL curl verification incl. mobile manifest preservation (step 6)
- Packaged smoke + Codex artifact oracle post-promotion (step 8)

## 7. Rollback plan

- **Before promotion:** abandon the candidate; production pointer never
  moved. No client impact.
- **Service revision unhealthy:** move Cloud Run traffic back to the previous
  ready revision; this does not touch the signed channel pointer.
- **Bad release after promotion:** never repoint the channel to an older
  version. Publish a strictly newer corrective release through the same
  ceremony. Installed clients hold in `awaiting_launch_receipt` and
  auto-roll-back to their retained previous slot within the bounded 10-minute
  window if the new build never demonstrates a healthy launch (proven in the
  staging e2e and `update-rollback` oracles).
- **ReleaseSet v2 feed (when v2 publication begins):** typed CAS pointer
  rollback restores exactly the one retained previous generation; clients
  refuse the resulting downgrade on the feed and only their local retained
  slot may go backward (proven in the staging e2e).

## 8. Honest non-claims

- This plan does not claim automatic update delivery on production. That
  becomes true only after the owner ceremony above promotes rc.18 and the
  rc.17 → rc.18 update-path acceptance (step 9) is recorded.
- The v2 ReleaseSet public candidate for production remains unpublished; the
  sanctioned production lane for this tag is still the §10 v1 macOS arm64
  compatibility procedure. The staging e2e proves the v2 machinery, not a
  production v2 receipt.
- No cross-platform target beyond darwin-arm64 gains support from this tag.
