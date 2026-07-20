# Linux x64 signed candidate build receipt

- Date: 2026-07-20
- Issue: [DIST-08 #8921](https://github.com/OpenAgentsInc/openagents/issues/8921)
  (Linux AppImage/DEB/RPM). Owned-runner provisioning also advances
  [DIST-04 #8917](https://github.com/OpenAgentsInc/openagents/issues/8917).
- ProductSpec: `docs/deploy/openagents-desktop-cross-platform-release.md`
  (§§2-6, 9-11, 14.3).
- Status: **candidate evidence only**. Not published, tagged, promoted, or
  uploaded to any feed. #8921 stays OPEN.

This receipt records a native Linux x64 build of OpenAgents Desktop, the
per-format package inspection, the AppImage update/rollback acceptance, a
headless launch, and Ed25519 release-signature verification of each artifact
against the pinned public key. It is public-safe: it holds no secrets,
credentials, private paths, or network addresses.

## 1. Source and identity

| Field | Value |
| --- | --- |
| Source revision | `e1a0514568d94f8a951ec296d44e4497b212c086` (`origin/main` at build time) |
| Desktop version | `0.1.0-rc.25` |
| Channel | `rc` |
| Lockfile SHA-256 | `a3955abfce13f3de6b7ad85b102c2c22be2b1d94ad823cf9f63c35150fee684f` |
| Target key | `linux-x64` |
| Signing policy | `production` |

## 2. Owned build worker (DIST-04)

| Field | Value |
| --- | --- |
| Runner | GCE `oa-rel-worker-linux-x64`, project `openagentsgemini`, zone `us-central1-a` |
| Machine | `e2-standard-8`, 31 GiB RAM, 96 GB disk |
| OS | Ubuntu 24.04.4 LTS, `x86_64`, kernel 6.17-gcp |
| glibc | 2.39 |

The worker was already provisioned with the Linux packaging closure
(`dpkg-deb`, `rpmbuild`, `fakeroot`, `pkg-config`, `libasound2-dev`,
`libfuse2t64`, `xvfb`, GTK/NSS/GBM runtime libraries) and a rustup `stable`
toolchain. No extra instance was created. The repository was cloned fresh at
the pinned source revision. Dependencies came from the frozen lockfile.

### Toolchain identity (native component ledger)

| Tool | Version |
| --- | --- |
| Electron | 43.1.0 |
| Node | 24.18.0 |
| pnpm | 11.10.0 (`packageManager`) |
| Electron Forge | 7.11.2 |
| Rust | rustc 1.97.1 |
| C compiler | cc (Ubuntu 13.3.0) |

- Native component ledger ref:
  `sha256:c56637f64ff14053ec7559ce8bc8b10007bff7118684f331f5b24e1ffa58481b`
  (2 native components: `oa-desktop-audio`, `@anthropic-ai/claude-agent-sdk-linux-x64`).
- Post-package ASAR admission gate: **pass** (81 entries, 74 unpacked, 2
  closure components byte-verified against the ledger).
- The owned native helper `oa-desktop-audio` was built from source with the
  explicit `x86_64-unknown-linux-gnu` triple.

## 3. Artifacts and Ed25519 verification

Signing used the pinned OpenAgents release key (Ed25519,
kid `2dbe811d19f67528`; public half committed at
`apps/oa-updates/keys/release-pubkey.json`). Signing ran on the coordinator
host, not on the build worker, so the private key stays isolated from build
workers (ProductSpec §§5, 11). Each artifact was signed with
`apps/oa-updates/scripts/sign-release.ts` and verified with
`apps/oa-updates/scripts/verify-release.ts` (the reference fail-closed check
clients embed). Digests were confirmed identical on the worker and after
transfer.

| Format | Artifact | SHA-256 | Bytes | Signature verify |
| --- | --- | --- | --- | --- |
| AppImage | `OpenAgents-0.1.0-rc.25-rc-linux-x64.AppImage` | `a6072f0c64aad76ad7ea532a7b6efd7202f65fc02d33100351ae666d5c94bac5` | 225096605 | **OK** (kid 2dbe811d19f67528) |
| DEB | `OpenAgents-0.1.0-rc.25-rc-linux-x64.deb` | `af1274d25eb19555f4f9e8d2b1fb25dcb95ea8a37d2e15cc40a3b8375b0addda` | 165876430 | **OK** (kid 2dbe811d19f67528) |
| RPM | — | — | — | **blocked** (see §6) |

### Fail-closed negative tests (DEB signature)

| Tamper | Result |
| --- | --- |
| Wrong `sha256` in signature | REJECTED (`sha256 mismatch`) |
| Flipped signature byte | REJECTED (`ed25519 signature does not verify`) |
| Unknown `kid` | REJECTED (`kid ... is not the pinned key`) |

### ReleaseSet v2 relationship

ReleaseSet v2 (`apps/openagents-desktop/src/release-set-contract.ts`,
schema `openagents.desktop.release_set.v2`) requires all five closed target
keys before it is a valid canonical document, so a complete signed ReleaseSet
cannot be produced from Linux artifacts alone. The two artifacts above carry
the exact `{ sha256, byteLength }` that would populate the `linux-x64`
`appimage`/`deb` ReleaseSet entries, and each already has an independent pinned
Ed25519 provenance signature by the release key. Full ReleaseSet convergence,
signing, and atomic promotion are owned by the release coordinator (#8917) and
remain owner-gated.

## 4. AppImage (linux-x64) verification

- File type: type-2 AppImage, ELF 64-bit LSB, `x86-64`.
- `inspectAppImage(image, "x64")` → **valid** (`null`).
- `inspectAppImage(image, "arm64")` → **`architecture_mismatch`** (correct
  rejection of the wrong architecture).
- Native update/rollback acceptance
  (`scripts/linux-appimage-native-acceptance.ts --architecture x64`):
  **pass** — `install: pass`, `selectedMode: 0755`, `rollback: pass`,
  `healthyCommit: pass`. Caveat: this run used the single build for both the
  "current" and "candidate" image, so the byte-distinct rollback path is
  exercised by the applier state machine but restores identical bytes. A
  two-version rollback needs a second built version.
- Headless launch: the packaged AppImage boots under `xvfb` via
  `--appimage-extract-and-run` and runs the in-app smoke suite. All smoke
  checks pass (shell mount, composer focus/first-keystroke, runtime-gateway
  bootstrap protocol 12 / 11 capabilities, workspace tree/watch bridge,
  lifecycle correlation and journal, command-palette routing/close) **except**
  `command-second-instance-deep-link` (`ok:false`). That single failure is
  consistent with `--appimage-extract-and-run` mounting a fresh randomized path
  each launch, which defeats Electron single-instance detection; it is not a
  proven artifact defect. A stable-mount (FUSE) launch is the correct home for
  the second-instance deep-link check.

## 5. DEB (linux-x64) verification

Control metadata:

| Field | Value |
| --- | --- |
| Package | `openagents-desktop-rc` |
| Version | `0.1.0~rc.25` (Debian ordering of `-rc.`) |
| Architecture | `amd64` |
| Maintainer | `OpenAgents, Inc.` |
| Section / Priority | `devel` / `optional` |
| Homepage | `https://openagents.com` |
| Depends | libgtk-3-0, libnotify4, libnss3, xdg-utils, libatspi2.0-0, libdrm2, libgbm1, libxcb-dri3-0, and a trash-helper alternation |

Payload and identity (ProductSpec §2):

- Install root `/usr/lib/openagents-desktop-rc/`.
- Main executable `openagents-rc` — ELF 64-bit `x86-64`.
- `chrome-sandbox` present with SUID mode `-rwsr-xr-x`.
- Bundled native helper `oa-desktop-audio` — ELF `x86-64`.
- Bundled agent runtime `@anthropic-ai/claude-agent-sdk-linux-x64/claude` — ELF
  `x86-64`.
- **No foreign `aarch64` ELF payload** (negative scan clean).
- Desktop entry `com.openagents.desktop.rc.desktop` with
  `Exec=openagents-rc %U`, `Icon=com.openagents.desktop.rc`,
  `StartupWMClass=OpenAgents-RC`, `MimeType=x-scheme-handler/openagents-rc`.

DEB is a direct-download package-manager handoff. It carries **no in-app
rollback claim** (ProductSpec §§4, 12).

## 6. Blocked: RPM (linux-x64)

The RPM maker fails. `electron-installer-redhat@3.4.0`'s `resources/spec.ejs`
hardcodes the `%files` manifest to the package `name`:

```
/usr/bin/<name>
/usr/share/applications/<name>.desktop
/usr/share/pixmaps/<name>.png   (or hicolor .../<name>.png)
```

ProductSpec §2 mandates distinct Linux identities: package
`openagents-desktop-rc`, executable stem `openagents-rc`, desktop entry
`com.openagents.desktop.rc`. The maker stages the files under those correct
distinct names, but the generated spec still lists them under `<name>`, so
`rpmbuild` fails with `File not found: .../usr/bin/openagents-desktop-rc`
(and the `.desktop`/`.png` equivalents). The DEB path is unaffected because
`dpkg-deb` packages the staged tree without a name-keyed `%files` manifest.

Recommended fix (owner/DIST-08 follow-up): add a pnpm patch to
`electron-installer-redhat`'s `spec.ejs` — mirroring the existing
`electron-installer-common@0.10.4` patch — so `%files` references the actual
`bin` / `desktopFileName` / `iconName` basenames instead of `name`. This is a
code change requiring the full `pnpm run check` sweep and RPM-family
native reinstall/upgrade/uninstall evidence, so it is out of scope for this
docs-only candidate receipt.

## 7. Blocked: linux-arm64 (all formats)

Not built. The x64 worker has only the `x86_64-unknown-linux-gnu` Rust target
installed and no `aarch64-linux-gnu` cross-linker, so the `oa-desktop-audio`
cross-build for `aarch64-unknown-linux-gnu` cannot link, and arm64 Electron /
arm64 agent-runtime payloads are not present. ProductSpec §11 requires a native
`desktop-linux-arm64` owned runner for arm64 build and acceptance. arm64 is
recorded as a remaining gap, not faked.

## 8. Limitations and honesty notes

- Built on Ubuntu 24.04 (glibc 2.39). ProductSpec §3 names Ubuntu 22.04
  (glibc 2.35) as the reference floor; a native 22.04 build/host is still
  required before any glibc-2.35 support claim.
- AppImage bytes are not reproducible (embedded timestamps / block map), so the
  digest binds one specific build.
- To capture the DEB after the RPM maker crashed the shared `electron-forge
  make` process, the RPM maker entry was temporarily removed on the disposable
  worker checkout only. The DEB and AppImage maker configurations are unchanged
  from `origin/main`; the worker checkout is not committed.
- No artifact was uploaded to `updates.openagents.com`, GitHub Releases, or any
  other feed. No channel pointer was moved.

## 9. Close rule

#8921 remains **OPEN**. Closure needs native x64 **and** arm64 evidence for all
three formats (AppImage, DEB, RPM), the RPM `%files` fix, and full five-target
signed ReleaseSet convergence with atomic promotion (#8917). This receipt
records the x64 AppImage and DEB as signed candidates plus the precise RPM and
arm64 blockers.
