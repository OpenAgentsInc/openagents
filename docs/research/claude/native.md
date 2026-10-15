# Native modules

File type 2 is `MH_EXECUTE`. File type 6 is `MH_DYLIB`. Unless a row says
otherwise, linkage below is from the universal image, and the procedure
counts are from a static overview of the arm64 slice. Image base for every
such overview is `0x100000000`. String counts are defined string data in the
image, not a census of user-visible copy.

Embedded Go or Rust metadata is file content. It does not authenticate the
compiler, the dependency versions, or the runtime behavior. No binary in
this set carried cargo-auditable dependency metadata. A rustc commit below
is inferred from a `/rustc/<40-hex>/` path. Absence of a Go build record is
not proof that a binary contains no Go.

Electron Framework, `swift_addon.node`, `github-mcp-server`, the iOS
Simulator helper, its simulator bridge, and Squirrel were not given
procedure-level overviews. Their linkage and metadata are included.

## Overview sizes

| Arm64 image | Procedures | `__text` bytes | Defined strings | Segments |
| --- | --- | --- | --- | --- |
| Host stub `Claude` | 16 | 348 | 14,830 | 9 |
| `disclaimer` | 57 | 1,848 | 91 | 15 |
| `permission-fixer` | 300 | 13,708 | 645 | 27 |
| `chrome-native-host` | 1,978 | 776,508 | 921 | 21 |
| `app-cu-helper` | 1,831 | 676,848 | 1,115 | 21 |
| `computer_use.node` | 4,302 | 791,124 | 5,707 | 46 |
| `claude-native-binding.node` | 5,242 | 2,004,824 | 2,174 | 23 |
| `pty.node` (darwin-arm64) | 256 | 21,024 | 421 | 17 |
| `msal-node-runtime.node` | 675 | 94,484 | 2,225 | 17 |

The host stub's string count tracks a `__cstring` block of 14,832 bytes, which
is far larger than its 348-byte text section. `computer_use.node` is the
overview that contains Swift sections.

## claude-native-binding.node

`MH_DYLIB`. UUID `D33BA4B2-7723-3585-B28F-906AD202E43E`. Minimum macOS 11.0,
SDK 26.1, linked with LD 1230.1. The export sample is
`_napi_register_module_v1` only, so the image is an N-API addon. Linked
libraries include Security, AppKit, Carbon, ApplicationServices,
CoreServices, `libbsm`, and `libiconv`. The import sample begins with
Accessibility (`AXIsProcessTrustedWithOptions` and `AXUIElement*`).

Inferred rustc commit `48a229ceaefd4985c50990b14116b6d856af0985`, from a
compiler path at file offset 2,014,848 in the arm64 slice. No Go build
record. The arm64 overview has no Swift sections. Slice SHA-256
`3c367138df7aa0544c2b8bcbba8e8f0102e187dfd8183b0c86b5159ef2d00469`.

The JavaScript package is `@ant/claude-native` version `0.0.0`.

## computer_use.node

`MH_DYLIB`. UUID `F930F3D8-7311-3188-916A-5C0DF856A168`. Minimum macOS 12.0,
SDK 26.1, LD 1230.1. Swift module `ComputerUseSwift`. The export sample
includes `ComputerUseSwiftBindings` and
`ComputerUseSwiftEntry.napiRegisterModule`. `AXError` is bridged as an
`NSError`. The image links AppKit and ScreenCaptureKit, plus the Swift
runtime and `libresolv`. Counts: 32 dependencies, 11 exports, 707 imports.
No Go build record and no rustc path. Slice SHA-256
`a1cd3f06b6eb89993a283a70a93d968364e0dc6f007c7dc67d0178afa06eff30`.

This is the native side of the computer-use windows named in the archive
(`computerUseTeach.js`, `computerUseWatchRecord.js`). ScreenCaptureKit plus
the Accessibility imports are the capture and UI-element surface. The IPC
suffixes `CuTeach_*` and `CuWatchRecord*` are the renderer side of that
feature. The two were not joined by a call trace.

## swift_addon.node

`MH_DYLIB`. UUID `C4A349D8-FFB2-3512-BE24-AF1E1A1E11AA`. Minimum macOS 12.0,
SDK 26.1, LD 1230.1. Swift module `ClaudeSwift`, with
`ClaudeSwift.EventBridge` and `ClaudeSwift.Event` in the export sample
(`toNAPI`, `eventName`). Counts: 54 dependencies, 950 exports, 1,940
imports. Beyond AppKit and the Swift runtime, the link list includes
Virtualization, `vmnet`, ScreenCaptureKit, ServiceManagement,
UserNotifications, SwiftUI, AVFoundation, AVFAudio, CoreSpotlight, Metal,
and IOKit. No rustc path.

