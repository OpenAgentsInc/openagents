# macOS arm64 signed + notarized Desktop candidate receipt

- Date: 2026-07-20
- Program: [#8913](https://github.com/OpenAgentsInc/openagents/issues/8913)
- Issue: [DIST-06 #8919](https://github.com/OpenAgentsInc/openagents/issues/8919)
- ProductSpec: [OpenAgents Desktop cross-platform release](../openagents-desktop-cross-platform-release.md) §§2-7, 14.1
- Target key: `darwin-arm64` (Apple Silicon)
- Channel: `rc`
- Version: `0.1.0-rc.25`
- Source revision: `e1a0514568d94f8a951ec296d44e4497b212c086` (clean detached worktree off `origin/main`)
- Host: Apple Silicon (arm64) macOS. notarytool 1.1.2 (41), Node 24.13.1, pnpm 11.10.0, Electron 43, Electron Forge 7.11.2

## Status

This is a **signed + notarized CANDIDATE for acceptance evidence only**. It was
NOT published, tagged, promoted, or uploaded to any public feed. #8919 stays
**OPEN**: the `darwin-x64` (Intel) half of the release set is not built here
because no Intel Mac host is available, and the ProductSpec §14.1 native
clean-install / first-launch / agent-runtime / N-1-update / rollback and
Apple-Silicon migration acceptance receipts are still owed.

## Build

Built in an isolated staging workspace via the repository entrypoint
`pnpm run make:mac` (`scripts/stage-and-package.ts --target darwin-arm64 --mode make`),
which fails closed without the Developer ID identity and notary credentials. No
code changes were required.

- Staged component ledger: `sha256:d00e427fb49239888012744e6d06d78d08f676377cde31b9c7e531d05b8acafe` (3 components)
- Live post-package ASAR gate: **pass** (81 entries, 74 unpacked, 3 closure components byte-verified)

## Artifacts

Kept outside the repository (never committed). Canonical version-first,
channel-tagged names per ProductSpec §6:

| Artifact | SHA-256 | Bytes |
| --- | --- | --- |
| `OpenAgents-0.1.0-rc.25-rc-darwin-arm64.dmg` | `807a3e46b64f190ec445cd36804c834b4d60ff6c6d6e0445420a240105bef471` | 204470982 |
| `OpenAgents-0.1.0-rc.25-rc-darwin-arm64.zip` | `79fa00a91250564b8f5f79bc9cc4847df9ead6ad78a3015bb4111bea7327a5a4` | 206482418 |

## Notarization (Apple notary service, `xcrun notarytool submit --wait`)

| Layer | Submission ID | Status |
| --- | --- | --- |
| App (`OpenAgents RC.app`, zipped) | `4368c642-574b-42a7-82d8-4914be2eb80b` | Accepted |
| DMG (`OpenAgents-0.1.0-rc.25-rc-darwin-arm64.dmg`) | `b21b5b9e-8961-49f9-98dc-9c9ad819de93` | Accepted |

The app was signed, notarized, and stapled BEFORE the DMG/ZIP makers ran
(`preMake` hook), so both distributables carry an already-stapled app. The DMG
itself was then notarized and stapled (`postMake` hook). Both `notarytool`
submissions returned `Accepted`; both `stapler staple` actions succeeded.

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
| Bundle identifier | `codesign -dvvv <app>` | PASS — `Identifier=com.openagents.desktop.rc` (RC identity, ProductSpec §2) |
| Hardened runtime | `codesign --display --verbose=4 <app>` | PASS — CodeDirectory `flags=0x10000(runtime)`, `Runtime Version=26.4.0` |
| Architecture | `lipo -archs` / `file` on `Contents/MacOS/OpenAgents RC` | PASS — `Mach-O 64-bit executable arm64` (thin arm64) |
| Entitlements | `codesign -d --entitlements :- <app>` | PASS — `allow-jit`, `allow-unsigned-executable-memory`, `network.client`, `network.server` |
| CDHash | `codesign -dvvv <app>` | `5cfb91bc2501969319864b3fb9a80f1fc515e262` |

### Electron fuses (read from the distributed app)

`@electron/fuses read --app <app>` — all nine match `forge.config.ts`:

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
| `Info.plist` `ElectronAsarIntegrity` declared header hash (SHA256) | `9bf01ffb856f152b6dd99415be84286aeec1c28b810051989b61deb4eba7329f` |
| Actual `app.asar` header-string SHA256 (`@electron/asar getRawHeader`) | `9bf01ffb856f152b6dd99415be84286aeec1c28b810051989b61deb4eba7329f` |
| Match | PASS (104 entries listed) |

### ZIP-contained app (extracted with `ditto -x -k`)

| Check | Result |
| --- | --- |
| `xcrun stapler validate` | PASS |
| `spctl -a -t exec -vv` | PASS — `accepted`, `source=Notarized Developer ID` |
| `codesign --verify --deep --strict` | PASS (exit 0) |
| Architecture | PASS — arm64 |

## Remaining gap (why #8919 stays open)

1. **`darwin-x64` (Intel) build is not produced.** DIST-06 ships one macOS
   release set containing BOTH `darwin-arm64` and `darwin-x64` DMG+ZIP. The x64
   half requires an Intel Mac worker (`desktop-darwin-x64` slot, ProductSpec §11,
   "not admitted"). No Intel host is available, so the x64 artifacts, their
   notarization, and their verification are still owed. The release-set finalizer
   must refuse a set missing either architecture.
2. **Native acceptance receipts are still owed on real hosts.** ProductSpec §14.1
   requires, on downloaded candidate bytes and native Intel + Apple Silicon
   hardware: clean install, first launch, agent-runtime start, clean shutdown,
   N-1 update, interrupted-update recovery, and retained-slot rollback — plus the
   x64-installed-to-arm64 (Rosetta → full arm64 artifact) migration proof on
   Apple Silicon. This receipt covers signing/notarization/trust/architecture of
   the arm64 candidate only; it is not a support admission.

Per ProductSpec §1 and the issue close rule, DIST-06 closes only with real
signed/notarized downloaded-artifact receipts on BOTH Mac architectures; this
arm64 candidate is necessary partial evidence, not sufficient.
