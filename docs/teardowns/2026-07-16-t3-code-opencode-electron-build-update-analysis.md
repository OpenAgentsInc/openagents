# T3 Code and OpenCode Electron build and update analysis

Date: 2026-07-16

Status: source-backed teardown and OpenAgents recommendation, not an implementation-status claim

## 1. Scope and evidence

This analysis answers one narrow question: how do current T3 Code and OpenCode sources build, sign, publish, and update Electron applications across macOS, Windows, and Linux, and which parts should OpenAgents adopt?

The reference checkouts were fast-forwarded before inspection:

- T3 Code: `pingdotgg/t3code` `main` at `fdca15471d92e95e4ec5501f45dbf3ce81f8d991` (2026-07-16).
- OpenCode: `anomalyco/opencode` `dev` at `c69abee0c73253aebae65e87e4e1b9bfa8c38021` (2026-07-16).
- OpenAgents baseline: `OpenAgentsInc/openagents` `main` at `94140b89defc102875259d08b1e16a8ce12992d1` before this document was written.

This document extends, rather than replaces, the earlier [T3 Code teardown](./2026-07-13-t3-code-teardown.md), [OpenCode desktop teardown](./2026-07-10-opencode-desktop-app-teardown.md), and [T3 Code/OpenAgents Desktop gap analysis](./2026-07-15-t3-code-openagents-desktop-full-gap-analysis.md). Those documents are intentionally commit-pinned; this one records the current release implementations in more detail.

Primary source surfaces:

| Product    | Build and publication                                                                                                                                                                                      | Update runtime                                                                                                                                                                                                                                                            |
| ---------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| T3 Code    | `.github/workflows/release.yml`, `scripts/build-desktop-artifact.ts`, `scripts/merge-update-manifests.ts`, `scripts/release-smoke.ts`, `docs/operations/release.md`                                        | `apps/desktop/src/electron/ElectronUpdater.ts`, `apps/desktop/src/updates/DesktopUpdates.ts`, `apps/desktop/src/updates/updateMachine.ts`, `apps/desktop/src/updates/updateChannels.ts`                                                                                   |
| OpenCode   | `.github/workflows/publish.yml`, `packages/desktop/electron-builder.config.ts`, `packages/desktop/scripts/finalize-latest-yml.ts`, `packages/desktop/scripts/finalize-latest-json.ts`, `script/publish.ts` | `packages/desktop/src/main/updater.ts`, `packages/desktop/src/main/updater-controller.ts`, `packages/desktop/src/main/updater-subscriptions.ts`                                                                                                                           |
| OpenAgents | `apps/openagents-desktop/forge.config.ts`, `apps/openagents-desktop/scripts/prepare-macos-maker.ts`, `apps/openagents-desktop/scripts/release-preflight.ts`, `docs/desktop-production-release-runbook.md`  | `apps/openagents-desktop/src/update-contract.ts`, `apps/openagents-desktop/src/update-staging-contract.ts`, `apps/openagents-desktop/src/update-staging-host.ts`, `apps/openagents-desktop/src/macos-update-applier.ts`, `apps/openagents-desktop/src/update-rollback.ts` |

## 2. Executive conclusion

T3 Code has the stronger updater state machine and the more deliberate target-specific dependency staging. OpenCode has the broader production artifact matrix: it actually builds six OS/architecture pairs and three Linux package formats. Neither project has a release trust model that should replace OpenAgents' existing signed-manifest, fail-closed, first-launch receipt, and retained-slot rollback design.

OpenAgents should copy the **shape** of their matrix and packaging work, not either updater wholesale:

1. Add one immutable release set containing macOS arm64/x64, Windows arm64/x64, and Linux arm64/x64 artifacts.
2. Build every target on a target-capable owned runner, stage only that target's native dependencies, and test the result on its native OS/architecture.
3. Evolve the OpenAgents signed update manifest from one artifact to a target-indexed release set.
4. Keep OpenAgents' custom verifier and platform appliers. Do not adopt `electron-updater`, unsigned YAML metadata, downgrade flags, or GitHub-hosted release authority.
5. Fail the whole release when signing, notarization, target coverage, metadata convergence, clean install, N-1 update, or rollback evidence is missing.