A Go build record is embedded in the arm64 slice. Go `go1.25.14`, build
mode `c-archive`, `CGO_ENABLED=1`, `GOOS=darwin`, `GOARCH=arm64`. Module
path `usernet`, version `(devel)`. The dependency
`github.com/containers/gvisor-tap-vsock` `v0.8.9` is replaced by
`../../../third_party/gvisor-tap-vsock` `(devel)`. Also named: gVisor,
gopacket, dhcp, dns, and lz4. Build mode `c-archive` is the form used when
a Go archive is linked into another image. The load commands do not name a
separate Go dylib, so the record is content inside the Swift image. It does
not make the whole addon a Go program.

The JavaScript package is `@ant/claude-swift` version `1.0.0`. Combined with
the virtualization entitlement, the addon is the native surface for a local
VM-style guest and a userspace network stack. That combination was not
executed. The `smol-bin` images in [bundle](bundle.md) are a separate,
undecoded disk-image pair.

## node-pty

`pty.node` ships for darwin-arm64 and darwin-x64, with a `spawn-helper` for
each. The JavaScript package is `node-pty` `1.2.0-beta.14`. The archive also
contains `node_modules/node-pty/lib/worker/conoutSocketWorker.js` and
`.vite/build/pty-host/ptyHostWorker.js`. The arm64 `pty.node` overview used
the observed unpacked digest
`a8887e12780b6176c6584146053844a52fa95dd61677cb617d26bc64e0d1c26c`: 256
procedures and 21,024 bytes of `__text`. Go and Rust metadata were not
recovered for that file.

## github-mcp-server

`MH_EXECUTE`, universal. UUID `E855FCE2-472A-3009-D8C0-89212059B148`.
Minimum macOS 12.0, SDK 12.0. No linker version was recorded. Four
dependencies: `libSystem`, `libresolv`, CoreFoundation, and Security. The
export sample is empty. Unpacked size 39,731,138 bytes across both slices.

Go build record: Go `go1.25.12`, build mode `exe`, `CGO_ENABLED=0`,
trimpath, `darwin/arm64`. Main module
`github.com/github/github-mcp-server` `v1.6.0`, package path
`github.com/github/github-mcp-server/cmd/github-mcp-server`. VCS git
revision `7629b6e7337e1fc4b11a0ca5fdea4af4edd21f6e`, time
`2026-07-15T11:01:54Z`, modified false. Thirty-two dependencies. The MCP
one is `github.com/modelcontextprotocol/go-sdk` `v1.7.0-pre.1`. Also
present: `github.com/google/go-github/v89` `v89.0.0`. No Rust metadata.
The binary was not decompiled.

## MSAL and Office

`msal-node-runtime.node` is a thin arm64 Mach-O, unpacked size 359,456.
Its overview used the observed digest
`31c7a3cb54b0f72d235788ecc07197f33e206da16a3ebc42492f3f8f51fdadb9`: 675
procedures and 94,484 bytes of `__text`. No Go build record was recovered.

Beside it, unpacked: `libmsalruntime_arm64.dylib` (7,134,816 bytes). Inside
the archive payload: `office365-mcp-stdio.mjs` (7,288,531 bytes),
`pdf.worker.mjs`, and `pdfExtractorProcess.mjs`. Those scripts were not
read past the inventory. The URL scheme
`msauth.com.anthropic.claudefordesktop` and the Microsoft keychain groups
are the signing-side counterparts of this directory.

## Helpers

All four bare helpers are universal executables. Minimum macOS 11.0, SDK
26.1, LD 1230.1, unless noted.

**disclaimer.** UUID `34E6BEDC-185B-3D8C-B4BD-963F7ADE4D96`. Links only
`libSystem`. Exports `__mh_execute_header` and `_main`. Imports include
`posix_spawnp`, `responsibility_spawnattrs_setdisclaim`, `kqueue`,
`kevent`, and `waitpid`. Overview: 57 procedures, 1,848 bytes of `__text`.
No Go or Rust metadata. The iOS Simulator seatbelt profile's comments name
this binary as the TCC-disclaim wrapper for that helper. The import is the
observation. The comment is the profile author's claim. See
[bundle](bundle.md).

**permission-fixer.** UUID `7996A41F-FC0F-3DC3-AFD3-86D8767884CF`. Swift,
linked to Foundation, IOKit, and XPC (`libswiftIOKit`, `libswiftXPC`,
`libswiftFoundation`). Overview: 300 procedures, 13,708 bytes of `__text`,
with Swift type-ref, entry, and hooks sections. No Go or Rust metadata.

