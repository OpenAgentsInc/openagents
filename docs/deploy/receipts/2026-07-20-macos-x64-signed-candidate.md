# macOS x64 (Intel) signed + notarized Desktop candidate receipt

- Date: 2026-07-20
- Program: [#8913](https://github.com/OpenAgentsInc/openagents/issues/8913)
- Issue: [DIST-06 #8919](https://github.com/OpenAgentsInc/openagents/issues/8919)
- ProductSpec: [OpenAgents Desktop cross-platform release](../openagents-desktop-cross-platform-release.md) §§2-7, 14.1
- Target key: `darwin-x64` (Intel / x86_64)
- Channel: `rc`
- Version: `0.1.0-rc.25`
- Source revision: `e1a0514568d94f8a951ec296d44e4497b212c086` (identical to the arm64 candidate, so the two form one coherent macOS release set)
- Host: `imac-pro-bertha` (Intel iMac Pro), Tailscale `100.97.233.57`, macOS 15.7.4 (`x86_64`). Node 24.18.0, pnpm 11.10.0, rustc/cargo 1.90.0, Electron 43, Electron Forge 7.11.2, `xcrun notarytool`. Xcode Command Line Tools (clang/node-gyp) present.

## Status

This is a **signed + notarized CANDIDATE for acceptance evidence only**. It was
NOT published, tagged, promoted, or uploaded to any public feed. #8919 stays
**OPEN**: this receipt completes the `darwin-x64` half of the macOS release set
(the `darwin-arm64` half is
`docs/deploy/receipts/2026-07-20-macos-arm64-signed-candidate.md`, same source
revision and version), but the ProductSpec §14.1 native clean-install /
first-launch / agent-runtime / N-1-update / rollback and Rosetta→arm64 migration
acceptance receipts on real hardware are still owed, and the full five-target
ReleaseSet convergence/promotion is tracked separately under #8917.

## Build

Built on the Intel host over Tailnet via the repository entrypoint
`scripts/stage-and-package.ts --target darwin-x64 --mode make` (preceded by
`scripts/prepare-macos-maker.ts`, the x64 analogue of the arm64-hardcoded
`pnpm run make:mac`), which fails closed without the Developer ID identity and
notary credentials. No code changes were required. The owned native voice
helper `oa-desktop-audio` was compiled from the exported source with the
target's explicit Rust triple `x86_64-apple-darwin`. The Apple Foundation
Models Swift sidecar (`foundation-bridge`) is macOS-arm64-only and is correctly
NOT built or staged for this Intel target.

- Staged component ledger: 2 components (x64 voice helper + its manifest; the
  arm64-only `foundation-bridge` is absent by design — the arm64 candidate
  staged 3).
- Live post-package ASAR gate: **pass** (81 entries, 74 unpacked, 2 closure
  components byte-verified) against the staged ledger for `darwin-x64`.

## Artifacts

Kept outside the repository (never committed). Canonical version-first,
channel-tagged names per ProductSpec §6:

| Artifact | SHA-256 | Bytes |
| --- | --- | --- |
| `OpenAgents-0.1.0-rc.25-rc-darwin-x64.dmg` | `2675bf7ba5ba9d8ec334a41404b387f44a7556bc47b8f8cf4bbb54a18ae2228d` | 212294508 |
| `OpenAgents-0.1.0-rc.25-rc-darwin-x64.zip` | `a65e39d1105c4c370326570eb883eedb49aede42814a12a5ede78ea87f139679` | 214673703 |

## Notarization (Apple notary service, `xcrun notarytool submit --wait`)

| Layer | Submission ID | Status |
| --- | --- | --- |
| App (`OpenAgents RC.app`, zipped) | `78ae25e2-04fa-4b75-89cb-9ce8a3cb004d` | Accepted |
| DMG (`OpenAgents-0.1.0-rc.25-rc-darwin-x64.dmg`) | `dbfcdf90-0a8e-4cca-b11d-060257073267` | Accepted |

The app was signed, notarized, and stapled BEFORE the DMG/ZIP makers ran
(`preMake` hook), so both distributables carry an already-stapled app. The DMG
itself was then notarized and stapled (`postMake` hook: "The staple and
validate action worked!"). Both `notarytool` submissions returned `Accepted`.

## Verification results

All checks were run against the **distributed candidate bytes**: the app was
verified as mounted from the DMG, and independently after extraction from the
ZIP. Every check passed.

### DMG (image)

| Check | Command | Result |
| --- | --- | --- |
| Notarization staple | `xcrun stapler validate <dmg>` | PASS ("The validate action worked!") |
| Gatekeeper image assessment | `spctl -a -t open --context context:primary-signature -vv <dmg>` | PASS — `accepted`, `source=Notarized Developer ID`, `origin=Developer ID Application: OpenAgents, Inc. (HQWSG26L43)` |

### App (mounted from DMG)

| Check | Command | Result |
| --- | --- | --- |
| Deep code-sign verify | `codesign --verify --deep --strict --verbose=2 <app>` | PASS — `valid on disk`, `satisfies its Designated Requirement` |
| Gatekeeper exec assessment | `spctl -a -t exec -vv <app>` | PASS — `accepted`, `source=Notarized Developer ID` |
| Notarization staple | `xcrun stapler validate <app>` | PASS |
| Developer ID / Team ID | `codesign -dvvv <app>` | PASS — `Authority=Developer ID Application: OpenAgents, Inc. (HQWSG26L43)` → Developer ID CA → Apple Root CA; `TeamIdentifier=HQWSG26L43` |
| Bundle identifier | `codesign -dvvv <app>` / `Info.plist` | PASS — `Identifier=com.openagents.desktop.rc` (RC identity, ProductSpec §2) |
| Hardened runtime | `codesign --display --verbose=4 <app>` | PASS — CodeDirectory `flags=0x10000(runtime)`, `Runtime Version=26.4.0` |
| Architecture | `lipo -archs` / `file` on `Contents/MacOS/OpenAgents RC` | PASS — `Mach-O 64-bit executable x86_64` (thin x86_64) |
| Entitlements | `codesign -d --entitlements :- <app>` | PASS — `allow-jit`, `allow-unsigned-executable-memory`, `network.client`, `network.server` |
| CDHash | `codesign -dvvv <app>` | `b63166fe8f75c08ad30629a43c21976b925c5fa1` |

### Electron fuses (read from the distributed app)

`electron-fuses read --app <app>` — all nine match `forge.config.ts`:

| Fuse | Value |
| --- | --- |
| RunAsNode | Disabled |
| EnableCookieEncryption | Enabled |
| EnableNodeOptionsEnvironmentVariable | Disabled |
| EnableNodeCliInspectArguments | Disabled |
| EnableEmbeddedAsarIntegrityValidation | Enabled |
| OnlyLoadAppFromAsar | Enabled |
| LoadBrowserProcessSpecificV8Snapshot | Disabled |
| GrantFileProtocolExtraPrivileges | Disabled |
| WasmTrapHandlers | Enabled |

### ASAR integrity

| Check | Result |
| --- | --- |
| `Info.plist` `ElectronAsarIntegrity` declared header hash (SHA256) | `4e87be1d58dcefcecd384a28136d646772eeff8b8e83326021dae54f1c2ee943` |
| Actual `app.asar` header-string SHA256 (`@electron/asar getRawHeader`) | `4e87be1d58dcefcecd384a28136d646772eeff8b8e83326021dae54f1c2ee943` |
| Match | PASS (104 entries listed) |

### ZIP-contained app (extracted with `ditto -x -k`)

| Check | Result |
| --- | --- |
| `xcrun stapler validate` | PASS |
| `spctl -a -t exec -vv` | PASS — `accepted`, `source=Notarized Developer ID` |
| `codesign --verify --deep --strict` | PASS — `valid on disk`, `satisfies its Designated Requirement` |
| Architecture | PASS — `Mach-O 64-bit executable x86_64` |

## Signing-material destruction on the build host

The Developer ID `.p12` and passphrase (GCP Secret Manager) and the App Store
Connect notary `.p8` were transferred to the Intel host into a `chmod 700` temp
dir purely for this build and then destroyed. An ephemeral SSH keypair was used
for the session and removed at the end. All confirmed independently over a fresh
password-authenticated session AFTER the ephemeral key was removed:

| Check | Result |
| --- | --- |
| `security find-identity -v -p codesigning \| grep -c HQWSG26L43` | `0` (was `1` during the build; identity + private key deleted with `security delete-identity -Z`) |
| Transferred `.p12`, `.p8`, and the secret env file | shredded with `rm -P`; the temp secrets dir was removed (`SECDIR_EXISTS NO`) |
| Ephemeral SSH pubkey in `~/.ssh/authorized_keys` | removed (0 lines; key-based auth now returns `Permission denied (publickey)`) |
| Local copies on the coordinating Mac | shredded with `rm -P` |

The Node 24 toolchain, Rust, and the repository clone at `~/dist06/openagents`
were left on the Intel host (no secrets remain there). No password, passphrase,
`.p12`/`.p8` bytes, or key material was printed, logged, or committed.

## Remaining gap (why #8919 stays open)

With this receipt plus the arm64 receipt, BOTH macOS architectures now have real
signed + notarized + Gatekeeper-verified candidate artifacts at the same source
revision and version. DIST-06 nonetheless stays OPEN:

1. **Native acceptance receipts are still owed on real hosts.** ProductSpec
   §14.1 requires, on downloaded candidate bytes and native Intel + Apple
   Silicon hardware: clean install, first launch, agent-runtime start, clean
   shutdown, N-1 update, interrupted-update recovery, and retained-slot
   rollback — plus the x64-installed-to-arm64 (Rosetta → full arm64 artifact)
   migration proof on Apple Silicon. This receipt covers
   signing/notarization/trust/architecture of the x64 candidate only; it is not
   a support admission.
2. **The full five-target ReleaseSet must converge and be promoted via #8917.**
   The macOS pair is one part of the cross-platform set (macOS arm64/x64,
   Windows, Linux); the release-set finalizer must refuse any set missing an
   admitted target, and promotion is a separate gated step.

Per ProductSpec §1 and the issue close rule, DIST-06 closes only with the real
downloaded-artifact acceptance receipts above; this x64 candidate is necessary
partial evidence, not sufficient.