The target outcome is not “six independent installers.” It is one source revision, one version, one signed release-set manifest, six native builds, multiple package formats, and one atomic promotion decision.

## 3. Actual platform coverage

The distinction between code-path support and artifacts actually emitted by the production workflow matters.

| Product          | macOS                                                    | Windows                                                                                          | Linux                                | Actual production matrix                                                  |
| ---------------- | -------------------------------------------------------- | ------------------------------------------------------------------------------------------------ | ------------------------------------ | ------------------------------------------------------------------------- |
| T3 Code          | arm64 and x64; DMG and ZIP                               | x64 NSIS; arm64 build support exists but its workflow entry and manifest merge are commented out | x64 AppImage                         | Four OS/architecture jobs: macOS arm64, macOS x64, Windows x64, Linux x64 |
| OpenCode         | arm64 and x64; DMG, ZIP, and an additional `.app.tar.gz` | arm64 and x64 NSIS                                                                               | arm64 and x64 AppImage, DEB, and RPM | Six OS/architecture jobs                                                  |
| OpenAgents today | arm64; DMG and ZIP                                       | none                                                                                             | none                                 | One macOS arm64 job/process                                               |

Consequences:

- T3 Code is not currently a six-target reference. Its build script anticipates Windows arm64 and Linux arm64, but the live workflow only ships four target pairs and only one Linux format.
- OpenCode is the useful reference for complete target coverage and Linux package metadata.
- OpenAgents needs both a packaging expansion and an update-contract expansion; adding Forge makers alone would still leave the runtime unable to select or apply a non-macOS update.

## 4. T3 Code release pipeline

### 4.1 Release initiation and versioning

T3 Code uses one release workflow for stable and nightly publication:

- Semver tags initiate stable releases.
- A schedule runs every three hours for nightlies.
- Manual dispatch can request either channel; stable dispatch requires an explicit version.
- A scheduled nightly exits when the source revision is already represented by the latest nightly tag.
- Stable and nightly versions, tags, notes, npm dist-tags, and updater channels are derived in preflight rather than independently in each build job.
- The workflow locates the previous tag in the same channel so release notes do not mix stable and nightly histories.

The preflight runs repository checks before the matrix fans out. A successful stable release later synchronizes package versions back to the main branch. That reduces version drift, although coupling source mutation to release completion makes the publication workflow more complex.

### 4.2 Matrix and native build discipline

The production matrix is:

| Runner       | Target         | Output                                   |
| ------------ | -------------- | ---------------------------------------- |
| macOS 26     | `darwin-arm64` | DMG, ZIP, mac updater YAML               |
| macOS 26     | `darwin-x64`   | DMG, ZIP, arch-suffixed mac updater YAML |
| Ubuntu 24.04 | `linux-x64`    | AppImage, Linux updater YAML             |
| Windows 2025 | `win32-x64`    | NSIS EXE, blockmap, Windows updater YAML |

The important implementation is not the YAML matrix itself. `scripts/build-desktop-artifact.ts` creates a temporary, target-specific staged application and dependency closure:

1. Build shared desktop, server, and web outputs once for the target job.
2. Synthesize a minimal package/workspace for the requested OS and architecture.
3. Install production dependencies with that target in the package-manager architecture policy.
4. Copy platform-native helper binaries into the staged application.
5. Package with Electron Builder using a deterministic artifact convention.

That staging step avoids a common Electron failure: packaging whichever optional native binary happened to be installed on the orchestration host. It also gives T3 Code a place to include target-specific extras. The Windows artifact includes Linux-side helpers needed by its WSL execution path, including a prebuilt Linux `node-pty`, so first run does not depend on compilers or a network fetch.

T3 Code also leaves selected runtime/server resources outside ASAR. In particular, Windows-hosted Linux/WSL processes cannot read the Windows Electron ASAR as if it were a Linux filesystem. This is a concrete packaging boundary, not merely a performance preference.

### 4.3 Signing and notarization

T3 Code supports:

