# Owned-infra cross-platform Desktop candidate-build runbook

- Class: operator procedure
- Date: 2026-07-20
- Scope: OpenAgents Desktop signed candidate builds on owned infrastructure
- Related issues: DIST-04 (#8917), DIST-06 (#8919), DIST-07 (#8920),
  DIST-08 (#8921), DIST-12 (#8925), DIST-13 (#8926)
- Authority note: this runbook produces signed **candidate evidence** only. It
  does not publish, tag, promote a channel, or push to a public feed. The owner
  release ceremony (`pnpm run release`, DIST-13) owns promotion.

## Purpose

Reproduce signed and, on macOS, notarized OpenAgents Desktop candidate
artifacts for every release target using owned hosts, never GitHub-hosted CI.
Each target run ends with a public-safe receipt under
`docs/deploy/receipts/`, exact artifact hashes, and full trust verification.

## Target-to-host map

| Target        | Host                                              | Signing path                          |
| ------------- | ------------------------------------------------- | ------------------------------------- |
| `darwin-arm64`| Local Apple Silicon Mac                           | Developer ID cert already in keychain |
| `darwin-x64`  | Intel Mac over Tailnet (`imac-pro-bertha`)        | Ephemeral cert import, then destroy   |
| `linux-x64`   | GCE worker `oa-rel-worker-linux-x64`              | Ed25519 release-set signing on a Mac  |
| `linux-arm64` | Ephemeral GCE arm64 instance (create, then delete)| Ed25519 release-set signing on a Mac  |
| `win-x64`     | GCE `oa-rel-worker-win-x64` (currently blocked)   | Azure Trusted Signing (owner gate)    |

## Where the credentials live

Read values only into the tool that consumes them. Never print, echo, log,
commit, or transmit a credential value. The runbook names locations only.

- **Ed25519 release-set key:** `~/work/.secrets/openagents-release-signing.env`
  (canonical backup: GCP Secret Manager, project `openagentsgemini`). Public
  half committed at `apps/oa-updates/keys/release-pubkey.json`
  (kid `2dbe811d19f67528`).
- **Apple Developer ID Application cert:** GCP Secret Manager secrets
  `developer-id-application-p12` (base64 of the `.p12`) and
  `developer-id-application-p12-password`. Identity string
  `Developer ID Application: OpenAgents, Inc. (HQWSG26L43)`, team `HQWSG26L43`.
  The local Apple Silicon Mac already has this identity in its login keychain.
- **Apple notarization (App Store Connect API key):**
  `~/work/.secrets/appstoreconnect/private_keys/AuthKey_*.p8` plus key id and
  issuer id in `~/work/.secrets/appstoreconnect.env`, which also sets
  `OA_DEVELOPER_ID_APPLICATION`.
- **Tailnet SSH:** account name in `~/work/.secrets/tailnet.env`
  (`TAILNET_SSH_USERNAME`). The Intel Mac password is in
  `~/work/.secrets/imac-ssh.pass` (gitignored, and the same password unlocks that
  Mac's login keychain).
- **GCP automation service account:** prefix every `gcloud` command with
  `CLOUDSDK_CONFIG=/Users/christopherdavid/work/.secrets/gcloud-sa-config`. This
  uses the `oa-mvp-automation` service account and never disturbs the
  interactive default account.

## Working conventions (apply to every target)

- Start each build in a fresh worktree off current `origin/main`
  with `git fetch origin main` then `git worktree add --detach <path> origin/main`, and
  remove it when done. Never build in the canonical checkout, which is often
  dirty with other live work.
- Build every target from the SAME source commit and version so the artifacts
  form one coherent release set. Record that commit in every receipt.
- Keep build output OUTSIDE the repository. Never commit artifacts.
- Receipts are docs-only: push with `git push --no-verify origin HEAD:main`
  after running the STE and neutral-language guards by hand (see the last
  section). A code change (for example a packaging patch) needs full
  `pnpm run check` and a normal push.
- Do not publish, tag, promote, or push artifacts to a public feed.

## A. `darwin-arm64` — local Apple Silicon Mac

1. Confirm the signing identity is present:
   `security find-identity -v -p codesigning | grep HQWSG26L43` reports one
   valid identity. (Do not run `security find-generic-password`, which can open a
   blocking dialog.)
2. In a fresh worktree, `pnpm install --frozen-lockfile`.
3. Build, sign, and notarize:
   `pnpm run make:mac` (which stages `--target darwin-arm64` and drives
   `notarizeAndStapleApp` / `notarizeAndStapleDmg` from `forge.config.ts`,
   reading `OA_DEVELOPER_ID_APPLICATION` and the notarization variables from
   `~/work/.secrets/appstoreconnect.env`). This produces a DMG and a ZIP.
4. Verify against the produced and stapled candidate bytes (see the
   verification checklist). Architecture must be thin `arm64`.
5. Record hashes and notarization submission ids, write the receipt.

## B. `darwin-x64` — Intel Mac over Tailnet, ephemeral cert

The Intel Mac does not hold the Developer ID cert. Import it for the build,
then destroy it. The account password also unlocks the login keychain, so the
import is non-interactive with no GUI dialog.

1. **Reachability.** `tailscale status` for the live device list and IP (treat
   this as source of truth. The setup-time IP was `100.97.233.57` for
   `imac-pro-bertha`). Confirm `ping` and `nc -z <ip> 22`.
2. **SSH.** Password authentication needs
   `ssh -o PreferredAuthentications=password,keyboard-interactive`. Drive the
   `(user@host) Password:` prompt with `expect` (macOS has no `sshpass`), for
   example the reusable pattern that reads `IP`, `TAILNET_SSH_USERNAME`, and the
   password from env and sends the password on an `(?i)password:` match. To
   avoid repeating the password, bootstrap an ephemeral SSH key: generate a
   throwaway keypair, append its public half to the Mac's
   `~/.ssh/authorized_keys` with one password-authenticated command, then use
   key-based `ssh`/`scp` for the rest and remove that line at the end.
3. **Toolchain.** On the Mac, install Node 24 (through `nvm`), enable `pnpm`
   with `corepack`, clone the repository, `git checkout <release commit>`, and
   `pnpm install --frozen-lockfile`.
4. **Ephemeral cert import.** Fetch the `.p12` and its password from Secret
   Manager with the automation service account. Copy the decoded `.p12` and the
   `.p8` to a locked-down temp directory over `scp`. Then:
   - `security unlock-keychain -p <password> login.keychain-db`
   - `security import <p12> -k login.keychain-db -P <p12-password> -T /usr/bin/codesign -T /usr/bin/productsign`
   - `security set-key-partition-list -S apple-tool:,apple: -s -k <password> login.keychain-db`
   Confirm `security find-identity -v -p codesigning | grep -c HQWSG26L43`
   reports 1.
5. **Build.** Run the signed and notarized build for `darwin-x64` (inspect
   `package.json` and `scripts/stage-and-package.ts`. The `make:mac` script
   hardcodes `--target darwin-arm64`, so invoke the stage-and-package script
   with `--target darwin-x64 --mode make`). Set the notarization environment
   variables the runbook and `forge.config.ts` require.
6. **Verify.** Run the verification checklist against the stapled bytes.
   Architecture must be `x86_64`.
7. **Cleanup (mandatory, run even if the build fails).** Delete the imported
   identity and key from the keychain, `rm -P` (shred) the transferred `.p12`
   and `.p8` and their temp directory, and remove the ephemeral public-key line
   from `~/.ssh/authorized_keys`. Confirm
   `security find-identity -v -p codesigning | grep -c HQWSG26L43` is back to 0.
   The Node toolchain and repository clone may stay. The credentials may not.
8. Record hashes and submission ids, write the receipt, and state in it that
   all signing material was destroyed on the remote host.

## C. `linux-x64` and `linux-arm64` — GCE workers

1. **Start the worker.**
   `CLOUDSDK_CONFIG=... gcloud compute instances start oa-rel-worker-linux-x64 --zone us-central1-a --project openagentsgemini`.
   SSH with
   `gcloud compute ssh oa-rel-worker-linux-x64 --zone us-central1-a --tunnel-through-iap --project openagentsgemini`.
2. **Provision (DIST-04 owned-runner work).** Ensure Node 24, `pnpm`, `rustup`
   with the host target, and the Linux packaging closure (`dpkg-dev`, `rpm`,
   `fakeroot`, and the AppImage path). Clone fresh at the release commit and
   `pnpm install --frozen-lockfile`. Build the native helper
   (`oa-desktop-audio`) from source.
3. **Build.** Produce the AppImage, DEB, and RPM. RPM currently needs a
   `pnpm patch` to `electron-installer-redhat`'s `spec.ejs`, because its
   `%files` section hardcodes `/usr/bin/<name>` and conflicts with the distinct
   package, executable, and desktop identities. Mirror the existing
   `electron-installer-common` patch. That patch is a code change and needs full
   `pnpm run check`.
4. **Sign and verify.** Signing uses the Ed25519 release key on a Mac, isolated
   from the build worker. Follow the `sign-release` and `verify-release` path in
   `apps/oa-updates/docs/release-signing-runbook.md`. Verify each artifact
   against the pinned public key, confirm architecture, confirm the distinct
   identities, and run the fail-closed negatives (tampered hash, tampered
   signature, wrong kid).
5. **`linux-arm64`.** Create an ephemeral GCE arm64 instance (for example a
   `t2a-standard-8`), provision the same closure, build and verify the three
   arm64 formats, then **delete that instance** and confirm zero residual.
6. Record hashes, write the receipt.

## D. `win-x64` — blocked owner gate

Windows (DIST-07 #8920) requires the Azure Trusted Signing publisher. There is
no code-signing certificate in `~/work/.secrets` or Secret Manager, and the
issue forbids a self-signed fallback (`verifyUpdateCodeSignature: false` is
never allowed). This target stays blocked until the owner provisions Azure
Trusted Signing. The GCE worker `oa-rel-worker-win-x64` exists (currently
stopped) for the eventual build.

## Verification checklist (per artifact)

macOS (against the stapled and, where possible, downloaded bytes):

- `stapler validate` on the app and the DMG.
- `spctl` assessment reports `Notarized Developer ID`, origin
  `Developer ID Application: OpenAgents, Inc. (HQWSG26L43)`.
- `codesign --verify --deep --strict` valid on disk.
- Team id `HQWSG26L43`, expected bundle id, hardened runtime flag set.
- All nine Electron fuses match `forge.config.ts`.
- ASAR integrity header hash equals the actual `app.asar` header hash.
- Architecture matches the target (`arm64` or `x86_64`).

Linux (against the produced bytes):

- Signature verifies against the pinned Ed25519 public key
  (kid `2dbe811d19f67528`).
- Architecture matches the target. No foreign-architecture ELF is bundled.
- Distinct package, executable, and desktop identities are correct.
- Fail-closed negatives (tampered hash, tampered signature, wrong kid) are
  rejected.

Record for every artifact: file name, SHA-256, byte length, and (macOS) the
notarization submission id and acceptance state.

## Release-set convergence and promotion

A complete signed ReleaseSet v2 requires all five targets. The schema refuses a
partial set. The candidate artifacts from this runbook carry the exact
`{ sha256, byteLength }` that populate the per-target entries. Convergence and
atomic channel promotion are the DIST-04 coordinator (#8917) and the DIST-13
owner command (`pnpm run release`, #8926), which surface the remaining owner
gates (notarization credentials presence, first stable promotion) as explicit
named steps. This runbook never promotes.

## Owner gates (current)

- **Intel Mac for `darwin-x64`:** available over Tailnet
  (`imac-pro-bertha`). The password lives in `~/work/.secrets/imac-ssh.pass`.
- **Azure Trusted Signing for `win-x64`:** not provisioned. This blocks DIST-07.
- **GCP budget for the ephemeral arm64 instance:** modest. Delete the instance
  after the build.
- **Final promotion:** owner-only, through the DIST-13 command.

## STE and neutral-language checks for receipts and this runbook

New governed documents update the STE inventory. After adding or editing a doc:

- Regenerate the STE ledger and inventory
  (`pnpm run generate:ste-ledger`, and the receipt commits update
  `docs/ste/final-inventory.v1.json` and `docs/ste/migration-ledger.v1.json`).
- Run the STE structural check (`node --import tsx scripts/check-ste.ts --all`)
  and the STE control-semantic check
  (`node --import tsx scripts/check-ste-semantic.ts`). A deliberate normative
  change to a governed control file is acknowledged with
  `--capture <path>` to refresh that file's semantic baseline.
- Run the neutral-language and authority guards
  (`node scripts/vp1-retired-money-surface-guard.mjs .`,
  `node scripts/google-cloud-authority-guard.mjs`,
  `node scripts/zero-supported-bun-guard.mjs .`).

Only after those pass by hand, push the docs-only change with `--no-verify`.
