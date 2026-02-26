# OpenAgents macOS Release Plan (MVP)

## Goal

Define the macOS release flow for `apps/autopilot-desktop` with one script:

- versioning
- testing/build
- packaging
- signing/notarization
- publish

## Entry Point

- `scripts/release/macos-release.sh`

### Typical commands

Unsigned local build:

```bash
./scripts/release/macos-release.sh --version 0.1.1 --allow-unsigned
```

Signed + published build:

```bash
./scripts/release/macos-release.sh --version 0.1.1 --publish
```

Version bump mode:

```bash
./scripts/release/macos-release.sh --bump patch --publish
```

## Signing Setup

Signing/notarization setup is documented in:

- `docs/deploy/MACOS_SIGNING_NOTARIZATION.md`

Script-required env vars for signed mode:

- `MACOS_SIGNING_IDENTITY`
- `MACOS_TEAM_ID`
- `APPLE_NOTARIZATION_KEY`
- `APPLE_NOTARIZATION_KEY_ID`
- `APPLE_NOTARIZATION_ISSUER_ID`

If `--allow-unsigned` is not set, missing any of these fails the release.

## Artifact and Tag Contract (Current Script Behavior)

For target version `X.Y.Z`, script creates:

- `target/release/Autopilot-X.Y.Z.dmg`
- `target/release/Autopilot-X.Y.Z.dmg.sha256`

Script-managed git/release naming:

- tag: `vX.Y.Z`
- release title: `Autopilot vX.Y.Z`

Manual prerelease tags such as `autopilot-0.1.0-rc1` can still be used outside this script when needed.

## Preconditions

Script fails fast unless all are true:

1. Running on macOS.
2. Git working tree is clean.
3. Current branch is `main`.
4. Local `main` equals `origin/main`.
5. Exactly one of `--bump` or `--version` is provided.

## Required Tooling

- `cargo`
- `git`
- `hdiutil`
- `shasum`
- `gh` (only when `--publish`)
- `codesign` + `xcrun` (only for signed mode)

`cargo-bundle` is installed automatically if missing.

## Script Flow

1. Validate environment + git state.
2. Resolve version and verify tag does not already exist.
3. Update versions in `Cargo.toml` and `apps/autopilot-desktop/Cargo.toml`.
4. Run:
   - `scripts/lint/ownership-boundary-check.sh`
   - `cargo test --workspace`
   - `cargo build --release -p autopilot-desktop`
5. Bundle app with `cargo bundle`.
6. Sign app (`codesign`) unless `--allow-unsigned`.
7. Build DMG with `hdiutil`.
8. Notarize + staple DMG unless `--allow-unsigned`.
9. Write SHA256 checksum.
10. Commit version bump and create annotated git tag.
11. If `--publish`, push and run `gh release create` with DMG + checksum.

## Failure / Recovery

- Script exits non-zero on any failure.
- If failure happens before commit/tag, no history mutation remains.
- If commit/tag was created, script prints recovery commands.
- Script never force-pushes.