- Developer ID signing and Apple notarization when all Apple credentials are available.
- A provisioning profile and team identity for macOS passkey entitlements.
- Azure Trusted Signing for Windows when all Azure signing values are available.

The release-safety weakness is that both signing paths are conditional. When the required secret set is absent, the workflow records that signing is disabled and continues producing release artifacts. The existing T3 teardown also records a tested historical failure in which the app bundle was signed/notarized but the quarantined DMG was not accepted by Gatekeeper.

T3 Code therefore demonstrates how to configure signing, but not the publication invariant OpenAgents needs. A production release must never have an unsigned fallback.

### 4.4 Updater metadata merge

Electron Builder emits one updater YAML file per job. Both macOS architectures initially want the canonical mac manifest name, so T3 Code keeps the arm64 name canonical, suffixes the x64 file, and merges them during the release job.

The merge script validates and combines the per-architecture file records into one channel-specific manifest. It rejects conflicting versions and metadata rather than silently choosing one job's output. Windows merge support and fixtures already exist, but the production merge block is commented out with the Windows arm64 matrix entry.

The release job downloads all matrix artifacts, performs the merge, and publishes the GitHub release only after required build and CLI publication dependencies succeed. Stable and nightly use separate updater manifest names/channels.

This convergence step is worth adopting. Its output format and trust boundary are not: Electron updater YAML is not a separately signed OpenAgents release authority.

## 5. T3 Code update runtime

T3 Code wraps `electron-updater` in an Effect service and exposes a typed state machine rather than passing library events directly to the renderer.

### 5.1 State and policy

The modeled states include disabled, idle, checking, available, downloading, downloaded, up-to-date, and error. The runtime records host architecture, application architecture, and Apple translation state. It persists the selected stable/nightly channel and derives the default from the installed version.

Updates are disabled when:

- Electron is not packaged for production.
- An explicit environment switch disables updates.
- Linux is not running from an AppImage context.
- The configured update feed is unavailable.

It checks shortly after startup and then on a four-minute interval. Download and install remain explicit user operations. Progress notifications are bounded rather than broadcasting every byte event.

### 5.2 Architecture changes and channel changes

T3 Code detects an Intel application running under translation on Apple Silicon. It disables differential download for that case so the updater can obtain the complete arm64 application instead of applying an x64 delta to the wrong architecture.

Switching channels temporarily enables downgrade behavior, and the nightly channel permits prereleases. This is pragmatic for Electron Builder channels, but it conflicts with the OpenAgents monotonic-version and rollback invariants. OpenAgents should resolve a target from a signed channel release set and reserve rollback for its retained previous slot.

### 5.3 Installation coordination

Before `quitAndInstall`, T3 Code stops all backend pool instances concurrently, including Windows/WSL instances, and destroys application windows. That is a strong lifecycle pattern. An updater must own a bounded drain protocol for every child process, PTY, local server, and helper that can retain files or outlive the Electron shell.

The implementation also has schema-validated updater event payloads, typed errors, mock update-server coverage, merge fixtures, and release smoke tests. These are stronger than an ad hoc IPC wrapper and should influence OpenAgents' test shape.

## 6. OpenCode release pipeline

### 6.1 Product-channel identity

OpenCode treats development, beta, and production as distinct desktop identities:

- Separate application IDs, names, icons, and Linux desktop metadata prevent profile and shell integration collisions.
- Development builds do not update.
- Beta and production publish from separate release repositories, even though both consume a `latest` updater channel inside their own repository.

This is operationally clearer than asking one installed binary to mutate channels. OpenAgents already has stable and RC semantics; if it later adds nightly builds, a separate application identity and state directory is safer than a downgrade-capable channel switch.

### 6.2 Six-target matrix

OpenCode's desktop matrix actually runs all six pairs:

| Target        | Native runner strategy                                 | Packages                |
| ------------- | ------------------------------------------------------ | ----------------------- |
| macOS x64     | Intel macOS runner                                     | DMG, ZIP, `.app.tar.gz` |
| macOS arm64   | Apple Silicon macOS runner                             | DMG, ZIP, `.app.tar.gz` |
| Windows x64   | x64 Windows runner                                     | NSIS                    |
| Windows arm64 | x64 Windows runner with the ARM64 MSVC cross-toolchain | NSIS                    |
| Linux x64     | x64 Ubuntu runner                                      | AppImage, DEB, RPM      |
| Linux arm64   | arm64 Ubuntu runner                                    | AppImage, DEB, RPM      |

