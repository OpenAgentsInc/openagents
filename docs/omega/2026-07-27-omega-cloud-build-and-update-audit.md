# Omega cloud build and application update audit

- Date: 2026-07-27
- Status: current-state audit and migration recommendation
- Scope: Omega production builds, macOS signing and notarization, GitHub
  release publication, application updates, build isolation, and disk cleanup
- OpenAgents source: `dcef6ac737`
- Omega source: `5e669bedd3`
- Decision: stop normal Omega production builds on the owner's active
  workstation. Use a remote Apple build worker for macOS candidates. Keep a
  guarded local background job only as an emergency lane.

## 1. Result

Omega can build a macOS application in remote infrastructure.

Google Cloud Build cannot do this work by itself. Cloud Build runs each build
step in a Docker container on an ephemeral Google Cloud worker. It is a good
fit for Linux checks, containers, feed deployment, and artifact processing. It
does not provide Xcode, `codesign`, `hdiutil`, or a macOS host.

A remote macOS worker can build, sign, notarize, and package Omega. The current
Omega repository already contains most of the required build logic. Its
inherited Zed release workflow also proves the shape of a remote macOS build.
That workflow uses a large remote macOS runner, imports the Developer ID
certificate, uses an App Store Connect API key for notarization, and uploads
the result.

The inherited workflow does not run for OpenAgents. Every release job has a
repository-owner condition that permits only `zed-industries` or
`zed-extensions`. Its runner labels and cache secrets also belong to the
upstream system. Live GitHub state on 2026-07-27 showed that Actions is enabled
for `OpenAgentsInc/omega`, but the repository had no workflow runs.

The recommended end state is:

1. Google Cloud runs the Linux checks and the update service.
2. A remote Apple Silicon worker runs the macOS build.
3. A separate remote macOS job verifies the exact downloaded candidate.
4. A coordinator publishes the verified GitHub prerelease and admits the
   signed update record.
5. The owner's active workstation does not compile a production candidate.

## 2. Immediate operating rule

Do not start another normal Omega RC build on the owner's active workstation.

The canceled `0.2.0-rc19` build and the disk incident show that the current
lane can block the interactive session. Cleanup during the incident removed
more than 35 GB of accumulated Rust output from the shared target directory.
This value is not an exact peak measurement for one release build. It does show
that a 14 GB release workspace is not a safe assumption. A local build also
competes with development checks for the same Cargo artifact lock.

A future local production build must be an explicit emergency action. It must
use the background job contract in section 9. It must not use the shared
`target/` directory.

This rule does not stop normal development checks. It separates release
production from interactive development.

## 3. Current Omega release surfaces

| Surface | Current state | Finding |
| --- | --- | --- |
| `script/bundle-omega-rc` | Builds `aarch64-apple-darwin`, signs the app and disk image, submits both for notarization, staples both, writes a release record, and restores temporary source changes. | This is a useful portable build core, but it runs in the foreground and writes large output under the repository `target/` directory. |
| Build version | `OMEGA_RC_VERSION` is a constant in the script. | A remote trigger cannot select a version without a source change. The build interface needs an explicit version input that is checked against source state. |
| Build target | The script uses the repository target directory for release binaries, the disk image, the component cache, and records. | The build has no isolated `CARGO_TARGET_DIR`. It can fill the workstation disk and block other Cargo commands. |
| Disk controls | The inherited workflow has `script/clear-target-dir-if-larger-than`. The Omega RC script does not call it and has no free-space check. | Cleanup starts too late. The job needs a minimum-free-space gate before dependency work and before disk-image creation. |
| Secret cleanup | The script can import a base64 certificate into a temporary keychain and deletes that keychain on exit. It also deletes a temporary notarization key file. | This design can move to an ephemeral remote runner. |
| Candidate proof | The script writes artifact, source, toolchain, component, brand, identity, signing, and notarization facts into a release record. | Keep this record as the candidate contract. Add remote worker, run, and artifact references. |
| GitHub publication | Publication is a separate manual action. The RC build does not upload or publish the result. | A coordinator job must upload the exact verified bytes and mark the GitHub release as a prerelease. |
| Application update | `assets/settings/default.json` sets `auto_update` to `false`. The source says that it stays off until an owned update feed exists. | GitHub publication alone does not give Omega an update path. |
| Inherited release workflow | `.github/workflows/release.yml` has remote macOS build jobs for arm64 and x64. | All relevant jobs are limited to upstream repository owners and use upstream runner and cache services. |
| Inherited cache | `script/setup-sccache` uses Cloudflare R2 and upstream secrets. | Omega needs an OpenAgents-owned cache or a cache-free first milestone. Do not reuse upstream credentials. |

