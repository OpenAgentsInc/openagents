# OpenAgents Desktop Deployment Considerations (Modeled on Zed)

## Purpose

This document captures how `~/code/zed` builds and ships desktop releases, then translates that into practical deployment considerations for OpenAgents MVP.

Scope is desktop app build + release distribution for:

- macOS
- Windows
- Linux

This is a planning/spec document only. No CI/workflow files are added in this repo.

## What Zed Actually Does (Observed)

### 1. Release channels and version/tag discipline

Zed treats release channel as first-class runtime/build metadata.

- Channel source of truth: `crates/zed/RELEASE_CHANNEL`
- Channels used: `dev`, `nightly`, `preview`, `stable`
- Tag validation is strict in release workflows:
  - `stable` expects `v{version}`
  - `preview` expects `v{version}-pre`
  - nightly is handled by a dedicated `release_nightly` flow and moving `nightly` tag

Representative files:

- `.github/workflows/release.yml`
- `.github/workflows/release_nightly.yml`
- `script/determine-release-channel`
- `script/determine-release-channel.ps1`
- `crates/release_channel/src/lib.rs`

### 2. CI quality gates before packaging

Release tags do not immediately package. Zed first runs cross-platform checks:

- tests on macOS/Linux/Windows (`cargo nextest`)
- clippy on macOS/Linux/Windows
- script/workflow checks (`shellcheck`, `actionlint`, generated workflow drift checks)

Representative files:

- `.github/workflows/release.yml`
- `.github/workflows/run_tests.yml`
- `.cargo/ci-config.toml`
- `script/setup-sccache`, `script/setup-sccache.ps1`
- `script/clear-target-dir-if-larger-than*`

### 3. Per-OS bundling scripts (not one giant generic script)

Zed uses OS-specific scripts with explicit platform behavior:

- macOS: `script/bundle-mac`
- Linux: `script/bundle-linux`
- Windows: `script/bundle-windows.ps1`

This keeps signing/notarization/installer logic native to each platform.

### 4. macOS production flow

Key behaviors:

- Uses `cargo-bundle` (Zed-maintained branch) to generate `.app`
- Selects channel-specific bundle metadata by rewriting `Cargo.toml` bundle section
- Signs binaries and app bundle with Apple cert
- Builds `.dmg`
- Adds license text to DMG
- Notarizes with `notarytool` and staples ticket
- Publishes remote server artifact separately (`zed-remote-server-macos-{arch}.gz`)

Representative files:

- `script/bundle-mac`
- `crates/zed/Cargo.toml` (`package.metadata.bundle-*`)
- `crates/zed/contents/*/embedded.provisionprofile`

### 5. Windows production flow

Key behaviors:

- Builds multiple binaries (`zed`, `cli`, `auto_update_helper`, explorer integration bits)
- Builds `remote_server.exe` separately and zips it
- Produces signed installer with Inno Setup (`.exe`)
- Uses Azure Trusted Signing (`Invoke-TrustedSigning`) for code signing
- Publishes/updates WinGet after release
- Uses helper process for post-quit in-place updates (`auto_update_helper`)

Representative files:

- `script/bundle-windows.ps1`
- `crates/zed/resources/windows/sign.ps1`
- `crates/zed/resources/windows/zed.iss`
- `.github/workflows/after_release.yml`
- `crates/auto_update_helper/src/*`

### 6. Linux production flow

Key behaviors:

- Builds `zed` + `cli` and separate `remote_server`
- Creates `zed[-channel].app/` tarball
- Copies runtime dynamic libraries into package
- Emits desktop file + icons with channel-specific app identity
- Strips debug symbols in release artifacts
- Supports package-manager builds that disable in-app updater messaging via env var

Representative files:

- `script/bundle-linux`
- `script/install.sh`
- `script/install-linux`
- `docs/src/development/linux.md`

### 7. Artifact promotion/publication

For tagged releases:

- Build jobs upload artifacts to workflow artifacts
- `upload_release_assets` job assembles and uploads to GitHub release draft
- `validate_release_assets` checks complete expected asset set
- preview tags can auto-publish from draft

For nightly:

- nightly artifacts uploaded to object storage bucket (`zed-nightly-host`)
- `nightly/latest-sha` updated
- moving `nightly` git tag forced to current commit

Representative files:

- `.github/workflows/release.yml`
- `.github/workflows/release_nightly.yml`
- `script/create-draft-release`
- `script/upload-nightly`
- `script/lib/blob-store.sh`