The workflow uses architecture-matched macOS and Linux hosts. Windows arm64 is the exception: it runs on GitHub-hosted Windows because the normal Windows fleet lacks the ARM64 MSVC cross-compilation toolchain. That detail reinforces the general rule: a matrix declaration is not evidence of coverage until the compiler, native Node modules, and package maker support the target and an install smoke runs on the actual target architecture.

Before Electron packaging, each job builds the embedded OpenCode server/runtime and copies required WASM and native optional packages. The package metadata lists platform variants so each target gets its own native closure. This is the same useful principle as T3 Code's staging wrapper, expressed through OpenCode's monorepo build.

### 6.3 Package formats and OS integration

OpenCode's Electron Builder configuration includes:

- macOS hardened runtime, entitlements, notarization, DMG signing, and ZIP output.
- Windows one-click, per-user NSIS installation.
- Linux AppImage, DEB, and RPM packages for both architectures.
- Per-channel Linux application IDs, desktop file names, icons, and `StartupWMClass`.
- Compatibility handling for an older production Linux desktop entry during DEB/RPM transitions.

The Linux work is especially useful to OpenAgents. Adding DEB/RPM is not just choosing two makers; package identifiers, desktop files, icons, executable paths, upgrade compatibility, and uninstall behavior become durable public contracts.

### 6.4 Signing and publication barrier

OpenCode imports the Apple certificate, configures App Store Connect notarization credentials, requests notarization in Electron Builder, and explicitly signs the DMG. On Windows it uses Azure Trusted Signing. The workflow enumerates packaged executables, installers, and the bundled CLI and requires `Get-AuthenticodeSignature` to return `Valid`.

All matrix jobs upload into one draft GitHub release. A final publication job merges updater metadata and only then turns the draft into a public release. The draft therefore acts as a coarse atomic publication barrier.

OpenCode's macOS workflow does not show an equivalently explicit end-to-end Gatekeeper oracle for the downloaded outer artifact, and its updater configuration sets `verifyUpdateCodeSignature: false` on Windows. The latter disables Electron Updater's publisher verification even though CI signed and inspected the files. OpenAgents must not adopt that setting.

## 7. OpenCode updater metadata and runtime

### 7.1 Metadata finalization

OpenCode's finalizer produces:

- One merged macOS `latest-mac.yml` for x64 and arm64.
- One merged Windows `latest.yml` for x64 and arm64.
- Separate x64 and arm64 Linux updater manifests.
- A Tauri-compatible `latest.json` whose artifacts receive separate signatures.

The Electron application consumes the YAML metadata, not the Tauri signed JSON. Its effective Electron trust is therefore transport/repository integrity, artifact hashes in YAML, and operating-system signing—not a pinned signature over the release selection document.

The YAML finalizer is also less defensive than T3 Code's merger: it combines expected records but does not establish a strict release-set schema with exhaustive target coverage and one-version convergence. OpenAgents should make target completeness part of signature generation, not a best-effort postprocess.

### 7.2 Runtime behavior

OpenCode keeps the updater in the Electron main process and exposes a narrow subscription/controller API to the renderer. It models disabled, idle, checking, downloading, ready, up-to-date, installing, and error states.

Behavior includes:

- Updates only in packaged beta/production builds.
- An immediate startup check and a ten-minute polling interval.
- No automatic installation on quit.
- One pending operation at a time.
- A check operation that also downloads an available update.
- Persistence of a ready-version marker and clearing it once that version is installed; the current controller starts a fresh check/download after restart rather than restoring a ready artifact from the marker.
- Explicit user installation.
- Sidecar shutdown before `quitAndInstall`.
- Subscriber cleanup per destroyed renderer/web contents.

The implementation always enables downgrade behavior, and its declared progress value is not backed by a complete progress event path. T3 Code is the better reference for channel policy and progress modeling; OpenCode is the better reference for renderer subscription cleanup and single-operation deduplication.