## 4. Current OpenAgents build and update surfaces

OpenAgents has useful release controls, but it does not yet have a remote
Apple Silicon production lane.

| Surface | Current state | Reuse for Omega |
| --- | --- | --- |
| `docs/DEPLOYMENT.md` | One hub records release commands, signing custody, release tags, and rollback rules. | Add the final Omega route here only after the route exists. This audit stays under `docs/omega/` until then. |
| `apps/oa-updates` | Cloud Run serves signed application update data. Cloud Build creates its Linux container image. | Add an Omega product namespace after a separate contract and implementation change. |
| ReleaseSet v2 | The feed uses immutable candidate objects, a signed payload, exact artifact hashes and lengths, compare-and-swap promotion, one retained rollback generation, and fail-closed clients. | Reuse the protocol and storage laws. Do not put Omega bytes in the OpenAgents Desktop namespace. |
| Desktop staging | `stage-and-package.ts` creates a temporary workspace and removes it on success or failure. | Copy the isolated-workspace and final cleanup behavior into the Omega build coordinator. |
| Desktop build host map | OpenAgents Desktop uses the local Apple Silicon Mac, a remote Intel Mac, and Google Cloud Linux workers. | The remote Intel and Linux lanes do not replace an Apple Silicon Omega worker. |
| Release signing | The Developer ID certificate and App Store Connect key have documented recovery paths. The release key has a Google Secret Manager backup. | The remote job can receive bounded secret values through its runner secret store. It must create and delete a temporary keychain. |
| GitHub Actions | The OpenAgents repository has no `.github/workflows/` directory. `docs/DEPLOYMENT.md` records an organization billing lock. | Do not assume that an OpenAgents Actions workflow can run until a live probe succeeds. |

## 5. macOS remote-build options

### 5.1 GitHub-hosted macOS

GitHub provides standard Apple Silicon macOS runners. It also provides larger
Apple Silicon runners for eligible organization plans. Thus, a hosted macOS
build is technically possible.

The current hosted runner storage is the first blocker. GitHub documents only
14 GB of SSD for the standard arm64 macOS runner and for the arm64 macOS
larger runner. The incident measurement is accumulated shared output, not an
exact one-build peak. Therefore, it cannot prove that a build will fail on a
14 GB runner. It also cannot prove that the runner is safe. A complete probe
must measure the exact job.

Do not select a standard GitHub macOS label for production until a probe build
shows the complete candidate fits with at least 20 GB of free space at every
phase. A partial compile is not sufficient proof.

GitHub also documents a current billable price of USD 0.062 per minute for a
standard M1 or Intel macOS runner. Standard runners in public repositories can
have different included-use terms. Larger runners need an eligible
organization plan and a valid payment method. OpenAgents must resolve the
recorded billing lock before this lane can be trusted.

### 5.2 Namespace remote macOS

The inherited release workflow uses `namespace-profile-mac-large` and a remote
Rust cache. This is the closest existing example because it builds the same
large Rust application family.

This option is the preferred hosted probe if OpenAgents already has or chooses
an admitted Namespace account with sufficient disk. The probe must confirm:

- Apple Silicon architecture.
- At least 100 GB of free workspace before the build.
- A documented maximum job time of at least 90 minutes.
- Secret isolation and job deletion after completion.
- Artifact retention long enough for independent verification.
- A bounded price that complies with the current authority profile.

