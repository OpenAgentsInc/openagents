# T3UI-12 component census and packaged evidence receipt

- Date: 2026-07-17
- Program: [T3 Code UI full harvest](../../sol/2026-07-17-t3-code-ui-full-harvest-accepted-plan.md)
- Source pin: `pingdotgg/t3code@8b5469863ae1dd696e696de30240ec3da607962d`
- T3 component tree: `f45f45cb389357ca112e2552c80ec3b57926731b`
- Packaged OpenAgents source: `OpenAgentsInc/openagents@1f42cd2998b352575f3092e0ae5b88e5190e1fad`
- Scope: exact component census, mounted catalog, and local packaged candidate

## Exact census

The checked census covers all 151 non-test `.tsx` files below
`apps/web/src/components` at the source pin. A direct Git tree comparison
proved the recorded path and blob-id sequence byte-for-byte identical to the
pinned source: 151 rows, zero duplicates, and zero undisposed components.

| Disposition | Count | Meaning |
| --- | ---: | --- |
| Adapted | 99 | The user job is mounted through OpenAgents composition and typed authority. |
| Covered | 46 | A shared OpenAgents component family covers the job without a one-for-one port. |
| Rejected | 6 | T3-specific or currently unadmitted authority is intentionally excluded with a row-level reason. |

The six explicit exclusions are arbitrary project-script execution, forge
pull-request thread mutation, two T3/Clerk account components, T3 relay
installation, and renderer-side SSH password collection. Their unavailable or
boundary presentation remains mounted; their service/credential authority was
not copied.

Each census row names existing OpenAgents source evidence and valid mounted
fixture ids. The checked catalog is exactly the 24-state deterministic visual
lane; a fresh comparison reported zero different pixels for every frame.

## Packaged candidate

The isolated `darwin-arm64` unsigned-development package was built from the
published source commit above, not the mutable checkout. Staging emitted ledger
`sha256:6295cbd544c445835c3f1d1bf9484eedef8dac600373cee26a82ea63288b4b5c`.
Forge verified all seven closure components before copying, then its post-package
ASAR gate passed with 47 entries, 40 unpacked entries, and all seven closure
components byte-verified.

The packaged `0.1.0-rc.17` app passed the canonical React journey: exclusive
React workbench mount, composer focus and first keystroke, image attachment,
authoritative turn reconciliation, stable transcript geometry, Full Auto state,
history navigation, sidebar persistence, reload, empty new session, and clean
lifecycle teardown. The packaged Codex artifact resolved under a minimal PATH,
matched pinned version `0.144.1`, and returned identity digest
`29915529b97697def1a957b0505e770aa6a45744435d62fc263e98d7619e167a`.

## Verification

- The publication gate passed 216 Desktop test files: 2,079 passing and 39
  skipped, plus production build, compatibility Electron smoke, canonical
  React Electron smoke, protocol-generation checks, and repository policy
  guards.
- The census oracle adds four tests covering the exact source/tree/denominator,
  row dispositions, evidence paths, fixture membership, and narrow exclusions.
- The Sol document and 24-frame visual gates pass.

## Honest residuals

This receipt is not signed-release evidence. The package is explicitly
unsigned-development; Developer ID signing, notarization, stapling, release-feed
signing, promotion, and cross-platform artifacts remain separate release
authorities. The workspace owner ledger already carries the open packaged/signed
Desktop observation and release-worker actions, so this packet does not create a
duplicate owner request.

The older compatibility-renderer packaged smoke reproduced one editor-tab
recovery miss after reload (`stored: true`, `editor: false`) after all preceding
steps passed. The installed product uses the canonical React renderer, whose
packaged reload journey passed; the compatibility-only miss is recorded here and
is not presented as green installed evidence.

Accordingly, T3UI-01 through T3UI-12 are implemented and published, and the
pinned UI denominator has no undisposed component. Full signed T3 product parity
is not claimed until current signed installed evidence and the deliberately
excluded authorities are separately admitted or retained as product decisions.