## 8. Comparative assessment

| Concern                   | T3 Code                                        | OpenCode                                                              | OpenAgents recommendation                                                         |
| ------------------------- | ---------------------------------------------- | --------------------------------------------------------------------- | --------------------------------------------------------------------------------- |
| Actual target breadth     | Four pairs                                     | Six pairs                                                             | Six pairs                                                                         |
| macOS outputs             | DMG, ZIP                                       | DMG, ZIP, app tarball                                                 | DMG and ZIP initially                                                             |
| Windows outputs           | x64 NSIS                                       | x64/arm64 NSIS                                                        | x64/arm64 NSIS                                                                    |
| Linux outputs             | x64 AppImage                                   | x64/arm64 AppImage, DEB, RPM                                          | All three formats for both architectures, introduced behind install/upgrade proof |
| Native dependency closure | Strong explicit staging                        | Strong matrix-aware optional packages                                 | Explicit target build descriptor and staged closure                               |
| macOS release trust       | Signing optional; historical outer-DMG failure | Signs/notarizes app and DMG                                           | Preserve current fail-closed app+DMG sign/notarize/staple/Gatekeeper proof        |
| Windows release trust     | Azure signing optional                         | Azure signing and CI validation, but updater publisher check disabled | Signing mandatory; CI and install-time publisher verification                     |
| Update metadata           | Per-channel Electron YAML, strict-ish merge    | Per-target Electron YAML, weaker merge                                | Pinned Ed25519 signed target-indexed manifest                                     |
| Downgrade                 | Temporarily/channel enabled                    | Always enabled                                                        | Never as update policy; rollback uses retained slot                               |
| Publication barrier       | GitHub release job after matrix                | Draft GitHub release then finalize                                    | Immutable candidate objects then signed atomic channel-pointer promotion          |
| Updater ownership         | Typed Effect wrapper                           | Main-process controller                                               | Keep OpenAgents typed main-process host and platform appliers                     |
| CI authority              | GitHub workflow/hosted runners                 | GitHub workflow/hosted runners                                        | Owned runners/orchestrator; no GitHub Actions                                     |

## 9. OpenAgents baseline and the real gaps

OpenAgents is narrower, but its macOS release/update foundation is already stronger in several security-critical ways:

- Electron Forge packages only macOS arm64 today.
- The app and DMG have fail-closed Developer ID, notarization, stapling, and Gatekeeper checks.
- Unsigned local-development artifacts are conspicuously renamed and cannot enter the production publisher.
- The update manifest is Ed25519-signed and checked against a pinned key.
- Candidate artifact size, digest, channel, and monotonic version are verified before installation.
- The macOS applier rechecks bundle ID, version, Team ID, code signature, notarization, and staple after mounting the DMG.
- A separate tested retained-slot state machine models first-launch evidence and rollback instead of permissive version downgrade; its source explicitly says host/restart wiring is a later release exit, so the model is stronger than the currently integrated proof.

The gaps are structural:

1. `package.json` and Forge only expose macOS arm64 package/make commands and macOS makers.
2. The signed manifest describes one artifact rather than a target-indexed release set.
3. The only update applier is `MacOSUpdateApplier`.
4. Native audio preparation builds for the host rather than an explicit Rust target.
5. Codex and other bundled runtime packages are selected dynamically, but the dependency declaration and install process do not yet prove every OS/architecture package is available and staged.
6. Workspace `supportedArchitectures` is not a substitute for per-target dependency installation.
7. The production release runbook and preflight are macOS-specific.
8. There is no owned six-cell runner fleet, matrix convergence receipt, or cross-platform install/update laboratory in the current release contract.
9. The first-launch/rollback reducer is tested but not yet fully wired to the update host and restart path, so production rollback evidence still trails the modeled policy.

This means the work should start at the release contract and build descriptor, not by adding five more command-line switches to the current macOS script.

## 10. Recommended OpenAgents architecture

### 10.1 One release set, six native targets

Define a target key as an explicit closed enum:

```text
darwin-arm64
darwin-x64
win32-arm64
win32-x64
linux-arm64
linux-x64
```

Each matrix job receives the same immutable inputs:

- source commit;
- exact semantic version and channel;
- Electron/Node/pnpm lockfile identities;
- target key;
- expected package formats;
- native runtime package ledger;
- signing policy identifier;
- reproducibility/build invocation identifier.

Each job returns a signed build receipt and artifacts. A release-set finalizer refuses promotion unless every required target and format is present at the same version and source revision.

Use target-capable owned build runners and native test hosts:

- Apple Silicon and Intel Macs for the two macOS cells;
- native x64 Windows plus either native arm64 Windows or a reviewed ARM64 MSVC cross-build host; run install/update proof on native Windows arm64 in either case;
- native x64 and arm64 Linux hosts.

OpenAgents' no-GitHub-Actions invariant remains in force. The matrix can use the same concepts as the references while running through owned orchestration, existing Mac hardware, and owned cloud/VM workers. GitHub Releases must not become update authority.

### 10.2 A target-specific staging workspace

Adopt T3 Code's strongest build idea: create a clean staging workspace per target instead of packaging the developer checkout.

The staging builder should:

1. Validate that the source checkout is clean and exactly the promoted revision.
2. Install locked production dependencies for the requested target only.
3. Build `oa-desktop-audio` with an explicit Rust target triple.
4. Resolve and copy the exact Codex, Claude, shell, media, and other native packages for that target.
5. Emit a machine-readable native-component ledger with names, versions, architectures, hashes, and provenance.
6. Copy only the allowlisted application resources.
7. Run the package-content and ASAR oracle before invoking a maker.
8. Package on the native operating system.

Do not install all native variants into one shared checkout and hope the maker filters correctly. That produces nondeterministic closures, wastes artifact space, and makes architecture evidence ambiguous.

### 10.3 Packaging policy

Recommended first-class formats:

| Platform          | Required formats   | Rationale                                                                                                               |
| ----------------- | ------------------ | ----------------------------------------------------------------------------------------------------------------------- |
| macOS arm64/x64   | DMG and ZIP        | DMG for human install and current applier; ZIP for managed/diagnostic distribution and parity with current Forge output |
| Windows arm64/x64 | NSIS               | Established Electron install/update path, per-user operation, Azure signing support                                     |
| Linux arm64/x64   | AppImage, DEB, RPM | AppImage enables unprivileged atomic replacement; DEB/RPM provide native shell/package integration                      |

Every artifact name should include product, version, channel where necessary, platform, architecture, and format. Unlike OpenCode's versionless artifact filename, the object key should remain self-describing and immutable.

For Linux, choose stable package IDs and desktop-entry names before the first public package. OpenCode's legacy-entry compatibility code shows that those names become migration contracts. Test install, upgrade, reinstall, and uninstall independently for DEB and RPM.

### 10.4 Signed release manifest v2

Evolve the current contract; do not replace it with Electron Builder YAML. A conceptual manifest is:

```json
{
  "schemaVersion": 2,
  "channel": "stable",
  "version": "1.2.3",
  "sourceRevision": "...",
  "publishedAt": "...",
  "targets": {
    "darwin-arm64": {
      "preferred": "dmg",
      "artifacts": {
        "dmg": { "url": "...", "sha256": "...", "byteLength": 0 },
        "zip": { "url": "...", "sha256": "...", "byteLength": 0 }
      }
    }
  }
}
```

The real Effect Schema should additionally bound strings, byte lengths, target count, formats, minimum OS versions, native-component ledger hash, signing-policy identity, and build receipt references. It should reject unknown target/format combinations and duplicate semantic identities.

Sign the complete canonical manifest with the existing pinned release key. The selected artifact digest and length remain inside that signature. A tiny signed channel pointer may select an immutable versioned manifest, but no mutable object should be able to substitute a target or version without signature failure.

The finalizer must prove:

- exactly one version, channel, and source revision;
- all required target keys and formats;
- no duplicate target/format entries;
- artifact name, digest, length, and object existence;
- receipt/ledger correspondence;
- platform-signing evidence where required;
- monotonic promotion relative to the current channel head.