### 8. Post-release production updates

After a release is published:

- refreshes release metadata endpoint backing downloads
- redeploys site/release page
- posts release note notification
- publishes WinGet
- creates Sentry release

Representative file:

- `.github/workflows/after_release.yml`

### 9. Updater contract and distribution API

Client updater resolves release assets from endpoint pattern:

- `GET /releases/{channel}/{version|latest}/asset?asset=...&os=...&arch=...`
- response includes version + download URL

It separately resolves remote server artifacts using same endpoint shape with different `asset`.

Representative files:

- `crates/auto_update/src/auto_update.rs`
- `script/install.sh`
- `script/get-released-version`

## OpenAgents Deployment Considerations

### Cross-platform baseline (recommended)

1. Create an explicit release-channel source of truth (file + runtime exposure), even if MVP starts with just `stable`.
2. Make per-OS bundling scripts authoritative for packaging/signing.
3. Define immutable artifact names per OS/arch before writing updater code.
4. Keep release publication as "build everything first, promote after validation."
5. Separate "app artifact" and "remote helper/server artifact" if runtime architecture requires both.
6. Keep updater override for package-managed installs (`UPDATE_EXPLANATION`-style env) to avoid broken update UX.
7. Include symbol upload hooks (retry with backoff) and release object creation (Sentry or equivalent).
8. Enforce license/compliance check in release gates if distribution constraints differ by crate.
9. Keep CI build acceleration (sccache + linker strategy) as part of release reliability, not only dev UX.

### macOS considerations for OpenAgents

1. Use channel-specific bundle ID/name/icon from metadata, not ad hoc renaming.
2. Sign nested binaries and parent app in deterministic order.
3. Keep hardened runtime + entitlements managed in repo.
4. Produce DMG and notarize/staple in CI.
5. Treat provisioning profiles/keys as release-channel aware secrets.
6. Decide minimum supported macOS version now and keep it in build metadata.

### Windows considerations for OpenAgents

1. Decide installer system early (Inno Setup is proven in Zed) and standardize on one.
2. Use cloud-backed code signing (Azure Trusted Signing or equivalent) with timestamping.
3. Package updater helper if in-place overwrite is needed while app is running.
4. Keep installer metadata channel-aware (`stable` vs `preview`) to avoid app ID collisions.
5. Automate post-release distribution (WinGet) as a separate after-release step.
6. Run release on dedicated Windows runners with stable SDK/toolchain pinning.

### Linux considerations for OpenAgents

1. Ship tarball structure with clear app root (`.app`-style dir), `bin`, and `libexec`.
2. Encode runtime library strategy explicitly (bundle non-system libs; verify with `ldd`).
3. Generate `.desktop` entries and icons from template variables.
4. Keep app identity channel-specific so stable/preview can coexist.
5. Support package-manager path by disabling in-app auto-update and showing maintainer message.
6. Define glibc baseline and supported distros up front to reduce support ambiguity.

### Suggested MVP adoption order for OpenAgents

1. Standardize channel/version/artifact naming and add docs-level policy.
2. Add local scripts for `bundle-mac`, `bundle-windows`, `bundle-linux` (manual execution first).
3. Add signing/notarization secret contract and dry-run validation scripts.
4. Add release assembly + asset validation job design (without implementing workflows yet).
5. Add updater endpoint contract and client integration after artifact naming is stable.

## Source references used

Primary files inspected in `~/code/zed`:

- `.github/workflows/release.yml`
- `.github/workflows/release_nightly.yml`
- `.github/workflows/run_bundling.yml`
- `.github/workflows/after_release.yml`
- `.github/workflows/run_tests.yml`
- `script/bundle-mac`
- `script/bundle-windows.ps1`
- `script/bundle-linux`
- `script/determine-release-channel`
- `script/create-draft-release`
- `script/upload-nightly`
- `script/install.sh`
- `script/install-linux`
- `crates/auto_update/src/auto_update.rs`
- `crates/auto_update_helper/src/auto_update_helper.rs`
- `crates/auto_update_helper/src/updater.rs`
- `crates/release_channel/src/lib.rs`
- `crates/zed/Cargo.toml`
- `crates/zed/contents/*/embedded.provisionprofile`
- `crates/zed/resources/windows/sign.ps1`
- `crates/zed/resources/windows/zed.iss`
- `docs/src/development/linux.md`
