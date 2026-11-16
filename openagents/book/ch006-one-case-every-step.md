# 6. One case, every step

> **Status: unimplemented — design draft.** This chapter is a plan for the
> owner to edit. It ships no code yet, and paths marked planned do not exist.
> The first milestone recorded on 2026-11-13 ran on the old app, which is
> being removed; the new app earns it again here (see "The first milestone,
> on the old app").

## Premise

Chapters 1 to 5 each added one thing to the chat page: a text input, Send,
a transcript, a reply and the login card. This chapter joins their cases
into one acceptance case, written once as `coax drive` operations, and runs
it against the same build in the iOS Simulator and on a physical iPhone.
When it passes on both, the chat page is the book's first milestone.

## Draft

An acceptance case is a list of operations with expected answers. The
operations are the common `coax drive` vocabulary: `tree`, `tap ID`,
`type ID TEXT`, `state`, `layout`, `clock`, `logs` and `screenshot`
([Coax Native, "The agent interface and verification"](../../coax/native.md#the-agent-interface-and-verification)).
`tap` and `type` find a live row by ID in the committed semantic table and
go through the real input path. They never write state directly.

The chat page's case types a message with no session, logs in through the
card, receives the reply a part at a time from the fake service, then sends
again and stops:

```text
type composer.input hello
tap composer.send
type login.email you@example.com
tap login.send
clock data release auth.email_start
type login.code 123456
tap login.verify
clock data release auth.email_verify
clock data release chat.send
clock data release chat.part
clock data release chat.done
state transcript
type composer.input again
tap composer.send
clock data release chat.part
tap composer.stop
```

Assertions run on state and the semantic tree first. A picture comes last,
and only to catch a drawing fault the tree can't show. On the GPU kit that
matters more than before: the tree is the same rows the screen draws from,
so a passing tree is strong evidence, and a picture checks only what paint
adds.

A host that can't do an operation says so. A driver that implements only
`tree` and `tap` refuses `type` and does not claim the whole interface.
Before a run, the driver checks that the app it attached to was built from
the source under test; a stale build refuses with the command that rebuilds
it. The web driver does this today; the iOS driver must do it before this
chapter is implemented.

The simulator is evidence for the simulator. Keyboard input methods,
VoiceOver, one-time-code AutoFill, backgrounding and the frame-rate
objective while typing need a run on a physical iPhone before the chapter
names `ios-device` ([CONTRACT.md](CONTRACT.md#targets)). The GPU login
screen has not had that run yet. When a device check needs the owner's
phone, it goes in the workspace's `NEEDS_OWNER.md`.

### Who may drive a phone

This was decided on 2026-11-13 and still holds. A simulator's app shares
the Mac's file system, so the driver reaches it on a Unix socket. A
physical iPhone needs a network transport. The tunnel that `devicectl`
keeps to a paired phone is not one macOS exposes as a network interface, so
the driver uses TCP on the phone's own network, and the app admits a driver
only when all of these hold
([`apple/phone-drive.c`](../../apps/shared/host/apple/phone-drive.c)):

- **A development launch.** The app listens only when it is launched with
  `-OADrive tcp:PORT -OADriveToken TOKEN`. A launch argument is not saved,
  so the next normal launch has no listener. Without a token of 32 to 128
  characters it doesn't listen.
- **The local network.** The driver's address must be on the subnet of one
  of the phone's own interfaces that is up (Wi-Fi, or the USB link to the
  Mac), never cellular or loopback.
- **The launch's token.** The driver's first line, within five seconds, is
  `attach TOKEN`, compared in constant time. The runner makes a new token
  for each case.
- **One driver at a time.** A second connection waits until the first
  closes.

```shell
xcrun devicectl device process launch --device DEVICE --terminate-existing \
  com.openagents.app -- -OADrive tcp:47613 -OADriveToken TOKEN
coax/web/tool.sh drive native tcp:PHONE.local:47613 --token TOKEN --ops openagents/examples/cases/chat-page.ops
```

### The first milestone, on the old app

On 2026-11-13 the book recorded its first milestone
([#13516](https://github.com/OpenAgentsInc/openagents/issues/13516),
[#13517](https://github.com/OpenAgentsInc/openagents/issues/13517)): login,
one message, a fixture reply and Stop, from one shared Coax module
(`apps/shared/openagents-app.coax`), with byte-equal traces on the build
host, in headless Chrome, in the iPhone simulator and on an iPhone 17 Pro
Max. The iPhone drew that module with UIKit controls
(`SharedAppViewController.swift`) inside the old app. The device checks ran
then: the number pad on the code field, a Japanese input method sending
`日本語` once, non-Latin text, labels and the accessibility audit, and a
reply finishing once across backgrounding.

That run is history, not evidence for the new app. Its presentation was
UIKit, not the GPU kit, and the app it ran in is being replaced. What
carries over is the shape: the operation files and traces in
[`openagents/examples/cases`](../examples/cases) show what this chapter's
case must assert, and the device checks above are the list this chapter
runs again on the phone.

## What we build

A plan for what this chapter ships once the draft is approved:

- The chat page's acceptance case as one operation file and its trace.
- The lean app's driver: the common operations over the committed rows, a
  refusal for an operation it lacks, and the stale-build check.
- A run of the case in the iOS Simulator and on a physical iPhone, with the
  device checks recorded per target.

It builds on what exists today:

- [`apps/shared/host/drive.c`](../../apps/shared/host/drive.c)
- [`apps/shared/host/drive.h`](../../apps/shared/host/drive.h)
- [`apps/shared/host/apple/phone-drive.c`](../../apps/shared/host/apple/phone-drive.c)
- [`coax/web/drive.coax`](../../coax/web/drive.coax)
- [`apps/openagents-ios`](../../apps/openagents-ios)

### The chapter's check

- **Source:** `openagents/examples/cases/chat-page.ops`, over the chat page
  of chapters 1 to 5.
- **Trace:** each operation and its answer, one line each, in a committed
  `.trace` file; the simulator and the phone give the same bytes after the
  host's name line.
- **Refusal:** in the same case, the parts released after `tap
  composer.stop` each answer `late part dropped`; a driver with a wrong
  token gets `refused: wrong token`; an app built from another commit
  refuses the run with the rebuild command.
- **Independent check:** an XCUITest that runs the same steps through the
  accessibility tree with the system keyboard, with no `coax drive`, on the
  simulator and on the phone.

Planned paths: `openagents/examples/cases/chat-page.ops`. The earlier cases
in [`openagents/examples/cases`](../examples/cases) stay as the record of
the old app's milestone.

## Sources

- **Episodes:**
  - [Episode 281, *Coder Mobile*](../../docs/transcripts/281.md): one chat on several devices
- **Also:**
  - [Coax Native](../../coax/native.md)
  - [The iOS app's tests](../../apps/openagents-ios/README.md)
- **Promises covered** ([audit](../../docs/audits/2026-11-10-episode-289-port-audit.md#1-what-episode-289-promised)): P1, P16

## Decided

1. The driver reaches a physical iPhone over TCP on the phone's own
   network, and only a driver on that network with the launch's token may
   attach ("Who may drive a phone", 2026-11-13).