The existing workflow label is not proof that OpenAgents can use the service.
Runner access, billing, secrets, and cache ownership must all be established.

### 5.3 Dedicated remote Apple Silicon host

A dedicated Mac mini or an admitted macOS host can run an ephemeral
self-hosted runner. This gives enough disk and avoids the 14 GB hosted-runner
limit. It also removes release compilation from the active workstation.

The runner must be ephemeral. GitHub recommends an ephemeral registration for
one-job self-hosted runners and external retention of runner logs. After one
job, automation must remove the worktree, target directory, temporary
keychain, runner registration, and secret files.

This is the preferred owned-infrastructure fallback when hosted macOS storage
is not sufficient. The present remote Intel Mac is useful for x64 proof, but
it cannot produce the required arm64 candidate.

### 5.4 Google Cloud

Use Google Cloud for these parts:

- Linux compilation and tests.
- Source and dependency verification.
- Container builds.
- `oa-updates` deployment.
- Artifact storage.
- Release metadata and receipts.

Do not describe a Google Cloud container build as a macOS build. Cloud Build
executes Docker build steps. It cannot run the Apple package and Gatekeeper
oracles.

## 6. Recommended target design

Create one Omega-specific candidate workflow. Do not modify the generated
upstream release workflow for the first milestone.

### 6.1 Trigger and concurrency

The trigger is a manual RC request with these immutable inputs:

- Exact 40-character commit on `origin/main`.
- Version.
- Channel `rc`.
- Build reason and source issue references.

Use one concurrency key for the Omega RC channel. A newer request can cancel a
queued older request. It must not delete an artifact from a completed request.

The coordinator refuses:

- A commit that is not current `origin/main`.
- A dirty exported source tree.
- A version that is not strictly newer than the current RC.
- A version that does not match the release source.
- Missing signing or notarization inputs.

### 6.2 Linux preflight

Run source checks and tests on Google Cloud or a Linux hosted worker before
the macOS allocation starts.

The output is one immutable preflight receipt. It binds the source commit,
Cargo lock digest, toolchain pins, Omega delta gate, brand gate, and identity
gate.

Do not transfer a Linux Cargo target directory to the Mac. A remote `sccache`
store can keep compiler results by target and toolchain. Treat restored cache
data as untrusted input and verify the final artifact from source facts.

### 6.3 macOS candidate build

The remote Apple Silicon job:

1. Creates a fresh worktree or source export for the exact commit.
2. Creates an isolated target directory outside the checkout.
3. Checks free disk before install, compile, bundle, and disk-image creation.
4. Imports the Developer ID certificate into a new temporary keychain.
5. Reads the App Store Connect key only from a secret file with mode `0600`.
6. Runs the Omega RC build core.
7. Verifies the app and disk image with `codesign`, `spctl`, and `stapler`.
8. Uploads the disk image, release record, build log, and checksums.
9. Deletes the target directory, source export, temporary keychain, and secret
   files in an unconditional cleanup step.

Apple documents that `notarytool` accepts an App Store Connect API key. Apple
also provides a Notary API when the submission step must run without Xcode.
Omega can keep `notarytool` on the macOS worker for the first milestone.

### 6.4 Independent candidate verification

A different remote macOS job downloads the artifact from storage. It does not
reuse the build workspace.

It verifies:

- SHA-256 and byte length against the release record.
- Developer ID team `HQWSG26L43`.
- Hardened runtime and expected entitlements.
- `codesign --verify --deep --strict`.
- `spctl` acceptance for the disk image and application.
- `stapler validate` for the disk image and application.
- Omega brand and identity package gates.
- The bundled `omega-effectd` pin.
- A read-only launch smoke with isolated application data.

The verifier writes a public-safe receipt. A build job cannot verify its own
obligation.

### 6.5 Publication

The coordinator publishes only after the preflight, build, and independent
verification receipts are green.

It creates a GitHub prerelease with:

