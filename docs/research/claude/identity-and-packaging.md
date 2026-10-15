# Identity and packaging

## Bundle

Observed from the main Info.plist. The file is XML and the read reported no
parse limitation.

| Key | Value |
| --- | --- |
| `CFBundleIdentifier` | `com.anthropic.claudefordesktop` |
| `CFBundleDisplayName` / `CFBundleName` | Claude |
| `CFBundleShortVersionString` / `CFBundleVersion` | `2.37337.1` |
| `CFBundleExecutable` | Claude |
| `CFBundlePackageType` | `APPL` |
| `NSPrincipalClass` | `AtomApplication` |
| `NSMainNibFile` | `MainMenu` |
| `LSMinimumSystemVersion` | `13.0` |
| `LSApplicationCategoryType` | `public.app-category.developer-tools` |
| `NSHumanReadableCopyright` | `2025 Anthropic PBC` |
| `ElectronTeamID` | `Q6L2SF6YDW` |
| `DTSDKName` / `DTSDKBuild` | `macosx15.5` / `24F74` |
| `DTXcode` / `DTXcodeBuild` | `1640` / `16F6` |
| `DTCompiler` | `com.apple.compilers.llvm.clang.1_0` |

`LSEnvironment` sets `MallocNanoZone` to `0`. `NSQuitAlwaysKeepsWindows` is
false. `NSHighResolutionCapable` is true. App Transport Security allows local
networking and does not set `NSAllowsArbitraryLoads`.

The four Electron helper apps carry the same version string. Their bundle
identifier is `com.anthropic.claudefordesktop.helper`. Electron Framework's
own plist says `com.github.Electron.framework` and `CFBundleVersion` `44.7.0`.

### URL schemes and activities

- URL scheme `claude`.
- URL scheme `msauth.com.anthropic.claudefordesktop`, display name `MSAL`.
- User activity `com.anthropic.claude.code.session`.
- User activity `com.apple.corespotlightitem`.

### Documents and exported types

The bundle advertises a Desktop Extension type for `.dxt` and `.mcpb`, and a
Skill type for `.skill`. The exported type `com.anthropic.skill` conforms to
`public.zip-archive`. Other document roles cover folders, text, PDF, Word,
RTF, ODT, EPUB, PNG, JPEG, GIF, and WebP. Markdown is an imported type.

### Purpose strings

These are the strings the plist shows the system when it asks for access:

- AppleEvents: "Claude needs this so commands you run in Claude can control other apps."
- Documents: "Claude stores your scheduled tasks, live artifacts and other Cowork files here."
- Desktop and Downloads: access to work with files in those folders.
- Local network: "to reach development servers and other local devices."
- Microphone: "for dictation and for content that you allow to use it."
- Speech recognition: "for voice dictation."
- Camera and location: "for content that you allow to use it."
- Bluetooth: "to connect to hardware accessories." The peripheral string is "This app needs access to Bluetooth."
- Audio capture: "This app needs access to audio capture."

There is no photos-library purpose string. The signature still carries the
photos-library entitlement, below.

## Signature

The main universal Mach-O is signed with the hardened runtime.

- Identifier `com.anthropic.claudefordesktop`, team `Q6L2SF6YDW`.
- Format `Mach-O universal (x86_64 arm64)`.
- Authorities: Developer ID Application: Anthropic PBC (Q6L2SF6YDW),
  Developer ID Certification Authority, Apple Root CA.
- CDHash `922ad85442372ac0dc58d14ccc04a7a78b420297` (sha256).
- Timestamp 13 October 2026, 9:16:21 AM.
- Designated requirement pins the identifier, the Apple generic anchor, and
  leaf OU `Q6L2SF6YDW`.

The signature inspection was a digest-verified snapshot of that Mach-O. It
did not re-check the whole bundle as a filesystem trust evaluation. The
entitlement dictionary in that snapshot is:

- `com.apple.application-identifier` = `Q6L2SF6YDW.com.anthropic.claudefordesktop`
- `com.apple.developer.team-identifier` = `Q6L2SF6YDW`
- `com.apple.security.automation.apple-events`
- `com.apple.security.cs.allow-jit`
- `com.apple.security.device.audio-input`
- `com.apple.security.device.bluetooth`
- `com.apple.security.device.camera`
- `com.apple.security.device.print`
- `com.apple.security.device.usb`
- `com.apple.security.personal-information.location`
- `com.apple.security.personal-information.photos-library`
- `com.apple.security.virtualization`
- Keychain groups, each prefixed with `Q6L2SF6YDW.`:
  `com.anthropic.claude.webauthn`, `com.anthropic.claude.hwkey`,
  `com.microsoft.identity.universalstorage`,
  `com.microsoft.workplacejoin`, `com.microsoft.workplacejoin.v2`.