### 10.5 Platform appliers

Keep selection and verification common; make only installation platform-specific.

**macOS:** generalize the existing applier to both architectures. Verify host/target compatibility, mounted bundle architecture, bundle ID, version, Developer ID Team ID, notarization, staple, and digest before the retained-slot swap. Handle an Intel installation on Apple Silicon as an explicit full-artifact architecture migration, borrowing T3 Code's awareness without borrowing differential update behavior.

**Windows:** introduce a Windows applier around the signed NSIS artifact. Before execution, verify manifest digest/length and Authenticode publisher identity through Windows trust APIs. Drain all child processes, launch the installer in a bounded handoff, and record install/first-launch receipts. Never configure `verifyUpdateCodeSignature: false`; OpenAgents' own verification must be at least as strict as the OS and installer path.

**Linux AppImage:** stage a complete AppImage beside the current one, verify the signed manifest and executable architecture, retain the previous image, atomically replace the selected image/symlink, and use the existing first-launch/rollback model. This is the closest Linux analogue to the current macOS retained slot.

**Linux DEB/RPM:** initially treat native packages as explicit user/admin installer handoffs rather than claiming application-owned atomic rollback. Verify the package digest and package identity first, then invoke the system installer path with clear state. Only claim unattended in-app application and rollback after a privileged helper and package-manager transaction contract exist. AppImage can be the first Linux auto-update path while DEB/RPM rely on repositories or user-mediated upgrades.

### 10.6 Channel policy

Keep stable and RC monotonic and separately signed. Do not adopt either reference's downgrade flags.

If nightly builds become useful:

- give them a separate application ID, update key/state root, channel pointer, visual identity, and telemetry label;
- never allow a nightly build to overwrite stable application state;
- promote an already-proven release set between channels when possible rather than rebuilding the same version;
- retain rollback as a local previous-slot transition, not a remote version downgrade.

### 10.7 Publication sequence

Recommended release transaction:

1. A release coordinator fixes the source revision, version, channel, target set, and policy.
2. Common tests pass once at that revision.
3. Six native jobs build, sign, and upload to immutable candidate object keys.
4. Per-target workers run package-content, architecture, signature, and clean-install checks.
5. Test hosts update from the prior channel version and exercise first launch, child-runtime startup, shutdown, and retained-slot rollback.
6. The coordinator validates all receipts and creates the canonical manifest.
7. An isolated signer signs the manifest after policy validation.
8. A candidate channel serves the exact signed release set for downloaded-artifact smoke.
9. One atomic pointer/traffic promotion makes the signed set current.
10. Post-promotion probes resolve every target and archive the release receipt.

No artifact should be public-current while another target is still building. OpenCode's draft release is a useful analogy; immutable object staging plus an atomic signed pointer is the OpenAgents-compatible mechanism.

## 11. Required release gates

### 11.1 Common gates

- Clean, current `origin/main` at one exact revision.
- Version/channel monotonicity and no artifact overwrite.
- Lockfile and generated-contract checks.
- Unit, integration, type, format, and invariant tests.
- Renderer privilege and Electron fuse assertions.
- Complete target/format matrix convergence.
- Signed manifest canonicalization and pinned-key verification.
- Candidate-object download and hash/length verification.

### 11.2 Per-target package gates

- Executable and every native helper report the expected architecture.
- Only allowlisted native/runtime packages exist.
- Bundled Codex/Claude/helper versions and hashes match the native ledger.
- No development files, signing secrets, source maps contrary to policy, or foreign-architecture binaries leak into the artifact.
- Application starts in a clean OS user profile and opens a bounded workroom smoke.
- N-1 installs and updates to N; N first-launch receipt settles; deliberate failure rolls back where the platform contract promises rollback.
- Child processes, PTYs, local servers, and helpers stop before replacement.

### 11.3 Platform trust gates

- macOS: sign app and DMG, notarize both required layers, staple, `codesign` verify, `spctl` verify mounted/downloaded artifact, validate Team ID and entitlements on both architectures.
- Windows: sign installer, app executables, bundled CLIs/helpers, and uninstallers; require `Get-AuthenticodeSignature`/Windows trust success and the expected publisher before publication and again before install.
- Linux: verify manifest signature and artifact digest on every format; inspect package metadata and payload; test DEB/RPM repository signatures if repositories are offered; test AppImage replacement and executable bit preservation.