- Tag `v<version>`.
- The exact disk image.
- The exact release record.
- Checksums.
- The independent verification receipt.
- Public-safe release notes.

The coordinator must not mark an RC as the latest stable release.

### 6.6 Owned application update

GitHub Releases is a candidate mirror. It is not the authoritative update
channel.

Add an Omega namespace to the OpenAgents Updates service, for example:

```text
/desktop/omega/rc/v2/pointer.json
/desktop/omega/rc/candidates/<sha256>/release-set.json
/desktop/omega/rc/candidates/<sha256>/release-set.sig.json
```

This is a proposed path, not an implemented route.

The Omega update contract must have:

- A product-specific signing pin.
- Immutable candidate bytes.
- Exact artifact hash and byte length.
- A monotonic version rule.
- Compare-and-swap promotion.
- One retained rollback generation.
- A first-launch receipt and automatic local rollback.
- A compatibility plan for the inherited updater.
- No dependency on a Zed production service.

Keep `auto_update` off until the client, feed, signing, first-launch, and
rollback acceptance tests are complete.

## 7. Disk and cache policy

The release runner must not depend on a long-lived Cargo target directory.

Use these controls:

1. Set `CARGO_TARGET_DIR` to a job-specific path.
2. Require at least 100 GB free before compile. Record the observed value.
3. Check free space again before `cargo bundle` and before `hdiutil`.
4. Stop with a typed `insufficient_disk` result before the volume reaches a
   dangerous state.
5. Use `CARGO_INCREMENTAL=0` for release builds.
6. Use a bounded remote `sccache` store for speed. Key it by Rust toolchain,
   target, profile, Cargo lock digest, and source inputs.
7. Never cache certificates, notary keys, release private keys, signed
   artifacts, or temporary keychains.
8. Delete the job target directory on success, failure, cancellation, and
   timeout.
9. Retain only the final artifact, release record, receipts, checksums, and a
   bounded log.
10. Apply retention rules to old workflow artifacts and caches.

The current upstream cleanup script uses 350 GB and 200 GB target thresholds.
That is a control for a large persistent runner. It is not a safe fit for a
14 GB hosted Mac.

## 8. Work plan

### Phase 0: stop local production builds

- Status: effective by this audit.
- Keep the current RC build canceled.
- Do not install or publish a candidate from the canceled build.

### Phase 1: runner probe

- Add an Omega-only manual workflow.
- Do not provide production secrets.
- Export the exact source and compile the release target.
- Record disk use after dependency fetch, compile, bundle preparation, and
  cleanup.
- Prefer the existing Namespace large-mac shape if access and budget are
  admitted.
- If no hosted worker has sufficient disk, admit a dedicated remote Apple
  Silicon worker.

Exit condition: one complete unsigned probe at the exact target with at least
20 GB free at the peak and zero job residue.

### Phase 2: signed remote candidate

- Add the temporary keychain and notary secret seams.
- Run `script/bundle-omega-rc`.
- Upload candidate artifacts and receipts.
- Run the separate verification job.

Exit condition: one signed and notarized RC candidate is independently
verified without using the active workstation.

### Phase 3: cloud publication

- Add coordinator publication with a GitHub prerelease.
- Add idempotency, concurrency, cancellation, and artifact retention.
- Record the trigger, actor role, source commit, run, and receipt references.

Exit condition: a remote candidate can publish once and a repeated command
does not create conflicting tags or releases.

### Phase 4: owned Omega updates

- Add the Omega feed contract and service namespace.
- Add the client pin, download, verification, staging, first-launch, and
  rollback paths.
- Run a staging update and rollback with throwaway keys.
- Run a signed RC update from the last installed RC.

Exit condition: the default `auto_update` value can change only after all
named acceptance gates pass.

## 9. Emergency local background job

This lane reduces interactive blocking. It does not meet the goal of moving
production builds off the device.

If a remote worker is unavailable and an owner-authorized emergency local
build is necessary, add one repository-owned supervisor command with:

```text
script/omega-rc-job start --commit <sha> --version <version>
script/omega-rc-job status <job-id>
script/omega-rc-job log <job-id>
script/omega-rc-job cancel <job-id>
script/omega-rc-job collect <job-id>
```

The supervisor contract is:

- Start from a fresh detached worktree at the exact remote commit.
- Put source, target, log, status, and temporary secrets under one validated
  job root.
- Refuse a start below 100 GB free.
- Record the process group, start time, source commit, and build phase.
- Run independently of the initiating terminal.
- Stream a bounded log and write an atomic status file.
- Check free space between phases.
- Cancel the exact owned process group only.
- Unmount a job-owned disk image if the job exits during image creation.
- Delete the worktree, target directory, temporary keychain, temporary secret
  files, and scratch images on every terminal result.
- Preserve final candidate outputs only after hash verification.
- Never run `cargo clean` against the shared development target.
- Never delete an unrecognized worktree or process.

The command should use a launchd-owned or equivalent durable background
process. A shell with `command &` is not sufficient because the initiating
terminal can exit and ownership becomes ambiguous.

## 10. Decisions and blockers

| Item | Decision |
| --- | --- |
| Can macOS build in the cloud? | Yes. A remote macOS worker can build and sign it. |
| Can Google Cloud Build replace the Mac? | No. It can run the Linux and service parts only. |
| Can a standard GitHub-hosted arm64 Mac build Omega now? | Not proven. The documented 14 GB disk is below the observed Omega build demand. |
| Should production compilation continue on the active workstation? | No. |
| Best first hosted probe | The large remote macOS service already represented by the inherited Namespace workflow, if access, disk, billing, and authority are admitted. |
| Best owned fallback | An ephemeral self-hosted runner on a separate Apple Silicon Mac. |
| Does GitHub publication enable app updates? | No. The owned Omega update feed is a separate phase. |
| Should `auto_update` be enabled now? | No. |
| Is the canceled RC ready to publish? | No. It was canceled before a complete candidate and independent verification. |

## 11. Sources

Repository sources:

- Omega `script/bundle-omega-rc`.
- Omega `.github/workflows/release.yml`.
- Omega `script/clear-target-dir-if-larger-than`.
- Omega `script/setup-sccache`.
- Omega `assets/settings/default.json`.
- OpenAgents `docs/DEPLOYMENT.md`.
- OpenAgents
  `docs/deploy/2026-07-20-owned-infra-cross-platform-desktop-build-runbook.md`.
- OpenAgents `apps/oa-updates/docs/release-set-v2-feed-runbook.md`.
- OpenAgents `apps/oa-updates/docs/release-signing-runbook.md`.
- OpenAgents `apps/oa-updates/scripts/deploy-cloudrun.sh`.
- OpenAgents `apps/openagents-desktop/scripts/stage-and-package.ts`.

Current external sources:

- [GitHub-hosted runner reference](https://docs.github.com/en/actions/reference/runners/github-hosted-runners).
- [GitHub larger runner reference](https://docs.github.com/en/actions/reference/runners/larger-runners).
- [GitHub Actions runner pricing](https://docs.github.com/en/billing/reference/actions-runner-pricing).
- [GitHub self-hosted runner reference](https://docs.github.com/en/actions/reference/runners/self-hosted-runners).
- [GitHub dependency cache guidance](https://docs.github.com/en/actions/concepts/workflows-and-actions/dependency-caching).
- [Google Cloud Build overview](https://docs.cloud.google.com/build/docs/overview).
- [Apple notarization tool migration](https://developer.apple.com/documentation/technotes/tn3147-migrating-to-the-latest-notarization-tool).
- [Apple Notary API submission](https://developer.apple.com/documentation/notaryapi/submitting-software-for-notarization-over-the-web).

## 12. Non-claims

This audit does not create a remote runner.
It does not enable a paid service.
It does not move a signing secret.
It does not publish an Omega release.
It does not deploy an Omega update feed.
It does not claim that the canceled `0.2.0-rc19` build produced a candidate.
