# OpenAgents macOS Release Plan (MVP)

## Goal

Define **our** production deployment process for macOS only, with one command/script that can run the full release flow:

- versioning
- tagging
- verification/testing
- building/packaging
- signing/notarization
- publishing

## Scope

In scope:

- `apps/autopilot-desktop` release flow on macOS
- Git tag + GitHub release publication
- `.app` + `.dmg` artifact generation
- optional code-sign + notarize + staple

Out of scope (for now):

- Windows
- Linux
- multi-channel release complexity (`preview`, `nightly`, etc.)

## Proposed Single Entry Point

Create one script:

- `scripts/release/macos-release.sh`

Primary command:

```bash
./scripts/release/macos-release.sh --bump patch --publish
```

Alternative explicit version:

```bash
./scripts/release/macos-release.sh --version 0.1.1 --publish
```

## Release Artifact Contract

For version `X.Y.Z`, script must produce:

- `target/release/Autopilot-X.Y.Z.dmg`
- `target/release/Autopilot-X.Y.Z.dmg.sha256`

And publish GitHub release/tag:

- tag: `vX.Y.Z`
- release title: `Autopilot vX.Y.Z`
- assets: DMG + checksum

## Version Source of Truth

Initial MVP rule:

- Keep `version` aligned in:
  - `Cargo.toml` workspace package version
  - `apps/autopilot-desktop/Cargo.toml` package version

Script updates both in one step before commit/tag.

## Required Preconditions

Script fails fast unless all are true:

1. Running on macOS.
2. Git working tree is clean.
3. On `main` branch.
4. Local `main` is up to date with `origin/main`.
5. Either `--bump {patch|minor|major}` or `--version X.Y.Z` is provided (not both).

## Required Tooling

- `cargo`
- `git`
- `gh`
- `hdiutil`
- `codesign` (if signing enabled)
- `xcrun notarytool` + `xcrun stapler` (if notarization enabled)

Script installs `cargo-bundle` if missing.

## Signing and Notarization Inputs

Support two modes:

1. Unsigned local release (`--allow-unsigned`).
2. Production signed release (default in CI).

For signed mode, require:

- `MACOS_SIGNING_IDENTITY`
- `MACOS_TEAM_ID`
- `APPLE_NOTARIZATION_KEY`
- `APPLE_NOTARIZATION_KEY_ID`
- `APPLE_NOTARIZATION_ISSUER_ID`

If signed mode is requested and any secret is missing, fail.

## End-to-End Flow (Script Behavior)

1. **Preflight checks**
   - validate OS/tools/branch/clean tree/upstream sync
2. **Resolve target version**
   - from `--version` or computed from `--bump`
   - ensure tag `vX.Y.Z` does not already exist
3. **Update versions**
   - update workspace/app versions
   - run minimal consistency check
4. **Quality gates**
   - `scripts/lint/ownership-boundary-check.sh`
   - `cargo test --workspace`
   - `cargo build --release -p autopilot-desktop`
5. **Bundle app**
   - `cargo bundle --release -p autopilot-desktop`
   - locate `.app` output
6. **Sign app (if enabled)**
   - sign nested binaries and `.app`
7. **Build DMG**
   - package `.app` into `Autopilot-X.Y.Z.dmg`
8. **Notarize/staple (if enabled)**
   - submit DMG with `notarytool`
   - staple ticket
9. **Checksums**
   - write `Autopilot-X.Y.Z.dmg.sha256`
10. **Commit + tag**
    - commit version bump only
    - create annotated tag `vX.Y.Z`
11. **Push + publish (if `--publish`)**
    - push commit + tag
    - `gh release create vX.Y.Z` with assets

## Failure and Rollback Rules

- Any failure stops immediately with non-zero exit.
- If failure occurs before commit/tag, no git history mutation remains.
- If commit exists but tag/publish fails, script prints exact recovery commands.
- Script never force-pushes.

## Initial Implementation Phases

1. **Phase 1**
   - implement preflight + version bump + tests + build + DMG
   - no signing/notarization
2. **Phase 2**
   - add signing/notarization and checksum generation
3. **Phase 3**
   - add publish path (tag + GitHub release via `gh`)

## Definition of Done

This plan is complete when:

1. Running one command from `main` can produce a versioned DMG.
2. Same command can optionally publish tag + GitHub release.
3. Script behavior is deterministic, fail-fast, and documented with recovery steps.
