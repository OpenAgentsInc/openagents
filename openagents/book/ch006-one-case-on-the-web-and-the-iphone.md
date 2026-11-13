# 6. One case on the web and the iPhone

> **Status: partly implemented.** The login case and the cancellation case
> run with byte-equal traces in headless Chrome, in the iPhone simulator and
> on a physical iPhone, and the independent check passes on both iPhones
> ([#13516](https://github.com/OpenAgentsInc/openagents/issues/13516),
> [#13517](https://github.com/OpenAgentsInc/openagents/issues/13517)). The
> driver's stale-build refusal on the iPhone, dictation and Android remain.
> See "What remains".

## Premise

Chapters 2 to 5 built an app with state, components, routes and effects. This
chapter runs it on two hosts and proves they agree: one acceptance case,
written once as `coax drive` operations, passes in headless Chrome and in the
iPhone app.

## Draft

An acceptance case is a list of operations with expected answers. The
operations are the common `coax drive` vocabulary: `tree`, `tap ID`,
`type ID TEXT`, `state`, `layout`, `clock`, `logs` and `screenshot`
([Coax Native, "The agent interface and verification"](../../coax/native.md#the-agent-interface-and-verification)).
`tap` and `type` find a live element by ID and go through the real input
path. They never write state directly.

The first case opens the login screen, types an email, requests a code,
releases a fake answer, enters the code and checks that the account state and
the route changed. A second case cancels the request and releases a late
answer after you leave the screen.

```text
tap home.login
type account.email you@example.com
tap account.submit
clock data release auth.email_start
state account.step
```

Assertions run on state and the semantic tree first. A picture comes last,
and only to catch a drawing fault that the tree can't show. The web and the
iPhone may look different. Their action and result sequences must match.

On the web, `coax/web/tool.sh drive web` already runs these operations in
headless Chrome. On the iPhone, the shared native driver
([`apps/shared/host/drive.c`](../../apps/shared/host/drive.c)) speaks the
same operations, but the iPhone app's shell does not use it yet. Its UI tests
use XCTest identifiers, coordinates and timed waits. Connecting the app's
launch path to the driver is stage 1 of
[Coax Native's migration](../../coax/native.md#migration-and-implementation-stages),
and this chapter is its first consumer.

A host that can't do an operation says so. A driver that implements only
`screenshot` and `tap` refuses `type` and does not claim the whole
interface. Before a run, the driver checks that the app it attached to was
built from the source under test; a stale build refuses with the command
that rebuilds it.

The simulator is evidence for the simulator. Keyboard input methods,
VoiceOver, backgrounding and the platform SDKs need a run on a real device
before a chapter names `ios-device` as a target
([CONTRACT.md](CONTRACT.md#targets)). When the work reaches that point, the
device run goes in the workspace's `NEEDS_OWNER.md` if it needs the owner's
phone or key.

### Who may drive a phone

A simulator's app shares the Mac's file system, so the driver reaches it on
a Unix socket. A physical iPhone needs a network transport. Two were open:
a TCP socket on the local network, or the tunnel that `devicectl` keeps to
a paired phone. The tunnel is out: `devicectl` reports its address (an
`fd..::1` address), but macOS doesn't expose it as a network interface, so
a socket on the Mac can't reach an app through it.

The driver uses TCP on the phone's own network, and the app admits a
driver only when all of these hold
([`apple/phone-drive.c`](../../apps/shared/host/apple/phone-drive.c)):

- **A development launch.** The app listens only when it is launched with
  `-OADrive tcp:PORT -OADriveToken TOKEN` (with `devicectl device process
  launch`). A launch argument is not saved, so the next normal launch has
  no listener. Without a token of 32 to 128 characters it doesn't listen
  at all.
- **The local network.** The driver's address must be on the subnet of one
  of the phone's own interfaces that is up (Wi-Fi, or the USB link to the
  Mac), never the cellular one, or loopback. Anything else is answered
  `refused: only a driver on this phone's own network may attach` and
  closed.
- **The launch's token.** The driver's first line, within five seconds, is
  `attach TOKEN`, compared in constant time. A wrong one is answered
  `refused: wrong token` and closed. The runner makes a new token for each
  case (`openssl rand -hex 16`) and passes it at launch.
- **One driver at a time.** A second connection waits until the first
  closes.

The app only accepts connections. It never browses or dials, so no Local
Network prompt stood in the way.

```shell
xcrun devicectl device process launch --device DEVICE --terminate-existing \
  com.openagents.app -- -OADrive tcp:47613 -OADriveToken TOKEN
coax/web/tool.sh drive native tcp:PHONE.local:47613 --token TOKEN --ops openagents/examples/cases/login.ops
```

## What we build

A plan for what this chapter ships once the draft is approved:

- The login case and the cancellation case as operation files.
- The iPhone app's shell connected to `apps/shared/host/drive.c`, reporting
  the operations it supports.
- A runner that plays one operation file against the web build and the
  simulator build and compares the two traces line by line.

It builds on what exists today:

- [`coax/web/drive.coax`](../../coax/web/drive.coax)
- [`coax/web/drive-dom.coax`](../../coax/web/drive-dom.coax)
- [`apps/shared/host/drive.c`](../../apps/shared/host/drive.c)
- [`apps/shared/host/drive.h`](../../apps/shared/host/drive.h)
- [`apps/openagents-ios`](../../apps/openagents-ios)
- [`docs/metaverse/fast-shots.md`](../../docs/metaverse/fast-shots.md)

### The chapter's check

- **Source:** `openagents/examples/cases/login.ops` and
  `openagents/examples/cases/login-cancel.ops`, over the app of chapters 2
  to 5.
- **Trace:** each operation and its answer, one line each; the web trace and
  the iPhone trace are byte-equal after the host's name line.
- **Refusal:** the cancellation case; the late answer changes nothing, on
  both hosts.
- **Independent check:** an XCUITest,
  [`SharedAppUITests`](../../apps/openagents-ios/StarterUITests/SharedAppUITests.swift),
  logs in by hand through the accessibility tree, with no `coax drive`. It
  sees the shell and `y...@example.com`, as `login.trace` says. A wrong code
  leaves it on the login screen with `That code isn't right.`

Recorded on 2026-11-13, with the runner of chapter 11
([`apps/shared/cases.coax`](../../apps/shared/cases.coax)):

```shell
coax/web/tool.sh apps/shared/cases.coax web login login-cancel
coax/web/tool.sh apps/shared/cases.coax ios-simulator DEVICE login login-cancel
```

Both cases match
[`login.trace`](../examples/cases/login.trace) and
[`login-cancel.trace`](../examples/cases/login-cancel.trace) on web
(headless Chrome on macOS) and on ios-simulator (iPhone 17, iOS 26.5). The
iPhone app answers `coax drive` when it is launched with `-OADrive SOCKET`,
through [`apps/shared/host/drive.c`](../../apps/shared/host/drive.c) and
[`apple/phone-drive.c`](../../apps/shared/host/apple/phone-drive.c). It
refuses an operation it lacks, and it refuses a driver tap on a shown
control that no real press delivered. The case is over the shared app of
chapter 11
([`apps/shared/openagents-app.coax`](../../apps/shared/openagents-app.coax)),
because chapters 2 to 5 have no separate module yet. The app is the Coax
Native starter (build 1068 and later). `-OASharedApp YES` or `-OADrive
SOCKET` opens the shared app in place of the starter's page.

The independent check ran in the same simulator, and passed:

```shell
xcodebuild -project apps/openagents-ios/OpenAgents.xcodeproj -scheme OpenAgents \
  -destination id=DEVICE CODE_SIGNING_ALLOWED=NO \
  -only-testing:OpenAgentsUITests/SharedAppUITests test
```

### On a physical iPhone

Recorded on 2026-11-13
([#13517](https://github.com/OpenAgentsInc/openagents/issues/13517)), on an
iPhone 17 Pro Max with iOS 26.6.2 on the same Wi-Fi as the Mac (macOS 26.4,
Xcode 26.6). The app is a Debug build with development signing (the team's
Apple Development certificate and its development profile for
`com.openagents.app`), installed with `devicectl device install app`. There
was no TestFlight build and no upload.

```shell
xcodebuild -project apps/openagents-ios/OpenAgents.xcodeproj -scheme OpenAgents \
  -configuration Debug -destination id=UDID -derivedDataPath build-device build
xcrun devicectl device install app --device DEVICE \
  apps/openagents-ios/build-device/Build/Products/Debug-iphoneos/OpenAgents.app
coax/web/tool.sh apps/shared/cases.coax ios-device DEVICE
```

All five cases of chapter 11 matched their traces byte for byte, the same
bytes as on cpu, the web and the simulator. Among them,
[`login.trace`](../examples/cases/login.trace) and
[`login-cancel.trace`](../examples/cases/login-cancel.trace) are this
chapter's. The refusals held on the phone too: a driver with a wrong token
got `refused: wrong token`, the driver refused to start without `--token`,
and an app launched with a five-character token didn't listen.

The first launch on the phone crashed before it drew anything: the shared
app's state was built on the main thread's stack, which is 1 MB on a phone
and 8 MB in the simulator. The app now zeroes it in place.

The independent check, `SharedAppUITests`, passed on the phone, and so did
the device checks in
[`SharedAppDeviceUITests`](../../apps/openagents-ios/StarterUITests/SharedAppDeviceUITests.swift),
which drive the app by hand through the accessibility tree:

```shell
xcodebuild -project apps/openagents-ios/OpenAgents.xcodeproj -scheme OpenAgents \
  -configuration Debug -destination id=UDID -derivedDataPath build-device \
  -only-testing:OpenAgentsUITests/SharedAppUITests \
  -only-testing:OpenAgentsUITests/SharedAppDeviceUITests test
```

| Check | What ran on the phone | Result |
| --- | --- | --- |
| One-time code | The code field brings up the number pad (digits, no letters); `123456` tapped on its keys logs in | Passed |
| An input method | The Japanese (Romaji) keyboard, given to the app by launch arguments so the phone's settings don't change: `nihongo` tapped key by key is marked text `にほんご`, the candidate `日本語` commits it, and only `日本語` is sent, once | Passed |
| Non-Latin text | `こんにちは 你好 🙂` typed into the composer comes back whole in the transcript | Passed |
| VoiceOver labels | Every button, field and text on the home, login (both steps), chats and chat screens has a label | Passed |
| Accessibility audit | `performAccessibilityAudit` on the same five screens | Passed, with two issues set aside (below) |
| Background | The app goes to the Home Screen while a reply is pending and comes back to the same conversation: one message, the whole reply once, no Stop notice | Passed |

The audit first failed on Dynamic Type: the labels and fields didn't follow
the text size setting. They do now. It also reports two issues that are not
the shared screen's. The masked address `y...@example.com` is "not
human-readable", but the mask is the point. The system keyboard's
suggestion bar has an empty slot with no description. The test sets those
two aside by name and fails on any other issue.

In the background check the fakes answer every two seconds
(`-OASharedAppHandSeconds 2`), so the turn is still pending when the app
leaves. When the app came back, the reply had already finished, once.

Not covered on the phone:

- **Dictation.** XCUITest can't speak to the phone, so nothing here drives
  dictation. A person checks it by hand (the workspace's `NEEDS_OWNER.md`).
- **VoiceOver itself.** The checks read the labels VoiceOver reads and run
  the audit; nobody moved through the screens with VoiceOver on.
- **One-time code AutoFill.** The field is a one-time-code field, but the
  suggestion above the keyboard needs a real text message.

## What remains

- **Stale builds.** The iPhone driver doesn't check yet that the app was
  built from the source under test. The web driver does.
- **Dictation, VoiceOver on, AutoFill.** The device checks above don't
  cover them; a person checks them on the phone.
- **Android.** Neither the emulator nor a phone has run the cases.

Planned paths: none new. The cases are in
[`openagents/examples/cases`](../examples/cases).

## Sources

- **Episodes:**
  - [Episode 281, *Coder Mobile*](../../docs/transcripts/281.md): one chat on several devices
- **Also:**
  - [Coax Native](../../coax/native.md), stages 1 and 2
  - [Fast shots](../../docs/metaverse/fast-shots.md): one headless browser per checkout
  - [The iOS app's tests](../../apps/openagents-ios/README.md)
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P1, P16

## Decided

1. The driver reaches a physical iPhone over TCP on the phone's own
   network, and only a driver on that network with the launch's token may
   attach ("Who may drive a phone", 2026-11-13).