## 12. Delivery plan

### Phase 0: contract and orchestration

- Define target/format schemas and release manifest v2.
- Define build, native-component, signing, install, first-launch, and rollback receipts.
- Build the owned release coordinator and immutable candidate/promotion flow.
- Keep production publication on the current macOS arm64 path until the new target has equivalent evidence.

### Phase 1: macOS x64

- Refactor the current macOS build into a target-specific staging builder.
- Add Intel-native packaging and sign/notarize/staple/Gatekeeper proof.
- Merge arm64/x64 into one signed release set.
- Prove architecture migration on Apple Silicon and ordinary Intel N-1 updates.

This is the smallest change that forces the manifest and matrix abstractions to become real without adding a second operating system simultaneously.

### Phase 2: Windows x64 and Linux x64

- Add NSIS/Azure signing and a Windows verifier/applier.
- Add AppImage, DEB, and RPM packaging; make AppImage the first Linux auto-update path.
- Add clean OS test hosts and cross-platform child-process drain tests.

### Phase 3: Windows arm64 and Linux arm64

- Add native arm64 runner/toolchain capacity.
- Prove every bundled agent/helper dependency actually publishes arm64 packages.
- Repeat install, update, rollback, and package-manager migration evidence instead of treating arm64 as a compiler flag.

### Phase 4: channel expansion

- Add an isolated nightly identity only if faster field feedback justifies its operational cost.
- Promote already-verified immutable artifacts between channels where policy allows.

## 13. Patterns to adopt and reject

Adopt from T3 Code:

- target-specific staging workspaces and native dependency closure;
- a strict metadata convergence step;
- explicit updater state and schema-validated events;
- Apple Silicon translation/architecture awareness;
- bounded progress and operation ownership;
- shutdown of all sidecars before install;
- mock update server and release-manifest fixtures.

Adopt from OpenCode:

- a real six-target build matrix;
- distinct application identity for unstable channels;
- complete Linux AppImage/DEB/RPM metadata and migration care;
- signing all Windows executables, not only the outer installer;
- post-signature enumeration and verification;
- draft/candidate publication as an all-target barrier;
- renderer subscription cleanup and single-operation deduplication.

Reject from both:

- GitHub Actions or GitHub Releases as OpenAgents release authority;
- release artifacts that continue when signing credentials are absent;
- unsigned Electron updater YAML as the channel selection authority;
- `allowDowngrade` as normal update/channel behavior;
- disabled Windows update-signature verification;
- assuming a maker's success proves clean install, N-1 update, first launch, or rollback;
- treating a declared matrix cell as supported without native dependency and host evidence.

## 14. Decisions that should be explicit before implementation

1. Which minimum OS versions are supported per target?
2. Will Windows installation be per-user only, and what is the exact publisher identity?
3. Are DEB/RPM direct downloads, signed repositories, or both?
4. Is AppImage the only Linux format with in-app atomic update/rollback initially?
5. Which owned runner inventory provides Intel macOS and Windows arm64 capacity?
6. Does RC keep the production application identity and state root, or become a separate install?
7. Which bundled runtimes lack Windows/Linux arm64 distributions today?
8. What evidence is required before a new target moves from experimental download to channel-required?

These choices alter public install identity, update compatibility, or release invariants. They should be recorded in the owning product/release contracts before code claims platform support.

## 15. Bottom line

OpenCode proves the six-target packaging matrix is practical. T3 Code provides the better model for target staging, metadata convergence, updater state, and sidecar lifecycle. OpenAgents already has the strongest trust and rollback core.

The right synthesis is therefore:

> OpenCode's target breadth plus T3 Code's build discipline, underneath OpenAgents' signed release-set authority, fail-closed platform verification, and receipt-backed rollback.

That yields one consistent release process across macOS, Windows, and Linux without weakening the policy that currently makes the macOS arm64 path trustworthy.
