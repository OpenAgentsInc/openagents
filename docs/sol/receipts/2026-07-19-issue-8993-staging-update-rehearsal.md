# Issue #8993 — staging update rehearsal evidence

- Issue: #8993
- Date: 2026-07-19
- Authority: `AUTHORITY.md` revision 5, `grant.autonomous_rc_release_and_communication`
- Actor: Codex open-issue sweep
- Production promotion: **not performed**

## What was established

An isolated ReleaseSet v2 staging service is deployed at
`https://oa-updates-staging-157437760789.us-central1.run.app`. Its backing
bucket is versioned, its signing key is stored only in Google Secret Manager,
and the checked-in public staging pin is distinct from production trust. The
service currently returns no channel pointer because no candidate has passed
the packaged update rehearsal; it has not modified the production update
service, stable channel, or signed Desktop feed.

The real public arm64 artifacts for `0.1.0-rc.19` and `0.1.0-rc.20` were then
used with the production updater and release-acceptance driver. The rehearsal
proved both distributed DMGs have stapled outer tickets and that Gatekeeper
accepts their contained apps as Notarized Developer ID software. It also
found three concrete mismatches before any channel promotion:

1. the native updater assumed a mounted bundle named `OpenAgents.app`, while
   the RC artifact contains `OpenAgents RC.app`;
2. it assumed the stable bundle identifier for RC artifacts and rejected the
   real `com.openagents.desktop.rc` identity;
3. the packaging pipeline stapled the out-directory app only after MakerDMG
   had already captured an unstapled copy, so the app inside the immutable
   DMG fails `syspolicy_check distribution` with `Notary Ticket Missing`.

The first two mismatches are repaired by discovering exactly one ordinary app
directory and then verifying its channel-specific signed identity and bounded
executable name. The packaging order is repaired by notarizing and stapling
the signed app before any maker snapshots it, followed by separate
notarization and stapling of the completed DMG. The acceptance driver now also
provides real external-state migration evidence, child-runtime drain evidence,
and a launch/clean-shutdown receipt instead of stopping at byte replacement.

## Honest remaining gate

Existing RC19/RC20 bytes cannot satisfy the updater's offline app-ticket gate;
published artifacts are immutable and are not replaced. A strictly newer RC
must be built with the repaired order and must pass the complete
previous-version → candidate → launch receipt → retained-slot rollback
rehearsal before #8993 can close. This machine has the Developer ID identity
but did not have an App Store Connect notary credential available during this
run, so no corrected candidate was fabricated and the issue remains open.

This is a credential availability gate, not permission to weaken notarization,
promote the empty staging pointer, or claim cross-platform release evidence.

## Verification performed

- focused native updater tests, including channel-named app discovery,
  ambiguous-DMG refusal, channel identity, and path-safe executable names;
- real `xcrun stapler validate` against both RC DMGs;
- real `spctl --assess --type execute --verbose=4` against the contained app;
- real `xcrun syspolicy_check distribution` exposing the missing nested-app
  ticket;
- release acceptance through signed-feed verification, digest-verified
  staging, interruption/reopen, migration admission, and native candidate
  verification, where it correctly stopped at the missing ticket.
