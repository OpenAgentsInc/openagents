# Update And Release System Audit

**STATUS: HISTORICAL — point-in-time record (accurate as of its
date). Not current direction; consult MASTER_ROADMAP.**


Date: 2026-06-11

This is system #49 from the Bun/Effect terminal-agent systems list. It defines
how the terminal agent should publish, install, verify, roll back, and report
updates across CLI, TUI, Pylon, package, and managed environments.

## Target

Build an update and release system with signed release metadata, platform
support declarations, rollout channels, compatibility checks, smoke receipts,
and rollback paths.

## User-Visible Capability

Users should be able to:

- See the installed version and channel.
- Check for updates.
- Read whether an update is required, recommended, or blocked.
- Install an update safely.
- Roll back when supported.
- Understand platform support and known blockers.
- See release notes focused on runtime behavior and safety changes.

Managed environments should be able to pin versions and deny automatic
updates.

## Release Model

Each release should include:

- Version.
- Channel.
- Platform artifacts.
- Checksums and signatures.
- Minimum runtime requirements.
- Data migration requirements.
- Compatibility matrix.
- Smoke receipt refs.
- Known blockers.
- Rollback policy.
- Deprecation and support dates.

Release state should be queryable from the terminal without contacting
unapproved third-party endpoints.

## Bun/Effect Boundary

Use Effect services for:

- `ReleaseMetadataService`: fetches and verifies release manifests.
- `UpdatePolicyService`: resolves channel, pin, and managed policy.
- `InstallerService`: installs or prepares platform artifacts.
- `RollbackService`: restores a previous version where supported.
- `ReleaseSmokeService`: verifies post-install behavior.
- `ReleaseProjectionService`: shows version and update state.

Use Schema for release manifests, platform artifacts, channels, and smoke
receipts. Use Scope for temporary installer material. Use Schedule for update
checks with jitter.

## Safety Rules

- Verify signatures and checksums before install.
- Do not auto-update while a run is active unless policy says it is safe.
- Never run migrations without a restore point or rollback boundary.
- Managed pins override user channel preferences.
- Release notes cannot claim capabilities without receipt refs.
- Failed updates leave the previous working version intact where possible.
- Update checks must not expose private repo or user data.

## OpenAgents Translation Notes

As of 2026-06-11, OpenAgents has Pylon release preparation docs, launch gates,
package-launcher smokes, product-promise gates, and MVP exit-review issues.
The terminal-agent README does not yet include an update/release audit.

Related open issue anchors:

- #4772 MVP exit review and door-open gate.
- #4786 Autopilot MVP ladder.
- #4785 settlement visibility law for claims involving payout visibility.
- #4768 proof smoke before public capability claims.

No release copy should say a terminal-agent capability is stable until the
release manifest, install smoke, rollback story, and claim receipts support
it.

## Tests

Minimum coverage:

- Verify release manifest signatures and checksums.
- Install and reject platform-specific artifacts.
- Respect channel pins and managed policy.
- Block unsafe updates during active runs.
- Run post-install smoke checks.
- Roll back after failed installation.
- Preserve local settings and event logs.
- Ensure release notes link only to supported receipt refs.

## Decision

Updates and releases should be governed by signed metadata and receipts, not
ad hoc version strings or optimistic product copy.