The AppleEvents, virtualization, camera, microphone, and keychain groups
line up with computer use, the Swift virtualization addon, dictation, and
the MSAL / WebAuthn material described in the later chapters. The entitlements
grant the capability. They do not show that a given session used it.

## Developer ID profile

`Contents/embedded.provisionprofile` is 12,521 bytes. It is CMS `signedData`
(`1.2.840.113549.1.7.2`) wrapping an XML plist.

| Field | Value |
| --- | --- |
| Name | `Claude - Developer ID - 20260410` |
| App ID name | `Claude for Desktop` |
| Team | Anthropic PBC / `Q6L2SF6YDW` |
| Platform | `OSX` |
| `ProvisionsAllDevices` | true |
| `IsXcodeManaged` | false |
| Created | 2026-05-30 |
| Expires | 2044-05-25 |
| Time to live | 6570 days |
| UUID | `7ecaa873-983d-4f9b-a2af-90079e8f7261` |

Entitlement keys inside the profile: application identifier, team identifier,
`com.apple.developer.associated-domains` = `*`, and
`keychain-access-groups` = `Q6L2SF6YDW.*`. One developer certificate. No
device UDID list.

## Host binary

`Contents/MacOS/Claude` is a universal Mach-O executable.

| Slice | File offset | Size |
| --- | --- | --- |
| x86_64 | 16384 | 34,096 |
| arm64 | 65536 | 54,528 |

Minimum macOS 13.0, SDK 26.5, linked with LLD 23.0. Two load commands:
`@rpath/Electron Framework.framework/Electron Framework` and `libSystem`.
The export sample is `__mh_execute_header`. Imports include `_ElectronMain`,
`_ElectronInitializeICUandStartNode`, the RunAsNode fuse check, `getenv`,
and `freopen`. UUID `4C4C44AF-5555-3144-A1E7-7E571B4F50E9`.

A static overview of the arm64 slice counted 16 procedures, 9 segments, and
348 bytes of `__text`. The image base is `0x100000000`. The slice's string
count was 14,830, which tracks a `__cstring` block of 14,832 bytes. That
count is defined string data in a section of that size, not 14,830 distinct
product messages.

## Electron fuses

The fuse sentinel is inside Electron Framework (421,312,576 bytes, universal).
Wire version 1. Nine fuses. One complete sentinel on each slice, and the
states match.

| Index | Fuse | State |
| --- | --- | --- |
| 0 | `RunAsNode` | disabled |
| 1 | `EnableCookieEncryption` | enabled |
| 2 | `EnableNodeOptionsEnvironmentVariable` | disabled |
| 3 | `EnableNodeCliInspectArguments` | disabled |
| 4 | `EnableEmbeddedAsarIntegrityValidation` | enabled |
| 5 | `OnlyLoadAppFromAsar` | enabled |
| 6 | `LoadBrowserProcessSpecificV8Snapshot` | disabled |
| 7 | `GrantFileProtocolExtraPrivileges` | enabled |
| 8 | `WasmTrapHandlers` | enabled |

Sentinel file offsets: x86_64 slice at `173729328`, arm64 slice at
`377741564`.

Derived from those bytes, for both slices: Node CLI inspect arguments are
not honored, `ELECTRON_RUN_AS_NODE` is not honored, embedded ASAR integrity
validation is enforced, and application code is restricted to the ASAR. A
disabled inspect fuse means the Node CLI switch is ignored. It does not
prove the application has no other debug switch of its own. The host stub
imports the RunAsNode fuse check; that import was not traced through a run.

## Update framework

Squirrel ships as `com.github.Squirrel`, copyright "Copyright © 2013 GitHub.
All rights reserved." The binary exports `SQRLUpdater`, `SQRLDownloader`,
`SQRLShipItLauncher`, `SQRLUpdate`, and `SQRLZipArchiver`. Minimum macOS
13.0, SDK 26.5, LLD 23.0. `ShipIt` is present under the framework's
Resources. Mantle is `com.electron.mantle` version `1.0`. ReactiveObjC is
`com.electron.reactive` version `3.1.0`.

The application inventory found no privileged helper and no launchd plist.
