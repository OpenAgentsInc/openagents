# Linux RPM (x64) and linux-arm64 signed candidate build receipt

- Date: 2026-07-20
- Issue: [DIST-08 #8921](https://github.com/OpenAgentsInc/openagents/issues/8921)
  (Linux AppImage/DEB/RPM). Owned-runner provisioning also advances
  [DIST-04 #8917](https://github.com/OpenAgentsInc/openagents/issues/8917).
- ProductSpec: `docs/deploy/openagents-desktop-cross-platform-release.md`
  (§§2-6, 9-11, 14.3).
- Companion receipt: `docs/deploy/receipts/2026-07-20-linux-signed-candidate.md`
  (x64 AppImage + DEB, and the RPM/arm64 blockers this receipt now closes).
- Status: **candidate evidence only**. Not published, tagged, promoted, or
  uploaded to any feed. #8921 stays OPEN.

This receipt records two things:

1. The code fix that unblocks the Linux **RPM** maker, plus a native **RPM
   x64** build, inspection, and Ed25519 verification.
2. A native **linux-arm64** build of all three formats (AppImage, DEB, RPM),
   inspection, and Ed25519 verification, produced on a dedicated arm64 GCE
   worker that was created for this build and then deleted.

It is public-safe: it holds no secrets, credentials, private key material, or
private paths.

## 1. Source and identity

| Field | Value |
| --- | --- |
| Build base | `21dce16cde48243bea717d782f90fccd8e05fbab` (`origin/main` at task start) plus the DIST-08 RPM `%files` patch |
| Patch commit on `main` | `573c905410f9ce742450719f38890d001960a933` |
| Desktop version | `0.1.0-rc.25` |
| Channel | `rc` |
| Lockfile SHA-256 (at `573c905410`) | `d1294d7401e09204196848872ba53a2b6c18da0d265ca4b84a89435554fdee3e` |
| Signing policy | `production` |

The desktop version (`0.1.0-rc.25`) and channel (`rc`) match the companion
x64 AppImage/DEB candidate, so the four x64 artifacts plus the three arm64
artifacts form one coherent `0.1.0-rc.25` Linux candidate set.

## 2. RPM `%files` fix (the code change)

`electron-installer-redhat@3.4.0`'s `resources/spec.ejs` hardcodes the `%files`
manifest to the package `name` (`/usr/bin/<name>`, `<name>.desktop`,
`<name>.png`). ProductSpec §2 requires **distinct** RC Linux identities:
package `openagents-desktop-rc`, executable stem `openagents-rc`, desktop entry
`com.openagents.desktop.rc`. The RPM maker stages the files under those correct
distinct names, but the generated spec still listed them under `<name>`, so
`rpmbuild` failed with `File not found: .../usr/bin/openagents-desktop-rc`.

Fix: a pnpm patch to `spec.ejs` — mirroring the existing
`electron-installer-common@0.10.4` patch — so `%files` references the actual
`binaryName` / `desktopFileName` / `iconName` basenames (each falling back to
`name`) instead of `name`:

```
/usr/bin/<%= binaryName || name %>
/usr/share/applications/<%= desktopFileName || name %>.desktop
/usr/share/pixmaps/<%= iconName || name %>.png            (or hicolor .../<iconName>.png)
```

- Patch file: `patches/electron-installer-redhat@3.4.0.patch`, registered in
  `pnpm-workspace.yaml` `patchedDependencies`.
- This is a **code change**. It went through the full `pnpm run check`
  (fmt + lint) **green** and the pre-push `check:fast` gate **green**, and was
  pushed to `main` normally (no `--no-verify`) as `573c905410`.

## 3. RPM x64

| Field | Value |
| --- | --- |
| Runner | GCE `oa-rel-worker-linux-x64`, project `openagentsgemini`, zone `us-central1-a`, `e2-standard-8` |
| OS | Ubuntu 24.04.4 LTS, `x86_64`, glibc 2.39 |
| Electron / Node / pnpm / rustc | 43.1.0 / 24.18.0 / 11.10.0 / 1.97.1 |
| Staged ledger ref | `sha256:b8eb31bf5b6c882bd4a7f9730ddf57314df1903bad2097046689c090a1f1dc44` |
| Post-package ASAR gate | **pass** (81 entries, 74 unpacked, 2 closure components byte-verified) |

Build command (the only supported path):
`node --import tsx scripts/stage-and-package.ts --target linux-x64 --mode make`.
All three linux-x64 makers (AppImage, DEB, **RPM**) now succeed; the RPM maker
no longer fails `File not found`.

| Field | Value |
| --- | --- |
| Artifact | `OpenAgents-0.1.0-rc.25-rc-linux-x64.rpm` |
| SHA-256 | `428b9d83f2f8538ba73423aa2471da19fe8d44377270098110673a17c83f570b` |
| Bytes | 176260621 |
| Signature verify | **OK** (kid `2dbe811d19f67528`) |

RPM header and §2 identity:

- `NAME` = `openagents-desktop-rc`, `VERSION` = `0.1.0.rc.25`, `RELEASE` = `1`,
  `ARCH` = `x86_64`, `LICENSE` = `MIT`, `URL` = `https://openagents.com`.
- Payload paths: `/usr/bin/openagents-rc` (symlink to
  `../lib/openagents-desktop-rc/openagents-rc`),
  `/usr/lib/openagents-desktop-rc/`,
  `/usr/share/applications/com.openagents.desktop.rc.desktop`,
  `/usr/share/pixmaps/com.openagents.desktop.rc.png`.
- Desktop entry: `Name=OpenAgents RC`, `Exec=openagents-rc %U`,
  `Icon=com.openagents.desktop.rc`, `StartupWMClass=OpenAgents-RC`,
  `MimeType=x-scheme-handler/openagents-rc`.

Architecture:

- Main executable `openagents-rc` — ELF 64-bit LSB PIE, `x86-64`.
- 10 `x86-64` ELF payload files.
- **No foreign `aarch64` ELF payload** (negative scan clean).

Fail-closed negative tests (RPM x64 signature):

| Tamper | Result |
| --- | --- |
| Wrong `sha256` in signature | REJECTED (`sha256 mismatch`) |
| Flipped signature byte | REJECTED (`ed25519 signature does not verify`) |
| Unknown `kid` | REJECTED (`kid ... is not the pinned key`) |

## 4. linux-arm64 (AppImage, DEB, RPM)

| Field | Value |
| --- | --- |
| Runner | GCE `oa-rel-worker-linux-arm64`, project `openagentsgemini`, zone `us-central1-a`, `t2a-standard-8` (Ampere Altra, arm64) |
| OS | Ubuntu 24.04.4 LTS, `aarch64`, glibc 2.39 |
| Electron / Node / pnpm / rustc | 43.1.0 / 24.18.0 / 11.10.0 / 1.97.1 (`aarch64-unknown-linux-gnu` target) |
| Staged ledger ref | `sha256:a1f91ebd8a44825ed95d23797c3c53f0fff787f1d5f63dc5bc5ab31625ce5d4a` |
| Post-package ASAR gate | **pass** (81 entries, 74 unpacked, 2 closure components byte-verified) |

The instance was created for this build only and **deleted** after the build,
inspection, and transfer completed (see §6). The owned native helper
`oa-desktop-audio` and its arm64 closure were built natively on the arm64 host;
no cross-linking was used.

Build command:
`node --import tsx scripts/stage-and-package.ts --target linux-arm64 --mode make`.
All three arm64 makers succeeded (`ARM64_MAKE_EXIT=0`).

| Format | Artifact | SHA-256 | Bytes | Signature verify |
| --- | --- | --- | --- | --- |
| AppImage | `OpenAgents-0.1.0-rc.25-rc-linux-arm64.AppImage` | `257e57459b62009a712c83f02e08a01da73610180367446badc81dbe0dc1f6d4` | 224675408 | **OK** (kid 2dbe811d19f67528) |
| DEB | `OpenAgents-0.1.0-rc.25-rc-linux-arm64.deb` | `7585c3df4e05228001de808307d716aaa39fe0f31fc8b1a27fa606843b4da4f8` | 164270022 | **OK** (kid 2dbe811d19f67528) |
| RPM | `OpenAgents-0.1.0-rc.25-rc-linux-arm64.rpm` | `2f85492585fbe968a34578262a56e8fd53a024a170c73aef40a2a78ad8f7fb5a` | 167773001 | **OK** (kid 2dbe811d19f67528) |

### AppImage (arm64)

- ELF magic `7f454c46`, AppImage marker `41 49 02`, ELF `e_machine` = **183**
  (`aarch64`).
- `inspectAppImage(image, "arm64")` → **valid** (`null`).
- `inspectAppImage(image, "x64")` → **`architecture_mismatch`** (correct
  rejection of the wrong architecture).

### DEB (arm64)

- Control: `Package` = `openagents-desktop-rc`, `Version` = `0.1.0~rc.25`,
  `Architecture` = `arm64`, `Maintainer` = `OpenAgents, Inc.`
- `/usr/bin/openagents-rc` (symlink), main executable ELF `ARM aarch64`,
  native helper `resources/native/arm64/oa-desktop-audio`,
  `com.openagents.desktop.rc.desktop`, `com.openagents.desktop.rc.png`.
- **No foreign `x86-64` ELF payload** (negative scan clean).

### RPM (arm64)

- `NAME` = `openagents-desktop-rc`, `VERSION` = `0.1.0.rc.25`, `ARCH` =
  `aarch64`.
- Payload paths: `/usr/bin/openagents-rc`, `/usr/lib/openagents-desktop-rc/`,
  `/usr/share/applications/com.openagents.desktop.rc.desktop`,
  `/usr/share/pixmaps/com.openagents.desktop.rc.png`.
- Desktop entry: `Name=OpenAgents RC`, `Exec=openagents-rc %U`,
  `Icon=com.openagents.desktop.rc`, `StartupWMClass=OpenAgents-RC`,
  `MimeType=x-scheme-handler/openagents-rc`.
- Main executable `openagents-rc`, native helper `oa-desktop-audio`, and the
  bundled `@anthropic-ai/claude-agent-sdk-linux-arm64/claude` are all ELF
  `ARM aarch64`; 10 `aarch64` ELF payload files; **no foreign `x86-64` ELF**.
- Fail-closed negative (flipped signature byte): REJECTED
  (`ed25519 signature does not verify`).

## 5. Signing

Signing used the pinned OpenAgents release key (Ed25519,
kid `2dbe811d19f67528`; public half committed at
`apps/oa-updates/keys/release-pubkey.json`). Every artifact was signed on the
coordinator host with `apps/oa-updates/scripts/sign-release.ts` and verified
with `apps/oa-updates/scripts/verify-release.ts` (the reference fail-closed
check clients embed). Signing ran on the coordinator, not on either build
worker, so the private key stays isolated from build workers (ProductSpec
§§5, 11). Each artifact's SHA-256 was confirmed identical on the worker and
after transfer.

## 6. arm64 worker lifecycle (residual-zero)

| Step | Value |
| --- | --- |
| Created | `oa-rel-worker-linux-arm64`, `t2a-standard-8`, Ubuntu 24.04 arm64, 100 GB pd-balanced, `us-central1-a`, no external IP |
| Provisioned | apt build closure (`build-essential`, `pkg-config`, `libasound2-dev`, `fakeroot`, `rpm`, `dpkg-dev`, `libfuse2t64`, `xvfb`, GTK/NSS/GBM/notify runtime), Node 24.18.0 (NodeSource), pnpm 11.10.0 (corepack), rustup + native `aarch64-unknown-linux-gnu` |
| Deleted | Deleted after build/inspection/transfer; `gcloud compute instances describe … --zone us-central1-a` returns **not found** |

The x64 worker `oa-rel-worker-linux-x64` was left **running** (it is the
standing x64 release worker).

Note: a separate, pre-existing `oa-rel-worker-linux-arm64` instance in zone
`us-central1-b` (created 2026-07-16, `TERMINATED`/stopped since 2026-07-19)
predates this task and was **not** created by it. It was left untouched under
multi-agent hygiene.

## 7. Limitations and honesty notes

- **glibc floor.** Both workers ran Ubuntu 24.04 (glibc 2.39). ProductSpec §3
  names Ubuntu 22.04 (glibc 2.35) as the reference floor; a native 22.04
  build/host is still required before any glibc-2.35 support claim, for both
  x64 and arm64.
- **Reproducibility.** AppImage bytes are not reproducible (embedded
  timestamps / block map), so each digest binds one specific build.
- **No feed movement.** No artifact was uploaded to `updates.openagents.com`,
  GitHub Releases, or any other feed. No channel pointer was moved.
- **ReleaseSet.** ReleaseSet v2
  (`apps/openagents-desktop/src/release-set-contract.ts`) requires all five
  closed target keys before it is a valid canonical document, so a complete
  signed ReleaseSet cannot be produced from Linux artifacts alone. Full
  five-target convergence, signing, and atomic promotion are owned by the
  release coordinator (#8917) and remain owner-gated on the macOS-x64 and
  Windows targets.

## 8. Close rule

#8921 remains **OPEN**. This receipt closes the two prior blockers recorded in
`2026-07-20-linux-signed-candidate.md` §§6-7 (the RPM `%files` fix, RPM x64,
and all three arm64 formats), all signed against the pinned release key with
§2 distinct identities and correct per-target architecture. The remaining gaps
are the native glibc-2.35 floor host and full five-target signed ReleaseSet
convergence via #8917 (blocked on macOS-x64 + Windows owner gates).