**chrome-native-host.** UUID `D6868054-B23D-3EF1-9EBD-973EDE72A743`. Three
dependencies: `libiconv`, CoreFoundation, and `libSystem`. Exports
`__mh_execute_header`, `_main`, and `_rust_eh_personality`. Same inferred
rustc commit `48a229ceaefd4985c50990b14116b6d856af0985`. Overview: 1,978
procedures, 776,508 bytes of `__text`. The manifest name
`@ant/chrome-native-host` matches this helper's file name. The helper was
not traced to the Chrome MCP package.

**app-cu-helper.** UUID `ABF9EDA8-11B2-3E96-87AC-A812A1B5DE59`. Links
ApplicationServices, CoreGraphics, AppKit, Foundation, `libobjc`,
`libiconv`, CoreFoundation, and `libSystem`. Exports the same three symbols
as the Chrome host, including `_rust_eh_personality`. The import sample is
Accessibility: `AXIsProcessTrusted`, `AXUIElementCreateApplication`,
`AXUIElementPerformAction`, `AXUIElementSetAttributeValue`, and related
`AX*` calls. Same inferred rustc commit. Overview: 1,831 procedures,
676,848 bytes of `__text`.

The shared rustc hash across `claude-native-binding.node`,
`chrome-native-host`, and `app-cu-helper` is one inferred compiler commit.
It is not a verified rustc version, and it is not proof the three binaries
were produced by one build invocation.

## iOS Simulator helper

**claude-ios-sim.** `MH_EXECUTE`. UUID
`EE972F54-0FD3-3E6A-92D4-142AA35C0080`. Minimum macOS **15.0**, SDK 26.1,
LD 1230.1. Forty-five dependencies, export sample limited to the Mach-O
header, 1,884 imports. The first two dependencies are private frameworks:
`/Library/Developer/PrivateFrameworks/CoreSimulator.framework` and
`/System/Library/PrivateFrameworks/AccessibilityPlatformTranslation.framework`.
The import sample includes `SimDevice`, `SimDeviceSet`, `SimDeviceType`,
`SimRuntime`, `SimServiceContext`, and `AXPTranslator`, plus AVFoundation
and AppKit. No Go or Rust metadata.

The helper bundle identifier is `com.anthropic.claude.ios-sim`.
`LSMinimumSystemVersion` is `15.0`. `LSBackgroundOnly` is true.

**SimulatorFrameworkBridge-iOS.** File type 2, platform `IOSSIMULATOR`,
minimum 15.0, SDK 26.1, LD 1230.1. UUID
`EB89AC4B-0E7C-3EE4-8946-6AFAE8C59E89`. Swift module `SimulatorIPC`
(`IPCListener.bind`, file descriptors). Thirty-one dependencies, 701
exports, 570 imports. Links Photos, Contacts, UserNotifications, CoreData,
CoreGraphics, and Foundation. This is an iOS Simulator executable carried
inside the Mac helper bundle.

## Electron Framework

`MH_DYLIB`. UUID `4C4C44BE-5555-3144-A1E3-83408F6C571F`. Minimum macOS 13.0,
SDK 26.5, LLD 23.0. Sixty-nine dependencies, 11,346 exports, 5,157 imports.
The export sample includes `_ElectronMain`,
`_ElectronInitializeICUandStartNode`, V8 heap helpers, and Rust-mangled
symbols (`__RNvCsbzz5UGvbnag_7___rustc35___rust_no_alloc_shim_is_unstable_v2`).
No rustc commit path and no cargo-auditable section were recovered. No Go
build record. The framework was not decompiled. Sibling libraries:
`libffmpeg.dylib`, `libvk_swiftshader.dylib`, and
`Helpers/chrome_crashpad_handler`.

## Linkage model

An arm64 load-command walk of the bundle covered 15 roots, 21 images, and
635 edges. Coverage is partial. Thirty-seven edges resolved to an image
inside the bundle, such as Electron Framework. Five hundred ninety-eight
were left undetermined because their install names are absolute system
libraries outside the modeled root. Absolute `/System` and `/usr/lib` paths
were not evaluated. Lazy-load commands were not traversed. `@rpath` and
`@loader_path` depend on dyld state that was not modeled. Code-signing
library validation was not evaluated.

Seventeen findings share one shape: the modeled search checks an Electron
`Libraries/*.framework` path before the real Mantle, ReactiveObjC, or
Squirrel framework, and that earlier file is absent. `ShipIt` has the same
pattern for Mantle and ReactiveObjC. A compatible library placed on the
earlier path could be preferred over the shipped framework, subject to
signing checks this pass did not perform. One root has no architecture in
the walk: the x64 `node-pty` `spawn-helper`.
